/* cairo - a vector graphics library with display and print output
 *
 * Copyright (c) 2007 Netlabs
 * Copyright (c) 2006 Mozilla Corporation
 * Copyright (c) 2006 Red Hat, Inc.
 * Copyright (c) 2011 Andrea Canciani
 *
 * Permission to use, copy, modify, distribute, and sell this software
 * and its documentation for any purpose is hereby granted without
 * fee, provided that the above copyright notice appear in all copies
 * and that both that copyright notice and this permission notice
 * appear in supporting documentation, and that the name of
 * the authors not be used in advertising or publicity pertaining to
 * distribution of the software without specific, written prior
 * permission. The authors make no representations about the
 * suitability of this software for any purpose.  It is provided "as
 * is" without express or implied warranty.
 *
 * THE AUTHORS DISCLAIM ALL WARRANTIES WITH REGARD TO THIS
 * SOFTWARE, INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
 * FITNESS, IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY SPECIAL,
 * INDIRECT OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER
 * RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
 * OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR
 * IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 *
 * Authors: Peter Weilbacher <mozilla@weilbacher.org>
 *	    Vladimir Vukicevic <vladimir@pobox.com>
 *	    Carl Worth <cworth@cworth.org>
 *          Andrea Canciani <ranma42@gmail.com>
 */

#include "cairoint.h"

#include "cairo-time-private.h"

#if HAVE_CLOCK_GETTIME
#if defined(CLOCK_MONOTONIC_RAW)
#define CAIRO_CLOCK CLOCK_MONOTONIC_RAW
#elif defined(CLOCK_MONOTONIC)
#define CAIRO_CLOCK CLOCK_MONOTONIC
#endif
#endif

#if defined(__APPLE__)
#include <mach/mach_time.h>

static cairo_always_inline double
_cairo_time_1s (void)
{
    mach_timebase_info_data_t freq;

    mach_timebase_info (&freq);

    return 1000000000. * freq.denom / freq.numer;
}

cairo_time_t
_cairo_time_get (void)
{
    return mach_absolute_time ();
}

#elif _WIN32
#include <windows.h>

static cairo_always_inline double
_cairo_time_1s (void)
{
    LARGE_INTEGER freq;

    QueryPerformanceFrequency (&freq);

    return freq.QuadPart;
}

#ifndef HAVE_UINT64_T
static cairo_always_inline cairo_time_t
_cairo_time_from_large_integer (LARGE_INTEGER t)
{
    cairo_int64_t r;

    r = _cairo_int64_lsl (_cairo_int32_to_int64 (t.HighPart), 32);
    r = _cairo_int64_add (r, _cairo_int32_to_int64 (t.LowPart));

    return r;
}
#else
static cairo_always_inline cairo_time_t
_cairo_time_from_large_integer (LARGE_INTEGER t)
{
    return t.QuadPart;
}
#endif

cairo_time_t
_cairo_time_get (void)
{
    LARGE_INTEGER t;

    QueryPerformanceCounter (&t);

    return _cairo_time_from_large_integer(t);
}

#elif defined(CAIRO_CLOCK)
#include <time.h>

static cairo_always_inline double
_cairo_time_1s (void)
{
    return 1000000000;
}

cairo_time_t
_cairo_time_get (void)
{
    struct timespec t;
    cairo_time_t r;

    clock_gettime (CAIRO_CLOCK, &t);

    r = _cairo_double_to_int64 (_cairo_time_1s ());
    r = _cairo_int64_mul (r, _cairo_int32_to_int64 (t.tv_sec));
    r = _cairo_int64_add (r, _cairo_int32_to_int64 (t.tv_nsec));

    return r;
}

#else
#include <sys/time.h>

static cairo_always_inline double
_cairo_time_1s (void)
{
    return 1000000;
}

cairo_time_t
_cairo_time_get (void)
{
    struct timeval t;
    cairo_time_t r;

    gettimeofday (&t, NULL);

    r = _cairo_double_to_int64 (_cairo_time_1s ());
    r = _cairo_int64_mul (r, _cairo_int32_to_int64 (t.tv_sec));
    r = _cairo_int64_add (r, _cairo_int32_to_int64 (t.tv_usec));

    return r;
}

#endif

int
_cairo_time_cmp (const void *a,
		 const void *b)
{
    const cairo_time_t *ta = a, *tb = b;
    return _cairo_int64_cmp (*ta, *tb);
}

static double
_cairo_time_ticks_per_sec (void)
{
    static double ticks = 0;

    if (unlikely (ticks == 0))
	ticks = _cairo_time_1s ();

    return ticks;
}

static double
_cairo_time_s_per_tick (void)
{
    static double s = 0;

    if (unlikely (s == 0))
	s = 1. / _cairo_time_ticks_per_sec ();

    return s;
}

double
_cairo_time_to_s (cairo_time_t t)
{
    return _cairo_int64_to_double (t) * _cairo_time_s_per_tick ();
}

cairo_time_t
_cairo_time_from_s (double t)
{
    return _cairo_double_to_int64 (t * _cairo_time_ticks_per_sec ());
}
