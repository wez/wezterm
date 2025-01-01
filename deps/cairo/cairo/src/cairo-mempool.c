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
 * notice, a recipoolent may use your version of this file under either
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

#include "cairoint.h"

#include "cairo-mempool-private.h"
#include "cairo-list-inline.h"

/* a simple buddy allocator for memory pools
 * XXX fragmentation? use Doug Lea's malloc?
 */

#define BITTEST(p, n)  ((p)->map[(n) >> 3] &   (128 >> ((n) & 7)))
#define BITSET(p, n)   ((p)->map[(n) >> 3] |=  (128 >> ((n) & 7)))
#define BITCLEAR(p, n) ((p)->map[(n) >> 3] &= ~(128 >> ((n) & 7)))

static void
clear_bits (cairo_mempool_t *pool, size_t first, size_t last)
{
    size_t i, n = last;
    size_t first_full = (first + 7) & ~7;
    size_t past_full = last & ~7;
    size_t bytes;

    if (n > first_full)
	n = first_full;
    for (i = first; i < n; i++)
	  BITCLEAR (pool, i);

    if (past_full > first_full) {
	bytes = past_full - first_full;
	bytes = bytes >> 3;
	memset (pool->map + (first_full >> 3), 0, bytes);
    }

    if (past_full < n)
	past_full = n;
    for (i = past_full; i < last; i++)
	BITCLEAR (pool, i);
}

static void
free_bits (cairo_mempool_t *pool, size_t start, int bits, cairo_bool_t clear)
{
    struct _cairo_memblock *block;

    if (clear)
	clear_bits (pool, start, start + (((size_t) 1) << bits));

    block = pool->blocks + start;
    block->bits = bits;

    cairo_list_add (&block->link, &pool->free[bits]);

    pool->free_bytes += ((size_t) 1) << (bits + pool->min_bits);
    if (bits > pool->max_free_bits)
	pool->max_free_bits = bits;
}

/* Add a chunk to the free list */
static void
free_blocks (cairo_mempool_t *pool,
	     size_t first,
	     size_t last,
	     cairo_bool_t clear)
{
    size_t i, len;
    int bits = 0;

    for (i = first, len = 1; i < last; i += len) {
        /* To avoid cost quadratic in the number of different
	 * blocks produced from this chunk of store, we have to
	 * use the size of the previous block produced from this
	 * chunk as the starting point to work out the size of the
	 * next block we can produce. If you look at the binary
	 * representation of the starting points of the blocks
	 * produced, you can see that you first of all increase the
	 * size of the blocks produced up to some maximum as the
	 * address dealt with gets offsets added on which zap out
	 * low order bits, then decrease as the low order bits of the
	 * final block produced get added in. E.g. as you go from
	 * 001 to 0111 you generate blocks
	 * of size 001 at 001 taking you to 010
	 * of size 010 at 010 taking you to 100
	 * of size 010 at 100 taking you to 110
	 * of size 001 at 110 taking you to 111
	 * So the maximum total cost of the loops below this comment
	 * is one trip from the lowest blocksize to the highest and
	 * back again.
	 */
	while (bits < pool->num_sizes - 1) {
	    size_t next_bits = bits + 1;
	    size_t next_len = len << 1;

	    if (i + next_bits > last) {
		/* off end of chunk to be freed */
	        break;
	    }

	    if (i & (next_len - 1)) /* block would not be on boundary */
	        break;

	    bits = next_bits;
	    len = next_len;
	}

	do {
	    if (i + len <= last && /* off end of chunk to be freed */
		(i & (len - 1)) == 0) /* block would not be on boundary */
		break;

	    bits--; len >>=1;
	} while (len);

	if (len == 0)
	    break;

	free_bits (pool, i, bits, clear);
    }
}

static struct _cairo_memblock *
get_buddy (cairo_mempool_t *pool, size_t offset, int bits)
{
    struct _cairo_memblock *block;

    if (offset + (((size_t) 1) << bits) >= pool->num_blocks)
	return NULL; /* invalid */

    if (BITTEST (pool, offset + (((size_t) 1) << bits) - 1))
	return NULL; /* buddy is allocated */

    block = pool->blocks + offset;
    if (block->bits != bits)
	return NULL; /* buddy is partially allocated */

    return block;
}

static void
merge_buddies (cairo_mempool_t *pool,
	       struct _cairo_memblock *block,
	       int max_bits)
{
    size_t block_offset = block - pool->blocks;
    int bits = block->bits;

    while (bits < max_bits - 1) {
	/* while you can, merge two blocks and get a legal block size */
	size_t buddy_offset = block_offset ^ (((size_t) 1) << bits);

	block = get_buddy (pool, buddy_offset, bits);
	if (block == NULL)
	    break;

	cairo_list_del (&block->link);

	/* Merged block starts at buddy */
	if (buddy_offset < block_offset)
	    block_offset = buddy_offset;

	bits++;
    }

    block = pool->blocks + block_offset;
    block->bits = bits;
    cairo_list_add (&block->link, &pool->free[bits]);

    if (bits > pool->max_free_bits)
	pool->max_free_bits = bits;
}

/* attempt to merge all available buddies up to a particular size */
static int
merge_bits (cairo_mempool_t *pool, int max_bits)
{
    struct _cairo_memblock *block, *buddy, *next;
    int bits;

    for (bits = 0; bits < max_bits - 1; bits++) {
	cairo_list_foreach_entry_safe (block, next,
				       struct _cairo_memblock,
				       &pool->free[bits],
				       link)
	{
	    size_t buddy_offset = (block - pool->blocks) ^ (((size_t) 1) << bits);

	    buddy = get_buddy (pool, buddy_offset, bits);
	    if (buddy == NULL)
		continue;

	    if (buddy == next) {
		next = cairo_container_of (buddy->link.next,
					   struct _cairo_memblock,
					   link);
	    }

	    cairo_list_del (&block->link);
	    merge_buddies (pool, block, max_bits);
	}
    }

    return pool->max_free_bits;
}

/* find store for 1 << bits blocks */
static void *
buddy_malloc (cairo_mempool_t *pool, int bits)
{
    size_t past, offset;
    struct _cairo_memblock *block;
    int b;

    if (bits > pool->max_free_bits && bits > merge_bits (pool, bits))
	return NULL;

    /* Find a list with blocks big enough on it */
    block = NULL;
    for (b = bits; b <= pool->max_free_bits; b++) {
	if (! cairo_list_is_empty (&pool->free[b])) {
	    block = cairo_list_first_entry (&pool->free[b],
					    struct _cairo_memblock,
					    link);
	    break;
	}
    }
    assert (block != NULL);

    cairo_list_del (&block->link);

    while (cairo_list_is_empty (&pool->free[pool->max_free_bits])) {
	if (--pool->max_free_bits == -1)
	    break;
    }

    /* Mark end of allocated area */
    offset = block - pool->blocks;
    past = offset + (((size_t) 1) << bits);
    BITSET (pool, past - 1);
    block->bits = bits;

    /* If we used a larger free block than we needed, free the rest */
    pool->free_bytes -= ((size_t) 1) << (b + pool->min_bits);
    free_blocks (pool, past, offset + (((size_t) 1) << b), 0);

    return pool->base + ((block - pool->blocks) << pool->min_bits);
}

cairo_status_t
_cairo_mempool_init (cairo_mempool_t *pool,
		      void *base, size_t bytes,
		      int min_bits, int num_sizes)
{
    uintptr_t tmp;
    int num_blocks;
    int i;

    /* Align the start to an integral chunk */
    tmp = ((uintptr_t) base) & ((((size_t) 1) << min_bits) - 1);
    if (tmp) {
	tmp = (((size_t) 1) << min_bits) - tmp;
	base = (char *)base + tmp;
	bytes -= tmp;
    }

    assert ((((uintptr_t) base) & ((((size_t) 1) << min_bits) - 1)) == 0);
    assert (num_sizes < ARRAY_LENGTH (pool->free));

    pool->base = base;
    pool->free_bytes = 0;
    pool->max_bytes = bytes;
    pool->max_free_bits = -1;

    num_blocks = bytes >> min_bits;
    pool->blocks = calloc (num_blocks, sizeof (struct _cairo_memblock));
    if (pool->blocks == NULL)
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    pool->num_blocks = num_blocks;
    pool->min_bits = min_bits;
    pool->num_sizes = num_sizes;

    for (i = 0; i < ARRAY_LENGTH (pool->free); i++)
	cairo_list_init (&pool->free[i]);

    pool->map = _cairo_malloc ((num_blocks + 7) >> 3);
    if (pool->map == NULL) {
	free (pool->blocks);
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    memset (pool->map, -1, (num_blocks + 7) >> 3);
    clear_bits (pool, 0, num_blocks);

    /* Now add all blocks to the free list */
    free_blocks (pool, 0, num_blocks, 1);

    return CAIRO_STATUS_SUCCESS;
}

void *
_cairo_mempool_alloc (cairo_mempool_t *pool, size_t bytes)
{
    size_t size;
    int bits;

    size = ((size_t) 1) << pool->min_bits;
    for (bits = 0; size < bytes; bits++)
	size <<= 1;
    if (bits >= pool->num_sizes)
	return NULL;

    return buddy_malloc (pool, bits);
}

void
_cairo_mempool_free (cairo_mempool_t *pool, void *storage)
{
    size_t block_offset;
    struct _cairo_memblock *block;

    block_offset = ((char *)storage - pool->base) >> pool->min_bits;
    block = pool->blocks + block_offset;

    BITCLEAR (pool, block_offset + ((((size_t) 1) << block->bits) - 1));
    pool->free_bytes += ((size_t) 1) << (block->bits + pool->min_bits);

    merge_buddies (pool, block, pool->num_sizes);
}

void
_cairo_mempool_fini (cairo_mempool_t *pool)
{
    free (pool->map);
    free (pool->blocks);
}
