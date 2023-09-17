/*
 * Copyright © 2004 Carl Worth
 * Copyright © 2006 Red Hat, Inc.
 * Copyright © 2008 Chris Wilson
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

#include "cairoint.h"

#include "cairo-error-private.h"
#include "cairo-freelist-private.h"
#include "cairo-combsort-inline.h"
#include "cairo-contour-inline.h"
#include "cairo-contour-private.h"

void
_cairo_contour_init (cairo_contour_t *contour,
		     int direction)
{
    contour->direction = direction;
    contour->chain.points = contour->embedded_points;
    contour->chain.next = NULL;
    contour->chain.num_points = 0;
    contour->chain.size_points = ARRAY_LENGTH (contour->embedded_points);
    contour->tail = &contour->chain;
}

cairo_int_status_t
__cairo_contour_add_point (cairo_contour_t *contour,
			  const cairo_point_t *point)
{
    cairo_contour_chain_t *tail = contour->tail;
    cairo_contour_chain_t *next;

    assert (tail->next == NULL);

    next = _cairo_malloc_ab_plus_c (tail->size_points*2,
				    sizeof (cairo_point_t),
				    sizeof (cairo_contour_chain_t));
    if (unlikely (next == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    next->size_points = tail->size_points*2;
    next->num_points = 1;
    next->points = (cairo_point_t *)(next+1);
    next->next = NULL;
    tail->next = next;
    contour->tail = next;

    next->points[0] = *point;
    return CAIRO_INT_STATUS_SUCCESS;
}

static void
first_inc (cairo_contour_t *contour,
	   cairo_point_t **p,
	   cairo_contour_chain_t **chain)
{
    if (*p == (*chain)->points + (*chain)->num_points) {
	assert ((*chain)->next);
	*chain = (*chain)->next;
	*p = &(*chain)->points[0];
    } else
	++*p;
}

static void
last_dec (cairo_contour_t *contour,
	  cairo_point_t **p,
	  cairo_contour_chain_t **chain)
{
    if (*p == (*chain)->points) {
	cairo_contour_chain_t *prev;
	assert (*chain != &contour->chain);
	for (prev = &contour->chain; prev->next != *chain; prev = prev->next)
	    ;
	*chain = prev;
	*p = &(*chain)->points[(*chain)->num_points-1];
    } else
	--*p;
}

void
_cairo_contour_reverse (cairo_contour_t *contour)
{
    cairo_contour_chain_t *first_chain, *last_chain;
    cairo_point_t *first, *last;

    contour->direction = -contour->direction;

    if (contour->chain.num_points <= 1)
	return;

    first_chain = &contour->chain;
    last_chain = contour->tail;

    first = &first_chain->points[0];
    last = &last_chain->points[last_chain->num_points-1];

    while (first != last) {
	cairo_point_t p;

	p = *first;
	*first = *last;
	*last = p;

	first_inc (contour, &first, &first_chain);
	last_dec (contour, &last, &last_chain);
    }
}

cairo_int_status_t
_cairo_contour_add (cairo_contour_t *dst,
		    const cairo_contour_t *src)
{
    const cairo_contour_chain_t *chain;
    cairo_int_status_t status;
    int i;

    for (chain = &src->chain; chain; chain = chain->next) {
	for (i = 0; i < chain->num_points; i++) {
	    status = _cairo_contour_add_point (dst, &chain->points[i]);
	    if (unlikely (status))
		return status;
	}
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

static inline cairo_bool_t
iter_next (cairo_contour_iter_t *iter)
{
    if (iter->point == &iter->chain->points[iter->chain->size_points-1]) {
	iter->chain = iter->chain->next;
	if (iter->chain == NULL)
	    return FALSE;

	iter->point = &iter->chain->points[0];
	return TRUE;
    } else {
	iter->point++;
	return TRUE;
    }
}

static cairo_bool_t
iter_equal (const cairo_contour_iter_t *i1,
	    const cairo_contour_iter_t *i2)
{
    return i1->chain == i2->chain && i1->point == i2->point;
}

static void
iter_init (cairo_contour_iter_t *iter, cairo_contour_t *contour)
{
    iter->chain = &contour->chain;
    iter->point = &contour->chain.points[0];
}

static void
iter_init_last (cairo_contour_iter_t *iter, cairo_contour_t *contour)
{
    iter->chain = contour->tail;
    iter->point = &contour->tail->points[contour->tail->num_points-1];
}

static const cairo_contour_chain_t *prev_const_chain(const cairo_contour_t *contour,
						     const cairo_contour_chain_t *chain)
{
    const cairo_contour_chain_t *prev;

    if (chain == &contour->chain)
	return NULL;

    for (prev = &contour->chain; prev->next != chain; prev = prev->next)
	;

    return prev;
}

cairo_int_status_t
_cairo_contour_add_reversed (cairo_contour_t *dst,
			     const cairo_contour_t *src)
{
    const cairo_contour_chain_t *last;
    cairo_int_status_t status;
    int i;

    if (src->chain.num_points == 0)
	return CAIRO_INT_STATUS_SUCCESS;

    for (last = src->tail; last; last = prev_const_chain (src, last)) {
	for (i = last->num_points-1; i >= 0; i--) {
	    status = _cairo_contour_add_point (dst, &last->points[i]);
	    if (unlikely (status))
		return status;
	}
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_uint64_t
point_distance_sq (const cairo_point_t *p1,
		   const cairo_point_t *p2)
{
    int32_t dx = p1->x - p2->x;
    int32_t dy = p1->y - p2->y;
    return _cairo_int32x32_64_mul (dx, dx) + _cairo_int32x32_64_mul (dy, dy);
}

#define DELETED(p) ((p)->x == INT_MIN && (p)->y == INT_MAX)
#define MARK_DELETED(p) ((p)->x = INT_MIN, (p)->y = INT_MAX)

static cairo_bool_t
_cairo_contour_simplify_chain (cairo_contour_t *contour, const double tolerance,
			       const cairo_contour_iter_t *first,
			       const cairo_contour_iter_t *last)
{
    cairo_contour_iter_t iter, furthest;
    uint64_t max_error;
    int x0, y0;
    int nx, ny;
    int count;

    iter = *first;
    iter_next (&iter);
    if (iter_equal (&iter, last))
	return FALSE;

    x0 = first->point->x;
    y0 = first->point->y;
    nx = last->point->y - y0;
    ny = x0 - last->point->x;

    count = 0;
    max_error = 0;
    do {
	cairo_point_t *p = iter.point;
	if (! DELETED(p)) {
	    uint64_t d = (uint64_t)nx * (x0 - p->x) + (uint64_t)ny * (y0 - p->y);
	    if (d * d > max_error) {
		max_error = d * d;
		furthest = iter;
	    }
	    count++;
	}
	iter_next (&iter);
    } while (! iter_equal (&iter, last));
    if (count == 0)
	return FALSE;

    if (max_error > tolerance * ((uint64_t)nx * nx + (uint64_t)ny * ny)) {
	cairo_bool_t simplified;

	simplified = FALSE;
	simplified |= _cairo_contour_simplify_chain (contour, tolerance,
						     first, &furthest);
	simplified |= _cairo_contour_simplify_chain (contour, tolerance,
						     &furthest, last);
	return simplified;
    } else {
	iter = *first;
	iter_next (&iter);
	do {
	    MARK_DELETED (iter.point);
	    iter_next (&iter);
	} while (! iter_equal (&iter, last));

	return TRUE;
    }
}

void
_cairo_contour_simplify (cairo_contour_t *contour, double tolerance)
{
    cairo_contour_chain_t *chain;
    cairo_point_t *last = NULL;
    cairo_contour_iter_t iter, furthest;
    cairo_bool_t simplified;
    uint64_t max = 0;
    int i;

    if (contour->chain.num_points <= 2)
	return;

    tolerance = tolerance * CAIRO_FIXED_ONE;
    tolerance *= tolerance;

    /* stage 1: vertex reduction */
    for (chain = &contour->chain; chain; chain = chain->next) {
	for (i = 0; i < chain->num_points; i++) {
	    if (last == NULL ||
		point_distance_sq (last, &chain->points[i]) > tolerance) {
		last = &chain->points[i];
	    } else {
		MARK_DELETED (&chain->points[i]);
	    }
	}
    }

    /* stage2: polygon simplification using Douglas-Peucker */
    do {
	last = &contour->chain.points[0];
	iter_init (&furthest, contour);
	max = 0;
	for (chain = &contour->chain; chain; chain = chain->next) {
	    for (i = 0; i < chain->num_points; i++) {
		uint64_t d;

		if (DELETED (&chain->points[i]))
		    continue;

		d = point_distance_sq (last, &chain->points[i]);
		if (d > max) {
		    furthest.chain = chain;
		    furthest.point = &chain->points[i];
		    max = d;
		}
	    }
	}
	assert (max);

	simplified = FALSE;
	iter_init (&iter, contour);
	simplified |= _cairo_contour_simplify_chain (contour, tolerance,
						     &iter, &furthest);

	iter_init_last (&iter, contour);
	if (! iter_equal (&furthest, &iter))
	    simplified |= _cairo_contour_simplify_chain (contour, tolerance,
							 &furthest, &iter);
    } while (simplified);

    iter_init (&iter, contour);
    for (chain = &contour->chain; chain; chain = chain->next) {
	int num_points = chain->num_points;
	chain->num_points = 0;
	for (i = 0; i < num_points; i++) {
	    if (! DELETED(&chain->points[i])) {
		if (iter.point != &chain->points[i])
		    *iter.point = chain->points[i];
		iter.chain->num_points++;
		iter_next (&iter);
	    }
	}
    }

    if (iter.chain) {
	cairo_contour_chain_t *next;

	for (chain = iter.chain->next; chain; chain = next) {
	    next = chain->next;
	    free (chain);
	}

	iter.chain->next = NULL;
	contour->tail = iter.chain;
    }
}

void
_cairo_contour_reset (cairo_contour_t *contour)
{
    _cairo_contour_fini (contour);
    _cairo_contour_init (contour, contour->direction);
}

void
_cairo_contour_fini (cairo_contour_t *contour)
{
    cairo_contour_chain_t *chain, *next;

    for (chain = contour->chain.next; chain; chain = next) {
	next = chain->next;
	free (chain);
    }
}

void
_cairo_debug_print_contour (FILE *file, cairo_contour_t *contour)
{
    cairo_contour_chain_t *chain;
    int num_points, size_points;
    int i;

    num_points = 0;
    size_points = 0;
    for (chain = &contour->chain; chain; chain = chain->next) {
	num_points += chain->num_points;
	size_points += chain->size_points;
    }

    fprintf (file, "contour: direction=%d, num_points=%d / %d\n",
	     contour->direction, num_points, size_points);

    num_points = 0;
    for (chain = &contour->chain; chain; chain = chain->next) {
	for (i = 0; i < chain->num_points; i++) {
	    fprintf (file, "  [%d] = (%f, %f)\n",
		     num_points++,
		     _cairo_fixed_to_double (chain->points[i].x),
		     _cairo_fixed_to_double (chain->points[i].y));
	}
    }
}

void
__cairo_contour_remove_last_chain (cairo_contour_t *contour)
{
    cairo_contour_chain_t *chain;

    if (contour->tail == &contour->chain)
	return;

    for (chain = &contour->chain; chain->next != contour->tail; chain = chain->next)
	;
    free (contour->tail);
    contour->tail = chain;
    chain->next = NULL;
}
