/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
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

#include "cairo-boxes-private.h"
#include "cairo-contour-private.h"
#include "cairo-error-private.h"

#define DEBUG_POLYGON 0

#if DEBUG_POLYGON && !NDEBUG
static void
assert_last_edge_is_valid(cairo_polygon_t *polygon,
			  const cairo_box_t *limit)
{
    cairo_edge_t *edge;
    cairo_fixed_t x;

    edge = &polygon->edges[polygon->num_edges-1];

    assert (edge->bottom > edge->top);
    assert (edge->top >= limit->p1.y);
    assert (edge->bottom <= limit->p2.y);

    x = _cairo_edge_compute_intersection_x_for_y (&edge->line.p1,
						  &edge->line.p2,
						  edge->top);
    assert (x >= limit->p1.x);
    assert (x <= limit->p2.x);

    x = _cairo_edge_compute_intersection_x_for_y (&edge->line.p1,
						  &edge->line.p2,
						  edge->bottom);
    assert (x >= limit->p1.x);
    assert (x <= limit->p2.x);
}
#else
#define assert_last_edge_is_valid(p, l)
#endif

static void
_cairo_polygon_add_edge (cairo_polygon_t *polygon,
			 const cairo_point_t *p1,
			 const cairo_point_t *p2,
			 int dir);

void
_cairo_polygon_limit (cairo_polygon_t *polygon,
		     const cairo_box_t *limits,
		     int num_limits)
{
    int n;

    polygon->limits = limits;
    polygon->num_limits = num_limits;

    if (polygon->num_limits) {
	polygon->limit = limits[0];
	for (n = 1; n < num_limits; n++) {
	    if (limits[n].p1.x < polygon->limit.p1.x)
		polygon->limit.p1.x = limits[n].p1.x;

	    if (limits[n].p1.y < polygon->limit.p1.y)
		polygon->limit.p1.y = limits[n].p1.y;

	    if (limits[n].p2.x > polygon->limit.p2.x)
		polygon->limit.p2.x = limits[n].p2.x;

	    if (limits[n].p2.y > polygon->limit.p2.y)
		polygon->limit.p2.y = limits[n].p2.y;
	}
    }
}

void
_cairo_polygon_limit_to_clip (cairo_polygon_t *polygon,
			      const cairo_clip_t *clip)
{
    if (clip)
	_cairo_polygon_limit (polygon, clip->boxes, clip->num_boxes);
    else
	_cairo_polygon_limit (polygon, 0, 0);
}

void
_cairo_polygon_init (cairo_polygon_t *polygon,
		     const cairo_box_t *limits,
		     int num_limits)
{
    VG (VALGRIND_MAKE_MEM_UNDEFINED (polygon, sizeof (cairo_polygon_t)));

    polygon->status = CAIRO_STATUS_SUCCESS;

    polygon->num_edges = 0;

    polygon->edges = polygon->edges_embedded;
    polygon->edges_size = ARRAY_LENGTH (polygon->edges_embedded);

    polygon->extents.p1.x = polygon->extents.p1.y = INT32_MAX;
    polygon->extents.p2.x = polygon->extents.p2.y = INT32_MIN;

    _cairo_polygon_limit (polygon, limits, num_limits);
}

void
_cairo_polygon_init_with_clip (cairo_polygon_t *polygon,
			       const cairo_clip_t *clip)
{
    if (clip)
	_cairo_polygon_init (polygon, clip->boxes, clip->num_boxes);
    else
	_cairo_polygon_init (polygon, 0, 0);
}

cairo_status_t
_cairo_polygon_init_boxes (cairo_polygon_t *polygon,
			   const cairo_boxes_t *boxes)
{
    const struct _cairo_boxes_chunk *chunk;
    int i;

    VG (VALGRIND_MAKE_MEM_UNDEFINED (polygon, sizeof (cairo_polygon_t)));

    polygon->status = CAIRO_STATUS_SUCCESS;

    polygon->num_edges = 0;

    polygon->edges = polygon->edges_embedded;
    polygon->edges_size = ARRAY_LENGTH (polygon->edges_embedded);
    if (boxes->num_boxes > ARRAY_LENGTH (polygon->edges_embedded)/2) {
	polygon->edges_size = 2 * boxes->num_boxes;
	polygon->edges = _cairo_malloc_ab (polygon->edges_size,
					   2*sizeof(cairo_edge_t));
	if (unlikely (polygon->edges == NULL))
	    return polygon->status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    polygon->extents.p1.x = polygon->extents.p1.y = INT32_MAX;
    polygon->extents.p2.x = polygon->extents.p2.y = INT32_MIN;

    polygon->limits = NULL;
    polygon->num_limits = 0;

    for (chunk = &boxes->chunks; chunk != NULL; chunk = chunk->next) {
	for (i = 0; i < chunk->count; i++) {
	    cairo_point_t p1, p2;

	    p1 = chunk->base[i].p1;
	    p2.x = p1.x;
	    p2.y = chunk->base[i].p2.y;
	    _cairo_polygon_add_edge (polygon, &p1, &p2, 1);

	    p1 = chunk->base[i].p2;
	    p2.x = p1.x;
	    p2.y = chunk->base[i].p1.y;
	    _cairo_polygon_add_edge (polygon, &p1, &p2, 1);
	}
    }

    return polygon->status;
}

cairo_status_t
_cairo_polygon_init_box_array (cairo_polygon_t *polygon,
			       cairo_box_t *boxes,
			       int num_boxes)
{
    int i;

    VG (VALGRIND_MAKE_MEM_UNDEFINED (polygon, sizeof (cairo_polygon_t)));

    polygon->status = CAIRO_STATUS_SUCCESS;

    polygon->num_edges = 0;

    polygon->edges = polygon->edges_embedded;
    polygon->edges_size = ARRAY_LENGTH (polygon->edges_embedded);
    if (num_boxes > ARRAY_LENGTH (polygon->edges_embedded)/2) {
	polygon->edges_size = 2 * num_boxes;
	polygon->edges = _cairo_malloc_ab (polygon->edges_size,
					   2*sizeof(cairo_edge_t));
	if (unlikely (polygon->edges == NULL))
	    return polygon->status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    polygon->extents.p1.x = polygon->extents.p1.y = INT32_MAX;
    polygon->extents.p2.x = polygon->extents.p2.y = INT32_MIN;

    polygon->limits = NULL;
    polygon->num_limits = 0;

    for (i = 0; i < num_boxes; i++) {
	cairo_point_t p1, p2;

	p1 = boxes[i].p1;
	p2.x = p1.x;
	p2.y = boxes[i].p2.y;
	_cairo_polygon_add_edge (polygon, &p1, &p2, 1);

	p1 = boxes[i].p2;
	p2.x = p1.x;
	p2.y = boxes[i].p1.y;
	_cairo_polygon_add_edge (polygon, &p1, &p2, 1);
    }

    return polygon->status;
}


void
_cairo_polygon_fini (cairo_polygon_t *polygon)
{
    if (polygon->edges != polygon->edges_embedded)
	free (polygon->edges);

    VG (VALGRIND_MAKE_MEM_UNDEFINED (polygon, sizeof (cairo_polygon_t)));
}

/* make room for at least one more edge */
static cairo_bool_t
_cairo_polygon_grow (cairo_polygon_t *polygon)
{
    cairo_edge_t *new_edges;
    int old_size = polygon->edges_size;
    int new_size = 4 * old_size;

    if (CAIRO_INJECT_FAULT ()) {
	polygon->status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	return FALSE;
    }

    if (polygon->edges == polygon->edges_embedded) {
	new_edges = _cairo_malloc_ab (new_size, sizeof (cairo_edge_t));
	if (new_edges != NULL)
	    memcpy (new_edges, polygon->edges, old_size * sizeof (cairo_edge_t));
    } else {
	new_edges = _cairo_realloc_ab (polygon->edges,
		                       new_size, sizeof (cairo_edge_t));
    }

    if (unlikely (new_edges == NULL)) {
	polygon->status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	return FALSE;
    }

    polygon->edges = new_edges;
    polygon->edges_size = new_size;

    return TRUE;
}

static void
_add_edge (cairo_polygon_t *polygon,
	   const cairo_point_t *p1,
	   const cairo_point_t *p2,
	   int top, int bottom,
	   int dir)
{
    cairo_edge_t *edge;

    assert (top < bottom);

    if (unlikely (polygon->num_edges == polygon->edges_size)) {
	if (! _cairo_polygon_grow (polygon))
	    return;
    }

    edge = &polygon->edges[polygon->num_edges++];
    edge->line.p1 = *p1;
    edge->line.p2 = *p2;
    edge->top = top;
    edge->bottom = bottom;
    edge->dir = dir;

    if (top < polygon->extents.p1.y)
	polygon->extents.p1.y = top;
    if (bottom > polygon->extents.p2.y)
	polygon->extents.p2.y = bottom;

    if (p1->x < polygon->extents.p1.x || p1->x > polygon->extents.p2.x) {
	cairo_fixed_t x = p1->x;
	if (top != p1->y)
	    x = _cairo_edge_compute_intersection_x_for_y (p1, p2, top);
	if (x < polygon->extents.p1.x)
	    polygon->extents.p1.x = x;
	if (x > polygon->extents.p2.x)
	    polygon->extents.p2.x = x;
    }

    if (p2->x < polygon->extents.p1.x || p2->x > polygon->extents.p2.x) {
	cairo_fixed_t x = p2->x;
	if (bottom != p2->y)
	    x = _cairo_edge_compute_intersection_x_for_y (p1, p2, bottom);
	if (x < polygon->extents.p1.x)
	    polygon->extents.p1.x = x;
	if (x > polygon->extents.p2.x)
	    polygon->extents.p2.x = x;
    }
}

static void
_add_clipped_edge (cairo_polygon_t *polygon,
		   const cairo_point_t *p1,
		   const cairo_point_t *p2,
		   const int top, const int bottom,
		   const int dir)
{
    cairo_point_t bot_left, top_right;
    cairo_fixed_t top_y, bot_y;
    int n;

    for (n = 0; n < polygon->num_limits; n++) {
	const cairo_box_t *limits = &polygon->limits[n];
	cairo_fixed_t pleft, pright;

	if (top >= limits->p2.y)
	    continue;
	if (bottom <= limits->p1.y)
	    continue;

	bot_left.x = limits->p1.x;
	bot_left.y = limits->p2.y;

	top_right.x = limits->p2.x;
	top_right.y = limits->p1.y;

	/* The useful region */
	top_y = MAX (top, limits->p1.y);
	bot_y = MIN (bottom, limits->p2.y);

	/* The projection of the edge on the horizontal axis */
	pleft = MIN (p1->x, p2->x);
	pright = MAX (p1->x, p2->x);

	if (limits->p1.x <= pleft && pright <= limits->p2.x) {
	    /* Projection of the edge completely contained in the box:
	     * clip vertically by restricting top and bottom */

	    _add_edge (polygon, p1, p2, top_y, bot_y, dir);
	    assert_last_edge_is_valid (polygon, limits);
	} else if (pright <= limits->p1.x) {
	    /* Projection of the edge to the left of the box:
	     * replace with the left side of the box (clipped top/bottom) */

	    _add_edge (polygon, &limits->p1, &bot_left, top_y, bot_y, dir);
	    assert_last_edge_is_valid (polygon, limits);
	} else if (limits->p2.x <= pleft) {
	    /* Projection of the edge to the right of the box:
	     * replace with the right side of the box (clipped top/bottom) */

	    _add_edge (polygon, &top_right, &limits->p2, top_y, bot_y, dir);
	    assert_last_edge_is_valid (polygon, limits);
	} else {
	    /* The edge and the box intersect in a generic way */
	    cairo_fixed_t left_y, right_y;
	    cairo_bool_t top_left_to_bottom_right;

	    /*
	     * The edge intersects the lines corresponding to the left
	     * and right sides of the limit box at left_y and right_y,
	     * but we need to add edges for the range from top_y to
	     * bot_y.
	     *
	     * For both intersections, there are three cases:
	     *
	     *  1) It is outside the vertical range of the limit
	     *     box. In this case we can simply further clip the
	     *     edge we will be emitting (i.e. restrict its
	     *     top/bottom limits to those of the limit box).
	     *
	     *  2) It is inside the vertical range of the limit
	     *     box. In this case, we need to add the vertical edge
	     *     connecting the correct vertex to the intersection,
	     *     in order to preserve the winding count.
	     *
	     *  3) It is exactly on the box. In this case, do nothing.
	     *
	     * These operations restrict the active range (stored in
	     * top_y/bot_y) so that the p1-p2 edge is completely
	     * inside the box if it is clipped to this vertical range.
	     */

	    top_left_to_bottom_right = (p1->x <= p2->x) == (p1->y <= p2->y);
	    if (top_left_to_bottom_right) {
		if (pleft >= limits->p1.x) {
		    left_y = top_y;
		} else {
		    left_y = _cairo_edge_compute_intersection_y_for_x (p1, p2,
								       limits->p1.x);
		    if (_cairo_edge_compute_intersection_x_for_y (p1, p2, left_y) < limits->p1.x)
			left_y++;
		}

		left_y = MIN (left_y, bot_y);
		if (top_y < left_y) {
		    _add_edge (polygon, &limits->p1, &bot_left,
			       top_y, left_y, dir);
		    assert_last_edge_is_valid (polygon, limits);
		    top_y = left_y;
		}

		if (pright <= limits->p2.x) {
		    right_y = bot_y;
		} else {
		    right_y = _cairo_edge_compute_intersection_y_for_x (p1, p2,
									limits->p2.x);
		    if (_cairo_edge_compute_intersection_x_for_y (p1, p2, right_y) > limits->p2.x)
			right_y--;
		}

		right_y = MAX (right_y, top_y);
		if (bot_y > right_y) {
		    _add_edge (polygon, &top_right, &limits->p2,
			       right_y, bot_y, dir);
		    assert_last_edge_is_valid (polygon, limits);
		    bot_y = right_y;
		}
	    } else {
		if (pright <= limits->p2.x) {
		    right_y = top_y;
		} else {
		    right_y = _cairo_edge_compute_intersection_y_for_x (p1, p2,
									limits->p2.x);
		    if (_cairo_edge_compute_intersection_x_for_y (p1, p2, right_y) > limits->p2.x)
			right_y++;
		}

		right_y = MIN (right_y, bot_y);
		if (top_y < right_y) {
		    _add_edge (polygon, &top_right, &limits->p2,
			       top_y, right_y, dir);
		    assert_last_edge_is_valid (polygon, limits);
		    top_y = right_y;
		}

		if (pleft >= limits->p1.x) {
		    left_y = bot_y;
		} else {
		    left_y = _cairo_edge_compute_intersection_y_for_x (p1, p2,
								       limits->p1.x);
		    if (_cairo_edge_compute_intersection_x_for_y (p1, p2, left_y) < limits->p1.x)
			left_y--;
		}

		left_y = MAX (left_y, top_y);
		if (bot_y > left_y) {
		    _add_edge (polygon, &limits->p1, &bot_left,
			       left_y, bot_y, dir);
		    assert_last_edge_is_valid (polygon, limits);
		    bot_y = left_y;
		}
	    }

	    if (top_y != bot_y) {
		_add_edge (polygon, p1, p2, top_y, bot_y, dir);
		assert_last_edge_is_valid (polygon, limits);
	    }
	}
    }
}

static void
_cairo_polygon_add_edge (cairo_polygon_t *polygon,
			 const cairo_point_t *p1,
			 const cairo_point_t *p2,
			 int dir)
{
    /* drop horizontal edges */
    if (p1->y == p2->y)
	return;

    if (p1->y > p2->y) {
	const cairo_point_t *t;
	t = p1, p1 = p2, p2 = t;
	dir = -dir;
    }

    if (polygon->num_limits) {
	if (p2->y <= polygon->limit.p1.y)
	    return;

	if (p1->y >= polygon->limit.p2.y)
	    return;

	_add_clipped_edge (polygon, p1, p2, p1->y, p2->y, dir);
    } else
	_add_edge (polygon, p1, p2, p1->y, p2->y, dir);
}

cairo_status_t
_cairo_polygon_add_external_edge (void *polygon,
				  const cairo_point_t *p1,
				  const cairo_point_t *p2)
{
    _cairo_polygon_add_edge (polygon, p1, p2, 1);
    return _cairo_polygon_status (polygon);
}

cairo_status_t
_cairo_polygon_add_line (cairo_polygon_t *polygon,
			 const cairo_line_t *line,
			 int top, int bottom,
			 int dir)
{
    /* drop horizontal edges */
    if (line->p1.y == line->p2.y)
	return CAIRO_STATUS_SUCCESS;

    if (bottom <= top)
	return CAIRO_STATUS_SUCCESS;

    if (polygon->num_limits) {
	if (line->p2.y <= polygon->limit.p1.y)
	    return CAIRO_STATUS_SUCCESS;

	if (line->p1.y >= polygon->limit.p2.y)
	    return CAIRO_STATUS_SUCCESS;

	_add_clipped_edge (polygon, &line->p1, &line->p2, top, bottom, dir);
    } else
	_add_edge (polygon, &line->p1, &line->p2, top, bottom, dir);

    return polygon->status;
}

cairo_status_t
_cairo_polygon_add_contour (cairo_polygon_t *polygon,
			    const cairo_contour_t *contour)
{
    const struct _cairo_contour_chain *chain;
    const cairo_point_t *prev = NULL;
    int i;

    if (contour->chain.num_points <= 1)
	return CAIRO_INT_STATUS_SUCCESS;

    prev = &contour->chain.points[0];
    for (chain = &contour->chain; chain; chain = chain->next) {
	for (i = 0; i < chain->num_points; i++) {
	    _cairo_polygon_add_edge (polygon, prev, &chain->points[i],
				     contour->direction);
	    prev = &chain->points[i];
	}
    }

    return polygon->status;
}

void
_cairo_polygon_translate (cairo_polygon_t *polygon, int dx, int dy)
{
    int n;

    dx = _cairo_fixed_from_int (dx);
    dy = _cairo_fixed_from_int (dy);

    polygon->extents.p1.x += dx;
    polygon->extents.p2.x += dx;
    polygon->extents.p1.y += dy;
    polygon->extents.p2.y += dy;

    for (n = 0; n < polygon->num_edges; n++) {
	cairo_edge_t *e = &polygon->edges[n];

	e->top += dy;
	e->bottom += dy;

	e->line.p1.x += dx;
	e->line.p2.x += dx;
	e->line.p1.y += dy;
	e->line.p2.y += dy;
    }
}
