/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/*
 * Copyright © 2004 Carl Worth
 * Copyright © 2006 Red Hat, Inc.
 * Copyright © 2008 Chris Wilson
 * Copyright © 2014 Intel Corporation
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
 *	Carl D. Worth <cworth@cworth.org>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 *
 */

#include "cairoint.h"

#include "cairo-line-inline.h"
#include "cairo-slope-private.h"

static int
line_compare_for_y_against_x (const cairo_line_t *a,
			      int32_t y,
			      int32_t x)
{
    int32_t adx, ady;
    int32_t dx, dy;
    cairo_int64_t L, R;

    if (x < a->p1.x && x < a->p2.x)
	return 1;
    if (x > a->p1.x && x > a->p2.x)
	return -1;

    adx = a->p2.x - a->p1.x;
    dx = x - a->p1.x;

    if (adx == 0)
	return -dx;
    if (dx == 0 || (adx ^ dx) < 0)
	return adx;

    dy = y - a->p1.y;
    ady = a->p2.y - a->p1.y;

    L = _cairo_int32x32_64_mul (dy, adx);
    R = _cairo_int32x32_64_mul (dx, ady);

    return _cairo_int64_cmp (L, R);
}

/*
 * We need to compare the x-coordinates of a pair of lines for a particular y,
 * without loss of precision.
 *
 * The x-coordinate along an edge for a given y is:
 *   X = A_x + (Y - A_y) * A_dx / A_dy
 *
 * So the inequality we wish to test is:
 *   A_x + (Y - A_y) * A_dx / A_dy ∘ B_x + (Y - B_y) * B_dx / B_dy,
 * where ∘ is our inequality operator.
 *
 * By construction, we know that A_dy and B_dy (and (Y - A_y), (Y - B_y)) are
 * all positive, so we can rearrange it thus without causing a sign change:
 *   A_dy * B_dy * (A_x - B_x) ∘ (Y - B_y) * B_dx * A_dy
 *                                 - (Y - A_y) * A_dx * B_dy
 *
 * Given the assumption that all the deltas fit within 32 bits, we can compute
 * this comparison directly using 128 bit arithmetic. For certain, but common,
 * input we can reduce this down to a single 32 bit compare by inspecting the
 * deltas.
 *
 * (And put the burden of the work on developing fast 128 bit ops, which are
 * required throughout the tessellator.)
 *
 * See the similar discussion for _slope_compare().
 */
static int
lines_compare_x_for_y_general (const cairo_line_t *a,
			       const cairo_line_t *b,
			       int32_t y)
{
    /* XXX: We're assuming here that dx and dy will still fit in 32
     * bits. That's not true in general as there could be overflow. We
     * should prevent that before the tessellation algorithm
     * begins.
     */
    int32_t dx = 0;
    int32_t adx = 0, ady = 0;
    int32_t bdx = 0, bdy = 0;
    enum {
       HAVE_NONE    = 0x0,
       HAVE_DX      = 0x1,
       HAVE_ADX     = 0x2,
       HAVE_DX_ADX  = HAVE_DX | HAVE_ADX,
       HAVE_BDX     = 0x4,
       HAVE_DX_BDX  = HAVE_DX | HAVE_BDX,
       HAVE_ADX_BDX = HAVE_ADX | HAVE_BDX,
       HAVE_ALL     = HAVE_DX | HAVE_ADX | HAVE_BDX
    } have_dx_adx_bdx = HAVE_ALL;

    ady = a->p2.y - a->p1.y;
    adx = a->p2.x - a->p1.x;
    if (adx == 0)
	have_dx_adx_bdx &= ~HAVE_ADX;

    bdy = b->p2.y - b->p1.y;
    bdx = b->p2.x - b->p1.x;
    if (bdx == 0)
	have_dx_adx_bdx &= ~HAVE_BDX;

    dx = a->p1.x - b->p1.x;
    if (dx == 0)
	have_dx_adx_bdx &= ~HAVE_DX;

#define L _cairo_int64x32_128_mul (_cairo_int32x32_64_mul (ady, bdy), dx)
#define A _cairo_int64x32_128_mul (_cairo_int32x32_64_mul (adx, bdy), y - a->p1.y)
#define B _cairo_int64x32_128_mul (_cairo_int32x32_64_mul (bdx, ady), y - b->p1.y)
    switch (have_dx_adx_bdx) {
    default:
    case HAVE_NONE:
	return 0;
    case HAVE_DX:
	/* A_dy * B_dy * (A_x - B_x) ∘ 0 */
	return dx; /* ady * bdy is positive definite */
    case HAVE_ADX:
	/* 0 ∘  - (Y - A_y) * A_dx * B_dy */
	return adx; /* bdy * (y - a->top.y) is positive definite */
    case HAVE_BDX:
	/* 0 ∘ (Y - B_y) * B_dx * A_dy */
	return -bdx; /* ady * (y - b->top.y) is positive definite */
    case HAVE_ADX_BDX:
	/*  0 ∘ (Y - B_y) * B_dx * A_dy - (Y - A_y) * A_dx * B_dy */
	if ((adx ^ bdx) < 0) {
	    return adx;
	} else if (a->p1.y == b->p1.y) { /* common origin */
	    cairo_int64_t adx_bdy, bdx_ady;

	    /* ∴ A_dx * B_dy ∘ B_dx * A_dy */

	    adx_bdy = _cairo_int32x32_64_mul (adx, bdy);
	    bdx_ady = _cairo_int32x32_64_mul (bdx, ady);

	    return _cairo_int64_cmp (adx_bdy, bdx_ady);
	} else
	    return _cairo_int128_cmp (A, B);
    case HAVE_DX_ADX:
	/* A_dy * (A_x - B_x) ∘ - (Y - A_y) * A_dx */
	if ((-adx ^ dx) < 0) {
	    return dx;
	} else {
	    cairo_int64_t ady_dx, dy_adx;

	    ady_dx = _cairo_int32x32_64_mul (ady, dx);
	    dy_adx = _cairo_int32x32_64_mul (a->p1.y - y, adx);

	    return _cairo_int64_cmp (ady_dx, dy_adx);
	}
    case HAVE_DX_BDX:
	/* B_dy * (A_x - B_x) ∘ (Y - B_y) * B_dx */
	if ((bdx ^ dx) < 0) {
	    return dx;
	} else {
	    cairo_int64_t bdy_dx, dy_bdx;

	    bdy_dx = _cairo_int32x32_64_mul (bdy, dx);
	    dy_bdx = _cairo_int32x32_64_mul (y - b->p1.y, bdx);

	    return _cairo_int64_cmp (bdy_dx, dy_bdx);
	}
    case HAVE_ALL:
	/* XXX try comparing (a->p2.x - b->p2.x) et al */
	return _cairo_int128_cmp (L, _cairo_int128_sub (B, A));
    }
#undef B
#undef A
#undef L
}

static int
lines_compare_x_for_y (const cairo_line_t *a,
		       const cairo_line_t *b,
		       int32_t y)
{
    /* If the sweep-line is currently on an end-point of a line,
     * then we know its precise x value (and considering that we often need to
     * compare events at end-points, this happens frequently enough to warrant
     * special casing).
     */
    enum {
       HAVE_NEITHER = 0x0,
       HAVE_AX      = 0x1,
       HAVE_BX      = 0x2,
       HAVE_BOTH    = HAVE_AX | HAVE_BX
    } have_ax_bx = HAVE_BOTH;
    int32_t ax = 0, bx = 0;

    if (y == a->p1.y)
	ax = a->p1.x;
    else if (y == a->p2.y)
	ax = a->p2.x;
    else
	have_ax_bx &= ~HAVE_AX;

    if (y == b->p1.y)
	bx = b->p1.x;
    else if (y == b->p2.y)
	bx = b->p2.x;
    else
	have_ax_bx &= ~HAVE_BX;

    switch (have_ax_bx) {
    default:
    case HAVE_NEITHER:
	return lines_compare_x_for_y_general (a, b, y);
    case HAVE_AX:
	return -line_compare_for_y_against_x (b, y, ax);
    case HAVE_BX:
	return line_compare_for_y_against_x (a, y, bx);
    case HAVE_BOTH:
	return ax - bx;
    }
}

static int bbox_compare (const cairo_line_t *a,
			 const cairo_line_t *b)
{
    int32_t amin, amax;
    int32_t bmin, bmax;

    if (a->p1.x < a->p2.x) {
	amin = a->p1.x;
	amax = a->p2.x;
    } else {
	amin = a->p2.x;
	amax = a->p1.x;
    }

    if (b->p1.x < b->p2.x) {
	bmin = b->p1.x;
	bmax = b->p2.x;
    } else {
	bmin = b->p2.x;
	bmax = b->p1.x;
    }

    if (amax < bmin)
	return -1;

    if (amin > bmax)
	return +1;

    return 0;
}

int
_cairo_lines_compare_at_y (const cairo_line_t *a,
			      const cairo_line_t *b,
			      int y)
{
    cairo_slope_t sa, sb;
    int ret;

    if (cairo_lines_equal (a, b))
	return 0;

    /* Don't bother solving for abscissa if the edges' bounding boxes
     * can be used to order them.
     */
    ret = bbox_compare (a, b);
    if (ret)
	return ret;

    ret = lines_compare_x_for_y (a, b, y);
    if (ret)
	return ret;

    _cairo_slope_init (&sa, &a->p1, &a->p2);
    _cairo_slope_init (&sb, &b->p1, &b->p2);

    return _cairo_slope_compare (&sb, &sa);
}
