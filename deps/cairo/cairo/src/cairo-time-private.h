/* cairo - a vector graphics library with display and print output
 *
 * Copyright (C) 2011 Andrea Canciani
 *
 * Permission to use, copy, modify, distribute, and sell this software
 * and its documentation for any purpose is hereby granted without
 * fee, provided that the above copyright notice appear in all copies
 * and that both that copyright notice and this permission notice
 * appear in supporting documentation, and that the name of the
 * copyright holders not be used in advertising or publicity
 * pertaining to distribution of the software without specific,
 * written prior permission. The copyright holders make no
 * representations about the suitability of this software for any
 * purpose.  It is provided "as is" without express or implied
 * warranty.
 *
 * THE COPYRIGHT HOLDERS DISCLAIM ALL WARRANTIES WITH REGARD TO THIS
 * SOFTWARE, INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
 * FITNESS, IN NO EVENT SHALL THE COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * SPECIAL, INDIRECT OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN
 * AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING
 * OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS
 * SOFTWARE.
 *
 * Authors: Andrea Canciani <ranma42@gmail.com>
 *
 */

#ifndef CAIRO_TIME_PRIVATE_H
#define CAIRO_TIME_PRIVATE_H

#include "cairo-compiler-private.h"
#include "cairo-wideint-private.h"

/* Make the base type signed for easier arithmetic */
typedef cairo_int64_t cairo_time_t;

#define _cairo_time_add _cairo_int64_add
#define _cairo_time_sub _cairo_int64_sub
#define _cairo_time_gt  _cairo_int64_gt
#define _cairo_time_lt  _cairo_int64_lt

#define _cairo_time_to_double   _cairo_int64_to_double
#define _cairo_time_from_double _cairo_double_to_int64

cairo_private int
_cairo_time_cmp (const void *a,
		 const void *b);

cairo_private double
_cairo_time_to_s (cairo_time_t t);

cairo_private cairo_time_t
_cairo_time_from_s (double t);

cairo_private cairo_time_t
_cairo_time_get (void);

static cairo_always_inline cairo_time_t
_cairo_time_get_delta (cairo_time_t t)
{
    cairo_time_t now;

    now = _cairo_time_get ();

    return _cairo_time_sub (now, t);
}

static cairo_always_inline double
_cairo_time_to_ns (cairo_time_t t)
{
    return 1.e9 * _cairo_time_to_s (t);
}

static cairo_always_inline cairo_time_t
_cairo_time_max (cairo_time_t a, cairo_time_t b)
{
    if (_cairo_int64_gt (a, b))
	return a;
    else
	return b;
}

static cairo_always_inline cairo_time_t
_cairo_time_min (cairo_time_t a, cairo_time_t b)
{
    if (_cairo_int64_lt (a, b))
	return a;
    else
	return b;
}

#endif
