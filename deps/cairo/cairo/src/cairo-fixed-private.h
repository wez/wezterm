/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* Cairo - a vector graphics library with display and print output
 *
 * Copyright © 2007 Mozilla Corporation
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
 * The Initial Developer of the Original Code is Mozilla Foundation
 *
 * Contributor(s):
 *	Vladimir Vukicevic <vladimir@pobox.com>
 */

#ifndef CAIRO_FIXED_PRIVATE_H
#define CAIRO_FIXED_PRIVATE_H

#include "cairo-fixed-type-private.h"

#include "cairo-wideint-private.h"
#include "cairoint.h"

/* Implementation */

#if (CAIRO_FIXED_BITS != 32)
# error CAIRO_FIXED_BITS must be 32, and the type must be a 32-bit type.
# error To remove this limitation, you will have to fix the tessellator.
#endif

#define CAIRO_FIXED_ONE        ((cairo_fixed_t)(1 << CAIRO_FIXED_FRAC_BITS))
#define CAIRO_FIXED_ONE_DOUBLE ((double)(1 << CAIRO_FIXED_FRAC_BITS))
#define CAIRO_FIXED_EPSILON    ((cairo_fixed_t)(1))

#define CAIRO_FIXED_MAX        INT32_MAX /* Maximum fixed point value */
#define CAIRO_FIXED_MIN        INT32_MIN /* Minimum fixed point value */
#define CAIRO_FIXED_MAX_DOUBLE (((double) CAIRO_FIXED_MAX) / CAIRO_FIXED_ONE_DOUBLE)
#define CAIRO_FIXED_MIN_DOUBLE (((double) CAIRO_FIXED_MIN) / CAIRO_FIXED_ONE_DOUBLE)

#define CAIRO_FIXED_ERROR_DOUBLE (1. / (2 * CAIRO_FIXED_ONE_DOUBLE))

#define CAIRO_FIXED_FRAC_MASK  ((cairo_fixed_t)(((cairo_fixed_unsigned_t)(-1)) >> (CAIRO_FIXED_BITS - CAIRO_FIXED_FRAC_BITS)))
#define CAIRO_FIXED_WHOLE_MASK (~CAIRO_FIXED_FRAC_MASK)

static inline cairo_fixed_t
_cairo_fixed_from_int (int i)
{
    return i << CAIRO_FIXED_FRAC_BITS;
}

/* This is the "magic number" approach to converting a double into fixed
 * point as described here:
 *
 * http://www.stereopsis.com/sree/fpu2006.html (an overview)
 * http://www.d6.com/users/checker/pdfs/gdmfp.pdf (in detail)
 *
 * The basic idea is to add a large enough number to the double that the
 * literal floating point is moved up to the extent that it forces the
 * double's value to be shifted down to the bottom of the mantissa (to make
 * room for the large number being added in). Since the mantissa is, at a
 * given moment in time, a fixed point integer itself, one can convert a
 * float to various fixed point representations by moving around the point
 * of a floating point number through arithmetic operations. This behavior
 * is reliable on most modern platforms as it is mandated by the IEEE-754
 * standard for floating point arithmetic.
 *
 * For our purposes, a "magic number" must be carefully selected that is
 * both large enough to produce the desired point-shifting effect, and also
 * has no lower bits in its representation that would interfere with our
 * value at the bottom of the mantissa. The magic number is calculated as
 * follows:
 *
 *          (2 ^ (MANTISSA_SIZE - FRACTIONAL_SIZE)) * 1.5
 *
 * where in our case:
 *  - MANTISSA_SIZE for 64-bit doubles is 52
 *  - FRACTIONAL_SIZE for 16.16 fixed point is 16
 *
 * Although this approach provides a very large speedup of this function
 * on a wide-array of systems, it does come with two caveats:
 *
 * 1) It uses banker's rounding as opposed to arithmetic rounding.
 * 2) It doesn't function properly if the FPU is in single-precision
 *    mode.
 */

/* The 16.16 number must always be available */
#define CAIRO_MAGIC_NUMBER_FIXED_16_16 (103079215104.0)

#if CAIRO_FIXED_BITS <= 32
#define CAIRO_MAGIC_NUMBER_FIXED ((1LL << (52 - CAIRO_FIXED_FRAC_BITS)) * 1.5)

/* For 32-bit fixed point numbers */
static inline cairo_fixed_t
_cairo_fixed_from_double (double d)
{
    union {
        double d;
        int32_t i[2];
    } u;

    u.d = d + CAIRO_MAGIC_NUMBER_FIXED;
#ifdef FLOAT_WORDS_BIGENDIAN
    return u.i[1];
#else
    return u.i[0];
#endif
}

#else
# error Please define a magic number for your fixed point type!
# error See cairo-fixed-private.h for details.
#endif

static inline cairo_fixed_t
_cairo_fixed_from_double_clamped (double d, double tolerance)
{
    if (d > CAIRO_FIXED_MAX_DOUBLE - tolerance)
       d = CAIRO_FIXED_MAX_DOUBLE - tolerance;
    else if (d < CAIRO_FIXED_MIN_DOUBLE + tolerance)
       d = CAIRO_FIXED_MIN_DOUBLE + tolerance;

    return _cairo_fixed_from_double (d);
}

static inline cairo_fixed_t
_cairo_fixed_from_26_6 (uint32_t i)
{
#if CAIRO_FIXED_FRAC_BITS > 6
    return i << (CAIRO_FIXED_FRAC_BITS - 6);
#else
    return i >> (6 - CAIRO_FIXED_FRAC_BITS);
#endif
}

static inline cairo_fixed_t
_cairo_fixed_from_16_16 (uint32_t i)
{
#if CAIRO_FIXED_FRAC_BITS > 16
    return i << (CAIRO_FIXED_FRAC_BITS - 16);
#else
    return i >> (16 - CAIRO_FIXED_FRAC_BITS);
#endif
}

static inline double
_cairo_fixed_to_double (cairo_fixed_t f)
{
    return ((double) f) / CAIRO_FIXED_ONE_DOUBLE;
}

static inline int
_cairo_fixed_is_integer (cairo_fixed_t f)
{
    return (f & CAIRO_FIXED_FRAC_MASK) == 0;
}

static inline cairo_fixed_t
_cairo_fixed_floor (cairo_fixed_t f)
{
    return f & ~CAIRO_FIXED_FRAC_MASK;
}

static inline cairo_fixed_t
_cairo_fixed_ceil (cairo_fixed_t f)
{
    return _cairo_fixed_floor (f + CAIRO_FIXED_FRAC_MASK);
}

static inline cairo_fixed_t
_cairo_fixed_round (cairo_fixed_t f)
{
    return _cairo_fixed_floor (f + (CAIRO_FIXED_FRAC_MASK+1)/2);
}

static inline cairo_fixed_t
_cairo_fixed_round_down (cairo_fixed_t f)
{
    return _cairo_fixed_floor (f + CAIRO_FIXED_FRAC_MASK/2);
}

static inline int
_cairo_fixed_integer_part (cairo_fixed_t f)
{
    return f >> CAIRO_FIXED_FRAC_BITS;
}

static inline int
_cairo_fixed_integer_round (cairo_fixed_t f)
{
    return _cairo_fixed_integer_part (f + (CAIRO_FIXED_FRAC_MASK+1)/2);
}

static inline int
_cairo_fixed_integer_round_down (cairo_fixed_t f)
{
    return _cairo_fixed_integer_part (f + CAIRO_FIXED_FRAC_MASK/2);
}

static inline int
_cairo_fixed_fractional_part (cairo_fixed_t f)
{
    return f & CAIRO_FIXED_FRAC_MASK;
}

static inline int
_cairo_fixed_integer_floor (cairo_fixed_t f)
{
    if (f >= 0)
        return f >> CAIRO_FIXED_FRAC_BITS;
    else
        return -((-f - 1) >> CAIRO_FIXED_FRAC_BITS) - 1;
}

static inline int
_cairo_fixed_integer_ceil (cairo_fixed_t f)
{
    if (f > 0)
	return ((f - 1)>>CAIRO_FIXED_FRAC_BITS) + 1;
    else
	return - ((cairo_fixed_t)(-(cairo_fixed_unsigned_t)f) >> CAIRO_FIXED_FRAC_BITS);
}

/* A bunch of explicit 16.16 operators; we need these
 * to interface with pixman and other backends that require
 * 16.16 fixed point types.
 */
static inline cairo_fixed_16_16_t
_cairo_fixed_to_16_16 (cairo_fixed_t f)
{
#if (CAIRO_FIXED_FRAC_BITS == 16) && (CAIRO_FIXED_BITS == 32)
    return f;
#elif CAIRO_FIXED_FRAC_BITS > 16
    /* We're just dropping the low bits, so we won't ever got over/underflow here */
    return f >> (CAIRO_FIXED_FRAC_BITS - 16);
#else
    cairo_fixed_16_16_t x;

    /* Handle overflow/underflow by clamping to the lowest/highest
     * value representable as 16.16
     */
    if ((f >> CAIRO_FIXED_FRAC_BITS) < INT16_MIN) {
	x = INT32_MIN;
    } else if ((f >> CAIRO_FIXED_FRAC_BITS) > INT16_MAX) {
	x = INT32_MAX;
    } else {
	x = f << (16 - CAIRO_FIXED_FRAC_BITS);
    }

    return x;
#endif
}

static inline cairo_fixed_16_16_t
_cairo_fixed_16_16_from_double (double d)
{
    union {
        double d;
        int32_t i[2];
    } u;

    u.d = d + CAIRO_MAGIC_NUMBER_FIXED_16_16;
#ifdef FLOAT_WORDS_BIGENDIAN
    return u.i[1];
#else
    return u.i[0];
#endif
}

static inline int
_cairo_fixed_16_16_floor (cairo_fixed_16_16_t f)
{
    if (f >= 0)
	return f >> 16;
    else
	return -((-f - 1) >> 16) - 1;
}

static inline double
_cairo_fixed_16_16_to_double (cairo_fixed_16_16_t f)
{
    return ((double) f) / (double) (1 << 16);
}

#if CAIRO_FIXED_BITS == 32

static inline cairo_fixed_t
_cairo_fixed_mul (cairo_fixed_t a, cairo_fixed_t b)
{
    cairo_int64_t temp = _cairo_int32x32_64_mul (a, b);
    return _cairo_int64_to_int32(_cairo_int64_rsl (temp, CAIRO_FIXED_FRAC_BITS));
}

/* computes round (a * b / c) */
static inline cairo_fixed_t
_cairo_fixed_mul_div (cairo_fixed_t a, cairo_fixed_t b, cairo_fixed_t c)
{
    cairo_int64_t ab  = _cairo_int32x32_64_mul (a, b);
    cairo_int64_t c64 = _cairo_int32_to_int64 (c);
    return _cairo_int64_to_int32 (_cairo_int64_divrem (ab, c64).quo);
}

/* computes floor (a * b / c) */
static inline cairo_fixed_t
_cairo_fixed_mul_div_floor (cairo_fixed_t a, cairo_fixed_t b, cairo_fixed_t c)
{
    return _cairo_int64_32_div (_cairo_int32x32_64_mul (a, b), c);
}

/* compute y from x so that (x,y), p1, and p2 are collinear */
static inline cairo_fixed_t
_cairo_edge_compute_intersection_y_for_x (const cairo_point_t *p1,
					  const cairo_point_t *p2,
					  cairo_fixed_t x)
{
    cairo_fixed_t y, dx;

    if (x == p1->x)
	return p1->y;
    if (x == p2->x)
	return p2->y;

    y = p1->y;
    dx = p2->x - p1->x;
    if (dx != 0)
	y += _cairo_fixed_mul_div_floor (x - p1->x, p2->y - p1->y, dx);

    return y;
}

/* compute x from y so that (x,y), p1, and p2 are collinear */
static inline cairo_fixed_t
_cairo_edge_compute_intersection_x_for_y (const cairo_point_t *p1,
					  const cairo_point_t *p2,
					  cairo_fixed_t y)
{
    cairo_fixed_t x, dy;

    if (y == p1->y)
	return p1->x;
    if (y == p2->y)
	return p2->x;

    x = p1->x;
    dy = p2->y - p1->y;
    if (dy != 0)
	x += _cairo_fixed_mul_div_floor (y - p1->y, p2->x - p1->x, dy);

    return x;
}

/* Intersect two segments based on the algorithm described at
 * http://paulbourke.net/geometry/pointlineplane/. This implementation
 * uses floating point math. */
static inline cairo_bool_t
_slow_segment_intersection (const cairo_point_t *seg1_p1,
			    const cairo_point_t *seg1_p2,
			    const cairo_point_t *seg2_p1,
			    const cairo_point_t *seg2_p2,
			    cairo_point_t *intersection)
{
    double denominator, u_a, u_b;
    double seg1_dx, seg1_dy, seg2_dx, seg2_dy, seg_start_dx, seg_start_dy;

    seg1_dx = _cairo_fixed_to_double (seg1_p2->x - seg1_p1->x);
    seg1_dy = _cairo_fixed_to_double (seg1_p2->y - seg1_p1->y);
    seg2_dx = _cairo_fixed_to_double (seg2_p2->x - seg2_p1->x);
    seg2_dy = _cairo_fixed_to_double (seg2_p2->y - seg2_p1->y);
    denominator = (seg2_dy * seg1_dx) - (seg2_dx * seg1_dy);
    if (denominator == 0)
	return FALSE;

    seg_start_dx = _cairo_fixed_to_double (seg1_p1->x - seg2_p1->x);
    seg_start_dy = _cairo_fixed_to_double (seg1_p1->y - seg2_p1->y);
    u_a = ((seg2_dx * seg_start_dy) - (seg2_dy * seg_start_dx)) / denominator;
    u_b = ((seg1_dx * seg_start_dy) - (seg1_dy * seg_start_dx)) / denominator;

    if (u_a <= 0 || u_a >= 1 || u_b <= 0 || u_b >= 1)
	return FALSE;

    intersection->x = seg1_p1->x + _cairo_fixed_from_double ((u_a * seg1_dx));
    intersection->y = seg1_p1->y + _cairo_fixed_from_double ((u_a * seg1_dy));
    return TRUE;
}

#else
# error Please define multiplication and other operands for your fixed-point type size
#endif

#endif /* CAIRO_FIXED_PRIVATE_H */
