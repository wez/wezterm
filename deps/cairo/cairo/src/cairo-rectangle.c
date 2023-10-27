/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2005 Red Hat, Inc.
 * Copyright © 2006 Red Hat, Inc.
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

#include "cairo-box-inline.h"

const cairo_rectangle_int_t _cairo_empty_rectangle = { 0, 0, 0, 0 };
const cairo_rectangle_int_t _cairo_unbounded_rectangle = {
     CAIRO_RECT_INT_MIN, CAIRO_RECT_INT_MIN,
     CAIRO_RECT_INT_MAX - CAIRO_RECT_INT_MIN,
     CAIRO_RECT_INT_MAX - CAIRO_RECT_INT_MIN,
};

cairo_private void
_cairo_box_from_doubles (cairo_box_t *box,
			 double *x1, double *y1,
			 double *x2, double *y2)
{
    box->p1.x = _cairo_fixed_from_double (*x1);
    box->p1.y = _cairo_fixed_from_double (*y1);
    box->p2.x = _cairo_fixed_from_double (*x2);
    box->p2.y = _cairo_fixed_from_double (*y2);
}

cairo_private void
_cairo_box_to_doubles (const cairo_box_t *box,
		       double *x1, double *y1,
		       double *x2, double *y2)
{
    *x1 = _cairo_fixed_to_double (box->p1.x);
    *y1 = _cairo_fixed_to_double (box->p1.y);
    *x2 = _cairo_fixed_to_double (box->p2.x);
    *y2 = _cairo_fixed_to_double (box->p2.y);
}

void
_cairo_box_from_rectangle (cairo_box_t                 *box,
			   const cairo_rectangle_int_t *rect)
{
    box->p1.x = _cairo_fixed_from_int (rect->x);
    box->p1.y = _cairo_fixed_from_int (rect->y);
    box->p2.x = _cairo_fixed_from_int (rect->x + rect->width);
    box->p2.y = _cairo_fixed_from_int (rect->y + rect->height);
}

void
_cairo_boxes_get_extents (const cairo_box_t *boxes,
			  int num_boxes,
			  cairo_box_t *extents)
{
    assert (num_boxes > 0);
    *extents = *boxes;
    while (--num_boxes)
	_cairo_box_add_box (extents, ++boxes);
}

/* XXX We currently have a confusing mix of boxes and rectangles as
 * exemplified by this function.  A #cairo_box_t is a rectangular area
 * represented by the coordinates of the upper left and lower right
 * corners, expressed in fixed point numbers.  A #cairo_rectangle_int_t is
 * also a rectangular area, but represented by the upper left corner
 * and the width and the height, as integer numbers.
 *
 * This function converts a #cairo_box_t to a #cairo_rectangle_int_t by
 * increasing the area to the nearest integer coordinates.  We should
 * standardize on #cairo_rectangle_fixed_t and #cairo_rectangle_int_t, and
 * this function could be renamed to the more reasonable
 * _cairo_rectangle_fixed_round.
 */

void
_cairo_box_round_to_rectangle (const cairo_box_t     *box,
			       cairo_rectangle_int_t *rectangle)
{
    rectangle->x = _cairo_fixed_integer_floor (box->p1.x);
    rectangle->y = _cairo_fixed_integer_floor (box->p1.y);
    rectangle->width = _cairo_fixed_integer_ceil (box->p2.x) - rectangle->x;
    rectangle->height = _cairo_fixed_integer_ceil (box->p2.y) - rectangle->y;
}

cairo_bool_t
_cairo_rectangle_intersect (cairo_rectangle_int_t *dst,
			    const cairo_rectangle_int_t *src)
{
    int x1, y1, x2, y2;

    x1 = MAX (dst->x, src->x);
    y1 = MAX (dst->y, src->y);
    /* Beware the unsigned promotion, fortunately we have bits to spare
     * as (CAIRO_RECT_INT_MAX - CAIRO_RECT_INT_MIN) < UINT_MAX
     */
    x2 = MIN (dst->x + (int) dst->width,  src->x + (int) src->width);
    y2 = MIN (dst->y + (int) dst->height, src->y + (int) src->height);

    if (x1 >= x2 || y1 >= y2) {
	dst->x = 0;
	dst->y = 0;
	dst->width  = 0;
	dst->height = 0;

	return FALSE;
    } else {
	dst->x = x1;
	dst->y = y1;
	dst->width  = x2 - x1;
	dst->height = y2 - y1;

	return TRUE;
    }
}

/* Extends the dst rectangle to also contain src.
 * If one of the rectangles is empty, the result is undefined
 */
void
_cairo_rectangle_union (cairo_rectangle_int_t *dst,
			const cairo_rectangle_int_t *src)
{
    int x1, y1, x2, y2;

    x1 = MIN (dst->x, src->x);
    y1 = MIN (dst->y, src->y);
    /* Beware the unsigned promotion, fortunately we have bits to spare
     * as (CAIRO_RECT_INT_MAX - CAIRO_RECT_INT_MIN) < UINT_MAX
     */
    x2 = MAX (dst->x + (int) dst->width,  src->x + (int) src->width);
    y2 = MAX (dst->y + (int) dst->height, src->y + (int) src->height);

    dst->x = x1;
    dst->y = y1;
    dst->width  = x2 - x1;
    dst->height = y2 - y1;
}

#define P1x (line->p1.x)
#define P1y (line->p1.y)
#define P2x (line->p2.x)
#define P2y (line->p2.y)
#define B1x (box->p1.x)
#define B1y (box->p1.y)
#define B2x (box->p2.x)
#define B2y (box->p2.y)

/*
 * Check whether any part of line intersects box.  This function essentially
 * computes whether the ray starting at line->p1 in the direction of line->p2
 * intersects the box before it reaches p2.  Normally, this is done
 * by dividing by the lengths of the line projected onto each axis.  Because
 * we're in fixed point, this function does a bit more work to avoid having to
 * do the division -- we don't care about the actual intersection point, so
 * it's of no interest to us.
 */

cairo_bool_t
_cairo_box_intersects_line_segment (const cairo_box_t *box, cairo_line_t *line)
{
    cairo_fixed_t t1=0, t2=0, t3=0, t4=0;
    cairo_int64_t t1y, t2y, t3x, t4x;

    cairo_fixed_t xlen, ylen;

    if (_cairo_box_contains_point (box, &line->p1) ||
	_cairo_box_contains_point (box, &line->p2))
	return TRUE;

    xlen = P2x - P1x;
    ylen = P2y - P1y;

    if (xlen) {
	if (xlen > 0) {
	    t1 = B1x - P1x;
	    t2 = B2x - P1x;
	} else {
	    t1 = P1x - B2x;
	    t2 = P1x - B1x;
	    xlen = - xlen;
	}

	if ((t1 < 0 || t1 > xlen) &&
	    (t2 < 0 || t2 > xlen))
	    return FALSE;
    } else {
	/* Fully vertical line -- check that X is in bounds */
	if (P1x < B1x || P1x > B2x)
	    return FALSE;
    }

    if (ylen) {
	if (ylen > 0) {
	    t3 = B1y - P1y;
	    t4 = B2y - P1y;
	} else {
	    t3 = P1y - B2y;
	    t4 = P1y - B1y;
	    ylen = - ylen;
	}

	if ((t3 < 0 || t3 > ylen) &&
	    (t4 < 0 || t4 > ylen))
	    return FALSE;
    } else {
	/* Fully horizontal line -- check Y */
	if (P1y < B1y || P1y > B2y)
	    return FALSE;
    }

    /* If we had a horizontal or vertical line, then it's already been checked */
    if (P1x == P2x || P1y == P2y)
	return TRUE;

    /* Check overlap.  Note that t1 < t2 and t3 < t4 here. */
    t1y = _cairo_int32x32_64_mul (t1, ylen);
    t2y = _cairo_int32x32_64_mul (t2, ylen);
    t3x = _cairo_int32x32_64_mul (t3, xlen);
    t4x = _cairo_int32x32_64_mul (t4, xlen);

    if (_cairo_int64_lt(t1y, t4x) &&
	_cairo_int64_lt(t3x, t2y))
	return TRUE;

    return FALSE;
}

static cairo_status_t
_cairo_box_add_spline_point (void *closure,
			     const cairo_point_t *point,
			     const cairo_slope_t *tangent)
{
    _cairo_box_add_point (closure, point);

    return CAIRO_STATUS_SUCCESS;
}

/* assumes a has been previously added */
void
_cairo_box_add_curve_to (cairo_box_t *extents,
			 const cairo_point_t *a,
			 const cairo_point_t *b,
			 const cairo_point_t *c,
			 const cairo_point_t *d)
{
    _cairo_box_add_point (extents, d);
    if (!_cairo_box_contains_point (extents, b) ||
	!_cairo_box_contains_point (extents, c))
    {
	cairo_status_t status;

	status = _cairo_spline_bound (_cairo_box_add_spline_point,
				      extents, a, b, c, d);
	assert (status == CAIRO_STATUS_SUCCESS);
    }
}

void
_cairo_rectangle_int_from_double (cairo_rectangle_int_t *recti,
				  const cairo_rectangle_t *rectf)
{
	recti->x = floor (rectf->x);
	recti->y = floor (rectf->y);
	recti->width  = ceil (rectf->x + rectf->width) - floor (rectf->x);
	recti->height = ceil (rectf->y + rectf->height) - floor (rectf->y);
}
