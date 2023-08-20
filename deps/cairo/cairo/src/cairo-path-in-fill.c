/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2008 Chris Wilson
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
 * The Initial Developer of the Original Code is Chris Wilson.
 *
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"
#include "cairo-path-fixed-private.h"

typedef struct cairo_in_fill {
    double tolerance;
    cairo_bool_t on_edge;
    int winding;

    cairo_fixed_t x, y;

    cairo_bool_t has_current_point;
    cairo_point_t current_point;
    cairo_point_t first_point;
} cairo_in_fill_t;

static void
_cairo_in_fill_init (cairo_in_fill_t	*in_fill,
		     double		 tolerance,
		     double		 x,
		     double		 y)
{
    in_fill->on_edge = FALSE;
    in_fill->winding = 0;
    in_fill->tolerance = tolerance;

    in_fill->x = _cairo_fixed_from_double (x);
    in_fill->y = _cairo_fixed_from_double (y);

    in_fill->has_current_point = FALSE;
    in_fill->current_point.x = 0;
    in_fill->current_point.y = 0;
}

static void
_cairo_in_fill_fini (cairo_in_fill_t *in_fill)
{
}

static int
edge_compare_for_y_against_x (const cairo_point_t *p1,
			      const cairo_point_t *p2,
			      cairo_fixed_t y,
			      cairo_fixed_t x)
{
    cairo_fixed_t adx, ady;
    cairo_fixed_t dx, dy;
    cairo_int64_t L, R;

    adx = p2->x - p1->x;
    dx = x - p1->x;

    if (adx == 0)
	return -dx;
    if ((adx ^ dx) < 0)
	return adx;

    dy = y - p1->y;
    ady = p2->y - p1->y;

    L = _cairo_int32x32_64_mul (dy, adx);
    R = _cairo_int32x32_64_mul (dx, ady);

    return _cairo_int64_cmp (L, R);
}

static void
_cairo_in_fill_add_edge (cairo_in_fill_t *in_fill,
			 const cairo_point_t *p1,
			 const cairo_point_t *p2)
{
    int dir;

    if (in_fill->on_edge)
	return;

    /* count the number of edge crossing to -∞ */

    dir = 1;
    if (p2->y < p1->y) {
	const cairo_point_t *tmp;

	tmp = p1;
	p1 = p2;
	p2 = tmp;

	dir = -1;
    }

    /* First check whether the query is on an edge */
    if ((p1->x == in_fill->x && p1->y == in_fill->y) ||
	(p2->x == in_fill->x && p2->y == in_fill->y) ||
	(! (p2->y < in_fill->y || p1->y > in_fill->y ||
	   (p1->x > in_fill->x && p2->x > in_fill->x) ||
	   (p1->x < in_fill->x && p2->x < in_fill->x)) &&
	 edge_compare_for_y_against_x (p1, p2, in_fill->y, in_fill->x) == 0))
    {
	in_fill->on_edge = TRUE;
	return;
    }

    /* edge is entirely above or below, note the shortening rule */
    if (p2->y <= in_fill->y || p1->y > in_fill->y)
	return;

    /* edge lies wholly to the right */
    if (p1->x >= in_fill->x && p2->x >= in_fill->x)
	return;

    if ((p1->x <= in_fill->x && p2->x <= in_fill->x) ||
	edge_compare_for_y_against_x (p1, p2, in_fill->y, in_fill->x) < 0)
    {
	in_fill->winding += dir;
    }
}

static cairo_status_t
_cairo_in_fill_move_to (void *closure,
			const cairo_point_t *point)
{
    cairo_in_fill_t *in_fill = closure;

    /* implicit close path */
    if (in_fill->has_current_point) {
	_cairo_in_fill_add_edge (in_fill,
				 &in_fill->current_point,
				 &in_fill->first_point);
    }

    in_fill->first_point = *point;
    in_fill->current_point = *point;
    in_fill->has_current_point = TRUE;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_in_fill_line_to (void *closure,
			const cairo_point_t *point)
{
    cairo_in_fill_t *in_fill = closure;

    if (in_fill->has_current_point)
	_cairo_in_fill_add_edge (in_fill, &in_fill->current_point, point);

    in_fill->current_point = *point;
    in_fill->has_current_point = TRUE;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_in_fill_add_point (void *closure,
                          const cairo_point_t *point,
                          const cairo_slope_t *tangent)
{
    return _cairo_in_fill_line_to (closure, point);
};

static cairo_status_t
_cairo_in_fill_curve_to (void *closure,
			 const cairo_point_t *b,
			 const cairo_point_t *c,
			 const cairo_point_t *d)
{
    cairo_in_fill_t *in_fill = closure;
    cairo_spline_t spline;
    cairo_fixed_t top, bot, left;

    /* first reject based on bbox */
    bot = top = in_fill->current_point.y;
    if (b->y < top) top = b->y;
    if (b->y > bot) bot = b->y;
    if (c->y < top) top = c->y;
    if (c->y > bot) bot = c->y;
    if (d->y < top) top = d->y;
    if (d->y > bot) bot = d->y;
    if (bot < in_fill->y || top > in_fill->y) {
	in_fill->current_point = *d;
	return CAIRO_STATUS_SUCCESS;
    }

    left = in_fill->current_point.x;
    if (b->x < left) left = b->x;
    if (c->x < left) left = c->x;
    if (d->x < left) left = d->x;
    if (left > in_fill->x) {
	in_fill->current_point = *d;
	return CAIRO_STATUS_SUCCESS;
    }

    /* XXX Investigate direct inspection of the inflections? */
    if (! _cairo_spline_init (&spline,
			      _cairo_in_fill_add_point,
			      in_fill,
			      &in_fill->current_point, b, c, d))
    {
	return CAIRO_STATUS_SUCCESS;
    }

    return _cairo_spline_decompose (&spline, in_fill->tolerance);
}

static cairo_status_t
_cairo_in_fill_close_path (void *closure)
{
    cairo_in_fill_t *in_fill = closure;

    if (in_fill->has_current_point) {
	_cairo_in_fill_add_edge (in_fill,
				 &in_fill->current_point,
				 &in_fill->first_point);

	in_fill->has_current_point = FALSE;
    }

    return CAIRO_STATUS_SUCCESS;
}

cairo_bool_t
_cairo_path_fixed_in_fill (const cairo_path_fixed_t	*path,
			   cairo_fill_rule_t	 fill_rule,
			   double		 tolerance,
			   double		 x,
			   double		 y)
{
    cairo_in_fill_t in_fill;
    cairo_status_t status;
    cairo_bool_t is_inside;

    if (_cairo_path_fixed_fill_is_empty (path))
	return FALSE;

    _cairo_in_fill_init (&in_fill, tolerance, x, y);

    status = _cairo_path_fixed_interpret (path,
					  _cairo_in_fill_move_to,
					  _cairo_in_fill_line_to,
					  _cairo_in_fill_curve_to,
					  _cairo_in_fill_close_path,
					  &in_fill);
    assert (status == CAIRO_STATUS_SUCCESS);

    _cairo_in_fill_close_path (&in_fill);

    if (in_fill.on_edge) {
	is_inside = TRUE;
    } else switch (fill_rule) {
    case CAIRO_FILL_RULE_EVEN_ODD:
	is_inside = in_fill.winding & 1;
	break;
    case CAIRO_FILL_RULE_WINDING:
	is_inside = in_fill.winding != 0;
	break;
    default:
	ASSERT_NOT_REACHED;
	is_inside = FALSE;
	break;
    }

    _cairo_in_fill_fini (&in_fill);

    return is_inside;
}
