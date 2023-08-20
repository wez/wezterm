/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2009 Chris Wilson
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
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-freed-pool-private.h"

#if HAS_FREED_POOL

void *
_freed_pool_get_search (freed_pool_t *pool)
{
    void *ptr;
    int i;

    for (i = ARRAY_LENGTH (pool->pool); i--;) {
	ptr = _atomic_fetch (&pool->pool[i]);
	if (ptr != NULL) {
	    _cairo_atomic_int_set_relaxed (&pool->top, i);
	    return ptr;
	}
    }

    /* empty */
    _cairo_atomic_int_set_relaxed (&pool->top, 0);
    return NULL;
}

void
_freed_pool_put_search (freed_pool_t *pool, void *ptr)
{
    int i;

    for (i = 0; i < ARRAY_LENGTH (pool->pool); i++) {
	if (_atomic_store (&pool->pool[i], ptr)) {
	    _cairo_atomic_int_set_relaxed (&pool->top, i + 1);
	    return;
	}
    }

    /* full */
    _cairo_atomic_int_set_relaxed (&pool->top, i);
    free (ptr);
}

void
_freed_pool_reset (freed_pool_t *pool)
{
    int i;

    for (i = 0; i < ARRAY_LENGTH (pool->pool); i++) {
	free (pool->pool[i]);
	pool->pool[i] = NULL;
    }

    _cairo_atomic_int_set_relaxed (&pool->top, 0);
}

#endif
