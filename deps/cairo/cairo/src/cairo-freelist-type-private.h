/*
 * Copyright © 2010 Joonas Pihlaja
 *
 * Permission to use, copy, modify, distribute, and sell this software and its
 * documentation for any purpose is hereby granted without fee, provided that
 * the above copyright notice appear in all copies and that both that copyright
 * notice and this permission notice appear in supporting documentation, and
 * that the name of the copyright holders not be used in advertising or
 * publicity pertaining to distribution of the software without specific,
 * written prior permission.  The copyright holders make no representations
 * about the suitability of this software for any purpose.  It is provided "as
 * is" without express or implied warranty.
 *
 * THE COPYRIGHT HOLDERS DISCLAIM ALL WARRANTIES WITH REGARD TO THIS SOFTWARE,
 * INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS, IN NO
 * EVENT SHALL THE COPYRIGHT HOLDERS BE LIABLE FOR ANY SPECIAL, INDIRECT OR
 * CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE,
 * DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
 * TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE
 * OF THIS SOFTWARE.
 */
#ifndef CAIRO_FREELIST_TYPE_H
#define CAIRO_FREELIST_TYPE_H

#include "cairo-types-private.h"
#include "cairo-compiler-private.h"

typedef struct _cairo_freelist_node cairo_freelist_node_t;
struct _cairo_freelist_node {
    cairo_freelist_node_t *next;
};

typedef struct _cairo_freelist {
    cairo_freelist_node_t *first_free_node;
    unsigned nodesize;
} cairo_freelist_t;

typedef struct _cairo_freelist_pool cairo_freelist_pool_t;
struct _cairo_freelist_pool {
    cairo_freelist_pool_t *next;
    unsigned size, rem;
    uint8_t *data;
};

typedef struct _cairo_freepool {
    cairo_freelist_node_t *first_free_node;
    cairo_freelist_pool_t *pools;
    cairo_freelist_pool_t *freepools;
    unsigned nodesize;
    cairo_freelist_pool_t embedded_pool;
    uint8_t embedded_data[1000];
} cairo_freepool_t;

#endif /* CAIRO_FREELIST_TYPE_H */
