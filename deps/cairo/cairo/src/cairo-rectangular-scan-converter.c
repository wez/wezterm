/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2009 Intel Corporation
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
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-combsort-inline.h"
#include "cairo-error-private.h"
#include "cairo-freelist-private.h"
#include "cairo-list-private.h"
#include "cairo-spans-private.h"

#include <setjmp.h>

typedef struct _rectangle {
    struct _rectangle *next, *prev;
    cairo_fixed_t left, right;
    cairo_fixed_t top, bottom;
    int32_t top_y, bottom_y;
    int dir;
} rectangle_t;

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

typedef struct {
    rectangle_t **start;
    pqueue_t stop;
    rectangle_t head, tail;
    rectangle_t *insert_cursor;
    int32_t current_y;
    int32_t xmin, xmax;

    struct coverage {
	struct cell {
	    struct cell *prev, *next;
	    int x, covered, uncovered;
	} head, tail, *cursor;
	unsigned int count;
	cairo_freepool_t pool;
    } coverage;

    cairo_half_open_span_t spans_stack[CAIRO_STACK_ARRAY_LENGTH (cairo_half_open_span_t)];
    cairo_half_open_span_t *spans;
    unsigned int num_spans;
    unsigned int size_spans;

    jmp_buf jmpbuf;
} sweep_line_t;

static inline int
rectangle_compare_start (const rectangle_t *a,
			 const rectangle_t *b)
{
    int cmp;

    cmp = a->top_y - b->top_y;
    if (cmp)
	return cmp;

    return a->left - b->left;
}

static inline int
rectangle_compare_stop (const rectangle_t *a,
			const rectangle_t *b)
{
    return a->bottom_y - b->bottom_y;
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

    if (unlikely (sweep->stop.size + 1 == sweep->stop.max_size)) {
	if (unlikely (! pqueue_grow (&sweep->stop)))
	    longjmp (sweep->jmpbuf,
		     _cairo_error (CAIRO_STATUS_NO_MEMORY));
    }

    elements = sweep->stop.elements;
    for (i = ++sweep->stop.size;
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
peek_stop (sweep_line_t *sweep)
{
    return sweep->stop.elements[PQ_FIRST_ENTRY];
}

CAIRO_COMBSORT_DECLARE (rectangle_sort, rectangle_t *, rectangle_compare_start)

static void
sweep_line_init (sweep_line_t *sweep)
{
    sweep->head.left = INT_MIN;
    sweep->head.next = &sweep->tail;
    sweep->tail.left = INT_MAX;
    sweep->tail.prev = &sweep->head;
    sweep->insert_cursor = &sweep->tail;

    _cairo_freepool_init (&sweep->coverage.pool, sizeof (struct cell));

    sweep->spans = sweep->spans_stack;
    sweep->size_spans = ARRAY_LENGTH (sweep->spans_stack);

    sweep->coverage.head.prev = NULL;
    sweep->coverage.head.x = INT_MIN;
    sweep->coverage.tail.next = NULL;
    sweep->coverage.tail.x = INT_MAX;

    pqueue_init (&sweep->stop);
}

static void
sweep_line_fini (sweep_line_t *sweep)
{
    _cairo_freepool_fini (&sweep->coverage.pool);
    pqueue_fini (&sweep->stop);

    if (sweep->spans != sweep->spans_stack)
	free (sweep->spans);
}

static inline void
add_cell (sweep_line_t *sweep, int x, int covered, int uncovered)
{
    struct cell *cell;

    cell = sweep->coverage.cursor;
    if (cell->x > x) {
	do {
	    UNROLL3({
		if (cell->prev->x < x)
		    break;
		cell = cell->prev;
	    })
	} while (TRUE);
    } else {
	if (cell->x == x)
	    goto found;

	do {
	    UNROLL3({
		cell = cell->next;
		if (cell->x >= x)
		    break;
	    })
	} while (TRUE);
    }

    if (x != cell->x) {
	struct cell *c;

	sweep->coverage.count++;

	c = _cairo_freepool_alloc (&sweep->coverage.pool);
	if (unlikely (c == NULL)) {
	    longjmp (sweep->jmpbuf,
		     _cairo_error (CAIRO_STATUS_NO_MEMORY));
	}

	cell->prev->next = c;
	c->prev = cell->prev;
	c->next = cell;
	cell->prev = c;

	c->x = x;
	c->covered = 0;
	c->uncovered = 0;

	cell = c;
    }

found:
    cell->covered += covered;
    cell->uncovered += uncovered;
    sweep->coverage.cursor = cell;
}

static inline void
_active_edges_to_spans (sweep_line_t	*sweep)
{
    int32_t y = sweep->current_y;
    rectangle_t *rectangle;
    int coverage, prev_coverage;
    int prev_x;
    struct cell *cell;

    sweep->num_spans = 0;
    if (sweep->head.next == &sweep->tail)
	return;

    sweep->coverage.head.next = &sweep->coverage.tail;
    sweep->coverage.tail.prev = &sweep->coverage.head;
    sweep->coverage.cursor = &sweep->coverage.tail;
    sweep->coverage.count = 0;

    /* XXX cell coverage only changes when a rectangle appears or
     * disappears. Try only modifying coverage at such times.
     */
    for (rectangle = sweep->head.next;
	 rectangle != &sweep->tail;
	 rectangle = rectangle->next)
    {
	int height;
	int frac, i;

	if (y == rectangle->bottom_y) {
	    height = rectangle->bottom & CAIRO_FIXED_FRAC_MASK;
	    if (height == 0)
		continue;
	} else
	    height = CAIRO_FIXED_ONE;
	if (y == rectangle->top_y)
	    height -= rectangle->top & CAIRO_FIXED_FRAC_MASK;
	height *= rectangle->dir;

	i = _cairo_fixed_integer_part (rectangle->left),
	frac = _cairo_fixed_fractional_part (rectangle->left);
	add_cell (sweep, i,
		  (CAIRO_FIXED_ONE-frac) * height,
		  frac * height);

	i = _cairo_fixed_integer_part (rectangle->right),
	frac = _cairo_fixed_fractional_part (rectangle->right);
	add_cell (sweep, i,
		  -(CAIRO_FIXED_ONE-frac) * height,
		  -frac * height);
    }

    if (2*sweep->coverage.count >= sweep->size_spans) {
	unsigned size;

	size = sweep->size_spans;
	while (size <= 2*sweep->coverage.count)
	    size <<= 1;

	if (sweep->spans != sweep->spans_stack)
	    free (sweep->spans);

	sweep->spans = _cairo_malloc_ab (size, sizeof (cairo_half_open_span_t));
	if (unlikely (sweep->spans == NULL))
	    longjmp (sweep->jmpbuf, _cairo_error (CAIRO_STATUS_NO_MEMORY));

	sweep->size_spans = size;
    }

    prev_coverage = coverage = 0;
    prev_x = INT_MIN;
    for (cell = sweep->coverage.head.next; cell != &sweep->coverage.tail; cell = cell->next) {
	if (cell->x != prev_x && coverage != prev_coverage) {
	    int n = sweep->num_spans++;
	    int c = coverage >> (CAIRO_FIXED_FRAC_BITS * 2 - 8);
	    sweep->spans[n].x = prev_x;
	    sweep->spans[n].inverse = 0;
	    sweep->spans[n].coverage = c - (c >> 8);
	    prev_coverage = coverage;
	}

	coverage += cell->covered;
	if (coverage != prev_coverage) {
	    int n = sweep->num_spans++;
	    int c = coverage >> (CAIRO_FIXED_FRAC_BITS * 2 - 8);
	    sweep->spans[n].x = cell->x;
	    sweep->spans[n].inverse = 0;
	    sweep->spans[n].coverage = c - (c >> 8);
	    prev_coverage = coverage;
	}
	coverage += cell->uncovered;
	prev_x = cell->x + 1;
    }
    _cairo_freepool_reset (&sweep->coverage.pool);

    if (sweep->num_spans) {
	if (prev_x <= sweep->xmax) {
	    int n = sweep->num_spans++;
	    int c = coverage >> (CAIRO_FIXED_FRAC_BITS * 2 - 8);
	    sweep->spans[n].x = prev_x;
	    sweep->spans[n].inverse = 0;
	    sweep->spans[n].coverage = c - (c >> 8);
	}

	if (coverage && prev_x < sweep->xmax) {
	    int n = sweep->num_spans++;
	    sweep->spans[n].x = sweep->xmax;
	    sweep->spans[n].inverse = 1;
	    sweep->spans[n].coverage = 0;
	}
    }
}

static inline void
sweep_line_delete (sweep_line_t	*sweep,
			     rectangle_t	*rectangle)
{
    if (sweep->insert_cursor == rectangle)
	sweep->insert_cursor = rectangle->next;

    rectangle->prev->next = rectangle->next;
    rectangle->next->prev = rectangle->prev;

    pqueue_pop (&sweep->stop);
}

static inline void
sweep_line_insert (sweep_line_t	*sweep,
		   rectangle_t	*rectangle)
{
    rectangle_t *pos;

    pos = sweep->insert_cursor;
    if (pos->left != rectangle->left) {
	if (pos->left > rectangle->left) {
	    do {
		UNROLL3({
		    if (pos->prev->left < rectangle->left)
			break;
		    pos = pos->prev;
		})
	    } while (TRUE);
	} else {
	    do {
		UNROLL3({
		    pos = pos->next;
		    if (pos->left >= rectangle->left)
			break;
		});
	    } while (TRUE);
	}
    }

    pos->prev->next = rectangle;
    rectangle->prev = pos->prev;
    rectangle->next = pos;
    pos->prev = rectangle;
    sweep->insert_cursor = rectangle;

    pqueue_push (sweep, rectangle);
}

static void
render_rows (sweep_line_t *sweep_line,
	     cairo_span_renderer_t *renderer,
	     int height)
{
    cairo_status_t status;

    _active_edges_to_spans (sweep_line);

    status = renderer->render_rows (renderer,
				    sweep_line->current_y, height,
				    sweep_line->spans,
				    sweep_line->num_spans);
    if (unlikely (status))
	longjmp (sweep_line->jmpbuf, status);
}

static cairo_status_t
generate (cairo_rectangular_scan_converter_t *self,
	  cairo_span_renderer_t	*renderer,
	  rectangle_t **rectangles)
{
    sweep_line_t sweep_line;
    rectangle_t *start, *stop;
    cairo_status_t status;

    sweep_line_init (&sweep_line);
    sweep_line.xmin = _cairo_fixed_integer_part (self->extents.p1.x);
    sweep_line.xmax = _cairo_fixed_integer_part (self->extents.p2.x);
    sweep_line.start = rectangles;
    if ((status = setjmp (sweep_line.jmpbuf)))
	goto out;

    sweep_line.current_y = _cairo_fixed_integer_part (self->extents.p1.y);
    start = *sweep_line.start++;
    do {
	if (start->top_y != sweep_line.current_y) {
	    render_rows (&sweep_line, renderer,
			 start->top_y - sweep_line.current_y);
	    sweep_line.current_y = start->top_y;
	}

	do {
	    sweep_line_insert (&sweep_line, start);
	    start = *sweep_line.start++;
	    if (start == NULL)
		goto end;
	    if (start->top_y != sweep_line.current_y)
		break;
	} while (TRUE);

	render_rows (&sweep_line, renderer, 1);

	stop = peek_stop (&sweep_line);
	while (stop->bottom_y == sweep_line.current_y) {
	    sweep_line_delete (&sweep_line, stop);
	    stop = peek_stop (&sweep_line);
	    if (stop == NULL)
		break;
	}

	sweep_line.current_y++;

	while (stop != NULL && stop->bottom_y < start->top_y) {
	    if (stop->bottom_y != sweep_line.current_y) {
		render_rows (&sweep_line, renderer,
			     stop->bottom_y - sweep_line.current_y);
		sweep_line.current_y = stop->bottom_y;
	    }

	    render_rows (&sweep_line, renderer, 1);

	    do {
		sweep_line_delete (&sweep_line, stop);
		stop = peek_stop (&sweep_line);
	    } while (stop != NULL && stop->bottom_y == sweep_line.current_y);

	    sweep_line.current_y++;
	}
    } while (TRUE);

  end:
    render_rows (&sweep_line, renderer, 1);

    stop = peek_stop (&sweep_line);
    while (stop->bottom_y == sweep_line.current_y) {
	sweep_line_delete (&sweep_line, stop);
	stop = peek_stop (&sweep_line);
	if (stop == NULL)
	    goto out;
    }

    while (++sweep_line.current_y < _cairo_fixed_integer_part (self->extents.p2.y)) {
	if (stop->bottom_y != sweep_line.current_y) {
	    render_rows (&sweep_line, renderer,
			 stop->bottom_y - sweep_line.current_y);
	    sweep_line.current_y = stop->bottom_y;
	}

	render_rows (&sweep_line, renderer, 1);

	do {
	    sweep_line_delete (&sweep_line, stop);
	    stop = peek_stop (&sweep_line);
	    if (stop == NULL)
		goto out;
	} while (stop->bottom_y == sweep_line.current_y);

    }

  out:
    sweep_line_fini (&sweep_line);

    return status;
}
static void generate_row(cairo_span_renderer_t *renderer,
			 const rectangle_t *r,
			 int y, int h,
			 uint16_t coverage)
{
    cairo_half_open_span_t spans[4];
    unsigned int num_spans = 0;
    int x1 = _cairo_fixed_integer_part (r->left);
    int x2 = _cairo_fixed_integer_part (r->right);
    if (x2 > x1) {
	if (! _cairo_fixed_is_integer (r->left)) {
	    spans[num_spans].x = x1;
	    spans[num_spans].coverage =
		coverage * (256 - _cairo_fixed_fractional_part (r->left)) >> 8;
	    num_spans++;
	    x1++;
	}

	if (x2 > x1) {
	    spans[num_spans].x = x1;
	    spans[num_spans].coverage = coverage - (coverage >> 8);
	    num_spans++;
	}

	if (! _cairo_fixed_is_integer (r->right)) {
	    spans[num_spans].x = x2++;
	    spans[num_spans].coverage =
		coverage * _cairo_fixed_fractional_part (r->right) >> 8;
	    num_spans++;
	}
    } else {
	spans[num_spans].x = x2++;
	spans[num_spans].coverage = coverage * (r->right - r->left) >> 8;
	num_spans++;
    }

    spans[num_spans].x = x2;
    spans[num_spans].coverage = 0;
    num_spans++;

    renderer->render_rows (renderer, y, h, spans, num_spans);
}

static cairo_status_t
generate_box (cairo_rectangular_scan_converter_t *self,
	      cairo_span_renderer_t	*renderer)
{
    const rectangle_t *r = self->chunks.base;
    int y1 = _cairo_fixed_integer_part (r->top);
    int y2 = _cairo_fixed_integer_part (r->bottom);
    if (y2 > y1) {
	if (! _cairo_fixed_is_integer (r->top)) {
	    generate_row(renderer, r, y1, 1,
			 256 - _cairo_fixed_fractional_part (r->top));
	    y1++;
	}

	if (y2 > y1)
	    generate_row(renderer, r, y1, y2-y1, 256);

	if (! _cairo_fixed_is_integer (r->bottom))
	    generate_row(renderer, r, y2, 1,
			 _cairo_fixed_fractional_part (r->bottom));
    } else
	generate_row(renderer, r, y1, 1, r->bottom - r->top);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_rectangular_scan_converter_generate (void			*converter,
					    cairo_span_renderer_t	*renderer)
{
    cairo_rectangular_scan_converter_t *self = converter;
    rectangle_t *rectangles_stack[CAIRO_STACK_ARRAY_LENGTH (rectangle_t *)];
    rectangle_t **rectangles;
    struct _cairo_rectangular_scan_converter_chunk *chunk;
    cairo_status_t status;
    int i, j;

    if (unlikely (self->num_rectangles == 0)) {
	return renderer->render_rows (renderer,
				      _cairo_fixed_integer_part (self->extents.p1.y),
				      _cairo_fixed_integer_part (self->extents.p2.y - self->extents.p1.y),
				      NULL, 0);
    }

    if (self->num_rectangles == 1)
	return generate_box (self, renderer);

    rectangles = rectangles_stack;
    if (unlikely (self->num_rectangles >= ARRAY_LENGTH (rectangles_stack))) {
	rectangles = _cairo_malloc_ab (self->num_rectangles + 1,
				       sizeof (rectangle_t *));
	if (unlikely (rectangles == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    j = 0;
    for (chunk = &self->chunks; chunk != NULL; chunk = chunk->next) {
	rectangle_t *rectangle;

	rectangle = chunk->base;
	for (i = 0; i < chunk->count; i++)
	    rectangles[j++] = &rectangle[i];
    }
    rectangle_sort (rectangles, j);
    rectangles[j] = NULL;

    status = generate (self, renderer, rectangles);

    if (rectangles != rectangles_stack)
	free (rectangles);

    return status;
}

static rectangle_t *
_allocate_rectangle (cairo_rectangular_scan_converter_t *self)
{
    rectangle_t *rectangle;
    struct _cairo_rectangular_scan_converter_chunk *chunk;

    chunk = self->tail;
    if (chunk->count == chunk->size) {
	int size;

	size = chunk->size * 2;
	chunk->next = _cairo_malloc_ab_plus_c (size,
					       sizeof (rectangle_t),
					       sizeof (struct _cairo_rectangular_scan_converter_chunk));

	if (unlikely (chunk->next == NULL))
	    return NULL;

	chunk = chunk->next;
	chunk->next = NULL;
	chunk->count = 0;
	chunk->size = size;
	chunk->base = chunk + 1;
	self->tail = chunk;
    }

    rectangle = chunk->base;
    return rectangle + chunk->count++;
}

cairo_status_t
_cairo_rectangular_scan_converter_add_box (cairo_rectangular_scan_converter_t *self,
					   const cairo_box_t *box,
					   int dir)
{
    rectangle_t *rectangle;

    rectangle = _allocate_rectangle (self);
    if (unlikely (rectangle == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    rectangle->dir = dir;
    rectangle->left  = MAX (box->p1.x, self->extents.p1.x);
    rectangle->right = MIN (box->p2.x, self->extents.p2.x);
    if (unlikely (rectangle->right <= rectangle->left)) {
	self->tail->count--;
	return CAIRO_STATUS_SUCCESS;
    }

    rectangle->top = MAX (box->p1.y, self->extents.p1.y);
    rectangle->top_y  = _cairo_fixed_integer_floor (rectangle->top);
    rectangle->bottom = MIN (box->p2.y, self->extents.p2.y);
    rectangle->bottom_y = _cairo_fixed_integer_floor (rectangle->bottom);
    if (likely (rectangle->bottom > rectangle->top))
	self->num_rectangles++;
    else
	self->tail->count--;

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_rectangular_scan_converter_destroy (void *converter)
{
    cairo_rectangular_scan_converter_t *self = converter;
    struct _cairo_rectangular_scan_converter_chunk *chunk, *next;

    for (chunk = self->chunks.next; chunk != NULL; chunk = next) {
	next = chunk->next;
	free (chunk);
    }
}

void
_cairo_rectangular_scan_converter_init (cairo_rectangular_scan_converter_t *self,
					const cairo_rectangle_int_t *extents)
{
    self->base.destroy = _cairo_rectangular_scan_converter_destroy;
    self->base.generate = _cairo_rectangular_scan_converter_generate;

    _cairo_box_from_rectangle (&self->extents, extents);

    self->chunks.base = self->buf;
    self->chunks.next = NULL;
    self->chunks.count = 0;
    self->chunks.size = sizeof (self->buf) / sizeof (rectangle_t);
    self->tail = &self->chunks;

    self->num_rectangles = 0;
}
