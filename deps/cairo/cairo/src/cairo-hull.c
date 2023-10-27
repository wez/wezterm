/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2003 University of Southern California
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
 * The Initial Developer of the Original Code is University of Southern
 * California.
 *
 * Contributor(s):
 *	Carl D. Worth <cworth@cworth.org>
 */

#include "cairoint.h"

#include "cairo-error-private.h"
#include "cairo-slope-private.h"

typedef struct cairo_hull {
    cairo_point_t point;
    cairo_slope_t slope;
    int discard;
    int id;
} cairo_hull_t;

static void
_cairo_hull_init (cairo_hull_t			*hull,
	          cairo_pen_vertex_t		*vertices,
		  int				 num_vertices)
{
    cairo_point_t *p, *extremum, tmp;
    int i;

    extremum = &vertices[0].point;
    for (i = 1; i < num_vertices; i++) {
	p = &vertices[i].point;
	if (p->y < extremum->y || (p->y == extremum->y && p->x < extremum->x))
	    extremum = p;
    }
    /* Put the extremal point at the beginning of the array */
    tmp = *extremum;
    *extremum = vertices[0].point;
    vertices[0].point = tmp;

    for (i = 0; i < num_vertices; i++) {
	hull[i].point = vertices[i].point;
	_cairo_slope_init (&hull[i].slope, &hull[0].point, &hull[i].point);

        /* give each point a unique id for later comparison */
        hull[i].id = i;

        /* Don't discard by default */
        hull[i].discard = 0;

	/* Discard all points coincident with the extremal point */
	if (i != 0 && hull[i].slope.dx == 0 && hull[i].slope.dy == 0)
	    hull[i].discard = 1;
    }
}

static inline cairo_int64_t
_slope_length (cairo_slope_t *slope)
{
    return _cairo_int64_add (_cairo_int32x32_64_mul (slope->dx, slope->dx),
			     _cairo_int32x32_64_mul (slope->dy, slope->dy));
}

static int
_cairo_hull_vertex_compare (const void *av, const void *bv)
{
    cairo_hull_t *a = (cairo_hull_t *) av;
    cairo_hull_t *b = (cairo_hull_t *) bv;
    int ret;

    /* Some libraries are reported to actually compare identical
     * pointers and require the result to be 0. This is the crazy world we
     * have to live in.
     */
    if (a == b)
	return 0;

    ret = _cairo_slope_compare (&a->slope, &b->slope);

    /*
     * In the case of two vertices with identical slope from the
     * extremal point discard the nearer point.
     */
    if (ret == 0) {
	int cmp;

	cmp = _cairo_int64_cmp (_slope_length (&a->slope),
				_slope_length (&b->slope));

	/*
	 * Use the points' ids to ensure a well-defined ordering,
	 * and avoid setting discard on both points.
	 */
	if (cmp < 0 || (cmp == 0 && a->id < b->id)) {
	    a->discard = 1;
	    ret = -1;
	} else {
	    b->discard = 1;
	    ret = 1;
	}
    }

    return ret;
}

static int
_cairo_hull_prev_valid (cairo_hull_t *hull, int num_hull, int index)
{
    /* hull[0] is always valid, and we never need to wraparound, (if
     * we are passed an index of 0 here, then the calling loop is just
     * about to terminate). */
    if (index == 0)
	return 0;

    do {
	index--;
    } while (hull[index].discard);

    return index;
}

static int
_cairo_hull_next_valid (cairo_hull_t *hull, int num_hull, int index)
{
    do {
	index = (index + 1) % num_hull;
    } while (hull[index].discard);

    return index;
}

static void
_cairo_hull_eliminate_concave (cairo_hull_t *hull, int num_hull)
{
    int i, j, k;
    cairo_slope_t slope_ij, slope_jk;

    i = 0;
    j = _cairo_hull_next_valid (hull, num_hull, i);
    k = _cairo_hull_next_valid (hull, num_hull, j);

    do {
	_cairo_slope_init (&slope_ij, &hull[i].point, &hull[j].point);
	_cairo_slope_init (&slope_jk, &hull[j].point, &hull[k].point);

	/* Is the angle formed by ij and jk concave? */
	if (_cairo_slope_compare (&slope_ij, &slope_jk) >= 0) {
	    if (i == k)
		return;
	    hull[j].discard = 1;
	    j = i;
	    i = _cairo_hull_prev_valid (hull, num_hull, j);
	} else {
	    i = j;
	    j = k;
	    k = _cairo_hull_next_valid (hull, num_hull, j);
	}
    } while (j != 0);
}

static void
_cairo_hull_to_pen (cairo_hull_t *hull, cairo_pen_vertex_t *vertices, int *num_vertices)
{
    int i, j = 0;

    for (i = 0; i < *num_vertices; i++) {
	if (hull[i].discard)
	    continue;
	vertices[j++].point = hull[i].point;
    }

    *num_vertices = j;
}

/* Given a set of vertices, compute the convex hull using the Graham
   scan algorithm. */
cairo_status_t
_cairo_hull_compute (cairo_pen_vertex_t *vertices, int *num_vertices)
{
    cairo_hull_t hull_stack[CAIRO_STACK_ARRAY_LENGTH (cairo_hull_t)];
    cairo_hull_t *hull;
    int num_hull = *num_vertices;

    if (CAIRO_INJECT_FAULT ())
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    if (num_hull > ARRAY_LENGTH (hull_stack)) {
	hull = _cairo_malloc_ab (num_hull, sizeof (cairo_hull_t));
	if (unlikely (hull == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    } else {
	hull = hull_stack;
    }

    _cairo_hull_init (hull, vertices, num_hull);

    qsort (hull + 1, num_hull - 1,
	   sizeof (cairo_hull_t), _cairo_hull_vertex_compare);

    _cairo_hull_eliminate_concave (hull, num_hull);

    _cairo_hull_to_pen (hull, vertices, num_vertices);

    if (hull != hull_stack)
	free (hull);

    return CAIRO_STATUS_SUCCESS;
}
