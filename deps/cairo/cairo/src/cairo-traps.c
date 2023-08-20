/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/*
 * Copyright © 2002 Keith Packard
 * Copyright © 2007 Red Hat, Inc.
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
 * The Initial Developer of the Original Code is Keith Packard
 *
 * Contributor(s):
 *	Keith R. Packard <keithp@keithp.com>
 *	Carl D. Worth <cworth@cworth.org>
 *
 * 2002-07-15: Converted from XRenderCompositeDoublePoly to #cairo_trap_t. Carl D. Worth
 */

#include "cairoint.h"

#include "cairo-box-inline.h"
#include "cairo-boxes-private.h"
#include "cairo-error-private.h"
#include "cairo-line-private.h"
#include "cairo-region-private.h"
#include "cairo-slope-private.h"
#include "cairo-traps-private.h"
#include "cairo-spans-private.h"

/* private functions */

void
_cairo_traps_init (cairo_traps_t *traps)
{
    VG (VALGRIND_MAKE_MEM_UNDEFINED (traps, sizeof (cairo_traps_t)));

    traps->status = CAIRO_STATUS_SUCCESS;

    traps->maybe_region = 1;
    traps->is_rectilinear = 0;
    traps->is_rectangular = 0;

    traps->num_traps = 0;

    traps->traps_size = ARRAY_LENGTH (traps->traps_embedded);
    traps->traps = traps->traps_embedded;

    traps->num_limits = 0;
    traps->has_intersections = FALSE;
}

void
_cairo_traps_limit (cairo_traps_t	*traps,
		    const cairo_box_t	*limits,
		    int			 num_limits)
{
    int i;

    traps->limits = limits;
    traps->num_limits = num_limits;

    traps->bounds = limits[0];
    for (i = 1; i < num_limits; i++)
	_cairo_box_add_box (&traps->bounds, &limits[i]);
}

void
_cairo_traps_init_with_clip (cairo_traps_t *traps,
			     const cairo_clip_t *clip)
{
    _cairo_traps_init (traps);
    if (clip)
	_cairo_traps_limit (traps, clip->boxes, clip->num_boxes);
}

void
_cairo_traps_clear (cairo_traps_t *traps)
{
    traps->status = CAIRO_STATUS_SUCCESS;

    traps->maybe_region = 1;
    traps->is_rectilinear = 0;
    traps->is_rectangular = 0;

    traps->num_traps = 0;
    traps->has_intersections = FALSE;
}

void
_cairo_traps_fini (cairo_traps_t *traps)
{
    if (traps->traps != traps->traps_embedded)
	free (traps->traps);

    VG (VALGRIND_MAKE_MEM_UNDEFINED (traps, sizeof (cairo_traps_t)));
}

/* make room for at least one more trap */
static cairo_bool_t
_cairo_traps_grow (cairo_traps_t *traps)
{
    cairo_trapezoid_t *new_traps;
    int new_size = 4 * traps->traps_size;

    if (CAIRO_INJECT_FAULT ()) {
	traps->status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	return FALSE;
    }

    if (traps->traps == traps->traps_embedded) {
	new_traps = _cairo_malloc_ab (new_size, sizeof (cairo_trapezoid_t));
	if (new_traps != NULL)
	    memcpy (new_traps, traps->traps, sizeof (traps->traps_embedded));
    } else {
	new_traps = _cairo_realloc_ab (traps->traps,
	                               new_size, sizeof (cairo_trapezoid_t));
    }

    if (unlikely (new_traps == NULL)) {
	traps->status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	return FALSE;
    }

    traps->traps = new_traps;
    traps->traps_size = new_size;
    return TRUE;
}

void
_cairo_traps_add_trap (cairo_traps_t *traps,
		       cairo_fixed_t top, cairo_fixed_t bottom,
		       const cairo_line_t *left,
		       const cairo_line_t *right)
{
    cairo_trapezoid_t *trap;

    assert (left->p1.y != left->p2.y);
    assert (right->p1.y != right->p2.y);
    assert (bottom > top);

    if (unlikely (traps->num_traps == traps->traps_size)) {
	if (unlikely (! _cairo_traps_grow (traps)))
	    return;
    }

    trap = &traps->traps[traps->num_traps++];
    trap->top = top;
    trap->bottom = bottom;
    trap->left = *left;
    trap->right = *right;
}

static void
_cairo_traps_add_clipped_trap (cairo_traps_t *traps,
			       cairo_fixed_t _top, cairo_fixed_t _bottom,
			       const cairo_line_t *_left,
			       const cairo_line_t *_right)
{
    /* Note: With the goofy trapezoid specification, (where an
     * arbitrary two points on the lines can specified for the left
     * and right edges), these limit checks would not work in
     * general. For example, one can imagine a trapezoid entirely
     * within the limits, but with two points used to specify the left
     * edge entirely to the right of the limits.  Fortunately, for our
     * purposes, cairo will never generate such a crazy
     * trapezoid. Instead, cairo always uses for its points the
     * extreme positions of the edge that are visible on at least some
     * trapezoid. With this constraint, it's impossible for both
     * points to be outside the limits while the relevant edge is
     * entirely inside the limits.
     */
    if (traps->num_limits) {
	const cairo_box_t *b = &traps->bounds;
	cairo_fixed_t top = _top, bottom = _bottom;
	cairo_line_t left = *_left, right = *_right;

	/* Trivially reject if trapezoid is entirely to the right or
	 * to the left of the limits. */
	if (left.p1.x >= b->p2.x && left.p2.x >= b->p2.x)
	    return;

	if (right.p1.x <= b->p1.x && right.p2.x <= b->p1.x)
	    return;

	/* And reject if the trapezoid is entirely above or below */
	if (top >= b->p2.y || bottom <= b->p1.y)
	    return;

	/* Otherwise, clip the trapezoid to the limits. We only clip
	 * where an edge is entirely outside the limits. If we wanted
	 * to be more clever, we could handle cases where a trapezoid
	 * edge intersects the edge of the limits, but that would
	 * require slicing this trapezoid into multiple trapezoids,
	 * and I'm not sure the effort would be worth it. */
	if (top < b->p1.y)
	    top = b->p1.y;

	if (bottom > b->p2.y)
	    bottom = b->p2.y;

	if (left.p1.x <= b->p1.x && left.p2.x <= b->p1.x)
	    left.p1.x = left.p2.x = b->p1.x;

	if (right.p1.x >= b->p2.x && right.p2.x >= b->p2.x)
	    right.p1.x = right.p2.x = b->p2.x;

	/* Trivial discards for empty trapezoids that are likely to
	 * be produced by our tessellators (most notably convex_quad
	 * when given a simple rectangle).
	 */
	if (top >= bottom)
	    return;

	/* cheap colinearity check */
	if (right.p1.x <= left.p1.x && right.p1.y == left.p1.y &&
	    right.p2.x <= left.p2.x && right.p2.y == left.p2.y)
	    return;

	_cairo_traps_add_trap (traps, top, bottom, &left, &right);
    } else
	_cairo_traps_add_trap (traps, _top, _bottom, _left, _right);
}

static int
_compare_point_fixed_by_y (const void *av, const void *bv)
{
    const cairo_point_t	*a = av, *b = bv;
    int ret = a->y - b->y;
    if (ret == 0)
	ret = a->x - b->x;
    return ret;
}

void
_cairo_traps_tessellate_convex_quad (cairo_traps_t *traps,
				     const cairo_point_t q[4])
{
    int a, b, c, d;
    int i;
    cairo_slope_t ab, ad;
    cairo_bool_t b_left_of_d;
    cairo_line_t left;
    cairo_line_t right;

    /* Choose a as a point with minimal y */
    a = 0;
    for (i = 1; i < 4; i++)
	if (_compare_point_fixed_by_y (&q[i], &q[a]) < 0)
	    a = i;

    /* b and d are adjacent to a, while c is opposite */
    b = (a + 1) % 4;
    c = (a + 2) % 4;
    d = (a + 3) % 4;

    /* Choose between b and d so that b.y is less than d.y */
    if (_compare_point_fixed_by_y (&q[d], &q[b]) < 0) {
	b = (a + 3) % 4;
	d = (a + 1) % 4;
    }

    /* Without freedom left to choose anything else, we have four
     * cases to tessellate.
     *
     * First, we have to determine the Y-axis sort of the four
     * vertices, (either abcd or abdc). After that we need to determine
     * which edges will be "left" and which will be "right" in the
     * resulting trapezoids. This can be determined by computing a
     * slope comparison of ab and ad to determine if b is left of d or
     * not.
     *
     * Note that "left of" here is in the sense of which edges should
     * be the left vs. right edges of the trapezoid. In particular, b
     * left of d does *not* mean that b.x is less than d.x.
     *
     * This should hopefully be made clear in the lame ASCII art
     * below. Since the same slope comparison is used in all cases, we
     * compute it before testing for the Y-value sort. */

    /* Note: If a == b then the ab slope doesn't give us any
     * information. In that case, we can replace it with the ac (or
     * equivalenly the bc) slope which gives us exactly the same
     * information we need. At worst the names of the identifiers ab
     * and b_left_of_d are inaccurate in this case, (would be ac, and
     * c_left_of_d). */
    if (q[a].x == q[b].x && q[a].y == q[b].y)
	_cairo_slope_init (&ab, &q[a], &q[c]);
    else
	_cairo_slope_init (&ab, &q[a], &q[b]);

    _cairo_slope_init (&ad, &q[a], &q[d]);

    b_left_of_d = _cairo_slope_compare (&ab, &ad) > 0;

    if (q[c].y <= q[d].y) {
	if (b_left_of_d) {
	    /* Y-sort is abcd and b is left of d, (slope(ab) > slope (ad))
	     *
	     *                      top bot left right
	     *        _a  a  a
	     *      / /  /|  |\      a.y b.y  ab   ad
	     *     b /  b |  b \
	     *    / /   | |   \ \    b.y c.y  bc   ad
	     *   c /    c |    c \
	     *  | /      \|     \ \  c.y d.y  cd   ad
	     *  d         d       d
	     */
	    left.p1  = q[a]; left.p2  = q[b];
	    right.p1 = q[a]; right.p2 = q[d];
	    _cairo_traps_add_clipped_trap (traps, q[a].y, q[b].y, &left, &right);
	    left.p1  = q[b]; left.p2  = q[c];
	    _cairo_traps_add_clipped_trap (traps, q[b].y, q[c].y, &left, &right);
	    left.p1  = q[c]; left.p2  = q[d];
	    _cairo_traps_add_clipped_trap (traps, q[c].y, q[d].y, &left, &right);
	} else {
	    /* Y-sort is abcd and b is right of d, (slope(ab) <= slope (ad))
	     *
	     *       a  a  a_
	     *      /|  |\  \ \     a.y b.y  ad  ab
	     *     / b  | b  \ b
	     *    / /   | |   \ \   b.y c.y  ad  bc
	     *   / c    | c    \ c
	     *  / /     |/      \ | c.y d.y  ad  cd
	     *  d       d         d
	     */
	    left.p1  = q[a]; left.p2  = q[d];
	    right.p1 = q[a]; right.p2 = q[b];
	    _cairo_traps_add_clipped_trap (traps, q[a].y, q[b].y, &left, &right);
	    right.p1 = q[b]; right.p2 = q[c];
	    _cairo_traps_add_clipped_trap (traps, q[b].y, q[c].y, &left, &right);
	    right.p1 = q[c]; right.p2 = q[d];
	    _cairo_traps_add_clipped_trap (traps, q[c].y, q[d].y, &left, &right);
	}
    } else {
	if (b_left_of_d) {
	    /* Y-sort is abdc and b is left of d, (slope (ab) > slope (ad))
	     *
	     *        a   a     a
	     *       //  / \    |\     a.y b.y  ab  ad
	     *     /b/  b   \   b \
	     *    / /    \   \   \ \   b.y d.y  bc  ad
	     *   /d/      \   d   \ d
	     *  //         \ /     \|  d.y c.y  bc  dc
	     *  c           c       c
	     */
	    left.p1  = q[a]; left.p2  = q[b];
	    right.p1 = q[a]; right.p2 = q[d];
	    _cairo_traps_add_clipped_trap (traps, q[a].y, q[b].y, &left, &right);
	    left.p1  = q[b]; left.p2  = q[c];
	    _cairo_traps_add_clipped_trap (traps, q[b].y, q[d].y, &left, &right);
	    right.p1 = q[d]; right.p2 = q[c];
	    _cairo_traps_add_clipped_trap (traps, q[d].y, q[c].y, &left, &right);
	} else {
	    /* Y-sort is abdc and b is right of d, (slope (ab) <= slope (ad))
	     *
	     *      a     a   a
	     *     /|    / \  \\       a.y b.y  ad  ab
	     *    / b   /   b  \b\
	     *   / /   /   /    \ \    b.y d.y  ad  bc
	     *  d /   d   /	 \d\
	     *  |/     \ /         \\  d.y c.y  dc  bc
	     *  c       c	   c
	     */
	    left.p1  = q[a]; left.p2  = q[d];
	    right.p1 = q[a]; right.p2 = q[b];
	    _cairo_traps_add_clipped_trap (traps, q[a].y, q[b].y, &left, &right);
	    right.p1 = q[b]; right.p2 = q[c];
	    _cairo_traps_add_clipped_trap (traps, q[b].y, q[d].y, &left, &right);
	    left.p1  = q[d]; left.p2  = q[c];
	    _cairo_traps_add_clipped_trap (traps, q[d].y, q[c].y, &left, &right);
	}
    }
}

static void add_tri (cairo_traps_t *traps,
		     int y1, int y2,
		     const cairo_line_t *left,
		     const cairo_line_t *right)
{
    if (y2 < y1) {
	int tmp = y1;
	y1 = y2;
	y2 = tmp;
    }

    if (_cairo_lines_compare_at_y (left, right, y1) > 0) {
	const cairo_line_t *tmp = left;
	left = right;
	right = tmp;
    }

    _cairo_traps_add_clipped_trap (traps, y1, y2, left, right);
}

void
_cairo_traps_tessellate_triangle_with_edges (cairo_traps_t *traps,
					     const cairo_point_t t[3],
					     const cairo_point_t edges[4])
{
    cairo_line_t lines[3];

    if (edges[0].y <= edges[1].y) {
	    lines[0].p1 = edges[0];
	    lines[0].p2 = edges[1];
    } else {
	    lines[0].p1 = edges[1];
	    lines[0].p2 = edges[0];
    }

    if (edges[2].y <= edges[3].y) {
	    lines[1].p1 = edges[2];
	    lines[1].p2 = edges[3];
    } else {
	    lines[1].p1 = edges[3];
	    lines[1].p2 = edges[2];
    }

    if (t[1].y == t[2].y) {
	add_tri (traps, t[0].y, t[1].y, &lines[0], &lines[1]);
	return;
    }

    if (t[1].y <= t[2].y) {
	    lines[2].p1 = t[1];
	    lines[2].p2 = t[2];
    } else {
	    lines[2].p1 = t[2];
	    lines[2].p2 = t[1];
    }

    if (((t[1].y - t[0].y) < 0) ^ ((t[2].y - t[0].y) < 0)) {
	add_tri (traps, t[0].y, t[1].y, &lines[0], &lines[2]);
	add_tri (traps, t[0].y, t[2].y, &lines[1], &lines[2]);
    } else if (abs(t[1].y - t[0].y) < abs(t[2].y - t[0].y)) {
	add_tri (traps, t[0].y, t[1].y, &lines[0], &lines[1]);
	add_tri (traps, t[1].y, t[2].y, &lines[2], &lines[1]);
    } else {
	add_tri (traps, t[0].y, t[2].y, &lines[1], &lines[0]);
	add_tri (traps, t[1].y, t[2].y, &lines[2], &lines[0]);
    }
}

/**
 * _cairo_traps_init_boxes:
 * @traps: a #cairo_traps_t
 * @box: an array box that will each be converted to a single trapezoid
 *       to store in @traps.
 *
 * Initializes a #cairo_traps_t to contain an array of rectangular
 * trapezoids.
 **/
cairo_status_t
_cairo_traps_init_boxes (cairo_traps_t	    *traps,
		         const cairo_boxes_t *boxes)
{
    cairo_trapezoid_t *trap;
    const struct _cairo_boxes_chunk *chunk;

    _cairo_traps_init (traps);

    while (traps->traps_size < boxes->num_boxes) {
	if (unlikely (! _cairo_traps_grow (traps))) {
	    _cairo_traps_fini (traps);
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	}
    }

    traps->num_traps = boxes->num_boxes;
    traps->is_rectilinear = TRUE;
    traps->is_rectangular = TRUE;
    traps->maybe_region = boxes->is_pixel_aligned;

    trap = &traps->traps[0];
    for (chunk = &boxes->chunks; chunk != NULL; chunk = chunk->next) {
	const cairo_box_t *box;
	int i;

	box = chunk->base;
	for (i = 0; i < chunk->count; i++) {
	    trap->top    = box->p1.y;
	    trap->bottom = box->p2.y;

	    trap->left.p1   = box->p1;
	    trap->left.p2.x = box->p1.x;
	    trap->left.p2.y = box->p2.y;

	    trap->right.p1.x = box->p2.x;
	    trap->right.p1.y = box->p1.y;
	    trap->right.p2   = box->p2;

	    box++, trap++;
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_traps_tessellate_rectangle (cairo_traps_t *traps,
				   const cairo_point_t *top_left,
				   const cairo_point_t *bottom_right)
{
    cairo_line_t left;
    cairo_line_t right;
    cairo_fixed_t top, bottom;

    if (top_left->y == bottom_right->y)
	return CAIRO_STATUS_SUCCESS;

    if (top_left->x == bottom_right->x)
	return CAIRO_STATUS_SUCCESS;

     left.p1.x =  left.p2.x = top_left->x;
     left.p1.y = right.p1.y = top_left->y;
    right.p1.x = right.p2.x = bottom_right->x;
     left.p2.y = right.p2.y = bottom_right->y;

     top = top_left->y;
     bottom = bottom_right->y;

    if (traps->num_limits) {
	cairo_bool_t reversed;
	int n;

	if (top >= traps->bounds.p2.y || bottom <= traps->bounds.p1.y)
	    return CAIRO_STATUS_SUCCESS;

	/* support counter-clockwise winding for rectangular tessellation */
	reversed = top_left->x > bottom_right->x;
	if (reversed) {
	    right.p1.x = right.p2.x = top_left->x;
	    left.p1.x = left.p2.x = bottom_right->x;
	}

	if (left.p1.x >= traps->bounds.p2.x || right.p1.x <= traps->bounds.p1.x)
	    return CAIRO_STATUS_SUCCESS;

	for (n = 0; n < traps->num_limits; n++) {
	    const cairo_box_t *limits = &traps->limits[n];
	    cairo_line_t _left, _right;
	    cairo_fixed_t _top, _bottom;

	    if (top >= limits->p2.y)
		continue;
	    if (bottom <= limits->p1.y)
		continue;

	    /* Trivially reject if trapezoid is entirely to the right or
	     * to the left of the limits. */
	    if (left.p1.x >= limits->p2.x)
		continue;
	    if (right.p1.x <= limits->p1.x)
		continue;

	    /* Otherwise, clip the trapezoid to the limits. */
	    _top = top;
	    if (_top < limits->p1.y)
		_top = limits->p1.y;

	    _bottom = bottom;
	    if (_bottom > limits->p2.y)
		_bottom = limits->p2.y;

	    if (_bottom <= _top)
		continue;

	    _left = left;
	    if (_left.p1.x < limits->p1.x) {
		_left.p1.x = limits->p1.x;
		_left.p1.y = limits->p1.y;
		_left.p2.x = limits->p1.x;
		_left.p2.y = limits->p2.y;
	    }

	    _right = right;
	    if (_right.p1.x > limits->p2.x) {
		_right.p1.x = limits->p2.x;
		_right.p1.y = limits->p1.y;
		_right.p2.x = limits->p2.x;
		_right.p2.y = limits->p2.y;
	    }

	    if (left.p1.x >= right.p1.x)
		continue;

	    if (reversed)
		_cairo_traps_add_trap (traps, _top, _bottom, &_right, &_left);
	    else
		_cairo_traps_add_trap (traps, _top, _bottom, &_left, &_right);
	}
    } else {
	_cairo_traps_add_trap (traps, top, bottom, &left, &right);
    }

    return traps->status;
}

void
_cairo_traps_translate (cairo_traps_t *traps, int x, int y)
{
    cairo_fixed_t xoff, yoff;
    cairo_trapezoid_t *t;
    int i;

    /* Ugh. The cairo_composite/(Render) interface doesn't allow
       an offset for the trapezoids. Need to manually shift all
       the coordinates to align with the offset origin of the
       intermediate surface. */

    xoff = _cairo_fixed_from_int (x);
    yoff = _cairo_fixed_from_int (y);

    for (i = 0, t = traps->traps; i < traps->num_traps; i++, t++) {
	t->top += yoff;
	t->bottom += yoff;
	t->left.p1.x += xoff;
	t->left.p1.y += yoff;
	t->left.p2.x += xoff;
	t->left.p2.y += yoff;
	t->right.p1.x += xoff;
	t->right.p1.y += yoff;
	t->right.p2.x += xoff;
	t->right.p2.y += yoff;
    }
}

void
_cairo_trapezoid_array_translate_and_scale (cairo_trapezoid_t *offset_traps,
                                            cairo_trapezoid_t *src_traps,
                                            int num_traps,
                                            double tx, double ty,
                                            double sx, double sy)
{
    int i;
    cairo_fixed_t xoff = _cairo_fixed_from_double (tx);
    cairo_fixed_t yoff = _cairo_fixed_from_double (ty);

    if (sx == 1.0 && sy == 1.0) {
        for (i = 0; i < num_traps; i++) {
            offset_traps[i].top = src_traps[i].top + yoff;
            offset_traps[i].bottom = src_traps[i].bottom + yoff;
            offset_traps[i].left.p1.x = src_traps[i].left.p1.x + xoff;
            offset_traps[i].left.p1.y = src_traps[i].left.p1.y + yoff;
            offset_traps[i].left.p2.x = src_traps[i].left.p2.x + xoff;
            offset_traps[i].left.p2.y = src_traps[i].left.p2.y + yoff;
            offset_traps[i].right.p1.x = src_traps[i].right.p1.x + xoff;
            offset_traps[i].right.p1.y = src_traps[i].right.p1.y + yoff;
            offset_traps[i].right.p2.x = src_traps[i].right.p2.x + xoff;
            offset_traps[i].right.p2.y = src_traps[i].right.p2.y + yoff;
        }
    } else {
        cairo_fixed_t xsc = _cairo_fixed_from_double (sx);
        cairo_fixed_t ysc = _cairo_fixed_from_double (sy);

        for (i = 0; i < num_traps; i++) {
            offset_traps[i].top = _cairo_fixed_mul (src_traps[i].top + yoff, ysc);
            offset_traps[i].bottom = _cairo_fixed_mul (src_traps[i].bottom + yoff, ysc);
            offset_traps[i].left.p1.x = _cairo_fixed_mul (src_traps[i].left.p1.x + xoff, xsc);
            offset_traps[i].left.p1.y = _cairo_fixed_mul (src_traps[i].left.p1.y + yoff, ysc);
            offset_traps[i].left.p2.x = _cairo_fixed_mul (src_traps[i].left.p2.x + xoff, xsc);
            offset_traps[i].left.p2.y = _cairo_fixed_mul (src_traps[i].left.p2.y + yoff, ysc);
            offset_traps[i].right.p1.x = _cairo_fixed_mul (src_traps[i].right.p1.x + xoff, xsc);
            offset_traps[i].right.p1.y = _cairo_fixed_mul (src_traps[i].right.p1.y + yoff, ysc);
            offset_traps[i].right.p2.x = _cairo_fixed_mul (src_traps[i].right.p2.x + xoff, xsc);
            offset_traps[i].right.p2.y = _cairo_fixed_mul (src_traps[i].right.p2.y + yoff, ysc);
        }
    }
}

static cairo_bool_t
_cairo_trap_contains (cairo_trapezoid_t *t, cairo_point_t *pt)
{
    cairo_slope_t slope_left, slope_pt, slope_right;

    if (t->top > pt->y)
	return FALSE;
    if (t->bottom < pt->y)
	return FALSE;

    _cairo_slope_init (&slope_left, &t->left.p1, &t->left.p2);
    _cairo_slope_init (&slope_pt, &t->left.p1, pt);

    if (_cairo_slope_compare (&slope_left, &slope_pt) < 0)
	return FALSE;

    _cairo_slope_init (&slope_right, &t->right.p1, &t->right.p2);
    _cairo_slope_init (&slope_pt, &t->right.p1, pt);

    if (_cairo_slope_compare (&slope_pt, &slope_right) < 0)
	return FALSE;

    return TRUE;
}

cairo_bool_t
_cairo_traps_contain (const cairo_traps_t *traps,
		      double x, double y)
{
    int i;
    cairo_point_t point;

    point.x = _cairo_fixed_from_double (x);
    point.y = _cairo_fixed_from_double (y);

    for (i = 0; i < traps->num_traps; i++) {
	if (_cairo_trap_contains (&traps->traps[i], &point))
	    return TRUE;
    }

    return FALSE;
}

static cairo_fixed_t
_line_compute_intersection_x_for_y (const cairo_line_t *line,
				    cairo_fixed_t y)
{
    return _cairo_edge_compute_intersection_x_for_y (&line->p1, &line->p2, y);
}

void
_cairo_traps_extents (const cairo_traps_t *traps,
		      cairo_box_t *extents)
{
    int i;

    if (traps->num_traps == 0) {
	extents->p1.x = extents->p1.y = 0;
	extents->p2.x = extents->p2.y = 0;
	return;
    }

    extents->p1.x = extents->p1.y = INT32_MAX;
    extents->p2.x = extents->p2.y = INT32_MIN;

    for (i = 0; i < traps->num_traps; i++) {
	const cairo_trapezoid_t *trap =  &traps->traps[i];

	if (trap->top < extents->p1.y)
	    extents->p1.y = trap->top;
	if (trap->bottom > extents->p2.y)
	    extents->p2.y = trap->bottom;

	if (trap->left.p1.x < extents->p1.x) {
	    cairo_fixed_t x = trap->left.p1.x;
	    if (trap->top != trap->left.p1.y) {
		x = _line_compute_intersection_x_for_y (&trap->left,
							trap->top);
		if (x < extents->p1.x)
		    extents->p1.x = x;
	    } else
		extents->p1.x = x;
	}
	if (trap->left.p2.x < extents->p1.x) {
	    cairo_fixed_t x = trap->left.p2.x;
	    if (trap->bottom != trap->left.p2.y) {
		x = _line_compute_intersection_x_for_y (&trap->left,
							trap->bottom);
		if (x < extents->p1.x)
		    extents->p1.x = x;
	    } else
		extents->p1.x = x;
	}

	if (trap->right.p1.x > extents->p2.x) {
	    cairo_fixed_t x = trap->right.p1.x;
	    if (trap->top != trap->right.p1.y) {
		x = _line_compute_intersection_x_for_y (&trap->right,
							trap->top);
		if (x > extents->p2.x)
		    extents->p2.x = x;
	    } else
		extents->p2.x = x;
	}
	if (trap->right.p2.x > extents->p2.x) {
	    cairo_fixed_t x = trap->right.p2.x;
	    if (trap->bottom != trap->right.p2.y) {
		x = _line_compute_intersection_x_for_y (&trap->right,
							trap->bottom);
		if (x > extents->p2.x)
		    extents->p2.x = x;
	    } else
		extents->p2.x = x;
	}
    }
}

static cairo_bool_t
_mono_edge_is_vertical (const cairo_line_t *line)
{
    return _cairo_fixed_integer_round_down (line->p1.x) == _cairo_fixed_integer_round_down (line->p2.x);
}

static cairo_bool_t
_traps_are_pixel_aligned (cairo_traps_t *traps,
			  cairo_antialias_t antialias)
{
    int i;

    if (antialias == CAIRO_ANTIALIAS_NONE) {
	for (i = 0; i < traps->num_traps; i++) {
	    if (! _mono_edge_is_vertical (&traps->traps[i].left)   ||
		! _mono_edge_is_vertical (&traps->traps[i].right))
	    {
		traps->maybe_region = FALSE;
		return FALSE;
	    }
	}
    } else {
	for (i = 0; i < traps->num_traps; i++) {
	    if (traps->traps[i].left.p1.x != traps->traps[i].left.p2.x   ||
		traps->traps[i].right.p1.x != traps->traps[i].right.p2.x ||
		! _cairo_fixed_is_integer (traps->traps[i].top)          ||
		! _cairo_fixed_is_integer (traps->traps[i].bottom)       ||
		! _cairo_fixed_is_integer (traps->traps[i].left.p1.x)    ||
		! _cairo_fixed_is_integer (traps->traps[i].right.p1.x))
	    {
		traps->maybe_region = FALSE;
		return FALSE;
	    }
	}
    }

    return TRUE;
}

/**
 * _cairo_traps_extract_region:
 * @traps: a #cairo_traps_t
 * @region: a #cairo_region_t
 *
 * Determines if a set of trapezoids are exactly representable as a
 * cairo region.  If so, the passed-in region is initialized to
 * the area representing the given traps.  It should be finalized
 * with cairo_region_fini().  If not, %CAIRO_INT_STATUS_UNSUPPORTED
 * is returned.
 *
 * Return value: %CAIRO_STATUS_SUCCESS, %CAIRO_INT_STATUS_UNSUPPORTED
 * or %CAIRO_STATUS_NO_MEMORY
 **/
cairo_int_status_t
_cairo_traps_extract_region (cairo_traps_t   *traps,
			     cairo_antialias_t antialias,
			     cairo_region_t **region)
{
    cairo_rectangle_int_t stack_rects[CAIRO_STACK_ARRAY_LENGTH (cairo_rectangle_int_t)];
    cairo_rectangle_int_t *rects = stack_rects;
    cairo_int_status_t status;
    int i, rect_count;

    /* we only treat this a hint... */
    if (antialias != CAIRO_ANTIALIAS_NONE && ! traps->maybe_region)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    if (! _traps_are_pixel_aligned (traps, antialias)) {
	traps->maybe_region = FALSE;
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    if (traps->num_traps > ARRAY_LENGTH (stack_rects)) {
	rects = _cairo_malloc_ab (traps->num_traps, sizeof (cairo_rectangle_int_t));

	if (unlikely (rects == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    rect_count = 0;
    for (i = 0; i < traps->num_traps; i++) {
	int x1, y1, x2, y2;

	if (antialias == CAIRO_ANTIALIAS_NONE) {
	    x1 = _cairo_fixed_integer_round_down (traps->traps[i].left.p1.x);
	    y1 = _cairo_fixed_integer_round_down (traps->traps[i].top);
	    x2 = _cairo_fixed_integer_round_down (traps->traps[i].right.p1.x);
	    y2 = _cairo_fixed_integer_round_down (traps->traps[i].bottom);
	} else {
	    x1 = _cairo_fixed_integer_part (traps->traps[i].left.p1.x);
	    y1 = _cairo_fixed_integer_part (traps->traps[i].top);
	    x2 = _cairo_fixed_integer_part (traps->traps[i].right.p1.x);
	    y2 = _cairo_fixed_integer_part (traps->traps[i].bottom);
	}

	if (x2 > x1 && y2 > y1) {
	    rects[rect_count].x = x1;
	    rects[rect_count].y = y1;
	    rects[rect_count].width  = x2 - x1;
	    rects[rect_count].height = y2 - y1;
	    rect_count++;
	}
    }


    *region = cairo_region_create_rectangles (rects, rect_count);
    status = (*region)->status;

    if (rects != stack_rects)
	free (rects);

    return status;
}

cairo_bool_t
_cairo_traps_to_boxes (cairo_traps_t *traps,
		       cairo_antialias_t antialias,
		       cairo_boxes_t *boxes)
{
    int i;

    for (i = 0; i < traps->num_traps; i++) {
	if (traps->traps[i].left.p1.x  != traps->traps[i].left.p2.x ||
	    traps->traps[i].right.p1.x != traps->traps[i].right.p2.x)
	    return FALSE;
    }

    _cairo_boxes_init (boxes);

    boxes->num_boxes    = traps->num_traps;
    boxes->chunks.base  = (cairo_box_t *) traps->traps;
    boxes->chunks.count = traps->num_traps;
    boxes->chunks.size  = traps->num_traps;

    if (antialias != CAIRO_ANTIALIAS_NONE) {
	for (i = 0; i < traps->num_traps; i++) {
	    /* Note the traps and boxes alias so we need to take the local copies first. */
	    cairo_fixed_t x1 = traps->traps[i].left.p1.x;
	    cairo_fixed_t x2 = traps->traps[i].right.p1.x;
	    cairo_fixed_t y1 = traps->traps[i].top;
	    cairo_fixed_t y2 = traps->traps[i].bottom;

	    boxes->chunks.base[i].p1.x = x1;
	    boxes->chunks.base[i].p1.y = y1;
	    boxes->chunks.base[i].p2.x = x2;
	    boxes->chunks.base[i].p2.y = y2;

	    if (boxes->is_pixel_aligned) {
		boxes->is_pixel_aligned =
		    _cairo_fixed_is_integer (x1) && _cairo_fixed_is_integer (y1) &&
		    _cairo_fixed_is_integer (x2) && _cairo_fixed_is_integer (y2);
	    }
	}
    } else {
	boxes->is_pixel_aligned = TRUE;

	for (i = 0; i < traps->num_traps; i++) {
	    /* Note the traps and boxes alias so we need to take the local copies first. */
	    cairo_fixed_t x1 = traps->traps[i].left.p1.x;
	    cairo_fixed_t x2 = traps->traps[i].right.p1.x;
	    cairo_fixed_t y1 = traps->traps[i].top;
	    cairo_fixed_t y2 = traps->traps[i].bottom;

	    /* round down here to match Pixman's behavior when using traps. */
	    boxes->chunks.base[i].p1.x = _cairo_fixed_round_down (x1);
	    boxes->chunks.base[i].p1.y = _cairo_fixed_round_down (y1);
	    boxes->chunks.base[i].p2.x = _cairo_fixed_round_down (x2);
	    boxes->chunks.base[i].p2.y = _cairo_fixed_round_down (y2);
	}
    }

    return TRUE;
}

/* moves trap points such that they become the actual corners of the trapezoid */
static void
_sanitize_trap (cairo_trapezoid_t *t)
{
    cairo_trapezoid_t s = *t;

#define FIX(lr, tb, p) \
    if (t->lr.p.y != t->tb) { \
        t->lr.p.x = s.lr.p2.x + _cairo_fixed_mul_div_floor (s.lr.p1.x - s.lr.p2.x, s.tb - s.lr.p2.y, s.lr.p1.y - s.lr.p2.y); \
        t->lr.p.y = s.tb; \
    }
    FIX (left,  top,    p1);
    FIX (left,  bottom, p2);
    FIX (right, top,    p1);
    FIX (right, bottom, p2);
}

cairo_private cairo_status_t
_cairo_traps_path (const cairo_traps_t *traps,
		   cairo_path_fixed_t  *path)
{
    int i;

    for (i = 0; i < traps->num_traps; i++) {
	cairo_status_t status;
	cairo_trapezoid_t trap = traps->traps[i];

	if (trap.top == trap.bottom)
	    continue;

	_sanitize_trap (&trap);

	status = _cairo_path_fixed_move_to (path, trap.left.p1.x, trap.top);
	if (unlikely (status)) return status;
	status = _cairo_path_fixed_line_to (path, trap.right.p1.x, trap.top);
	if (unlikely (status)) return status;
	status = _cairo_path_fixed_line_to (path, trap.right.p2.x, trap.bottom);
	if (unlikely (status)) return status;
	status = _cairo_path_fixed_line_to (path, trap.left.p2.x, trap.bottom);
	if (unlikely (status)) return status;
	status = _cairo_path_fixed_close_path (path);
	if (unlikely (status)) return status;
    }

    return CAIRO_STATUS_SUCCESS;
}

void
_cairo_debug_print_traps (FILE *file, const cairo_traps_t *traps)
{
    cairo_box_t extents;
    int n;

#if 0
    if (traps->has_limits) {
	printf ("%s: limits=(%d, %d, %d, %d)\n",
		filename,
		traps->limits.p1.x, traps->limits.p1.y,
		traps->limits.p2.x, traps->limits.p2.y);
    }
#endif

    _cairo_traps_extents (traps, &extents);
    fprintf (file, "extents=(%d, %d, %d, %d)\n",
	     extents.p1.x, extents.p1.y,
	     extents.p2.x, extents.p2.y);

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
}

struct cairo_trap_renderer {
    cairo_span_renderer_t base;
    cairo_traps_t *traps;
};

static cairo_status_t
span_to_traps (void *abstract_renderer, int y, int h,
	       const cairo_half_open_span_t *spans, unsigned num_spans)
{
    struct cairo_trap_renderer *r = abstract_renderer;
    cairo_fixed_t top, bot;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    top = _cairo_fixed_from_int (y);
    bot = _cairo_fixed_from_int (y + h);
    do {
	if (spans[0].coverage) {
	    cairo_fixed_t x0 = _cairo_fixed_from_int(spans[0].x);
	    cairo_fixed_t x1 = _cairo_fixed_from_int(spans[1].x);
	    cairo_line_t left = { { x0, top }, { x0, bot } },
			 right = { { x1, top }, { x1, bot } };
	    _cairo_traps_add_trap (r->traps, top, bot, &left, &right);
	}
	spans++;
    } while (--num_spans > 1);

    return CAIRO_STATUS_SUCCESS;
}

cairo_int_status_t
_cairo_rasterise_polygon_to_traps (cairo_polygon_t			*polygon,
				   cairo_fill_rule_t			 fill_rule,
				   cairo_antialias_t			 antialias,
				   cairo_traps_t *traps)
{
    struct cairo_trap_renderer renderer;
    cairo_scan_converter_t *converter;
    cairo_int_status_t status;
    cairo_rectangle_int_t r;

    TRACE ((stderr, "%s: fill_rule=%d, antialias=%d\n",
	    __FUNCTION__, fill_rule, antialias));
    assert(antialias == CAIRO_ANTIALIAS_NONE);

    renderer.traps = traps;
    renderer.base.render_rows = span_to_traps;

    _cairo_box_round_to_rectangle (&polygon->extents, &r);
    converter = _cairo_mono_scan_converter_create (r.x, r.y,
						   r.x + r.width,
						   r.y + r.height,
						   fill_rule);
    status = _cairo_mono_scan_converter_add_polygon (converter, polygon);
    if (likely (status == CAIRO_INT_STATUS_SUCCESS))
	status = converter->generate (converter, &renderer.base);
    converter->destroy (converter);
    return status;
}
