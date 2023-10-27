/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2004 Red Hat, Inc.
 * Copyright © 2005 Red Hat, Inc.
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
 * Contributor(s):
 *      Keith Packard <keithp@keithp.com>
 *	Graydon Hoare <graydon@redhat.com>
 *	Carl Worth <cworth@cworth.org>
 */

#include "cairoint.h"
#include "cairo-error-private.h"

static void
_cairo_cache_shrink_to_accommodate (cairo_cache_t *cache,
				    unsigned long  additional);

static cairo_bool_t
_cairo_cache_entry_is_non_zero (const void *entry)
{
    return ((const cairo_cache_entry_t *) entry)->size;
}


/**
 * _cairo_cache_init:
 * @cache: the #cairo_cache_t to initialise
 * @keys_equal: a function to return %TRUE if two keys are equal
 * @entry_destroy: destroy notifier for cache entries
 * @max_size: the maximum size for this cache
 * Returns: the newly created #cairo_cache_t
 *
 * Creates a new cache using the keys_equal() function to determine
 * the equality of entries.
 *
 * Data is provided to the cache in the form of user-derived version
 * of #cairo_cache_entry_t. A cache entry must be able to hold hash
 * code, a size, and the key/value pair being stored in the
 * cache. Sometimes only the key will be necessary, (as in
 * _cairo_cache_lookup()), and in these cases the value portion of the
 * entry need not be initialized.
 *
 * The units for max_size can be chosen by the caller, but should be
 * consistent with the units of the size field of cache entries. When
 * adding an entry with _cairo_cache_insert() if the total size of
 * entries in the cache would exceed max_size then entries will be
 * removed at random until the new entry would fit or the cache is
 * empty. Then the new entry is inserted.
 *
 * There are cases in which the automatic removal of entries is
 * undesired. If the cache entries have reference counts, then it is a
 * simple matter to use the reference counts to ensure that entries
 * continue to live even after being ejected from the cache. However,
 * in some cases the memory overhead of adding a reference count to
 * the entry would be objectionable. In such cases, the
 * _cairo_cache_freeze() and _cairo_cache_thaw() calls can be
 * used to establish a window during which no automatic removal of
 * entries will occur.
 **/
cairo_status_t
_cairo_cache_init (cairo_cache_t		*cache,
		   cairo_cache_keys_equal_func_t keys_equal,
		   cairo_cache_predicate_func_t  predicate,
		   cairo_destroy_func_t		 entry_destroy,
		   unsigned long		 max_size)
{
    cache->hash_table = _cairo_hash_table_create (keys_equal);
    if (unlikely (cache->hash_table == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    if (predicate == NULL)
	predicate = _cairo_cache_entry_is_non_zero;
    cache->predicate = predicate;
    cache->entry_destroy = entry_destroy;

    cache->max_size = max_size;
    cache->size = 0;

    cache->freeze_count = 0;

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_cache_pluck (void *entry, void *closure)
{
    _cairo_cache_remove (closure, entry);
}

/**
 * _cairo_cache_fini:
 * @cache: a cache to destroy
 *
 * Immediately destroys the given cache, freeing all resources
 * associated with it. As part of this process, the entry_destroy()
 * function, (as passed to _cairo_cache_init()), will be called for
 * each entry in the cache.
 **/
void
_cairo_cache_fini (cairo_cache_t *cache)
{
    _cairo_hash_table_foreach (cache->hash_table,
			       _cairo_cache_pluck,
			       cache);
    assert (cache->size == 0);
    _cairo_hash_table_destroy (cache->hash_table);
}

/**
 * _cairo_cache_freeze:
 * @cache: a cache with some precious entries in it (or about to be
 * added)
 *
 * Disable the automatic ejection of entries from the cache. For as
 * long as the cache is "frozen", calls to _cairo_cache_insert() will
 * add new entries to the cache regardless of how large the cache
 * grows. See _cairo_cache_thaw().
 *
 * Note: Multiple calls to _cairo_cache_freeze() will stack, in that
 * the cache will remain "frozen" until a corresponding number of
 * calls are made to _cairo_cache_thaw().
 **/
void
_cairo_cache_freeze (cairo_cache_t *cache)
{
    assert (cache->freeze_count >= 0);

    cache->freeze_count++;
}

/**
 * _cairo_cache_thaw:
 * @cache: a cache, just after the entries in it have become less
 * precious
 *
 * Cancels the effects of _cairo_cache_freeze().
 *
 * When a number of calls to _cairo_cache_thaw() is made corresponding
 * to the number of calls to _cairo_cache_freeze() the cache will no
 * longer be "frozen". If the cache had grown larger than max_size
 * while frozen, entries will immediately be ejected (by random) from
 * the cache until the cache is smaller than max_size. Also, the
 * automatic ejection of entries on _cairo_cache_insert() will resume.
 **/
void
_cairo_cache_thaw (cairo_cache_t *cache)
{
    assert (cache->freeze_count > 0);

    if (--cache->freeze_count == 0)
	_cairo_cache_shrink_to_accommodate (cache, 0);
}

/**
 * _cairo_cache_lookup:
 * @cache: a cache
 * @key: the key of interest
 * @entry_return: pointer for return value
 *
 * Performs a lookup in @cache looking for an entry which has a key
 * that matches @key, (as determined by the keys_equal() function
 * passed to _cairo_cache_init()).
 *
 * Return value: %TRUE if there is an entry in the cache that matches
 * @key, (which will now be in *entry_return). %FALSE otherwise, (in
 * which case *entry_return will be %NULL).
 **/
void *
_cairo_cache_lookup (cairo_cache_t	  *cache,
		     cairo_cache_entry_t  *key)
{
    return _cairo_hash_table_lookup (cache->hash_table,
				     (cairo_hash_entry_t *) key);
}

/**
 * _cairo_cache_remove_random:
 * @cache: a cache
 *
 * Remove a random entry from the cache.
 *
 * Return value: %TRUE if an entry was successfully removed.
 * %FALSE if there are no entries that can be removed.
 **/
static cairo_bool_t
_cairo_cache_remove_random (cairo_cache_t *cache)
{
    cairo_cache_entry_t *entry;

    entry = _cairo_hash_table_random_entry (cache->hash_table,
					    cache->predicate);
    if (unlikely (entry == NULL))
	return FALSE;

    _cairo_cache_remove (cache, entry);

    return TRUE;
}

/**
 * _cairo_cache_shrink_to_accommodate:
 * @cache: a cache
 * @additional: additional size requested in bytes
 *
 * If cache is not frozen, eject entries randomly until the size of
 * the cache is at least @additional bytes less than
 * cache->max_size. That is, make enough room to accommodate a new
 * entry of size @additional.
 **/
static void
_cairo_cache_shrink_to_accommodate (cairo_cache_t *cache,
				    unsigned long  additional)
{
    while (cache->size + additional > cache->max_size) {
	if (! _cairo_cache_remove_random (cache))
	    return;
    }
}

/**
 * _cairo_cache_insert:
 * @cache: a cache
 * @entry: an entry to be inserted
 *
 * Insert @entry into the cache. If an entry exists in the cache with
 * a matching key, then the old entry will be removed first, (and the
 * entry_destroy() callback will be called on it).
 *
 * Return value: %CAIRO_STATUS_SUCCESS if successful or
 * %CAIRO_STATUS_NO_MEMORY if insufficient memory is available.
 **/
cairo_status_t
_cairo_cache_insert (cairo_cache_t	 *cache,
		     cairo_cache_entry_t *entry)
{
    cairo_status_t status;

    if (entry->size && ! cache->freeze_count)
	_cairo_cache_shrink_to_accommodate (cache, entry->size);

    status = _cairo_hash_table_insert (cache->hash_table,
				       (cairo_hash_entry_t *) entry);
    if (unlikely (status))
	return status;

    cache->size += entry->size;

    return CAIRO_STATUS_SUCCESS;
}

/**
 * _cairo_cache_remove:
 * @cache: a cache
 * @entry: an entry that exists in the cache
 *
 * Remove an existing entry from the cache.
 **/
void
_cairo_cache_remove (cairo_cache_t	 *cache,
		     cairo_cache_entry_t *entry)
{
    cache->size -= entry->size;

    _cairo_hash_table_remove (cache->hash_table,
			      (cairo_hash_entry_t *) entry);

    if (cache->entry_destroy)
	cache->entry_destroy (entry);
}

/**
 * _cairo_cache_foreach:
 * @cache: a cache
 * @cache_callback: function to be called for each entry
 * @closure: additional argument to be passed to @cache_callback
 *
 * Call @cache_callback for each entry in the cache, in a
 * non-specified order.
 **/
void
_cairo_cache_foreach (cairo_cache_t		      *cache,
		      cairo_cache_callback_func_t      cache_callback,
		      void			      *closure)
{
    _cairo_hash_table_foreach (cache->hash_table,
			       cache_callback,
			       closure);
}

uintptr_t
_cairo_hash_string (const char *c)
{
    /* This is the djb2 hash. */
    uintptr_t hash = _CAIRO_HASH_INIT_VALUE;
    while (c && *c)
	hash = ((hash << 5) + hash) + *c++;
    return hash;
}

uintptr_t
_cairo_hash_bytes (uintptr_t hash,
		   const void *ptr,
		   unsigned int length)
{
    const uint8_t *bytes = ptr;
    /* This is the djb2 hash. */
    while (length--)
	hash = ((hash << 5) + hash) + *bytes++;
    return hash;
}
