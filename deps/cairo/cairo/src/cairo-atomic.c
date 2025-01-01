/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2007 Chris Wilson
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
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-atomic-private.h"
#include "cairo-mutex-private.h"

#ifdef HAS_ATOMIC_OPS
COMPILE_TIME_ASSERT(sizeof(void*) == sizeof(int) ||
		    sizeof(void*) == sizeof(long) ||
		    sizeof(void*) == sizeof(long long));
#else
void
_cairo_atomic_int_inc (cairo_atomic_int_t *x)
{
    CAIRO_MUTEX_LOCK (_cairo_atomic_mutex);
    *x += 1;
    CAIRO_MUTEX_UNLOCK (_cairo_atomic_mutex);
}

cairo_bool_t
_cairo_atomic_int_dec_and_test (cairo_atomic_int_t *x)
{
    cairo_bool_t ret;

    CAIRO_MUTEX_LOCK (_cairo_atomic_mutex);
    ret = --*x == 0;
    CAIRO_MUTEX_UNLOCK (_cairo_atomic_mutex);

    return ret;
}

cairo_atomic_int_t
_cairo_atomic_int_cmpxchg_return_old_impl (cairo_atomic_int_t *x, cairo_atomic_int_t oldv, cairo_atomic_int_t newv)
{
    cairo_atomic_int_t ret;

    CAIRO_MUTEX_LOCK (_cairo_atomic_mutex);
    ret = *x;
    if (ret == oldv)
	*x = newv;
    CAIRO_MUTEX_UNLOCK (_cairo_atomic_mutex);

    return ret;
}

void *
_cairo_atomic_ptr_cmpxchg_return_old_impl (void **x, void *oldv, void *newv)
{
    void *ret;

    CAIRO_MUTEX_LOCK (_cairo_atomic_mutex);
    ret = *x;
    if (ret == oldv)
	*x = newv;
    CAIRO_MUTEX_UNLOCK (_cairo_atomic_mutex);

    return ret;
}

#ifdef ATOMIC_OP_NEEDS_MEMORY_BARRIER
cairo_atomic_int_t
_cairo_atomic_int_get (cairo_atomic_int_t *x)
{
    cairo_atomic_int_t ret;

    CAIRO_MUTEX_LOCK (_cairo_atomic_mutex);
    ret = *x;
    CAIRO_MUTEX_UNLOCK (_cairo_atomic_mutex);

    return ret;
}

cairo_atomic_int_t
_cairo_atomic_int_get_relaxed (cairo_atomic_int_t *x)
{
    return _cairo_atomic_int_get (x);
}

void
_cairo_atomic_int_set_relaxed (cairo_atomic_int_t *x, cairo_atomic_int_t val)
{
    CAIRO_MUTEX_LOCK (_cairo_atomic_mutex);
    *x = val;
    CAIRO_MUTEX_UNLOCK (_cairo_atomic_mutex);
}

void*
_cairo_atomic_ptr_get (void **x)
{
    void *ret;

    CAIRO_MUTEX_LOCK (_cairo_atomic_mutex);
    ret = *x;
    CAIRO_MUTEX_UNLOCK (_cairo_atomic_mutex);

    return ret;
}
#endif

#endif
