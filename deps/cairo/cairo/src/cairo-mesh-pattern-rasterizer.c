/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright 2009 Andrea Canciani
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
 * The Initial Developer of the Original Code is Andrea Canciani.
 *
 * Contributor(s):
 *	Andrea Canciani <ranma42@gmail.com>
 */

#include "cairoint.h"

#include "cairo-array-private.h"
#include "cairo-pattern-private.h"

/*
 * Rasterizer for mesh patterns.
 *
 * This implementation is based on techniques derived from several
 * papers (available from ACM):
 *
 * - Lien, Shantz and Pratt "Adaptive Forward Differencing for
 *   Rendering Curves and Surfaces" (discussion of the AFD technique,
 *   bound of 1/sqrt(2) on step length without proof)
 *
 * - Popescu and Rosen, "Forward rasterization" (description of
 *   forward rasterization, proof of the previous bound)
 *
 * - Klassen, "Integer Forward Differencing of Cubic Polynomials:
 *   Analysis and Algorithms"
 *
 * - Klassen, "Exact Integer Hybrid Subdivision and Forward
 *   Differencing of Cubics" (improving the bound on the minimum
 *   number of steps)
 *
 * - Chang, Shantz and Rocchetti, "Rendering Cubic Curves and Surfaces
 *   with Integer Adaptive Forward Differencing" (analysis of forward
 *   differencing applied to Bezier patches)
 *
 * Notes:
 * - Poor performance expected in degenerate cases
 *
 * - Patches mostly outside the drawing area are drawn completely (and
 *   clipped), wasting time
 *
 * - Both previous problems are greatly reduced by splitting until a
 *   reasonably small size and clipping the new tiles: execution time
 *   is quadratic in the convex-hull diameter instead than linear to
 *   the painted area. Splitting the tiles doesn't change the painted
 *   area but (usually) reduces the bounding box area (bbox area can
 *   remain the same after splitting, but cannot grow)
 *
 * - The initial implementation used adaptive forward differencing,
 *   but simple forward differencing scored better in benchmarks
 *
 * Idea:
 *
 * We do a sampling over the cubic patch with step du and dv (in the
 * two parameters) that guarantees that any point of our sampling will
 * be at most at 1/sqrt(2) from its adjacent points. In formulae
 * (assuming B is the patch):
 *
 *   |B(u,v) - B(u+du,v)| < 1/sqrt(2)
 *   |B(u,v) - B(u,v+dv)| < 1/sqrt(2)
 *
 * This means that every pixel covered by the patch will contain at
 * least one of the samples, thus forward rasterization can be
 * performed. Sketch of proof (from Popescu and Rosen):
 *
 * Let's take the P pixel we're interested into. If we assume it to be
 * square, its boundaries define 9 regions on the plane:
 *
 * 1|2|3
 * -+-+-
 * 8|P|4
 * -+-+-
 * 7|6|5
 *
 * Let's check that the pixel P will contain at least one point
 * assuming that it is covered by the patch.
 *
 * Since the pixel is covered by the patch, its center will belong to
 * (at least) one of the quads:
 *
 *   {(B(u,v), B(u+du,v), B(u,v+dv), B(u+du,v+dv)) for u,v in [0,1]}
 *
 * If P doesn't contain any of the corners of the quad:
 *
 * - if one of the corners is in 1,3,5 or 7, other two of them have to
 *   be in 2,4,6 or 8, thus if the last corner is not in P, the length
 *   of one of the edges will be > 1/sqrt(2)
 *
 * - if none of the corners is in 1,3,5 or 7, all of them are in 2,4,6
 *   and/or 8. If they are all in different regions, they can't
 *   satisfy the distance constraint. If two of them are in the same
 *   region (let's say 2), no point is in 6 and again it is impossible
 *   to have the center of P in the quad respecting the distance
 *   constraint (both these assertions can be checked by continuity
 *   considering the length of the edges of a quad with the vertices
 *   on the edges of P)
 *
 * Each of the cases led to a contradiction, so P contains at least
 * one of the corners of the quad.
 */

/*
 * Make sure that errors are less than 1 in fixed point math if you
 * change these values.
 *
 * The error is amplified by about steps^3/4 times.
 * The rasterizer always uses a number of steps that is a power of 2.
 *
 * 256 is the maximum allowed number of steps (to have error < 1)
 * using 8.24 for the differences.
 */
#define STEPS_MAX_V 256.0
#define STEPS_MAX_U 256.0

/*
 * If the patch/curve is only partially visible, split it to a finer
 * resolution to get higher chances to clip (part of) it.
 *
 * These values have not been computed, but simply obtained
 * empirically (by benchmarking some patches). They should never be
 * greater than STEPS_MAX_V (or STEPS_MAX_U), but they can be as small
 * as 1 (depending on how much you want to spend time in splitting the
 * patch/curve when trying to save some rasterization time).
 */
#define STEPS_CLIP_V 64.0
#define STEPS_CLIP_U 64.0


/* Utils */
static inline double
sqlen (cairo_point_double_t p0, cairo_point_double_t p1)
{
    cairo_point_double_t delta;

    delta.x = p0.x - p1.x;
    delta.y = p0.y - p1.y;

    return delta.x * delta.x + delta.y * delta.y;
}

static inline int16_t
_color_delta_to_shifted_short (int32_t from, int32_t to, int shift)
{
    int32_t delta = to - from;

    /* We need to round toward zero, because otherwise adding the
     * delta 2^shift times can overflow */
    if (delta >= 0)
	return delta >> shift;
    else
	return -((-delta) >> shift);
}

/*
 * Convert a number of steps to the equivalent shift.
 *
 * Input: the square of the minimum number of steps
 *
 * Output: the smallest integer x such that 2^x > steps
 */
static inline int
sqsteps2shift (double steps_sq)
{
    int r;
    frexp (MAX (1.0, steps_sq), &r);
    return (r + 1) >> 1;
}

/*
 * FD functions
 *
 * A Bezier curve is defined (with respect to a parameter t in
 * [0,1]) from its nodes (x,y,z,w) like this:
 *
 *   B(t) = x(1-t)^3 + 3yt(1-t)^2 + 3zt^2(1-t) + wt^3
 *
 * To efficiently evaluate a Bezier curve, the rasterizer uses forward
 * differences. Given x, y, z, w (the 4 nodes of the Bezier curve), it
 * is possible to convert them to forward differences form and walk
 * over the curve using fd_init (), fd_down () and fd_fwd ().
 *
 * f[0] is always the value of the Bezier curve for "current" t.
 */

/*
 * Initialize the coefficient for forward differences.
 *
 * Input: x,y,z,w are the 4 nodes of the Bezier curve
 *
 * Output: f[i] is the i-th difference of the curve
 *
 * f[0] is the value of the curve for t==0, i.e. f[0]==x.
 *
 * The initial step is 1; this means that each step increases t by 1
 * (so fd_init () immediately followed by fd_fwd (f) n times makes
 * f[0] be the value of the curve for t==n).
 */
static inline void
fd_init (double x, double y, double z, double w, double f[4])
{
    f[0] = x;
    f[1] = w - x;
    f[2] = 6. * (w - 2. * z + y);
    f[3] = 6. * (w - 3. * z + 3. * y - x);
}

/*
 * Halve the step of the coefficients for forward differences.
 *
 * Input: f[i] is the i-th difference of the curve
 *
 * Output: f[i] is the i-th difference of the curve with half the
 *         original step
 *
 * f[0] is not affected, so the current t is not changed.
 *
 * The other coefficients are changed so that the step is half the
 * original step. This means that doing fd_fwd (f) n times with the
 * input f results in the same f[0] as doing fd_fwd (f) 2n times with
 * the output f.
 */
static inline void
fd_down (double f[4])
{
    f[3] *= 0.125;
    f[2] = f[2] * 0.25 - f[3];
    f[1] = (f[1] - f[2]) * 0.5;
}

/*
 * Perform one step of forward differences along the curve.
 *
 * Input: f[i] is the i-th difference of the curve
 *
 * Output: f[i] is the i-th difference of the curve after one step
 */
static inline void
fd_fwd (double f[4])
{
    f[0] += f[1];
    f[1] += f[2];
    f[2] += f[3];
}

/*
 * Transform to integer forward differences.
 *
 * Input: d[n] is the n-th difference (in double precision)
 *
 * Output: i[n] is the n-th difference (in fixed point precision)
 *
 * i[0] is 9.23 fixed point, other differences are 4.28 fixed point.
 */
static inline void
fd_fixed (double d[4], int32_t i[4])
{
    i[0] = _cairo_fixed_16_16_from_double (256 *  2 * d[0]);
    i[1] = _cairo_fixed_16_16_from_double (256 * 16 * d[1]);
    i[2] = _cairo_fixed_16_16_from_double (256 * 16 * d[2]);
    i[3] = _cairo_fixed_16_16_from_double (256 * 16 * d[3]);
}

/*
 * Perform one step of integer forward differences along the curve.
 *
 * Input: f[n] is the n-th difference
 *
 * Output: f[n] is the n-th difference
 *
 * f[0] is 9.23 fixed point, other differences are 4.28 fixed point.
 */
static inline void
fd_fixed_fwd (int32_t f[4])
{
    f[0] += (f[1] >> 5) + ((f[1] >> 4) & 1);
    f[1] += f[2];
    f[2] += f[3];
}

/*
 * Compute the minimum number of steps that guarantee that walking
 * over a curve will leave no holes.
 *
 * Input: p[0..3] the nodes of the Bezier curve
 *
 * Returns: the square of the number of steps
 *
 * Idea:
 *
 * We want to make sure that at every step we move by less than
 * 1/sqrt(2).
 *
 * The derivative of the cubic Bezier with nodes (p0, p1, p2, p3) is
 * the quadratic Bezier with nodes (p1-p0, p2-p1, p3-p2) scaled by 3,
 * so (since a Bezier curve is always bounded by its convex hull), we
 * can say that:
 *
 *  max(|B'(t)|) <= 3 max (|p1-p0|, |p2-p1|, |p3-p2|)
 *
 * We can improve this by noticing that a quadratic Bezier (a,b,c) is
 * bounded by the quad (a,lerp(a,b,t),lerp(b,c,t),c) for any t, so
 * (substituting the previous values, using t=0.5 and simplifying):
 *
 *  max(|B'(t)|) <= 3 max (|p1-p0|, |p2-p0|/2, |p3-p1|/2, |p3-p2|)
 *
 * So, to guarantee a maximum step length of 1/sqrt(2) we must do:
 *
 *   3 max (|p1-p0|, |p2-p0|/2, |p3-p1|/2, |p3-p2|) sqrt(2) steps
 */
static inline double
bezier_steps_sq (cairo_point_double_t p[4])
{
    double tmp = sqlen (p[0], p[1]);
    tmp = MAX (tmp, sqlen (p[2], p[3]));
    tmp = MAX (tmp, sqlen (p[0], p[2]) * .25);
    tmp = MAX (tmp, sqlen (p[1], p[3]) * .25);
    return 18.0 * tmp;
}

/*
 * Split a 1D Bezier cubic using de Casteljau's algorithm.
 *
 * Input: x,y,z,w the nodes of the Bezier curve
 *
 * Output: x0,y0,z0,w0 and x1,y1,z1,w1 are respectively the nodes of
 *         the first half and of the second half of the curve
 *
 * The output control nodes have to be distinct.
 */
static inline void
split_bezier_1D (double  x,  double  y,  double  z,  double  w,
		 double *x0, double *y0, double *z0, double *w0,
		 double *x1, double *y1, double *z1, double *w1)
{
    double tmp;

    *x0 = x;
    *w1 = w;

    tmp = 0.5 * (y + z);
    *y0 = 0.5 * (x + y);
    *z1 = 0.5 * (z + w);

    *z0 = 0.5 * (*y0 + tmp);
    *y1 = 0.5 * (tmp + *z1);

    *w0 = *x1 = 0.5 * (*z0 + *y1);
}

/*
 * Split a Bezier curve using de Casteljau's algorithm.
 *
 * Input: p[0..3] the nodes of the Bezier curve
 *
 * Output: fst_half[0..3] and snd_half[0..3] are respectively the
 *         nodes of the first and of the second half of the curve
 *
 * fst_half and snd_half must be different, but they can be the same as
 * nodes.
 */
static void
split_bezier (cairo_point_double_t p[4],
	      cairo_point_double_t fst_half[4],
	      cairo_point_double_t snd_half[4])
{
    split_bezier_1D (p[0].x, p[1].x, p[2].x, p[3].x,
		     &fst_half[0].x, &fst_half[1].x, &fst_half[2].x, &fst_half[3].x,
		     &snd_half[0].x, &snd_half[1].x, &snd_half[2].x, &snd_half[3].x);

    split_bezier_1D (p[0].y, p[1].y, p[2].y, p[3].y,
		     &fst_half[0].y, &fst_half[1].y, &fst_half[2].y, &fst_half[3].y,
		     &snd_half[0].y, &snd_half[1].y, &snd_half[2].y, &snd_half[3].y);
}


typedef enum _intersection {
    INSIDE = -1, /* the interval is entirely contained in the reference interval */
    OUTSIDE = 0, /* the interval has no intersection with the reference interval */
    PARTIAL = 1  /* the interval intersects the reference interval (but is not fully inside it) */
} intersection_t;

/*
 * Check if an interval if inside another.
 *
 * Input: a,b are the extrema of the first interval
 *        c,d are the extrema of the second interval
 *
 * Returns: INSIDE  iff [a,b) intersection [c,d) = [a,b)
 *          OUTSIDE iff [a,b) intersection [c,d) = {}
 *          PARTIAL otherwise
 *
 * The function assumes a < b and c < d
 *
 * Note: Bitwise-anding the results along each component gives the
 *       expected result for [a,b) x [A,B) intersection [c,d) x [C,D).
 */
static inline int
intersect_interval (double a, double b, double c, double d)
{
    if (c <= a && b <= d)
	return INSIDE;
    else if (a >= d || b <= c)
	return OUTSIDE;
    else
	return PARTIAL;
}

/*
 * Set the color of a pixel.
 *
 * Input: data is the base pointer of the image
 *        width, height are the dimensions of the image
 *        stride is the stride in bytes between adjacent rows
 *        x, y are the coordinates of the pixel to be colored
 *        r,g,b,a are the color components of the color to be set
 *
 * Output: the (x,y) pixel in data has the (r,g,b,a) color
 *
 * The input color components are not premultiplied, but the data
 * stored in the image is assumed to be in CAIRO_FORMAT_ARGB32 (8 bpc,
 * premultiplied).
 *
 * If the pixel to be set is outside the image, this function does
 * nothing.
 */
static inline void
draw_pixel (unsigned char *data, int width, int height, int stride,
	    int x, int y, uint16_t r, uint16_t g, uint16_t b, uint16_t a)
{
    if (likely (0 <= x && 0 <= y && x < width && y < height)) {
	uint32_t tr, tg, tb, ta;

	/* Premultiply and round */
	ta = a;
	tr = r * ta + 0x8000;
	tg = g * ta + 0x8000;
	tb = b * ta + 0x8000;

	tr += tr >> 16;
	tg += tg >> 16;
	tb += tb >> 16;

	*((uint32_t*) (data + y*(ptrdiff_t)stride + 4*x)) = ((ta << 16) & 0xff000000) |
	    ((tr >> 8) & 0xff0000) | ((tg >> 16) & 0xff00) | (tb >> 24);
    }
}

/*
 * Forward-rasterize a cubic curve using forward differences.
 *
 * Input: data is the base pointer of the image
 *        width, height are the dimensions of the image
 *        stride is the stride in bytes between adjacent rows
 *        ushift is log2(n) if n is the number of desired steps
 *        dxu[i], dyu[i] are the x,y forward differences of the curve
 *        r0,g0,b0,a0 are the color components of the start point
 *        r3,g3,b3,a3 are the color components of the end point
 *
 * Output: data will be changed to have the requested curve drawn in
 *         the specified colors
 *
 * The input color components are not premultiplied, but the data
 * stored in the image is assumed to be in CAIRO_FORMAT_ARGB32 (8 bpc,
 * premultiplied).
 *
 * The function draws n+1 pixels, that is from the point at step 0 to
 * the point at step n, both included. This is the discrete equivalent
 * to drawing the curve for values of the interpolation parameter in
 * [0,1] (including both extremes).
 */
static inline void
rasterize_bezier_curve (unsigned char *data, int width, int height, int stride,
			int ushift, double dxu[4], double dyu[4],
			uint16_t r0, uint16_t g0, uint16_t b0, uint16_t a0,
			uint16_t r3, uint16_t g3, uint16_t b3, uint16_t a3)
{
    int32_t xu[4], yu[4];
    int x0, y0, u, usteps = 1 << ushift;

    uint16_t r = r0, g = g0, b = b0, a = a0;
    int16_t dr = _color_delta_to_shifted_short (r0, r3, ushift);
    int16_t dg = _color_delta_to_shifted_short (g0, g3, ushift);
    int16_t db = _color_delta_to_shifted_short (b0, b3, ushift);
    int16_t da = _color_delta_to_shifted_short (a0, a3, ushift);

    fd_fixed (dxu, xu);
    fd_fixed (dyu, yu);

    /*
     * Use (dxu[0],dyu[0]) as origin for the forward differences.
     *
     * This makes it possible to handle much larger coordinates (the
     * ones that can be represented as cairo_fixed_t)
     */
    x0 = _cairo_fixed_from_double (dxu[0]);
    y0 = _cairo_fixed_from_double (dyu[0]);
    xu[0] = 0;
    yu[0] = 0;

    for (u = 0; u <= usteps; ++u) {
	/*
	 * This rasterizer assumes that pixels are integer aligned
	 * squares, so a generic (x,y) point belongs to the pixel with
	 * top-left coordinates (floor(x), floor(y))
	 */

	int x = _cairo_fixed_integer_floor (x0 + (xu[0] >> 15) + ((xu[0] >> 14) & 1));
	int y = _cairo_fixed_integer_floor (y0 + (yu[0] >> 15) + ((yu[0] >> 14) & 1));

	draw_pixel (data, width, height, stride, x, y, r, g, b, a);

	fd_fixed_fwd (xu);
	fd_fixed_fwd (yu);
	r += dr;
	g += dg;
	b += db;
	a += da;
    }
}

/*
 * Clip, split and rasterize a Bezier curve.
 *
 * Input: data is the base pointer of the image
 *        width, height are the dimensions of the image
 *        stride is the stride in bytes between adjacent rows
 *        p[i] is the i-th node of the Bezier curve
 *        c0[i] is the i-th color component at the start point
 *        c3[i] is the i-th color component at the end point
 *
 * Output: data will be changed to have the requested curve drawn in
 *         the specified colors
 *
 * The input color components are not premultiplied, but the data
 * stored in the image is assumed to be in CAIRO_FORMAT_ARGB32 (8 bpc,
 * premultiplied).
 *
 * The color components are red, green, blue and alpha, in this order.
 *
 * The function guarantees that it will draw the curve with a step
 * small enough to never have a distance above 1/sqrt(2) between two
 * consecutive points (which is needed to ensure that no hole can
 * appear when using this function to rasterize a patch).
 */
static void
draw_bezier_curve (unsigned char *data, int width, int height, int stride,
		   cairo_point_double_t p[4], double c0[4], double c3[4])
{
    double top, bottom, left, right, steps_sq;
    int i, v;

    top = bottom = p[0].y;
    for (i = 1; i < 4; ++i) {
	top    = MIN (top,    p[i].y);
	bottom = MAX (bottom, p[i].y);
    }

    /* Check visibility */
    v = intersect_interval (top, bottom, 0, height);
    if (v == OUTSIDE)
	return;

    left = right = p[0].x;
    for (i = 1; i < 4; ++i) {
	left  = MIN (left,  p[i].x);
	right = MAX (right, p[i].x);
    }

    v &= intersect_interval (left, right, 0, width);
    if (v == OUTSIDE)
	return;

    steps_sq = bezier_steps_sq (p);
    if (steps_sq >= (v == INSIDE ? STEPS_MAX_U * STEPS_MAX_U : STEPS_CLIP_U * STEPS_CLIP_U)) {
	/*
	 * The number of steps is greater than the threshold. This
	 * means that either the error would become too big if we
	 * directly rasterized it or that we can probably save some
	 * time by splitting the curve and clipping part of it
	 */
	cairo_point_double_t first[4], second[4];
	double midc[4];
	split_bezier (p, first, second);
	midc[0] = (c0[0] + c3[0]) * 0.5;
	midc[1] = (c0[1] + c3[1]) * 0.5;
	midc[2] = (c0[2] + c3[2]) * 0.5;
	midc[3] = (c0[3] + c3[3]) * 0.5;
	draw_bezier_curve (data, width, height, stride, first, c0, midc);
	draw_bezier_curve (data, width, height, stride, second, midc, c3);
    } else {
	double xu[4], yu[4];
	int ushift = sqsteps2shift (steps_sq), k;

	fd_init (p[0].x, p[1].x, p[2].x, p[3].x, xu);
	fd_init (p[0].y, p[1].y, p[2].y, p[3].y, yu);

	for (k = 0; k < ushift; ++k) {
	    fd_down (xu);
	    fd_down (yu);
	}

	rasterize_bezier_curve (data, width, height, stride, ushift,
				xu, yu,
				_cairo_color_double_to_short (c0[0]),
				_cairo_color_double_to_short (c0[1]),
				_cairo_color_double_to_short (c0[2]),
				_cairo_color_double_to_short (c0[3]),
				_cairo_color_double_to_short (c3[0]),
				_cairo_color_double_to_short (c3[1]),
				_cairo_color_double_to_short (c3[2]),
				_cairo_color_double_to_short (c3[3]));

	/* Draw the end point, to make sure that we didn't leave it
	 * out because of rounding */
	draw_pixel (data, width, height, stride,
		    _cairo_fixed_integer_floor (_cairo_fixed_from_double (p[3].x)),
		    _cairo_fixed_integer_floor (_cairo_fixed_from_double (p[3].y)),
		    _cairo_color_double_to_short (c3[0]),
		    _cairo_color_double_to_short (c3[1]),
		    _cairo_color_double_to_short (c3[2]),
		    _cairo_color_double_to_short (c3[3]));
    }
}

/*
 * Forward-rasterize a cubic Bezier patch using forward differences.
 *
 * Input: data is the base pointer of the image
 *        width, height are the dimensions of the image
 *        stride is the stride in bytes between adjacent rows
 *        vshift is log2(n) if n is the number of desired steps
 *        p[i][j], p[i][j] are the nodes of the Bezier patch
 *        col[i][j] is the j-th color component of the i-th corner
 *
 * Output: data will be changed to have the requested patch drawn in
 *         the specified colors
 *
 * The nodes of the patch are as follows:
 *
 * u\v 0    - >    1
 * 0  p00 p01 p02 p03
 * |  p10 p11 p12 p13
 * v  p20 p21 p22 p23
 * 1  p30 p31 p32 p33
 *
 * i.e. u varies along the first component (rows), v varies along the
 * second one (columns).
 *
 * The color components are red, green, blue and alpha, in this order.
 * c[0..3] are the colors in p00, p30, p03, p33 respectively
 *
 * The input color components are not premultiplied, but the data
 * stored in the image is assumed to be in CAIRO_FORMAT_ARGB32 (8 bpc,
 * premultiplied).
 *
 * If the patch folds over itself, the part with the highest v
 * parameter is considered above. If both have the same v, the one
 * with the highest u parameter is above.
 *
 * The function draws n+1 curves, that is from the curve at step 0 to
 * the curve at step n, both included. This is the discrete equivalent
 * to drawing the patch for values of the interpolation parameter in
 * [0,1] (including both extremes).
 */
static inline void
rasterize_bezier_patch (unsigned char *data, int width, int height, int stride, int vshift,
			cairo_point_double_t p[4][4], double col[4][4])
{
    double pv[4][2][4], cstart[4], cend[4], dcstart[4], dcend[4];
    int v, i, k;

    v = 1 << vshift;

    /*
     * pv[i][0] is the function (represented using forward
     * differences) mapping v to the x coordinate of the i-th node of
     * the Bezier curve with parameter u.
     * (Likewise p[i][0] gives the y coordinate).
     *
     * This means that (pv[0][0][0],pv[0][1][0]),
     * (pv[1][0][0],pv[1][1][0]), (pv[2][0][0],pv[2][1][0]) and
     * (pv[3][0][0],pv[3][1][0]) are the nodes of the Bezier curve for
     * the "current" v value (see the FD comments for more details).
     */
    for (i = 0; i < 4; ++i) {
	fd_init (p[i][0].x, p[i][1].x, p[i][2].x, p[i][3].x, pv[i][0]);
	fd_init (p[i][0].y, p[i][1].y, p[i][2].y, p[i][3].y, pv[i][1]);
	for (k = 0; k < vshift; ++k) {
	    fd_down (pv[i][0]);
	    fd_down (pv[i][1]);
	}
    }

    for (i = 0; i < 4; ++i) {
	cstart[i]  = col[0][i];
	cend[i]    = col[1][i];
	dcstart[i] = (col[2][i] - col[0][i]) / v;
	dcend[i]   = (col[3][i] - col[1][i]) / v;
    }

    v++;
    while (v--) {
	cairo_point_double_t nodes[4];
	for (i = 0; i < 4; ++i) {
	    nodes[i].x = pv[i][0][0];
	    nodes[i].y = pv[i][1][0];
	}

	draw_bezier_curve (data, width, height, stride, nodes, cstart, cend);

	for (i = 0; i < 4; ++i) {
	    fd_fwd (pv[i][0]);
	    fd_fwd (pv[i][1]);
	    cstart[i] += dcstart[i];
	    cend[i] += dcend[i];
	}
    }
}

/*
 * Clip, split and rasterize a Bezier cubic patch.
 *
 * Input: data is the base pointer of the image
 *        width, height are the dimensions of the image
 *        stride is the stride in bytes between adjacent rows
 *        p[i][j], p[i][j] are the nodes of the patch
 *        col[i][j] is the j-th color component of the i-th corner
 *
 * Output: data will be changed to have the requested patch drawn in
 *         the specified colors
 *
 * The nodes of the patch are as follows:
 *
 * u\v 0    - >    1
 * 0  p00 p01 p02 p03
 * |  p10 p11 p12 p13
 * v  p20 p21 p22 p23
 * 1  p30 p31 p32 p33
 *
 * i.e. u varies along the first component (rows), v varies along the
 * second one (columns).
 *
 * The color components are red, green, blue and alpha, in this order.
 * c[0..3] are the colors in p00, p30, p03, p33 respectively
 *
 * The input color components are not premultiplied, but the data
 * stored in the image is assumed to be in CAIRO_FORMAT_ARGB32 (8 bpc,
 * premultiplied).
 *
 * If the patch folds over itself, the part with the highest v
 * parameter is considered above. If both have the same v, the one
 * with the highest u parameter is above.
 *
 * The function guarantees that it will draw the patch with a step
 * small enough to never have a distance above 1/sqrt(2) between two
 * adjacent points (which guarantees that no hole can appear).
 *
 * This function can be used to rasterize a tile of PDF type 7
 * shadings (see http://www.adobe.com/devnet/pdf/pdf_reference.html).
 */
static void
draw_bezier_patch (unsigned char *data, int width, int height, int stride,
		     cairo_point_double_t p[4][4], double c[4][4])
{
    double top, bottom, left, right, steps_sq;
    int i, j, v;

    top = bottom = p[0][0].y;
    for (i = 0; i < 4; ++i) {
	for (j= 0; j < 4; ++j) {
	    top    = MIN (top,    p[i][j].y);
	    bottom = MAX (bottom, p[i][j].y);
	}
    }

    v = intersect_interval (top, bottom, 0, height);
    if (v == OUTSIDE)
	return;

    left = right = p[0][0].x;
    for (i = 0; i < 4; ++i) {
	for (j= 0; j < 4; ++j) {
	    left  = MIN (left,  p[i][j].x);
	    right = MAX (right, p[i][j].x);
	}
    }

    v &= intersect_interval (left, right, 0, width);
    if (v == OUTSIDE)
	return;

    steps_sq = 0;
    for (i = 0; i < 4; ++i)
	steps_sq = MAX (steps_sq, bezier_steps_sq (p[i]));

    if (steps_sq >= (v == INSIDE ? STEPS_MAX_V * STEPS_MAX_V : STEPS_CLIP_V * STEPS_CLIP_V)) {
	/* The number of steps is greater than the threshold. This
	 * means that either the error would become too big if we
	 * directly rasterized it or that we can probably save some
	 * time by splitting the curve and clipping part of it. The
	 * patch is only split in the v direction to guarantee that
	 * rasterizing each part will overwrite parts with low v with
	 * overlapping parts with higher v. */

	cairo_point_double_t first[4][4], second[4][4];
	double subc[4][4];

	for (i = 0; i < 4; ++i)
	    split_bezier (p[i], first[i], second[i]);

	for (i = 0; i < 4; ++i) {
	    subc[0][i] = c[0][i];
	    subc[1][i] = c[1][i];
	    subc[2][i] = 0.5 * (c[0][i] + c[2][i]);
	    subc[3][i] = 0.5 * (c[1][i] + c[3][i]);
	}

	draw_bezier_patch (data, width, height, stride, first, subc);

	for (i = 0; i < 4; ++i) {
	    subc[0][i] = subc[2][i];
	    subc[1][i] = subc[3][i];
	    subc[2][i] = c[2][i];
	    subc[3][i] = c[3][i];
	}
	draw_bezier_patch (data, width, height, stride, second, subc);
    } else {
	rasterize_bezier_patch (data, width, height, stride, sqsteps2shift (steps_sq), p, c);
    }
}

/*
 * Draw a tensor product shading pattern.
 *
 * Input: mesh is the mesh pattern
 *        data is the base pointer of the image
 *        width, height are the dimensions of the image
 *        stride is the stride in bytes between adjacent rows
 *
 * Output: data will be changed to have the pattern drawn on it
 *
 * data is assumed to be clear and its content is assumed to be in
 * CAIRO_FORMAT_ARGB32 (8 bpc, premultiplied).
 *
 * This function can be used to rasterize a PDF type 7 shading (see
 * http://www.adobe.com/devnet/pdf/pdf_reference.html).
 */
void
_cairo_mesh_pattern_rasterize (const cairo_mesh_pattern_t *mesh,
			       void                       *data,
			       int                         width,
			       int                         height,
			       int                         stride,
			       double                      x_offset,
			       double                      y_offset)
{
    cairo_point_double_t nodes[4][4];
    double colors[4][4];
    cairo_matrix_t p2u;
    unsigned int i, j, k, n;
    cairo_status_t status;
    const cairo_mesh_patch_t *patch;
    const cairo_color_t *c;

    assert (mesh->base.status == CAIRO_STATUS_SUCCESS);
    assert (mesh->current_patch == NULL);

    p2u = mesh->base.matrix;
    status = cairo_matrix_invert (&p2u);
    assert (status == CAIRO_STATUS_SUCCESS);

    n = _cairo_array_num_elements (&mesh->patches);
    patch = _cairo_array_index_const (&mesh->patches, 0);
    for (i = 0; i < n; i++) {
	for (j = 0; j < 4; j++) {
	    for (k = 0; k < 4; k++) {
		nodes[j][k] = patch->points[j][k];
		cairo_matrix_transform_point (&p2u, &nodes[j][k].x, &nodes[j][k].y);
		nodes[j][k].x += x_offset;
		nodes[j][k].y += y_offset;
	    }
	}

	c = &patch->colors[0];
	colors[0][0] = c->red;
	colors[0][1] = c->green;
	colors[0][2] = c->blue;
	colors[0][3] = c->alpha;

	c = &patch->colors[3];
	colors[1][0] = c->red;
	colors[1][1] = c->green;
	colors[1][2] = c->blue;
	colors[1][3] = c->alpha;

	c = &patch->colors[1];
	colors[2][0] = c->red;
	colors[2][1] = c->green;
	colors[2][2] = c->blue;
	colors[2][3] = c->alpha;

	c = &patch->colors[2];
	colors[3][0] = c->red;
	colors[3][1] = c->green;
	colors[3][2] = c->blue;
	colors[3][3] = c->alpha;

	draw_bezier_patch (data, width, height, stride, nodes, colors);
	patch++;
    }
}
