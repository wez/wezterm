/*
 * Copyright © 2004 Carl Worth
 * Copyright © 2006 Red Hat, Inc.
 * Copyright © 2009 Chris Wilson
 * Copyright © 2011 Intel Corporation
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

#include <setjmp.h>

typedef struct _rectangle rectangle_t;
typedef struct _edge edge_t;

struct _edge {
    edge_t *next, *prev;
    edge_t *right;
    cairo_fixed_t x, top;
    int a_or_b;
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

typedef struct _pqueue {
    int size, max_size;

    rectangle_t **elements;
    rectangle_t *elements_embedded[1024];
} pqueue_t;

typedef struct _sweep_line {
    rectangle_t **rectangles;
    pqueue_t pq;
    edge_t head, tail;
    edge_t *insert_left, *insert_right;
    int32_t current_y;
    int32_t last_y;

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
pqueue_init (pqueue_t *pq)
{
    pq->max_size = ARRAY_LENGTH (pq->elements_embedded);
    pq->size = 0;

    pq->elements = pq->elements_embedded;
    pq->elements[PQ_FIRST_ENTRY] = NULL;
}

static inline void
pqueue_fini (pqueue_t *pq)
{
    if (pq->elements != pq->elements_embedded)
	free (pq->elements);
}

static cairo_bool_t
pqueue_grow (pqueue_t *pq)
{
    rectangle_t **new_elements;
    pq->max_size *= 2;

    if (pq->elements == pq->elements_embedded) {
	new_elements = _cairo_malloc_ab (pq->max_size,
					 sizeof (rectangle_t *));
	if (unlikely (new_elements == NULL))
	    return FALSE;

	memcpy (new_elements, pq->elements_embedded,
		sizeof (pq->elements_embedded));
    } else {
	new_elements = _cairo_realloc_ab (pq->elements,
					  pq->max_size,
					  sizeof (rectangle_t *));
	if (unlikely (new_elements == NULL))
	    return FALSE;
    }

    pq->elements = new_elements;
    return TRUE;
}

static inline void
pqueue_push (sweep_line_t *sweep, rectangle_t *rectangle)
{
    rectangle_t **elements;
    int i, parent;

    if (unlikely (sweep->pq.size + 1 == sweep->pq.max_size)) {
	if (unlikely (! pqueue_grow (&sweep->pq))) {
	    longjmp (sweep->unwind,
		     _cairo_error (CAIRO_STATUS_NO_MEMORY));
	}
    }

    elements = sweep->pq.elements;
    for (i = ++sweep->pq.size;
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
pqueue_pop (pqueue_t *pq)
{
    rectangle_t **elements = pq->elements;
    rectangle_t *tail;
    int child, i;

    tail = elements[pq->size--];
    if (pq->size == 0) {
	elements[PQ_FIRST_ENTRY] = NULL;
	return;
    }

    for (i = PQ_FIRST_ENTRY;
	 (child = PQ_LEFT_CHILD_INDEX (i)) <= pq->size;
	 i = child)
    {
	if (child != pq->size &&
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
    return sweep_line->pq.elements[PQ_FIRST_ENTRY];
}

CAIRO_COMBSORT_DECLARE (_rectangle_sort,
			rectangle_t *,
			rectangle_compare_start)

static void
sweep_line_init (sweep_line_t	 *sweep_line,
		 rectangle_t	**rectangles,
		 int		  num_rectangles)
{
    _rectangle_sort (rectangles, num_rectangles);
    rectangles[num_rectangles] = NULL;
    sweep_line->rectangles = rectangles;

    sweep_line->head.x = INT32_MIN;
    sweep_line->head.right = NULL;
    sweep_line->head.dir = 0;
    sweep_line->head.next = &sweep_line->tail;
    sweep_line->tail.x = INT32_MAX;
    sweep_line->tail.right = NULL;
    sweep_line->tail.dir = 0;
    sweep_line->tail.prev = &sweep_line->head;

    sweep_line->insert_left = &sweep_line->tail;
    sweep_line->insert_right = &sweep_line->tail;

    sweep_line->current_y = INT32_MIN;
    sweep_line->last_y = INT32_MIN;

    pqueue_init (&sweep_line->pq);
}

static void
sweep_line_fini (sweep_line_t *sweep_line)
{
    pqueue_fini (&sweep_line->pq);
}

static void
end_box (sweep_line_t *sweep_line, edge_t *left, int32_t bot, cairo_boxes_t *out)
{
    if (likely (left->top < bot)) {
	cairo_status_t status;
	cairo_box_t box;

	box.p1.x = left->x;
	box.p1.y = left->top;
	box.p2.x = left->right->x;
	box.p2.y = bot;

	status = _cairo_boxes_add (out, CAIRO_ANTIALIAS_DEFAULT, &box);
	if (unlikely (status))
	    longjmp (sweep_line->unwind, status);
    }

    left->right = NULL;
}

/* Start a new trapezoid at the given top y coordinate, whose edges
 * are `edge' and `edge->next'. If `edge' already has a trapezoid,
 * then either add it to the traps in `traps', if the trapezoid's
 * right edge differs from `edge->next', or do nothing if the new
 * trapezoid would be a continuation of the existing one. */
static inline void
start_or_continue_box (sweep_line_t *sweep_line,
		       edge_t	*left,
		       edge_t	*right,
		       int		 top,
		       cairo_boxes_t *out)
{
    if (left->right == right)
	return;

    if (left->right != NULL) {
	if (right != NULL && left->right->x == right->x) {
	    /* continuation on right, so just swap edges */
	    left->right = right;
	    return;
	}

	end_box (sweep_line, left, top, out);
    }

    if (right != NULL && left->x != right->x) {
	left->top = top;
	left->right = right;
    }
}

static inline int is_zero(const int *winding)
{
    return winding[0] == 0 || winding[1] == 0;
}

static inline void
active_edges (sweep_line_t *sweep, cairo_boxes_t *out)
{
    int top = sweep->current_y;
    int winding[2] = { 0 };
    edge_t *pos;

    if (sweep->last_y == sweep->current_y)
	return;

    pos = sweep->head.next;
    if (pos == &sweep->tail)
	return;

    do {
	edge_t *left, *right;

	left = pos;
	do {
	    winding[left->a_or_b] += left->dir;
	    if (!is_zero (winding))
		break;
	    if (left->next == &sweep->tail)
		goto out;

	    if (unlikely (left->right != NULL))
		end_box (sweep, left, top, out);

	    left = left->next;
	} while (1);

	right = left->next;
	do {
	    if (unlikely (right->right != NULL))
		end_box (sweep, right, top, out);

	    winding[right->a_or_b] += right->dir;
	    if (is_zero (winding)) {
		/* skip co-linear edges */
		if (likely (right->x != right->next->x))
		    break;
	    }

	    right = right->next;
	} while (TRUE);

	start_or_continue_box (sweep, left, right, top, out);

	pos = right->next;
    } while (pos != &sweep->tail);

out:
    sweep->last_y = sweep->current_y;
}

static inline void
sweep_line_delete_edge (sweep_line_t *sweep_line, edge_t *edge, cairo_boxes_t *out)
{
    if (edge->right != NULL) {
	edge_t *next = edge->next;
	if (next->x == edge->x) {
	    next->top = edge->top;
	    next->right = edge->right;
	} else {
	    end_box (sweep_line, edge, sweep_line->current_y, out);
	}
    }

    if (sweep_line->insert_left == edge)
	sweep_line->insert_left = edge->next;
    if (sweep_line->insert_right == edge)
	sweep_line->insert_right = edge->next;

    edge->prev->next = edge->next;
    edge->next->prev = edge->prev;
}

static inline void
sweep_line_delete (sweep_line_t	*sweep,
		   rectangle_t	*rectangle,
		   cairo_boxes_t *out)
{
    sweep_line_delete_edge (sweep, &rectangle->left, out);
    sweep_line_delete_edge (sweep, &rectangle->right, out);

    pqueue_pop (&sweep->pq);
}

static inline void
insert_edge (edge_t *edge, edge_t *pos)
{
    if (pos->x != edge->x) {
	if (pos->x > edge->x) {
	    do {
		UNROLL3({
		    if (pos->prev->x <= edge->x)
			break;
		    pos = pos->prev;
		})
	    } while (TRUE);
	} else {
	    do {
		UNROLL3({
		    pos = pos->next;
		    if (pos->x >= edge->x)
			break;
		})
	    } while (TRUE);
	}
    }

    pos->prev->next = edge;
    edge->prev = pos->prev;
    edge->next = pos;
    pos->prev = edge;
}

static inline void
sweep_line_insert (sweep_line_t	*sweep, rectangle_t	*rectangle)
{
    edge_t *pos;

    /* right edge */
    pos = sweep->insert_right;
    insert_edge (&rectangle->right, pos);
    sweep->insert_right = &rectangle->right;

    /* left edge */
    pos = sweep->insert_left;
    if (pos->x > sweep->insert_right->x)
	pos = sweep->insert_right->prev;
    insert_edge (&rectangle->left, pos);
    sweep->insert_left = &rectangle->left;

    pqueue_push (sweep, rectangle);
}

static cairo_status_t
intersect (rectangle_t **rectangles, int num_rectangles, cairo_boxes_t *out)
{
    sweep_line_t sweep_line;
    rectangle_t *rectangle;
    cairo_status_t status;

    sweep_line_init (&sweep_line, rectangles, num_rectangles);
    if ((status = setjmp (sweep_line.unwind)))
	goto unwind;

    rectangle = rectangle_pop_start (&sweep_line);
    do {
	if (rectangle->top != sweep_line.current_y) {
	    rectangle_t *stop;

	    stop = rectangle_peek_stop (&sweep_line);
	    while (stop != NULL && stop->bottom < rectangle->top) {
		if (stop->bottom != sweep_line.current_y) {
		    active_edges (&sweep_line, out);
		    sweep_line.current_y = stop->bottom;
		}

		sweep_line_delete (&sweep_line, stop, out);

		stop = rectangle_peek_stop (&sweep_line);
	    }

	    active_edges (&sweep_line, out);
	    sweep_line.current_y = rectangle->top;
	}

	sweep_line_insert (&sweep_line, rectangle);
    } while ((rectangle = rectangle_pop_start (&sweep_line)) != NULL);

    while ((rectangle = rectangle_peek_stop (&sweep_line)) != NULL) {
	if (rectangle->bottom != sweep_line.current_y) {
	    active_edges (&sweep_line, out);
	    sweep_line.current_y = rectangle->bottom;
	}

	sweep_line_delete (&sweep_line, rectangle, out);
    }

unwind:
    sweep_line_fini (&sweep_line);
    return status;
}

static cairo_status_t
_cairo_boxes_intersect_with_box (const cairo_boxes_t *boxes,
				 const cairo_box_t *box,
				 cairo_boxes_t *out)
{
    cairo_status_t status;
    int i, j;

    if (out == boxes) { /* inplace update */
	struct _cairo_boxes_chunk *chunk;

	out->num_boxes = 0;
	for (chunk = &out->chunks; chunk != NULL; chunk = chunk->next) {
	    for (i = j = 0; i < chunk->count; i++) {
		cairo_box_t *b = &chunk->base[i];

		b->p1.x = MAX (b->p1.x, box->p1.x);
		b->p1.y = MAX (b->p1.y, box->p1.y);
		b->p2.x = MIN (b->p2.x, box->p2.x);
		b->p2.y = MIN (b->p2.y, box->p2.y);
		if (b->p1.x < b->p2.x && b->p1.y < b->p2.y) {
		    if (i != j)
			chunk->base[j] = *b;
		    j++;
		}
	    }
	    /* XXX unlink empty chains? */
	    chunk->count = j;
	    out->num_boxes += j;
	}
    } else {
	const struct _cairo_boxes_chunk *chunk;

	_cairo_boxes_clear (out);
	_cairo_boxes_limit (out, box, 1);
	for (chunk = &boxes->chunks; chunk != NULL; chunk = chunk->next) {
	    for (i = 0; i < chunk->count; i++) {
		status = _cairo_boxes_add (out,
					   CAIRO_ANTIALIAS_DEFAULT,
					   &chunk->base[i]);
		if (unlikely (status))
		    return status;
	    }
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_boxes_intersect (const cairo_boxes_t *a,
			const cairo_boxes_t *b,
			cairo_boxes_t *out)
{
    rectangle_t stack_rectangles[CAIRO_STACK_ARRAY_LENGTH (rectangle_t)];
    rectangle_t *rectangles;
    rectangle_t *stack_rectangles_ptrs[ARRAY_LENGTH (stack_rectangles) + 1];
    rectangle_t **rectangles_ptrs;
    const struct _cairo_boxes_chunk *chunk;
    cairo_status_t status;
    int i, j, count;

    if (unlikely (a->num_boxes == 0 || b->num_boxes == 0)) {
	_cairo_boxes_clear (out);
	return CAIRO_STATUS_SUCCESS;
    }

    if (a->num_boxes == 1) {
	cairo_box_t box = a->chunks.base[0];
	return _cairo_boxes_intersect_with_box (b, &box, out);
    }
    if (b->num_boxes == 1) {
	cairo_box_t box = b->chunks.base[0];
	return _cairo_boxes_intersect_with_box (a, &box, out);
    }

    rectangles = stack_rectangles;
    rectangles_ptrs = stack_rectangles_ptrs;
    count = a->num_boxes + b->num_boxes;
    if (count > ARRAY_LENGTH (stack_rectangles)) {
	rectangles = _cairo_malloc_ab_plus_c (count,
					      sizeof (rectangle_t) +
					      sizeof (rectangle_t *),
					      sizeof (rectangle_t *));
	if (unlikely (rectangles == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	rectangles_ptrs = (rectangle_t **) (rectangles + count);
    }

    j = 0;
    for (chunk = &a->chunks; chunk != NULL; chunk = chunk->next) {
	const cairo_box_t *box = chunk->base;
	for (i = 0; i < chunk->count; i++) {
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

	    rectangles[j].left.a_or_b = 0;
	    rectangles[j].left.right = NULL;
	    rectangles[j].right.a_or_b = 0;
	    rectangles[j].right.right = NULL;

	    rectangles[j].top = box[i].p1.y;
	    rectangles[j].bottom = box[i].p2.y;

	    rectangles_ptrs[j] = &rectangles[j];
	    j++;
	}
    }
    for (chunk = &b->chunks; chunk != NULL; chunk = chunk->next) {
	const cairo_box_t *box = chunk->base;
	for (i = 0; i < chunk->count; i++) {
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

	    rectangles[j].left.a_or_b = 1;
	    rectangles[j].left.right = NULL;
	    rectangles[j].right.a_or_b = 1;
	    rectangles[j].right.right = NULL;

	    rectangles[j].top = box[i].p1.y;
	    rectangles[j].bottom = box[i].p2.y;

	    rectangles_ptrs[j] = &rectangles[j];
	    j++;
	}
    }
    assert (j == count);

    _cairo_boxes_clear (out);
    status = intersect (rectangles_ptrs, j, out);
    if (rectangles != stack_rectangles)
	free (rectangles);

    return status;
}
