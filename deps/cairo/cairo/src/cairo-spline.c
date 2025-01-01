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

#include "cairo-box-inline.h"
#include "cairo-slope-private.h"

cairo_bool_t
_cairo_spline_intersects (const cairo_point_t *a,
			  const cairo_point_t *b,
			  const cairo_point_t *c,
			  const cairo_point_t *d,
			  const cairo_box_t *box)
{
    cairo_box_t bounds;

    if (_cairo_box_contains_point (box, a) ||
	_cairo_box_contains_point (box, b) ||
	_cairo_box_contains_point (box, c) ||
	_cairo_box_contains_point (box, d))
    {
	return TRUE;
    }

    bounds.p2 = bounds.p1 = *a;
    _cairo_box_add_point (&bounds, b);
    _cairo_box_add_point (&bounds, c);
    _cairo_box_add_point (&bounds, d);

    if (bounds.p2.x <= box->p1.x || bounds.p1.x >= box->p2.x ||
	bounds.p2.y <= box->p1.y || bounds.p1.y >= box->p2.y)
    {
	return FALSE;
    }

#if 0 /* worth refining? */
    bounds.p2 = bounds.p1 = *a;
    _cairo_box_add_curve_to (&bounds, b, c, d);
    if (bounds.p2.x <= box->p1.x || bounds.p1.x >= box->p2.x ||
	bounds.p2.y <= box->p1.y || bounds.p1.y >= box->p2.y)
    {
	return FALSE;
    }
#endif

    return TRUE;
}

cairo_bool_t
_cairo_spline_init (cairo_spline_t *spline,
		    cairo_spline_add_point_func_t add_point_func,
		    void *closure,
		    const cairo_point_t *a, const cairo_point_t *b,
		    const cairo_point_t *c, const cairo_point_t *d)
{
    /* If both tangents are zero, this is just a straight line */
    if (a->x == b->x && a->y == b->y && c->x == d->x && c->y == d->y)
	return FALSE;

    spline->add_point_func = add_point_func;
    spline->closure = closure;

    spline->knots.a = *a;
    spline->knots.b = *b;
    spline->knots.c = *c;
    spline->knots.d = *d;

    if (a->x != b->x || a->y != b->y)
	_cairo_slope_init (&spline->initial_slope, &spline->knots.a, &spline->knots.b);
    else if (a->x != c->x || a->y != c->y)
	_cairo_slope_init (&spline->initial_slope, &spline->knots.a, &spline->knots.c);
    else if (a->x != d->x || a->y != d->y)
	_cairo_slope_init (&spline->initial_slope, &spline->knots.a, &spline->knots.d);
    else
	return FALSE;

    if (c->x != d->x || c->y != d->y)
	_cairo_slope_init (&spline->final_slope, &spline->knots.c, &spline->knots.d);
    else if (b->x != d->x || b->y != d->y)
	_cairo_slope_init (&spline->final_slope, &spline->knots.b, &spline->knots.d);
    else
	return FALSE; /* just treat this as a straight-line from a -> d */

    /* XXX if the initial, final and vector are all equal, this is just a line */

    return TRUE;
}

static cairo_status_t
_cairo_spline_add_point (cairo_spline_t *spline,
			 const cairo_point_t *point,
			 const cairo_point_t *knot)
{
    cairo_point_t *prev;
    cairo_slope_t slope;

    prev = &spline->last_point;
    if (prev->x == point->x && prev->y == point->y)
	return CAIRO_STATUS_SUCCESS;

    _cairo_slope_init (&slope, point, knot);

    spline->last_point = *point;
    return spline->add_point_func (spline->closure, point, &slope);
}

static void
_lerp_half (const cairo_point_t *a, const cairo_point_t *b, cairo_point_t *result)
{
    result->x = a->x + ((b->x - a->x) >> 1);
    result->y = a->y + ((b->y - a->y) >> 1);
}

static void
_de_casteljau (cairo_spline_knots_t *s1, cairo_spline_knots_t *s2)
{
    cairo_point_t ab, bc, cd;
    cairo_point_t abbc, bccd;
    cairo_point_t final;

    _lerp_half (&s1->a, &s1->b, &ab);
    _lerp_half (&s1->b, &s1->c, &bc);
    _lerp_half (&s1->c, &s1->d, &cd);
    _lerp_half (&ab, &bc, &abbc);
    _lerp_half (&bc, &cd, &bccd);
    _lerp_half (&abbc, &bccd, &final);

    s2->a = final;
    s2->b = bccd;
    s2->c = cd;
    s2->d = s1->d;

    s1->b = ab;
    s1->c = abbc;
    s1->d = final;
}

/* Return an upper bound on the error (squared) that could result from
 * approximating a spline as a line segment connecting the two endpoints. */
static double
_cairo_spline_error_squared (const cairo_spline_knots_t *knots)
{
    double bdx, bdy, berr;
    double cdx, cdy, cerr;

    /* We are going to compute the distance (squared) between each of the b
     * and c control points and the segment a-b. The maximum of these two
     * distances will be our approximation error. */

    bdx = _cairo_fixed_to_double (knots->b.x - knots->a.x);
    bdy = _cairo_fixed_to_double (knots->b.y - knots->a.y);

    cdx = _cairo_fixed_to_double (knots->c.x - knots->a.x);
    cdy = _cairo_fixed_to_double (knots->c.y - knots->a.y);

    if (knots->a.x != knots->d.x || knots->a.y != knots->d.y) {
	/* Intersection point (px):
	 *     px = p1 + u(p2 - p1)
	 *     (p - px) ∙ (p2 - p1) = 0
	 * Thus:
	 *     u = ((p - p1) ∙ (p2 - p1)) / ∥p2 - p1∥²;
	 */

	double dx, dy, u, v;

	dx = _cairo_fixed_to_double (knots->d.x - knots->a.x);
	dy = _cairo_fixed_to_double (knots->d.y - knots->a.y);
	 v = dx * dx + dy * dy;

	u = bdx * dx + bdy * dy;
	if (u <= 0) {
	    /* bdx -= 0;
	     * bdy -= 0;
	     */
	} else if (u >= v) {
	    bdx -= dx;
	    bdy -= dy;
	} else {
	    bdx -= u/v * dx;
	    bdy -= u/v * dy;
	}

	u = cdx * dx + cdy * dy;
	if (u <= 0) {
	    /* cdx -= 0;
	     * cdy -= 0;
	     */
	} else if (u >= v) {
	    cdx -= dx;
	    cdy -= dy;
	} else {
	    cdx -= u/v * dx;
	    cdy -= u/v * dy;
	}
    }

    berr = bdx * bdx + bdy * bdy;
    cerr = cdx * cdx + cdy * cdy;
    if (berr > cerr)
	return berr;
    else
	return cerr;
}

static cairo_status_t
_cairo_spline_decompose_into (cairo_spline_knots_t *s1,
			      double tolerance_squared,
			      cairo_spline_t *result)
{
    cairo_spline_knots_t s2;
    cairo_status_t status;

    if (_cairo_spline_error_squared (s1) < tolerance_squared)
	return _cairo_spline_add_point (result, &s1->a, &s1->b);

    _de_casteljau (s1, &s2);

    status = _cairo_spline_decompose_into (s1, tolerance_squared, result);
    if (unlikely (status))
	return status;

    return _cairo_spline_decompose_into (&s2, tolerance_squared, result);
}

cairo_status_t
_cairo_spline_decompose (cairo_spline_t *spline, double tolerance)
{
    cairo_spline_knots_t s1;
    cairo_status_t status;

    s1 = spline->knots;
    spline->last_point = s1.a;
    status = _cairo_spline_decompose_into (&s1, tolerance * tolerance, spline);
    if (unlikely (status))
	return status;

    return spline->add_point_func (spline->closure,
				   &spline->knots.d, &spline->final_slope);
}

/* Note: this function is only good for computing bounds in device space. */
cairo_status_t
_cairo_spline_bound (cairo_spline_add_point_func_t add_point_func,
		     void *closure,
		     const cairo_point_t *p0, const cairo_point_t *p1,
		     const cairo_point_t *p2, const cairo_point_t *p3)
{
    double x0, x1, x2, x3;
    double y0, y1, y2, y3;
    double a, b, c;
    double t[4];
    int t_num = 0, i;
    cairo_status_t status;

    x0 = _cairo_fixed_to_double (p0->x);
    y0 = _cairo_fixed_to_double (p0->y);
    x1 = _cairo_fixed_to_double (p1->x);
    y1 = _cairo_fixed_to_double (p1->y);
    x2 = _cairo_fixed_to_double (p2->x);
    y2 = _cairo_fixed_to_double (p2->y);
    x3 = _cairo_fixed_to_double (p3->x);
    y3 = _cairo_fixed_to_double (p3->y);

    /* The spline can be written as a polynomial of the four points:
     *
     *   (1-t)³p0 + 3t(1-t)²p1 + 3t²(1-t)p2 + t³p3
     *
     * for 0≤t≤1.  Now, the X and Y components of the spline follow the
     * same polynomial but with x and y replaced for p.  To find the
     * bounds of the spline, we just need to find the X and Y bounds.
     * To find the bound, we take the derivative and equal it to zero,
     * and solve to find the t's that give the extreme points.
     *
     * Here is the derivative of the curve, sorted on t:
     *
     *   3t²(-p0+3p1-3p2+p3) + 2t(3p0-6p1+3p2) -3p0+3p1
     *
     * Let:
     *
     *   a = -p0+3p1-3p2+p3
     *   b =  p0-2p1+p2
     *   c = -p0+p1
     *
     * Gives:
     *
     *   a.t² + 2b.t + c = 0
     *
     * With:
     *
     *   delta = b*b - a*c
     *
     * the extreme points are at -c/2b if a is zero, at (-b±√delta)/a if
     * delta is positive, and at -b/a if delta is zero.
     */

#define ADD(t0) \
    { \
	double _t0 = (t0); \
	if (0 < _t0 && _t0 < 1) \
	    t[t_num++] = _t0; \
    }

#define FIND_EXTREMES(a,b,c) \
    { \
	if (a == 0) { \
	    if (b != 0) \
		ADD (-c / (2*b)); \
	} else { \
	    double b2 = b * b; \
	    double delta = b2 - a * c; \
	    if (delta > 0) { \
		cairo_bool_t feasible; \
		double _2ab = 2 * a * b; \
		/* We are only interested in solutions t that satisfy 0<t<1 \
		 * here.  We do some checks to avoid sqrt if the solutions \
		 * are not in that range.  The checks can be derived from: \
		 * \
		 *   0 < (-b±√delta)/a < 1 \
		 */ \
		if (_2ab >= 0) \
		    feasible = delta > b2 && delta < a*a + b2 + _2ab; \
		else if (-b / a >= 1) \
		    feasible = delta < b2 && delta > a*a + b2 + _2ab; \
		else \
		    feasible = delta < b2 || delta < a*a + b2 + _2ab; \
	        \
		if (unlikely (feasible)) { \
		    double sqrt_delta = sqrt (delta); \
		    ADD ((-b - sqrt_delta) / a); \
		    ADD ((-b + sqrt_delta) / a); \
		} \
	    } else if (delta == 0) { \
		ADD (-b / a); \
	    } \
	} \
    }

    /* Find X extremes */
    a = -x0 + 3*x1 - 3*x2 + x3;
    b =  x0 - 2*x1 + x2;
    c = -x0 + x1;
    FIND_EXTREMES (a, b, c);

    /* Find Y extremes */
    a = -y0 + 3*y1 - 3*y2 + y3;
    b =  y0 - 2*y1 + y2;
    c = -y0 + y1;
    FIND_EXTREMES (a, b, c);

    status = add_point_func (closure, p0, NULL);
    if (unlikely (status))
	return status;

    for (i = 0; i < t_num; i++) {
	cairo_point_t p;
	double x, y;
        double t_1_0, t_0_1;
        double t_2_0, t_0_2;
        double t_3_0, t_2_1_3, t_1_2_3, t_0_3;

        t_1_0 = t[i];          /*      t  */
        t_0_1 = 1 - t_1_0;     /* (1 - t) */

        t_2_0 = t_1_0 * t_1_0; /*      t  *      t  */
        t_0_2 = t_0_1 * t_0_1; /* (1 - t) * (1 - t) */

        t_3_0   = t_2_0 * t_1_0;     /*      t  *      t  *      t      */
        t_2_1_3 = t_2_0 * t_0_1 * 3; /*      t  *      t  * (1 - t) * 3 */
        t_1_2_3 = t_1_0 * t_0_2 * 3; /*      t  * (1 - t) * (1 - t) * 3 */
        t_0_3   = t_0_1 * t_0_2;     /* (1 - t) * (1 - t) * (1 - t)     */

        /* Bezier polynomial */
        x = x0 * t_0_3
          + x1 * t_1_2_3
          + x2 * t_2_1_3
          + x3 * t_3_0;
        y = y0 * t_0_3
          + y1 * t_1_2_3
          + y2 * t_2_1_3
          + y3 * t_3_0;

	p.x = _cairo_fixed_from_double (x);
	p.y = _cairo_fixed_from_double (y);
	status = add_point_func (closure, &p, NULL);
	if (unlikely (status))
	    return status;
    }

    return add_point_func (closure, p3, NULL);
}
