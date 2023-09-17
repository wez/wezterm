/* Cairo - a vector graphics library with display and print output
 *
 * Copyright © 2007 Chris Wilson
 * Copyright © 2009 Intel Corporation
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
 * The Initial Developer of the Original Code is Red Hat, Inc.
 *
 * Contributors(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_MEMPOOL_PRIVATE_H
#define CAIRO_MEMPOOL_PRIVATE_H

#include "cairo-compiler-private.h"
#include "cairo-error-private.h"

#include <stddef.h> /* for size_t */

CAIRO_BEGIN_DECLS

typedef struct _cairo_mempool cairo_mempool_t;

struct _cairo_mempool {
    char *base;
    struct _cairo_memblock {
	int bits;
	cairo_list_t link;
    } *blocks;
    cairo_list_t free[32];
    unsigned char *map;

    unsigned int num_blocks;
    int min_bits;     /* Minimum block size is 1 << min_bits */
    int num_sizes;
    int max_free_bits;

    size_t free_bytes;
    size_t max_bytes;
};

cairo_private cairo_status_t
_cairo_mempool_init (cairo_mempool_t *pool,
		     void *base,
		     size_t bytes,
		     int min_bits,
		     int num_sizes);

cairo_private void *
_cairo_mempool_alloc (cairo_mempool_t *pi, size_t bytes);

cairo_private void
_cairo_mempool_free (cairo_mempool_t *pi, void *storage);

cairo_private void
_cairo_mempool_fini (cairo_mempool_t *pool);

CAIRO_END_DECLS

#endif /* CAIRO_MEMPOOL_PRIVATE_H */
