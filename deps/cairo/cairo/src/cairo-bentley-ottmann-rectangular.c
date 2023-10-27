/*
 * Copyright © 2004 Carl Worth
 * Copyright © 2006 Red Hat, Inc.
 * Copyright © 2009 Chris Wilson
 *
 * This library is free software; you can redistribute it and/or
 * modify it either under the terms of the GNU Lesser General Public
 * License version 2.1 as published by the Free Software Foundation
 * (the "LGPL") or, at your option, under the terms of the Mozilla
 * Public License Version 1.1 (the "MPL"). If you do not alter this
 * notice, a recipient may use your version of this file under either
 * the MPL or the LGPL.
 *
 * You should have received a copy of the LGPL along with this library
 * in the file COPYING-LGPL-2.1; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Suite 500, Boston, MA 02110-1335, USA
 * You should have received a copy of the MPL along with this library
 * in the file COPYING-MPL-1.1
 *
 * The contents of this file are subject to the Mozilla Public License
 * Version 1.1 (the "License"); you may not use this file except in
 * compliance with the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * This software is distributed on an "AS IS" basis, WITHOUT WARRANTY
 * OF ANY KIND, either express or implied. See the LGPL or the MPL for
 * the specific language governing rights and limitations.
 *
 * The Original Code is the cairo graphics library.
 *
 * The Initial Developer of the Original Code is Carl Worth
 *
 * Contributor(s):
 *	Carl D. Worth <cworth@cworth.org>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

/* Provide definitions for standalone compilation */
#include "cairoint.h"

#include "cairo-boxes-private.h"
#include "cairo-error-private.h"
#include "cairo-combsort-inline.h"
#include "cairo-list-private.h"
#include "cairo-traps-private.h"

#include <setjmp.h>

typedef struct _rectangle rectangle_t;
typedef struct _edge edge_t;

struct _edge {
    edge_t *next, *prev;
    edge_t *right;
    cairo_fixed_t x, top;
    int dir;
};

struct _rectangle {
    edge_t left, right;
    int32_t top, bottom;
};

#define UNROLL3(x) x x x

/* the parent is always given by index/2 */
#define PQ_PARENT_INDEX(i) ((i) >> 1)
#define PQ_FIRST_ENTRY 1

/* left and right children are index * 2 and (index * 2) +1 respectively */
#define PQ_LEFT_CHILD_INDEX(i) ((i) << 1)

typedef struct _sweep_line {
    rectangle_t **rectangles;
    rectangle_t **stop;
    edge_t head, tail, *insert, *cursor;
    int32_t current_y;
    int32_t last_y;
    int stop_size;

    int32_t insert_x;
    cairo_fill_rule_t fill_rule;

    cairo_bool_t do_traps;
    void *container;

    jmp_buf unwind;
} sweep_line_t;

#define DEBUG_TRAPS 0

#if DEBUG_TRAPS
static void
dump_traps (cairo_traps_t *traps, const char *filename)
{
    FILE *file;
    int n;

    if (getenv ("CAIRO_DEBUG_TRAPS") == NULL)
	return;

    file = fopen (filename, "a");
    if (file != NULL) {
	for (n = 0; n < traps->num_traps; n++) {
	    fprintf (file, "%d %d L:(%d, %d), (%d, %d) R:(%d, %d), (%d, %d)\n",
		     traps->traps[n].top,
		     traps->traps[n].bottom,
		     traps->traps[n].left.p1.x,
		     traps->traps[n].left.p1.y,
		     traps->traps[n].left.p2.x,
		     traps->traps[n].left.p2.y,
		     traps->traps[n].right.p1.x,
		     traps->traps[n].right.p1.y,
		     traps->traps[n].right.p2.x,
		     traps->traps[n].right.p2.y);
	}
	fprintf (file, "\n");
	fclose (file);
    }
}
#else
#define dump_traps(traps, filename)
#endif

static inline int
rectangle_compare_start (const rectangle_t *a,
			 const rectangle_t *b)
{
    return a->top - b->top;
}

static inline int
rectangle_compare_stop (const rectangle_t *a,
			 const rectangle_t *b)
{
    return a->bottom - b->bottom;
}

static inline void
pqueue_push (sweep_line_t *sweep, rectangle_t *rectangle)
{
    rectangle_t **elements;
    int i, parent;

    elements = sweep->stop;
    for (i = ++sweep->stop_size;
	 i != PQ_FIRST_ENTRY &&
	 rectangle_compare_stop (rectangle,
				 elements[parent = PQ_PARENT_INDEX (i)]) < 0;
	 i = parent)
    {
	elements[i] = elements[parent];
    }

    elements[i] = rectangle;
}

static inline void
rectangle_pop_stop (sweep_line_t *sweep)
{
    rectangle_t **elements = sweep->stop;
    rectangle_t *tail;
    int child, i;

    tail = elements[sweep->stop_size--];
    if (sweep->stop_size == 0) {
	elements[PQ_FIRST_ENTRY] = NULL;
	return;
    }

    for (i = PQ_FIRST_ENTRY;
	 (child = PQ_LEFT_CHILD_INDEX (i)) <= sweep->stop_size;
	 i = child)
    {
	if (child != sweep->stop_size &&
	    rectangle_compare_stop (elements[child+1],
				    elements[child]) < 0)
	{
	    child++;
	}

	if (rectangle_compare_stop (elements[child], tail) >= 0)
	    break;

	elements[i] = elements[child];
    }
    elements[i] = tail;
}

static inline rectangle_t *
rectangle_pop_start (sweep_line_t *sweep_line)
{
    return *sweep_line->rectangles++;
}

static inline rectangle_t *
rectangle_peek_stop (sweep_line_t *sweep_line)
{
    return sweep_line->stop[PQ_FIRST_ENTRY];
}

CAIRO_COMBSORT_DECLARE (_rectangle_sort,
			rectangle_t *,
			rectangle_compare_start)

static void
sweep_line_init (sweep_line_t	 *sweep_line,
		 rectangle_t	**rectangles,
		 int		  num_rectangles,
		 cairo_fill_rule_t fill_rule,
		 cairo_bool_t	 do_traps,
		 void		*container)
{
    rectangles[-2] = NULL;
    rectangles[-1] = NULL;
    rectangles[num_rectangles] = NULL;
    sweep_line->rectangles = rectangles;
    sweep_line->stop = rectangles - 2;
    sweep_line->stop_size = 0;

    sweep_line->insert = NULL;
    sweep_line->insert_x = INT_MAX;
    sweep_line->cursor = &sweep_line->tail;

    sweep_line->head.dir = 0;
    sweep_line->head.x = INT32_MIN;
    sweep_line->head.right = NULL;
    sweep_line->head.prev = NULL;
    sweep_line->head.next = &sweep_line->tail;
    sweep_line->tail.prev = &sweep_line->head;
    sweep_line->tail.next = NULL;
    sweep_line->tail.right = NULL;
    sweep_line->tail.x = INT32_MAX;
    sweep_line->tail.dir = 0;

    sweep_line->current_y = INT32_MIN;
    sweep_line->last_y = INT32_MIN;

    sweep_line->fill_rule = fill_rule;
    sweep_line->container = container;
    sweep_line->do_traps = do_traps;
}

static void
edge_end_box (sweep_line_t *sweep_line, edge_t *left, int32_t bot)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    /* Only emit (trivial) non-degenerate trapezoids with positive height. */
    if (likely (left->top < bot)) {
	if (sweep_line->do_traps) {
	    cairo_line_t _left = {
		{ left->x, left->top },
		{ left->x, bot },
	    }, _right = {
		{ left->right->x, left->top },
		{ left->right->x, bot },
	    };
	    _cairo_traps_add_trap (sweep_line->container, left->top, bot, &_left, &_right);
	    status = _cairo_traps_status ((cairo_traps_t *) sweep_line->container);
	} else {
	    cairo_box_t box;

	    box.p1.x = left->x;
	    box.p1.y = left->top;
	    box.p2.x = left->right->x;
	    box.p2.y = bot;

	    status = _cairo_boxes_add (sweep_line->container,
				       CAIRO_ANTIALIAS_DEFAULT,
				       &box);
	}
    }
    if (unlikely (status))
	longjmp (sweep_line->unwind, status);

    left->right = NULL;
}

/* Start a new trapezoid at the given top y coordinate, whose edges
 * are `edge' and `edge->next'. If `edge' already has a trapezoid,
 * then either add it to the traps in `traps', if the trapezoid's
 * right edge differs from `edge->next', or do nothing if the new
 * trapezoid would be a continuation of the existing one. */
static inline void
edge_start_or_continue_box (sweep_line_t *sweep_line,
			    edge_t	*left,
			    edge_t	*right,
			    int		 top)
{
    if (left->right == right)
	return;

    if (left->right != NULL) {
	if (left->right->x == right->x) {
	    /* continuation on right, so just swap edges */
	    left->right = right;
	    return;
	}

	edge_end_box (sweep_line, left, top);
    }

    if (left->x != right->x) {
	left->top = top;
	left->right = right;
    }
}
/*
 * Merge two sorted edge lists.
 * Input:
 *  - head_a: The head of the first list.
 *  - head_b: The head of the second list; head_b cannot be NULL.
 * Output:
 * Returns the head of the merged list.
 *
 * Implementation notes:
 * To make it fast (in particular, to reduce to an insertion sort whenever
 * one of the two input lists only has a single element) we iterate through
 * a list until its head becomes greater than the head of the other list,
 * then we switch their roles. As soon as one of the two lists is empty, we
 * just attach the other one to the current list and exit.
 * Writes to memory are only needed to "switch" lists (as it also requires
 * attaching to the output list the list which we will be iterating next) and
 * to attach the last non-empty list.
 */
static edge_t *
merge_sorted_edges (edge_t *head_a, edge_t *head_b)
{
    edge_t *head, *prev;
    int32_t x;

    prev = head_a->prev;
    if (head_a->x <= head_b->x) {
	head = head_a;
    } else {
	head_b->prev = prev;
	head = head_b;
	goto start_with_b;
    }

    do {
	x = head_b->x;
	while (head_a != NULL && head_a->x <= x) {
	    prev = head_a;
	    head_a = head_a->next;
	}

	head_b->prev = prev;
	prev->next = head_b;
	if (head_a == NULL)
	    return head;

start_with_b:
	x = head_a->x;
	while (head_b != NULL && head_b->x <= x) {
	    prev = head_b;
	    head_b = head_b->next;
	}

	head_a->prev = prev;
	prev->next = head_a;
	if (head_b == NULL)
	    return head;
    } while (1);
}

/*
 * Sort (part of) a list.
 * Input:
 *  - list: The list to be sorted; list cannot be NULL.
 *  - limit: Recursion limit.
 * Output:
 *  - head_out: The head of the sorted list containing the first 2^(level+1) elements of the
 *              input list; if the input list has fewer elements, head_out be a sorted list
 *              containing all the elements of the input list.
 * Returns the head of the list of unprocessed elements (NULL if the sorted list contains
 * all the elements of the input list).
 *
 * Implementation notes:
 * Special case single element list, unroll/inline the sorting of the first two elements.
 * Some tail recursion is used since we iterate on the bottom-up solution of the problem
 * (we start with a small sorted list and keep merging other lists of the same size to it).
 */
static edge_t *
sort_edges (edge_t  *list,
	    unsigned int  level,
	    edge_t **head_out)
{
    edge_t *head_other, *remaining;
    unsigned int i;

    head_other = list->next;

    if (head_other == NULL) {
	*head_out = list;
	return NULL;
    }

    remaining = head_other->next;
    if (list->x <= head_other->x) {
	*head_out = list;
	head_other->next = NULL;
    } else {
	*head_out = head_other;
	head_other->prev = list->prev;
	head_other->next = list;
	list->prev = head_other;
	list->next = NULL;
    }

    for (i = 0; i < level && remaining; i++) {
	remaining = sort_edges (remaining, i, &head_other);
	*head_out = merge_sorted_edges (*head_out, head_other);
    }

    return remaining;
}

static edge_t *
merge_unsorted_edges (edge_t *head, edge_t *unsorted)
{
    sort_edges (unsorted, UINT_MAX, &unsorted);
    return merge_sorted_edges (head, unsorted);
}

static void
active_edges_insert (sweep_line_t *sweep)
{
    edge_t *prev;
    int x;

    x = sweep->insert_x;
    prev = sweep->cursor;
    if (prev->x > x) {
	do {
	    prev = prev->prev;
	} while (prev->x > x);
    } else {
	while (prev->next->x < x)
	    prev = prev->next;
    }

    prev->next = merge_unsorted_edges (prev->next, sweep->insert);
    sweep->cursor = sweep->insert;
    sweep->insert = NULL;
    sweep->insert_x = INT_MAX;
}

static inline void
active_edges_to_traps (sweep_line_t *sweep)
{
    int top = sweep->current_y;
    edge_t *pos;

    if (sweep->last_y == sweep->current_y)
	return;

    if (sweep->insert)
	active_edges_insert (sweep);

    pos = sweep->head.next;
    if (pos == &sweep->tail)
	return;

    if (sweep->fill_rule == CAIRO_FILL_RULE_WINDING) {
	do {
	    edge_t *left, *right;
	    int winding;

	    left = pos;
	    winding = left->dir;

	    right = left->next;

	    /* Check if there is a co-linear edge with an existing trap */
	    while (right->x == left->x) {
		if (right->right != NULL) {
		    assert (left->right == NULL);
		    /* continuation on left */
		    left->top = right->top;
		    left->right = right->right;
		    right->right = NULL;
		}
		winding += right->dir;
		right = right->next;
	    }

	    if (winding == 0) {
		if (left->right != NULL)
		    edge_end_box (sweep, left, top);
		pos = right;
		continue;
	    }

	    do {
		/* End all subsumed traps */
		if (unlikely (right->right != NULL))
		    edge_end_box (sweep, right, top);

		/* Greedily search for the closing edge, so that we generate
		 * the * maximal span width with the minimal number of
		 * boxes.
		 */
		winding += right->dir;
		if (winding == 0 && right->x != right->next->x)
		    break;

		right = right->next;
	    } while (TRUE);

	    edge_start_or_continue_box (sweep, left, right, top);

	    pos = right->next;
	} while (pos != &sweep->tail);
    } else {
	do {
	    edge_t *right = pos->next;
	    int count = 0;

	    do {
		/* End all subsumed traps */
		if (unlikely (right->right != NULL))
		    edge_end_box (sweep, right, top);

		    /* skip co-linear edges */
		if (++count & 1 && right->x != right->next->x)
		    break;

		right = right->next;
	    } while (TRUE);

	    edge_start_or_continue_box (sweep, pos, right, top);

	    pos = right->next;
	} while (pos != &sweep->tail);
    }

    sweep->last_y = sweep->current_y;
}

static inline void
sweep_line_delete_edge (sweep_line_t *sweep, edge_t *edge)
{
    if (edge->right != NULL) {
	edge_t *next = edge->next;
	if (next->x == edge->x) {
	    next->top = edge->top;
	    next->right = edge->right;
	} else
	    edge_end_box (sweep, edge, sweep->current_y);
    }

    if (sweep->cursor == edge)
	sweep->cursor = edge->prev;

    edge->prev->next = edge->next;
    edge->next->prev = edge->prev;
}

static inline cairo_bool_t
sweep_line_delete (sweep_line_t	*sweep, rectangle_t *rectangle)
{
    cairo_bool_t update;

    update = TRUE;
    if (sweep->fill_rule == CAIRO_FILL_RULE_WINDING &&
	rectangle->left.prev->dir == rectangle->left.dir)
    {
	update = rectangle->left.next != &rectangle->right;
    }

    sweep_line_delete_edge (sweep, &rectangle->left);
    sweep_line_delete_edge (sweep, &rectangle->right);

    rectangle_pop_stop (sweep);
    return update;
}

static inline void
sweep_line_insert (sweep_line_t	*sweep, rectangle_t *rectangle)
{
    if (sweep->insert)
	sweep->insert->prev = &rectangle->right;
    rectangle->right.next = sweep->insert;
    rectangle->right.prev = &rectangle->left;
    rectangle->left.next = &rectangle->right;
    rectangle->left.prev = NULL;
    sweep->insert = &rectangle->left;
    if (rectangle->left.x < sweep->insert_x)
	sweep->insert_x = rectangle->left.x;

    pqueue_push (sweep, rectangle);
}

static cairo_status_t
_cairo_bentley_ottmann_tessellate_rectangular (rectangle_t	**rectangles,
					       int			  num_rectangles,
					       cairo_fill_rule_t	  fill_rule,
					       cairo_bool_t		 do_traps,
					       void			*container)
{
    sweep_line_t sweep_line;
    rectangle_t *rectangle;
    cairo_status_t status;
    cairo_bool_t update;

    sweep_line_init (&sweep_line,
		     rectangles, num_rectangles,
		     fill_rule,
		     do_traps, container);
    if ((status = setjmp (sweep_line.unwind)))
	return status;

    update = FALSE;

    rectangle = rectangle_pop_start (&sweep_line);
    do {
	if (rectangle->top != sweep_line.current_y) {
	    rectangle_t *stop;

	    stop = rectangle_peek_stop (&sweep_line);
	    while (stop != NULL && stop->bottom < rectangle->top) {
		if (stop->bottom != sweep_line.current_y) {
		    if (update) {
			active_edges_to_traps (&sweep_line);
			update = FALSE;
		    }

		    sweep_line.current_y = stop->bottom;
		}

		update |= sweep_line_delete (&sweep_line, stop);
		stop = rectangle_peek_stop (&sweep_line);
	    }

	    if (update) {
		active_edges_to_traps (&sweep_line);
		update = FALSE;
	    }

	    sweep_line.current_y = rectangle->top;
	}

	do {
	    sweep_line_insert (&sweep_line, rectangle);
	} while ((rectangle = rectangle_pop_start (&sweep_line)) != NULL &&
		 sweep_line.current_y == rectangle->top);
	update = TRUE;
    } while (rectangle);

    while ((rectangle = rectangle_peek_stop (&sweep_line)) != NULL) {
	if (rectangle->bottom != sweep_line.current_y) {
	    if (update) {
		active_edges_to_traps (&sweep_line);
		update = FALSE;
	    }
	    sweep_line.current_y = rectangle->bottom;
	}

	update |= sweep_line_delete (&sweep_line, rectangle);
    }

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_bentley_ottmann_tessellate_rectangular_traps (cairo_traps_t *traps,
						     cairo_fill_rule_t fill_rule)
{
    rectangle_t stack_rectangles[CAIRO_STACK_ARRAY_LENGTH (rectangle_t)];
    rectangle_t *stack_rectangles_ptrs[ARRAY_LENGTH (stack_rectangles) + 3];
    rectangle_t *rectangles, **rectangles_ptrs;
    cairo_status_t status;
    int i;

   assert (traps->is_rectangular);

    if (unlikely (traps->num_traps <= 1)) {
        if (traps->num_traps == 1) {
            cairo_trapezoid_t *trap = traps->traps;
            if (trap->left.p1.x > trap->right.p1.x) {
                cairo_line_t tmp = trap->left;
                trap->left = trap->right;
                trap->right = tmp;
            }
        }
	return CAIRO_STATUS_SUCCESS;
    }

    dump_traps (traps, "bo-rects-traps-in.txt");

    rectangles = stack_rectangles;
    rectangles_ptrs = stack_rectangles_ptrs;
    if (traps->num_traps > ARRAY_LENGTH (stack_rectangles)) {
	rectangles = _cairo_malloc_ab_plus_c (traps->num_traps,
					      sizeof (rectangle_t) +
					      sizeof (rectangle_t *),
					      3*sizeof (rectangle_t *));
	if (unlikely (rectangles == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	rectangles_ptrs = (rectangle_t **) (rectangles + traps->num_traps);
    }

    for (i = 0; i < traps->num_traps; i++) {
	if (traps->traps[i].left.p1.x < traps->traps[i].right.p1.x) {
	    rectangles[i].left.x = traps->traps[i].left.p1.x;
	    rectangles[i].left.dir = 1;

	    rectangles[i].right.x = traps->traps[i].right.p1.x;
	    rectangles[i].right.dir = -1;
	} else {
	    rectangles[i].right.x = traps->traps[i].left.p1.x;
	    rectangles[i].right.dir = 1;

	    rectangles[i].left.x = traps->traps[i].right.p1.x;
	    rectangles[i].left.dir = -1;
	}

	rectangles[i].left.right = NULL;
	rectangles[i].right.right = NULL;

	rectangles[i].top = traps->traps[i].top;
	rectangles[i].bottom = traps->traps[i].bottom;

	rectangles_ptrs[i+2] = &rectangles[i];
    }
    /* XXX incremental sort */
    _rectangle_sort (rectangles_ptrs+2, i);

    _cairo_traps_clear (traps);
    status = _cairo_bentley_ottmann_tessellate_rectangular (rectangles_ptrs+2, i,
							    fill_rule,
							    TRUE, traps);
    traps->is_rectilinear = TRUE;
    traps->is_rectangular = TRUE;

    if (rectangles != stack_rectangles)
	free (rectangles);

    dump_traps (traps, "bo-rects-traps-out.txt");

    return status;
}

cairo_status_t
_cairo_bentley_ottmann_tessellate_boxes (const cairo_boxes_t *in,
					 cairo_fill_rule_t fill_rule,
					 cairo_boxes_t *out)
{
    rectangle_t stack_rectangles[CAIRO_STACK_ARRAY_LENGTH (rectangle_t)];
    rectangle_t *stack_rectangles_ptrs[ARRAY_LENGTH (stack_rectangles) + 3];
    rectangle_t *rectangles, **rectangles_ptrs;
    rectangle_t *stack_rectangles_chain[CAIRO_STACK_ARRAY_LENGTH (rectangle_t *) ];
    rectangle_t **rectangles_chain = NULL;
    const struct _cairo_boxes_chunk *chunk;
    cairo_status_t status;
    int i, j, y_min, y_max;

    if (unlikely (in->num_boxes == 0)) {
	_cairo_boxes_clear (out);
	return CAIRO_STATUS_SUCCESS;
    }

    if (in->num_boxes == 1) {
	if (in == out) {
	    cairo_box_t *box = &in->chunks.base[0];

	    if (box->p1.x > box->p2.x) {
		cairo_fixed_t tmp = box->p1.x;
		box->p1.x = box->p2.x;
		box->p2.x = tmp;
	    }
	} else {
	    cairo_box_t box = in->chunks.base[0];

	    if (box.p1.x > box.p2.x) {
		cairo_fixed_t tmp = box.p1.x;
		box.p1.x = box.p2.x;
		box.p2.x = tmp;
	    }

	    _cairo_boxes_clear (out);
	    status = _cairo_boxes_add (out, CAIRO_ANTIALIAS_DEFAULT, &box);
	    assert (status == CAIRO_STATUS_SUCCESS);
	}
	return CAIRO_STATUS_SUCCESS;
    }

    y_min = INT_MAX; y_max = INT_MIN;
    for (chunk = &in->chunks; chunk != NULL; chunk = chunk->next) {
	const cairo_box_t *box = chunk->base;
	for (i = 0; i < chunk->count; i++) {
	    if (box[i].p1.y < y_min)
		y_min = box[i].p1.y;
	    if (box[i].p1.y > y_max)
		y_max = box[i].p1.y;
	}
    }
    y_min = _cairo_fixed_integer_floor (y_min);
    y_max = _cairo_fixed_integer_floor (y_max) + 1;
    y_max -= y_min;

    if (y_max < in->num_boxes) {
	rectangles_chain = stack_rectangles_chain;
	if (y_max > ARRAY_LENGTH (stack_rectangles_chain)) {
	    rectangles_chain = _cairo_malloc_ab (y_max, sizeof (rectangle_t *));
	    if (unlikely (rectangles_chain == NULL))
		return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	}
	memset (rectangles_chain, 0, y_max * sizeof (rectangle_t*));
    }

    rectangles = stack_rectangles;
    rectangles_ptrs = stack_rectangles_ptrs;
    if (in->num_boxes > ARRAY_LENGTH (stack_rectangles)) {
	rectangles = _cairo_malloc_ab_plus_c (in->num_boxes,
					      sizeof (rectangle_t) +
					      sizeof (rectangle_t *),
					      3*sizeof (rectangle_t *));
	if (unlikely (rectangles == NULL)) {
	    if (rectangles_chain != stack_rectangles_chain)
		free (rectangles_chain);
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	}

	rectangles_ptrs = (rectangle_t **) (rectangles + in->num_boxes);
    }

    j = 0;
    for (chunk = &in->chunks; chunk != NULL; chunk = chunk->next) {
	const cairo_box_t *box = chunk->base;
	for (i = 0; i < chunk->count; i++) {
	    int h;

	    if (box[i].p1.x < box[i].p2.x) {
		rectangles[j].left.x = box[i].p1.x;
		rectangles[j].left.dir = 1;

		rectangles[j].right.x = box[i].p2.x;
		rectangles[j].right.dir = -1;
	    } else {
		rectangles[j].right.x = box[i].p1.x;
		rectangles[j].right.dir = 1;

		rectangles[j].left.x = box[i].p2.x;
		rectangles[j].left.dir = -1;
	    }

	    rectangles[j].left.right = NULL;
	    rectangles[j].right.right = NULL;

	    rectangles[j].top = box[i].p1.y;
	    rectangles[j].bottom = box[i].p2.y;

	    if (rectangles_chain) {
		h = _cairo_fixed_integer_floor (box[i].p1.y) - y_min;
		rectangles[j].left.next = (edge_t *)rectangles_chain[h];
		rectangles_chain[h] = &rectangles[j];
	    } else {
		rectangles_ptrs[j+2] = &rectangles[j];
	    }
	    j++;
	}
    }

    if (rectangles_chain) {
	j = 2;
	for (y_min = 0; y_min < y_max; y_min++) {
	    rectangle_t *r;
	    int start = j;
	    for (r = rectangles_chain[y_min]; r; r = (rectangle_t *)r->left.next)
		rectangles_ptrs[j++] = r;
	    if (j > start + 1)
		_rectangle_sort (rectangles_ptrs + start, j - start);
	}

	if (rectangles_chain != stack_rectangles_chain)
	    free (rectangles_chain);

	j -= 2;
    } else {
	_rectangle_sort (rectangles_ptrs + 2, j);
    }

    _cairo_boxes_clear (out);
    status = _cairo_bentley_ottmann_tessellate_rectangular (rectangles_ptrs+2, j,
							    fill_rule,
							    FALSE, out);
    if (rectangles != stack_rectangles)
	free (rectangles);

    return status;
}
