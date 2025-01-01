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

#ifndef CAIRO_FREED_POOL_H
#define CAIRO_FREED_POOL_H

#include "cairoint.h"
#include "cairo-atomic-private.h"

CAIRO_BEGIN_DECLS

#define DISABLE_FREED_POOLS 0

#if HAS_ATOMIC_OPS && ! DISABLE_FREED_POOLS
/* Keep a stash of recently freed clip_paths, since we need to
 * reallocate them frequently.
 */
#define MAX_FREED_POOL_SIZE 16
typedef struct {
    void *pool[MAX_FREED_POOL_SIZE];
    cairo_atomic_int_t top;
} freed_pool_t;

static cairo_always_inline void *
_atomic_fetch (void **slot)
{
    void *ptr;

    do {
        ptr = _cairo_atomic_ptr_get (slot);
    } while (! _cairo_atomic_ptr_cmpxchg (slot, ptr, NULL));

    return ptr;
}

static cairo_always_inline cairo_bool_t
_atomic_store (void **slot, void *ptr)
{
    return _cairo_atomic_ptr_cmpxchg (slot, NULL, ptr);
}

cairo_private void *
_freed_pool_get_search (freed_pool_t *pool);

static inline void *
_freed_pool_get (freed_pool_t *pool)
{
    void *ptr;
    int i;

    i = _cairo_atomic_int_get_relaxed (&pool->top) - 1;
    if (i < 0)
	i = 0;

    ptr = _atomic_fetch (&pool->pool[i]);
    if (likely (ptr != NULL)) {
	_cairo_atomic_int_set_relaxed (&pool->top, i);
	return ptr;
    }

    /* either empty or contended */
    return _freed_pool_get_search (pool);
}

cairo_private void
_freed_pool_put_search (freed_pool_t *pool, void *ptr);

static inline void
_freed_pool_put (freed_pool_t *pool, void *ptr)
{
    int i;

    i = _cairo_atomic_int_get_relaxed (&pool->top);
    if (likely (i < ARRAY_LENGTH (pool->pool) &&
		_atomic_store (&pool->pool[i], ptr)))
    {
	_cairo_atomic_int_set_relaxed (&pool->top, i + 1);
	return;
    }

    /* either full or contended */
    _freed_pool_put_search (pool, ptr);
}

cairo_private void
_freed_pool_reset (freed_pool_t *pool);

#define HAS_FREED_POOL 1

#else

/* A warning about an unused freed-pool in a build without atomics
 * enabled usually indicates a missing _freed_pool_reset() in the
 * static reset function */

typedef int freed_pool_t;

#define _freed_pool_get(pool) NULL
#define _freed_pool_put(pool, ptr) free(ptr)
#define _freed_pool_reset(ptr)

#endif

CAIRO_END_DECLS

#endif /* CAIRO_FREED_POOL_PRIVATE_H */
