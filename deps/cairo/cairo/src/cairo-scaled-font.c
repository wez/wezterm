/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/*
 * Copyright © 2005 Keith Packard
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
 * The Initial Developer of the Original Code is Keith Packard
 *
 * Contributor(s):
 *      Keith Packard <keithp@keithp.com>
 *	Carl D. Worth <cworth@cworth.org>
 *      Graydon Hoare <graydon@redhat.com>
 *      Owen Taylor <otaylor@redhat.com>
 *      Behdad Esfahbod <behdad@behdad.org>
 *      Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"
#include "cairo-array-private.h"
#include "cairo-error-private.h"
#include "cairo-image-surface-private.h"
#include "cairo-list-inline.h"
#include "cairo-pattern-private.h"
#include "cairo-scaled-font-private.h"
#include "cairo-surface-backend-private.h"

/**
 * SECTION:cairo-scaled-font
 * @Title: cairo_scaled_font_t
 * @Short_Description: Font face at particular size and options
 * @See_Also: #cairo_font_face_t, #cairo_matrix_t, #cairo_font_options_t
 *
 * #cairo_scaled_font_t represents a realization of a font face at a particular
 * size and transformation and a certain set of font options.
 **/

static uintptr_t
_cairo_scaled_font_compute_hash (cairo_scaled_font_t *scaled_font);

/* Global Glyph Cache
 *
 * We maintain a global pool of glyphs split between all active fonts. This
 * allows a heavily used individual font to cache more glyphs than we could
 * manage if we used per-font glyph caches, but at the same time maintains
 * fairness across all fonts and provides a cap on the maximum number of
 * global glyphs.
 *
 * The glyphs are allocated in pages, which are capped in the global pool.
 * Using pages means we can reduce the frequency at which we have to probe the
 * global pool and ameliorates the memory allocation pressure.
 */

/* XXX: This number is arbitrary---we've never done any measurement of this. */
#define MAX_GLYPH_PAGES_CACHED 512
static cairo_cache_t cairo_scaled_glyph_page_cache;

#define CAIRO_SCALED_GLYPH_PAGE_SIZE 32
struct _cairo_scaled_glyph_page {
    cairo_cache_entry_t cache_entry;
    cairo_scaled_font_t *scaled_font;
    cairo_list_t link;

    unsigned int num_glyphs;
    cairo_scaled_glyph_t glyphs[CAIRO_SCALED_GLYPH_PAGE_SIZE];
};

/*
 *  Notes:
 *
 *  To store rasterizations of glyphs, we use an image surface and the
 *  device offset to represent the glyph origin.
 *
 *  A device_transform converts from device space (a conceptual space) to
 *  surface space.  For simple cases of translation only, it's called a
 *  device_offset and is public API (cairo_surface_[gs]et_device_offset()).
 *  A possibly better name for those functions could have been
 *  cairo_surface_[gs]et_origin().  So, that's what they do: they set where
 *  the device-space origin (0,0) is in the surface.  If the origin is inside
 *  the surface, device_offset values are positive.  It may look like this:
 *
 *  Device space:
 *        (-x,-y) <-- negative numbers
 *           +----------------+
 *           |      .         |
 *           |      .         |
 *           |......(0,0) <---|-- device-space origin
 *           |                |
 *           |                |
 *           +----------------+
 *                    (width-x,height-y)
 *
 *  Surface space:
 *         (0,0) <-- surface-space origin
 *           +---------------+
 *           |      .        |
 *           |      .        |
 *           |......(x,y) <--|-- device_offset
 *           |               |
 *           |               |
 *           +---------------+
 *                     (width,height)
 *
 *  In other words: device_offset is the coordinates of the device-space
 *  origin relative to the top-left of the surface.
 *
 *  We use device offsets in a couple of places:
 *
 *    - Public API: To let toolkits like Gtk+ give user a surface that
 *      only represents part of the final destination (say, the expose
 *      area), but has the same device space as the destination.  In these
 *      cases device_offset is typically negative.  Example:
 *
 *           application window
 *           +---------------+
 *           |      .        |
 *           | (x,y).        |
 *           |......+---+    |
 *           |      |   | <--|-- expose area
 *           |      +---+    |
 *           +---------------+
 *
 *      In this case, the user of cairo API can set the device_space on
 *      the expose area to (-x,-y) to move the device space origin to that
 *      of the application window, such that drawing in the expose area
 *      surface and painting it in the application window has the same
 *      effect as drawing in the application window directly.  Gtk+ has
 *      been using this feature.
 *
 *    - Glyph surfaces: In most font rendering systems, glyph surfaces
 *      have an origin at (0,0) and a bounding box that is typically
 *      represented as (x_bearing,y_bearing,width,height).  Depending on
 *      which way y progresses in the system, y_bearing may typically be
 *      negative (for systems similar to cairo, with origin at top left),
 *      or be positive (in systems like PDF with origin at bottom left).
 *      No matter which is the case, it is important to note that
 *      (x_bearing,y_bearing) is the coordinates of top-left of the glyph
 *      relative to the glyph origin.  That is, for example:
 *
 *      Scaled-glyph space:
 *
 *        (x_bearing,y_bearing) <-- negative numbers
 *           +----------------+
 *           |      .         |
 *           |      .         |
 *           |......(0,0) <---|-- glyph origin
 *           |                |
 *           |                |
 *           +----------------+
 *                    (width+x_bearing,height+y_bearing)
 *
 *      Note the similarity of the origin to the device space.  That is
 *      exactly how we use the device_offset to represent scaled glyphs:
 *      to use the device-space origin as the glyph origin.
 *
 *  Now compare the scaled-glyph space to device-space and surface-space
 *  and convince yourself that:
 *
 *	(x_bearing,y_bearing) = (-x,-y) = - device_offset
 *
 *  That's right.  If you are not convinced yet, contrast the definition
 *  of the two:
 *
 *	"(x_bearing,y_bearing) is the coordinates of top-left of the
 *	 glyph relative to the glyph origin."
 *
 *	"In other words: device_offset is the coordinates of the
 *	 device-space origin relative to the top-left of the surface."
 *
 *  and note that glyph origin = device-space origin.
 */

static void
_cairo_scaled_font_fini_internal (cairo_scaled_font_t *scaled_font);

static void
_cairo_scaled_glyph_fini (cairo_scaled_font_t *scaled_font,
			  cairo_scaled_glyph_t *scaled_glyph)
{
    while (! cairo_list_is_empty (&scaled_glyph->dev_privates)) {
	cairo_scaled_glyph_private_t *private =
	    cairo_list_first_entry (&scaled_glyph->dev_privates,
				    cairo_scaled_glyph_private_t,
				    link);
	private->destroy (private, scaled_glyph, scaled_font);
    }

    _cairo_image_scaled_glyph_fini (scaled_font, scaled_glyph);

    if (scaled_glyph->surface != NULL)
	cairo_surface_destroy (&scaled_glyph->surface->base);

    if (scaled_glyph->path != NULL)
	_cairo_path_fixed_destroy (scaled_glyph->path);

    if (scaled_glyph->recording_surface != NULL) {
	cairo_status_t status;

	/* If the recording surface contains other fonts, destroying
	 * it while holding _cairo_scaled_glyph_page_cache_mutex will
	 * result in deadlock when the recording surface font is
	 * destroyed. Instead, move the recording surface to a list of
	 * surfaces to free and free it in
	 * _cairo_scaled_font_thaw_cache() after
	 * _cairo_scaled_glyph_page_cache_mutex is unlocked. */
	status = _cairo_array_append (&scaled_font->recording_surfaces_to_free, &scaled_glyph->recording_surface);
	assert (status == CAIRO_STATUS_SUCCESS);
    }

    if (scaled_glyph->color_surface != NULL)
	cairo_surface_destroy (&scaled_glyph->color_surface->base);
}

#define ZOMBIE 0
static const cairo_scaled_font_t _cairo_scaled_font_nil = {
    { ZOMBIE },			/* hash_entry */
    CAIRO_STATUS_NO_MEMORY,	/* status */
    CAIRO_REFERENCE_COUNT_INVALID,	/* ref_count */
    { 0, 0, 0, NULL },		/* user_data */
    NULL,			/* original_font_face */
    NULL,			/* font_face */
    { 1., 0., 0., 1., 0, 0},	/* font_matrix */
    { 1., 0., 0., 1., 0, 0},	/* ctm */
    { CAIRO_ANTIALIAS_DEFAULT,	/* options */
      CAIRO_SUBPIXEL_ORDER_DEFAULT,
      CAIRO_HINT_STYLE_DEFAULT,
      CAIRO_HINT_METRICS_DEFAULT} ,
    FALSE,			/* placeholder */
    FALSE,			/* holdover */
    TRUE,			/* finished */
    { 1., 0., 0., 1., 0, 0},	/* scale */
    { 1., 0., 0., 1., 0, 0},	/* scale_inverse */
    1.,				/* max_scale */
    { 0., 0., 0., 0., 0. },	/* extents */
    { 0., 0., 0., 0., 0. },	/* fs_extents */
    CAIRO_MUTEX_NIL_INITIALIZER,/* mutex */
    NULL,			/* glyphs */
    { NULL, NULL },		/* pages */
    FALSE,			/* cache_frozen */
    FALSE,			/* global_cache_frozen */
    { 0, 0, sizeof(cairo_surface_t*), NULL }, /* recording_surfaces_to_free */
    { NULL, NULL },		/* privates */
    NULL			/* backend */
};

/**
 * _cairo_scaled_font_set_error:
 * @scaled_font: a scaled_font
 * @status: a status value indicating an error
 *
 * Atomically sets scaled_font->status to @status and calls _cairo_error;
 * Does nothing if status is %CAIRO_STATUS_SUCCESS.
 *
 * All assignments of an error status to scaled_font->status should happen
 * through _cairo_scaled_font_set_error(). Note that due to the nature of
 * the atomic operation, it is not safe to call this function on the nil
 * objects.
 *
 * The purpose of this function is to allow the user to set a
 * breakpoint in _cairo_error() to generate a stack trace for when the
 * user causes cairo to detect an error.
 *
 * Return value: the error status.
 **/
cairo_status_t
_cairo_scaled_font_set_error (cairo_scaled_font_t *scaled_font,
			      cairo_status_t status)
{
    if (status == CAIRO_STATUS_SUCCESS)
	return status;

    /* Don't overwrite an existing error. This preserves the first
     * error, which is the most significant. */
    _cairo_status_set_error (&scaled_font->status, status);

    return _cairo_error (status);
}

/**
 * cairo_scaled_font_get_type:
 * @scaled_font: a #cairo_scaled_font_t
 *
 * This function returns the type of the backend used to create
 * a scaled font. See #cairo_font_type_t for available types.
 * However, this function never returns %CAIRO_FONT_TYPE_TOY.
 *
 * Return value: The type of @scaled_font.
 *
 * Since: 1.2
 **/
cairo_font_type_t
cairo_scaled_font_get_type (cairo_scaled_font_t *scaled_font)
{
    if (CAIRO_REFERENCE_COUNT_IS_INVALID (&scaled_font->ref_count))
	return CAIRO_FONT_TYPE_TOY;

    return scaled_font->backend->type;
}

/**
 * cairo_scaled_font_status:
 * @scaled_font: a #cairo_scaled_font_t
 *
 * Checks whether an error has previously occurred for this
 * scaled_font.
 *
 * Return value: %CAIRO_STATUS_SUCCESS or another error such as
 *   %CAIRO_STATUS_NO_MEMORY.
 *
 * Since: 1.0
 **/
cairo_status_t
cairo_scaled_font_status (cairo_scaled_font_t *scaled_font)
{
    return scaled_font->status;
}
slim_hidden_def (cairo_scaled_font_status);

/* Here we keep a unique mapping from
 * font_face/matrix/ctm/font_options => #cairo_scaled_font_t.
 *
 * Here are the things that we want to map:
 *
 *  a) All otherwise referenced #cairo_scaled_font_t's
 *  b) Some number of not otherwise referenced #cairo_scaled_font_t's
 *
 * The implementation uses a hash table which covers (a)
 * completely. Then, for (b) we have an array of otherwise
 * unreferenced fonts (holdovers) which are expired in
 * least-recently-used order.
 *
 * The cairo_scaled_font_create() code gets to treat this like a regular
 * hash table. All of the magic for the little holdover cache is in
 * cairo_scaled_font_reference() and cairo_scaled_font_destroy().
 */

/* This defines the size of the holdover array ... that is, the number
 * of scaled fonts we keep around even when not otherwise referenced
 */
#define CAIRO_SCALED_FONT_MAX_HOLDOVERS 256

typedef struct _cairo_scaled_font_map {
    cairo_scaled_font_t *mru_scaled_font;
    cairo_hash_table_t *hash_table;
    cairo_scaled_font_t *holdovers[CAIRO_SCALED_FONT_MAX_HOLDOVERS];
    int num_holdovers;
} cairo_scaled_font_map_t;

static cairo_scaled_font_map_t *cairo_scaled_font_map;

static int
_cairo_scaled_font_keys_equal (const void *abstract_key_a, const void *abstract_key_b);

static cairo_scaled_font_map_t *
_cairo_scaled_font_map_lock (void)
{
    CAIRO_MUTEX_LOCK (_cairo_scaled_font_map_mutex);

    if (cairo_scaled_font_map == NULL) {
	cairo_scaled_font_map = _cairo_malloc (sizeof (cairo_scaled_font_map_t));
	if (unlikely (cairo_scaled_font_map == NULL))
	    goto CLEANUP_MUTEX_LOCK;

	cairo_scaled_font_map->mru_scaled_font = NULL;
	cairo_scaled_font_map->hash_table =
	    _cairo_hash_table_create (_cairo_scaled_font_keys_equal);

	if (unlikely (cairo_scaled_font_map->hash_table == NULL))
	    goto CLEANUP_SCALED_FONT_MAP;

	cairo_scaled_font_map->num_holdovers = 0;
    }

    return cairo_scaled_font_map;

 CLEANUP_SCALED_FONT_MAP:
    free (cairo_scaled_font_map);
    cairo_scaled_font_map = NULL;
 CLEANUP_MUTEX_LOCK:
    CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_map_mutex);
    _cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
    return NULL;
}

static void
_cairo_scaled_font_map_unlock (void)
{
   CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_map_mutex);
}

void
_cairo_scaled_font_map_destroy (void)
{
    cairo_scaled_font_map_t *font_map;
    cairo_scaled_font_t *scaled_font;

    CAIRO_MUTEX_LOCK (_cairo_scaled_font_map_mutex);

    font_map = cairo_scaled_font_map;
    if (unlikely (font_map == NULL)) {
        goto CLEANUP_MUTEX_LOCK;
    }

    scaled_font = font_map->mru_scaled_font;
    if (scaled_font != NULL) {
	CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_map_mutex);
	cairo_scaled_font_destroy (scaled_font);
	CAIRO_MUTEX_LOCK (_cairo_scaled_font_map_mutex);
    }

    /* remove scaled_fonts starting from the end so that font_map->holdovers
     * is always in a consistent state when we release the mutex. */
    while (font_map->num_holdovers) {
	scaled_font = font_map->holdovers[font_map->num_holdovers-1];
	assert (! CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&scaled_font->ref_count));
	_cairo_hash_table_remove (font_map->hash_table,
				  &scaled_font->hash_entry);

	font_map->num_holdovers--;

	/* This releases the font_map lock to avoid the possibility of a
	 * recursive deadlock when the scaled font destroy closure gets
	 * called
	 */
	_cairo_scaled_font_fini (scaled_font);

	free (scaled_font);
    }

    _cairo_hash_table_destroy (font_map->hash_table);

    free (cairo_scaled_font_map);
    cairo_scaled_font_map = NULL;

 CLEANUP_MUTEX_LOCK:
    CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_map_mutex);
}

static void
_cairo_scaled_glyph_page_destroy (cairo_scaled_font_t *scaled_font,
				  cairo_scaled_glyph_page_t *page)
{
    unsigned int n;

    assert (!scaled_font->cache_frozen);
    assert (!scaled_font->global_cache_frozen);

    for (n = 0; n < page->num_glyphs; n++) {
	_cairo_hash_table_remove (scaled_font->glyphs,
				  &page->glyphs[n].hash_entry);
	_cairo_scaled_glyph_fini (scaled_font, &page->glyphs[n]);
    }

    cairo_list_del (&page->link);
    free (page);
}

static void
_cairo_scaled_glyph_page_pluck (void *closure)
{
    cairo_scaled_glyph_page_t *page = closure;
    cairo_scaled_font_t *scaled_font;

    assert (! cairo_list_is_empty (&page->link));

    scaled_font = page->scaled_font;

    /* The font is locked in _cairo_scaled_glyph_page_can_remove () */
    _cairo_scaled_glyph_page_destroy (scaled_font, page);
    CAIRO_MUTEX_UNLOCK (scaled_font->mutex);
}

/* If a scaled font wants to unlock the font map while still being
 * created (needed for user-fonts), we need to take extra care not
 * ending up with multiple identical scaled fonts being created.
 *
 * What we do is, we create a fake identical scaled font, and mark
 * it as placeholder, lock its mutex, and insert that in the fontmap
 * hash table.  This makes other code trying to create an identical
 * scaled font to just wait and retry.
 *
 * The reason we have to create a fake scaled font instead of just using
 * scaled_font is for lifecycle management: we need to (or rather,
 * other code needs to) reference the scaled_font in the hash table.
 * We can't do that on the input scaled_font as it may be freed by
 * font backend upon error.
 */

cairo_status_t
_cairo_scaled_font_register_placeholder_and_unlock_font_map (cairo_scaled_font_t *scaled_font)
{
    cairo_status_t status;
    cairo_scaled_font_t *placeholder_scaled_font;

    assert (CAIRO_MUTEX_IS_LOCKED (_cairo_scaled_font_map_mutex));

    status = scaled_font->status;
    if (unlikely (status))
	return status;

    placeholder_scaled_font = _cairo_malloc (sizeof (cairo_scaled_font_t));
    if (unlikely (placeholder_scaled_font == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    /* full initialization is wasteful, but who cares... */
    status = _cairo_scaled_font_init (placeholder_scaled_font,
				      scaled_font->font_face,
				      &scaled_font->font_matrix,
				      &scaled_font->ctm,
				      &scaled_font->options,
				      NULL);
    if (unlikely (status))
	goto FREE_PLACEHOLDER;

    placeholder_scaled_font->placeholder = TRUE;

    placeholder_scaled_font->hash_entry.hash
	= _cairo_scaled_font_compute_hash (placeholder_scaled_font);
    status = _cairo_hash_table_insert (cairo_scaled_font_map->hash_table,
				       &placeholder_scaled_font->hash_entry);
    if (unlikely (status))
	goto FINI_PLACEHOLDER;

    CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_map_mutex);
    CAIRO_MUTEX_LOCK (placeholder_scaled_font->mutex);

    return CAIRO_STATUS_SUCCESS;

  FINI_PLACEHOLDER:
    _cairo_scaled_font_fini_internal (placeholder_scaled_font);
  FREE_PLACEHOLDER:
    free (placeholder_scaled_font);

    return _cairo_scaled_font_set_error (scaled_font, status);
}

void
_cairo_scaled_font_unregister_placeholder_and_lock_font_map (cairo_scaled_font_t *scaled_font)
{
    cairo_scaled_font_t *placeholder_scaled_font;

    CAIRO_MUTEX_LOCK (_cairo_scaled_font_map_mutex);

    /* temporary hash value to match the placeholder */
    scaled_font->hash_entry.hash
	= _cairo_scaled_font_compute_hash (scaled_font);
    placeholder_scaled_font =
	_cairo_hash_table_lookup (cairo_scaled_font_map->hash_table,
				  &scaled_font->hash_entry);
    assert (placeholder_scaled_font != NULL);
    assert (placeholder_scaled_font->placeholder);
    assert (CAIRO_MUTEX_IS_LOCKED (placeholder_scaled_font->mutex));

    _cairo_hash_table_remove (cairo_scaled_font_map->hash_table,
			      &placeholder_scaled_font->hash_entry);

    CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_map_mutex);

    CAIRO_MUTEX_UNLOCK (placeholder_scaled_font->mutex);
    cairo_scaled_font_destroy (placeholder_scaled_font);

    CAIRO_MUTEX_LOCK (_cairo_scaled_font_map_mutex);
}

static void
_cairo_scaled_font_placeholder_wait_for_creation_to_finish (cairo_scaled_font_t *placeholder_scaled_font)
{
    /* reference the place holder so it doesn't go away */
    cairo_scaled_font_reference (placeholder_scaled_font);

    /* now unlock the fontmap mutex so creation has a chance to finish */
    CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_map_mutex);

    /* wait on placeholder mutex until we are awaken */
    CAIRO_MUTEX_LOCK (placeholder_scaled_font->mutex);

    /* ok, creation done.  just clean up and back out */
    CAIRO_MUTEX_UNLOCK (placeholder_scaled_font->mutex);
    cairo_scaled_font_destroy (placeholder_scaled_font);

    CAIRO_MUTEX_LOCK (_cairo_scaled_font_map_mutex);
}

/* Fowler / Noll / Vo (FNV) Hash (http://www.isthe.com/chongo/tech/comp/fnv/)
 *
 * Not necessarily better than a lot of other hashes, but should be OK, and
 * well tested with binary data.
 */

#define FNV_64_PRIME ((uint64_t)0x00000100000001B3)
#define FNV1_64_INIT ((uint64_t)0xcbf29ce484222325)

static uint64_t
_hash_matrix_fnv (const cairo_matrix_t	*matrix,
		  uint64_t		 hval)
{
    const uint8_t *buffer = (const uint8_t *) matrix;
    int len = sizeof (cairo_matrix_t);
    do {
	hval *= FNV_64_PRIME;
	hval ^= *buffer++;
    } while (--len);

    return hval;
}

static uint64_t
_hash_mix_bits (uint64_t hash)
{
    hash += hash << 12;
    hash ^= hash >> 7;
    hash += hash << 3;
    hash ^= hash >> 17;
    hash += hash << 5;
    return hash;
}

static uintptr_t
_cairo_scaled_font_compute_hash (cairo_scaled_font_t *scaled_font)
{
    uint64_t hash = FNV1_64_INIT;

    /* We do a bytewise hash on the font matrices */
    hash = _hash_matrix_fnv (&scaled_font->font_matrix, hash);
    hash = _hash_matrix_fnv (&scaled_font->ctm, hash);
    hash = _hash_mix_bits (hash);

    hash ^= (uintptr_t) scaled_font->original_font_face;
    hash ^= cairo_font_options_hash (&scaled_font->options);

    /* final mixing of bits */
    hash = _hash_mix_bits (hash);
    assert (hash != ZOMBIE);

    return hash;
}

static void
_cairo_scaled_font_init_key (cairo_scaled_font_t        *scaled_font,
			     cairo_font_face_t	        *font_face,
			     const cairo_matrix_t       *font_matrix,
			     const cairo_matrix_t       *ctm,
			     const cairo_font_options_t *options)
{
    scaled_font->status = CAIRO_STATUS_SUCCESS;
    scaled_font->placeholder = FALSE;
    scaled_font->font_face = font_face;
    scaled_font->original_font_face = font_face;
    scaled_font->font_matrix = *font_matrix;
    scaled_font->ctm = *ctm;
    /* ignore translation values in the ctm */
    scaled_font->ctm.x0 = 0.;
    scaled_font->ctm.y0 = 0.;
    _cairo_font_options_init_copy (&scaled_font->options, options);

    scaled_font->hash_entry.hash =
	_cairo_scaled_font_compute_hash (scaled_font);
}

static cairo_bool_t
_cairo_scaled_font_keys_equal (const void *abstract_key_a,
			       const void *abstract_key_b)
{
    const cairo_scaled_font_t *key_a = abstract_key_a;
    const cairo_scaled_font_t *key_b = abstract_key_b;

    return key_a->original_font_face == key_b->original_font_face &&
	    memcmp ((unsigned char *)(&key_a->font_matrix.xx),
		    (unsigned char *)(&key_b->font_matrix.xx),
		    sizeof(cairo_matrix_t)) == 0 &&
	    memcmp ((unsigned char *)(&key_a->ctm.xx),
		    (unsigned char *)(&key_b->ctm.xx),
		    sizeof(cairo_matrix_t)) == 0 &&
	    cairo_font_options_equal (&key_a->options, &key_b->options);
}

static cairo_bool_t
_cairo_scaled_font_matches (const cairo_scaled_font_t *scaled_font,
	                    const cairo_font_face_t *font_face,
			    const cairo_matrix_t *font_matrix,
			    const cairo_matrix_t *ctm,
			    const cairo_font_options_t *options)
{
    return scaled_font->original_font_face == font_face &&
	    memcmp ((unsigned char *)(&scaled_font->font_matrix.xx),
		    (unsigned char *)(&font_matrix->xx),
		    sizeof(cairo_matrix_t)) == 0 &&
	    memcmp ((unsigned char *)(&scaled_font->ctm.xx),
		    (unsigned char *)(&ctm->xx),
		    sizeof(cairo_matrix_t)) == 0 &&
	    cairo_font_options_equal (&scaled_font->options, options);
}

/*
 * Basic #cairo_scaled_font_t object management
 */

cairo_status_t
_cairo_scaled_font_init (cairo_scaled_font_t               *scaled_font,
			 cairo_font_face_t		   *font_face,
			 const cairo_matrix_t              *font_matrix,
			 const cairo_matrix_t              *ctm,
			 const cairo_font_options_t	   *options,
			 const cairo_scaled_font_backend_t *backend)
{
    cairo_status_t status;

    status = cairo_font_options_status ((cairo_font_options_t *) options);
    if (unlikely (status))
	return status;

    scaled_font->status = CAIRO_STATUS_SUCCESS;
    scaled_font->placeholder = FALSE;
    scaled_font->font_face = font_face;
    scaled_font->original_font_face = font_face;
    scaled_font->font_matrix = *font_matrix;
    scaled_font->ctm = *ctm;
    /* ignore translation values in the ctm */
    scaled_font->ctm.x0 = 0.;
    scaled_font->ctm.y0 = 0.;
    _cairo_font_options_init_copy (&scaled_font->options, options);

    cairo_matrix_multiply (&scaled_font->scale,
			   &scaled_font->font_matrix,
			   &scaled_font->ctm);

    scaled_font->max_scale = MAX (fabs (scaled_font->scale.xx) + fabs (scaled_font->scale.xy),
				  fabs (scaled_font->scale.yx) + fabs (scaled_font->scale.yy));
    scaled_font->scale_inverse = scaled_font->scale;
    status = cairo_matrix_invert (&scaled_font->scale_inverse);
    if (unlikely (status)) {
	/* If the font scale matrix is rank 0, just using an all-zero inverse matrix
	 * makes everything work correctly.  This make font size 0 work without
	 * producing an error.
	 *
	 * FIXME:  If the scale is rank 1, we still go into error mode.  But then
	 * again, that's what we do everywhere in cairo.
	 *
	 * Also, the check for == 0. below may be too harsh...
	 */
        if (_cairo_matrix_is_scale_0 (&scaled_font->scale)) {
	    cairo_matrix_init (&scaled_font->scale_inverse,
			       0, 0, 0, 0,
			       -scaled_font->scale.x0,
			       -scaled_font->scale.y0);
	} else
	    return status;
    }

    scaled_font->glyphs = _cairo_hash_table_create (NULL);
    if (unlikely (scaled_font->glyphs == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    cairo_list_init (&scaled_font->glyph_pages);
    scaled_font->cache_frozen = FALSE;
    scaled_font->global_cache_frozen = FALSE;
    _cairo_array_init (&scaled_font->recording_surfaces_to_free, sizeof (cairo_surface_t *));

    scaled_font->holdover = FALSE;
    scaled_font->finished = FALSE;

    CAIRO_REFERENCE_COUNT_INIT (&scaled_font->ref_count, 1);

    _cairo_user_data_array_init (&scaled_font->user_data);

    cairo_font_face_reference (font_face);
    scaled_font->original_font_face = NULL;

    CAIRO_RECURSIVE_MUTEX_INIT (scaled_font->mutex);

    cairo_list_init (&scaled_font->dev_privates);

    scaled_font->backend = backend;
    cairo_list_init (&scaled_font->link);

    return CAIRO_STATUS_SUCCESS;
}

static void _cairo_scaled_font_free_recording_surfaces (cairo_scaled_font_t *scaled_font)
{
    int num_recording_surfaces;
    cairo_surface_t *surface;

    num_recording_surfaces = _cairo_array_num_elements (&scaled_font->recording_surfaces_to_free);
    if (num_recording_surfaces > 0) {
	for (int i = 0; i < num_recording_surfaces; i++) {
	    _cairo_array_copy_element (&scaled_font->recording_surfaces_to_free, i, &surface);
	    cairo_surface_finish (surface);
	    cairo_surface_destroy (surface);
	}
	_cairo_array_truncate (&scaled_font->recording_surfaces_to_free, 0);
    }
}

void
_cairo_scaled_font_freeze_cache (cairo_scaled_font_t *scaled_font)
{
    /* ensure we do not modify an error object */
    assert (scaled_font->status == CAIRO_STATUS_SUCCESS);

    CAIRO_MUTEX_LOCK (scaled_font->mutex);
    scaled_font->cache_frozen = TRUE;
}

void
_cairo_scaled_font_thaw_cache (cairo_scaled_font_t *scaled_font)
{
    assert (scaled_font->cache_frozen);

    if (scaled_font->global_cache_frozen) {
	CAIRO_MUTEX_LOCK (_cairo_scaled_glyph_page_cache_mutex);
	_cairo_cache_thaw (&cairo_scaled_glyph_page_cache);
	CAIRO_MUTEX_UNLOCK (_cairo_scaled_glyph_page_cache_mutex);
	scaled_font->global_cache_frozen = FALSE;
    }

    _cairo_scaled_font_free_recording_surfaces (scaled_font);

    scaled_font->cache_frozen = FALSE;
    CAIRO_MUTEX_UNLOCK (scaled_font->mutex);
}

void
_cairo_scaled_font_reset_cache (cairo_scaled_font_t *scaled_font)
{
    cairo_scaled_glyph_page_t *page;

    CAIRO_MUTEX_LOCK (scaled_font->mutex);
    assert (! scaled_font->cache_frozen);
    assert (! scaled_font->global_cache_frozen);
    CAIRO_MUTEX_LOCK (_cairo_scaled_glyph_page_cache_mutex);

    cairo_list_foreach_entry (page,
			      cairo_scaled_glyph_page_t,
			      &scaled_font->glyph_pages,
			      link) {
	cairo_scaled_glyph_page_cache.size -= page->cache_entry.size;
	_cairo_hash_table_remove (cairo_scaled_glyph_page_cache.hash_table,
				  (cairo_hash_entry_t *) &page->cache_entry);
    }

    CAIRO_MUTEX_UNLOCK (_cairo_scaled_glyph_page_cache_mutex);

    /* Destroy scaled_font's pages while holding its lock only, and not the
     * global page cache lock. The destructor can cause us to recurse and
     * end up back here for a different scaled_font. */

    while (! cairo_list_is_empty (&scaled_font->glyph_pages)) {
	page = cairo_list_first_entry (&scaled_font->glyph_pages,
				       cairo_scaled_glyph_page_t,
				       link);
	_cairo_scaled_glyph_page_destroy (scaled_font, page);
    }

    CAIRO_MUTEX_UNLOCK (scaled_font->mutex);
}

cairo_status_t
_cairo_scaled_font_set_metrics (cairo_scaled_font_t	    *scaled_font,
				cairo_font_extents_t	    *fs_metrics)
{
    cairo_status_t status;
    double  font_scale_x, font_scale_y;

    scaled_font->fs_extents = *fs_metrics;

    status = _cairo_matrix_compute_basis_scale_factors (&scaled_font->font_matrix,
						  &font_scale_x, &font_scale_y,
						  1);
    if (unlikely (status))
	return status;

    /*
     * The font responded in unscaled units, scale by the font
     * matrix scale factors to get to user space
     */

    scaled_font->extents.ascent = fs_metrics->ascent * font_scale_y;
    scaled_font->extents.descent = fs_metrics->descent * font_scale_y;
    scaled_font->extents.height = fs_metrics->height * font_scale_y;
    scaled_font->extents.max_x_advance = fs_metrics->max_x_advance * font_scale_x;
    scaled_font->extents.max_y_advance = fs_metrics->max_y_advance * font_scale_y;

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_scaled_font_fini_internal (cairo_scaled_font_t *scaled_font)
{
    assert (! scaled_font->cache_frozen);
    assert (! scaled_font->global_cache_frozen);
    scaled_font->finished = TRUE;

    _cairo_scaled_font_reset_cache (scaled_font);
    _cairo_hash_table_destroy (scaled_font->glyphs);

    cairo_font_face_destroy (scaled_font->font_face);
    cairo_font_face_destroy (scaled_font->original_font_face);

    _cairo_scaled_font_free_recording_surfaces (scaled_font);
    _cairo_array_fini (&scaled_font->recording_surfaces_to_free);

    CAIRO_MUTEX_FINI (scaled_font->mutex);

    while (! cairo_list_is_empty (&scaled_font->dev_privates)) {
	cairo_scaled_font_private_t *private =
	    cairo_list_first_entry (&scaled_font->dev_privates,
				    cairo_scaled_font_private_t,
				    link);
	private->destroy (private, scaled_font);
    }

    if (scaled_font->backend != NULL && scaled_font->backend->fini != NULL)
	scaled_font->backend->fini (scaled_font);

    _cairo_user_data_array_fini (&scaled_font->user_data);
}

void
_cairo_scaled_font_fini (cairo_scaled_font_t *scaled_font)
{
    /* Release the lock to avoid the possibility of a recursive
     * deadlock when the scaled font destroy closure gets called. */
    CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_map_mutex);
    _cairo_scaled_font_fini_internal (scaled_font);
    CAIRO_MUTEX_LOCK (_cairo_scaled_font_map_mutex);
}

void
_cairo_scaled_font_attach_private (cairo_scaled_font_t *scaled_font,
				   cairo_scaled_font_private_t *private,
				   const void *key,
				   void (*destroy) (cairo_scaled_font_private_t *,
						    cairo_scaled_font_t *))
{
    private->key = key;
    private->destroy = destroy;
    cairo_list_add (&private->link, &scaled_font->dev_privates);
}

cairo_scaled_font_private_t *
_cairo_scaled_font_find_private (cairo_scaled_font_t *scaled_font,
				 const void *key)
{
    cairo_scaled_font_private_t *priv;

    cairo_list_foreach_entry (priv, cairo_scaled_font_private_t,
			      &scaled_font->dev_privates, link)
    {
	if (priv->key == key) {
	    if (priv->link.prev != &scaled_font->dev_privates)
		cairo_list_move (&priv->link, &scaled_font->dev_privates);
	    return priv;
	}
    }

    return NULL;
}

void
_cairo_scaled_glyph_attach_private (cairo_scaled_glyph_t *scaled_glyph,
				   cairo_scaled_glyph_private_t *private,
				   const void *key,
				   void (*destroy) (cairo_scaled_glyph_private_t *,
						    cairo_scaled_glyph_t *,
						    cairo_scaled_font_t *))
{
    private->key = key;
    private->destroy = destroy;
    cairo_list_add (&private->link, &scaled_glyph->dev_privates);
}

cairo_scaled_glyph_private_t *
_cairo_scaled_glyph_find_private (cairo_scaled_glyph_t *scaled_glyph,
				 const void *key)
{
    cairo_scaled_glyph_private_t *priv;

    cairo_list_foreach_entry (priv, cairo_scaled_glyph_private_t,
			      &scaled_glyph->dev_privates, link)
    {
	if (priv->key == key) {
	    if (priv->link.prev != &scaled_glyph->dev_privates)
		cairo_list_move (&priv->link, &scaled_glyph->dev_privates);
	    return priv;
	}
    }

    return NULL;
}

/**
 * cairo_scaled_font_create:
 * @font_face: a #cairo_font_face_t
 * @font_matrix: font space to user space transformation matrix for the
 *       font. In the simplest case of a N point font, this matrix is
 *       just a scale by N, but it can also be used to shear the font
 *       or stretch it unequally along the two axes. See
 *       cairo_set_font_matrix().
 * @ctm: user to device transformation matrix with which the font will
 *       be used.
 * @options: options to use when getting metrics for the font and
 *           rendering with it.
 *
 * Creates a #cairo_scaled_font_t object from a font face and matrices that
 * describe the size of the font and the environment in which it will
 * be used.
 *
 * Return value: a newly created #cairo_scaled_font_t. Destroy with
 *  cairo_scaled_font_destroy()
 *
 * Since: 1.0
 **/
cairo_scaled_font_t *
cairo_scaled_font_create (cairo_font_face_t          *font_face,
			  const cairo_matrix_t       *font_matrix,
			  const cairo_matrix_t       *ctm,
			  const cairo_font_options_t *options)
{
    cairo_status_t status;
    cairo_scaled_font_map_t *font_map;
    cairo_font_face_t *original_font_face = font_face;
    cairo_scaled_font_t key, *old = NULL, *scaled_font = NULL, *dead = NULL;
    double det;

    status = font_face->status;
    if (unlikely (status))
	return _cairo_scaled_font_create_in_error (status);

    det = _cairo_matrix_compute_determinant (font_matrix);
    if (! ISFINITE (det))
	return _cairo_scaled_font_create_in_error (_cairo_error (CAIRO_STATUS_INVALID_MATRIX));

    det = _cairo_matrix_compute_determinant (ctm);
    if (! ISFINITE (det))
	return _cairo_scaled_font_create_in_error (_cairo_error (CAIRO_STATUS_INVALID_MATRIX));

    status = cairo_font_options_status ((cairo_font_options_t *) options);
    if (unlikely (status))
	return _cairo_scaled_font_create_in_error (status);

    /* Note that degenerate ctm or font_matrix *are* allowed.
     * We want to support a font size of 0. */

    font_map = _cairo_scaled_font_map_lock ();
    if (unlikely (font_map == NULL))
	return _cairo_scaled_font_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    scaled_font = font_map->mru_scaled_font;
    if (scaled_font != NULL &&
	_cairo_scaled_font_matches (scaled_font,
	                            font_face, font_matrix, ctm, options))
    {
	assert (scaled_font->hash_entry.hash != ZOMBIE);
	assert (! scaled_font->placeholder);

	if (likely (scaled_font->status == CAIRO_STATUS_SUCCESS)) {
	    /* We increment the reference count manually here, (rather
	     * than calling into cairo_scaled_font_reference), since we
	     * must modify the reference count while our lock is still
	     * held. */
	    _cairo_reference_count_inc (&scaled_font->ref_count);
	    _cairo_scaled_font_map_unlock ();
	    return scaled_font;
	}

	/* the font has been put into an error status - abandon the cache */
	_cairo_hash_table_remove (font_map->hash_table,
				  &scaled_font->hash_entry);
	scaled_font->hash_entry.hash = ZOMBIE;
	dead = scaled_font;
	font_map->mru_scaled_font = NULL;
    }

    _cairo_scaled_font_init_key (&key, font_face, font_matrix, ctm, options);

    while ((scaled_font = _cairo_hash_table_lookup (font_map->hash_table,
						    &key.hash_entry)))
    {
	if (! scaled_font->placeholder)
	    break;

	/* If the scaled font is being created (happens for user-font),
	 * just wait until it's done, then retry */
	_cairo_scaled_font_placeholder_wait_for_creation_to_finish (scaled_font);
    }

    if (scaled_font != NULL) {
	/* If the original reference count is 0, then this font must have
	 * been found in font_map->holdovers, (which means this caching is
	 * actually working). So now we remove it from the holdovers
	 * array, unless we caught the font in the middle of destruction.
	 */
	if (! CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&scaled_font->ref_count)) {
	    if (scaled_font->holdover) {
		int i;

		for (i = 0; i < font_map->num_holdovers; i++) {
		    if (font_map->holdovers[i] == scaled_font) {
			font_map->num_holdovers--;
			memmove (&font_map->holdovers[i],
				 &font_map->holdovers[i+1],
				 (font_map->num_holdovers - i) * sizeof (cairo_scaled_font_t*));
			break;
		    }
		}

		scaled_font->holdover = FALSE;
	    }

	    /* reset any error status */
	    scaled_font->status = CAIRO_STATUS_SUCCESS;
	}

	if (likely (scaled_font->status == CAIRO_STATUS_SUCCESS)) {
	    /* We increment the reference count manually here, (rather
	     * than calling into cairo_scaled_font_reference), since we
	     * must modify the reference count while our lock is still
	     * held. */

	    old = font_map->mru_scaled_font;
	    font_map->mru_scaled_font = scaled_font;
	    /* increment reference count for the mru cache */
	    _cairo_reference_count_inc (&scaled_font->ref_count);
	    /* and increment for the returned reference */
	    _cairo_reference_count_inc (&scaled_font->ref_count);
	    _cairo_scaled_font_map_unlock ();

	    cairo_scaled_font_destroy (old);
	    if (font_face != original_font_face)
		cairo_font_face_destroy (font_face);

	    return scaled_font;
	}

	/* the font has been put into an error status - abandon the cache */
	_cairo_hash_table_remove (font_map->hash_table,
				  &scaled_font->hash_entry);
	scaled_font->hash_entry.hash = ZOMBIE;
    }


    /* Otherwise create it and insert it into the hash table. */
    if (font_face->backend->get_implementation != NULL) {
	font_face = font_face->backend->get_implementation (font_face,
							    font_matrix,
							    ctm,
							    options);
	if (unlikely (font_face->status)) {
	    _cairo_scaled_font_map_unlock ();
	    return _cairo_scaled_font_create_in_error (font_face->status);
	}
    }

    status = font_face->backend->scaled_font_create (font_face, font_matrix,
						     ctm, options, &scaled_font);
    if (unlikely (status)) {
	_cairo_scaled_font_map_unlock ();
	if (font_face != original_font_face)
	    cairo_font_face_destroy (font_face);

	if (dead != NULL)
	    cairo_scaled_font_destroy (dead);

	return _cairo_scaled_font_create_in_error (status);
    }
    /* Or did we encounter an error whilst constructing the scaled font? */
    if (unlikely (scaled_font->status)) {
	_cairo_scaled_font_map_unlock ();
	if (font_face != original_font_face)
	    cairo_font_face_destroy (font_face);

	if (dead != NULL)
	    cairo_scaled_font_destroy (dead);

	return scaled_font;
    }

    /* Our caching above is defeated if the backend switches fonts on us -
     * e.g. old incarnations of toy-font-face and lazily resolved
     * ft-font-faces
     */
    assert (scaled_font->font_face == font_face);
    assert (! scaled_font->cache_frozen);
    assert (! scaled_font->global_cache_frozen);

    scaled_font->original_font_face =
	cairo_font_face_reference (original_font_face);

    scaled_font->hash_entry.hash = _cairo_scaled_font_compute_hash(scaled_font);

    status = _cairo_hash_table_insert (font_map->hash_table,
				       &scaled_font->hash_entry);
    if (likely (status == CAIRO_STATUS_SUCCESS)) {
	old = font_map->mru_scaled_font;
	font_map->mru_scaled_font = scaled_font;
	_cairo_reference_count_inc (&scaled_font->ref_count);
    }

    _cairo_scaled_font_map_unlock ();

    cairo_scaled_font_destroy (old);
    if (font_face != original_font_face)
	cairo_font_face_destroy (font_face);

    if (dead != NULL)
	cairo_scaled_font_destroy (dead);

    if (unlikely (status)) {
	/* We can't call _cairo_scaled_font_destroy here since it expects
	 * that the font has already been successfully inserted into the
	 * hash table. */
	_cairo_scaled_font_fini_internal (scaled_font);
	free (scaled_font);
	return _cairo_scaled_font_create_in_error (status);
    }

    return scaled_font;
}
slim_hidden_def (cairo_scaled_font_create);

static cairo_scaled_font_t *_cairo_scaled_font_nil_objects[CAIRO_STATUS_LAST_STATUS + 1];

/* XXX This should disappear in favour of a common pool of error objects. */
cairo_scaled_font_t *
_cairo_scaled_font_create_in_error (cairo_status_t status)
{
    cairo_scaled_font_t *scaled_font;

    assert (status != CAIRO_STATUS_SUCCESS);

    if (status == CAIRO_STATUS_NO_MEMORY)
	return (cairo_scaled_font_t *) &_cairo_scaled_font_nil;

    CAIRO_MUTEX_LOCK (_cairo_scaled_font_error_mutex);
    scaled_font = _cairo_scaled_font_nil_objects[status];
    if (unlikely (scaled_font == NULL)) {
	scaled_font = _cairo_malloc (sizeof (cairo_scaled_font_t));
	if (unlikely (scaled_font == NULL)) {
	    CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_error_mutex);
	    _cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	    return (cairo_scaled_font_t *) &_cairo_scaled_font_nil;
	}

	*scaled_font = _cairo_scaled_font_nil;
	scaled_font->status = status;
	_cairo_scaled_font_nil_objects[status] = scaled_font;
    }
    CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_error_mutex);

    return scaled_font;
}

void
_cairo_scaled_font_reset_static_data (void)
{
    int status;

    CAIRO_MUTEX_LOCK (_cairo_scaled_font_error_mutex);
    for (status = CAIRO_STATUS_SUCCESS;
	 status <= CAIRO_STATUS_LAST_STATUS;
	 status++)
    {
	free (_cairo_scaled_font_nil_objects[status]);
	_cairo_scaled_font_nil_objects[status] = NULL;
    }
    CAIRO_MUTEX_UNLOCK (_cairo_scaled_font_error_mutex);

    CAIRO_MUTEX_LOCK (_cairo_scaled_glyph_page_cache_mutex);
    if (cairo_scaled_glyph_page_cache.hash_table != NULL) {
	_cairo_cache_fini (&cairo_scaled_glyph_page_cache);
	cairo_scaled_glyph_page_cache.hash_table = NULL;
    }
    CAIRO_MUTEX_UNLOCK (_cairo_scaled_glyph_page_cache_mutex);
}

/**
 * cairo_scaled_font_reference:
 * @scaled_font: a #cairo_scaled_font_t, (may be %NULL in which case
 * this function does nothing)
 *
 * Increases the reference count on @scaled_font by one. This prevents
 * @scaled_font from being destroyed until a matching call to
 * cairo_scaled_font_destroy() is made.
 *
 * Use cairo_scaled_font_get_reference_count() to get the number of
 * references to a #cairo_scaled_font_t.
 *
 * Returns: the referenced #cairo_scaled_font_t
 *
 * Since: 1.0
 **/
cairo_scaled_font_t *
cairo_scaled_font_reference (cairo_scaled_font_t *scaled_font)
{
    if (scaled_font == NULL ||
	    CAIRO_REFERENCE_COUNT_IS_INVALID (&scaled_font->ref_count))
	return scaled_font;

    assert (CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&scaled_font->ref_count));

    _cairo_reference_count_inc (&scaled_font->ref_count);

    return scaled_font;
}
slim_hidden_def (cairo_scaled_font_reference);

/**
 * cairo_scaled_font_destroy:
 * @scaled_font: a #cairo_scaled_font_t
 *
 * Decreases the reference count on @font by one. If the result
 * is zero, then @font and all associated resources are freed.
 * See cairo_scaled_font_reference().
 *
 * Since: 1.0
 **/
void
cairo_scaled_font_destroy (cairo_scaled_font_t *scaled_font)
{
    cairo_scaled_font_t *lru = NULL;
    cairo_scaled_font_map_t *font_map;

    assert (CAIRO_MUTEX_IS_UNLOCKED (_cairo_scaled_font_map_mutex));

    if (scaled_font == NULL ||
	    CAIRO_REFERENCE_COUNT_IS_INVALID (&scaled_font->ref_count))
	return;

    assert (CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&scaled_font->ref_count));

    font_map = _cairo_scaled_font_map_lock ();
    assert (font_map != NULL);

    if (! _cairo_reference_count_dec_and_test (&scaled_font->ref_count))
	goto unlock;

    assert (! scaled_font->cache_frozen);
    assert (! scaled_font->global_cache_frozen);

    /* Another thread may have resurrected the font whilst we waited */
    if (! CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&scaled_font->ref_count)) {
	if (! scaled_font->placeholder &&
	    scaled_font->hash_entry.hash != ZOMBIE)
	{
	    /* Another thread may have already inserted us into the holdovers */
	    if (scaled_font->holdover)
		goto unlock;

	    /* Rather than immediately destroying this object, we put it into
	     * the font_map->holdovers array in case it will get used again
	     * soon (and is why we must hold the lock over the atomic op on
	     * the reference count). To make room for it, we do actually
	     * destroy the least-recently-used holdover.
	     */

	    if (font_map->num_holdovers == CAIRO_SCALED_FONT_MAX_HOLDOVERS) {
		lru = font_map->holdovers[0];
		assert (! CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&lru->ref_count));

		_cairo_hash_table_remove (font_map->hash_table,
					  &lru->hash_entry);

		font_map->num_holdovers--;
		memmove (&font_map->holdovers[0],
			 &font_map->holdovers[1],
			 font_map->num_holdovers * sizeof (cairo_scaled_font_t*));
	    }

	    font_map->holdovers[font_map->num_holdovers++] = scaled_font;
	    scaled_font->holdover = TRUE;
	} else
	    lru = scaled_font;
    }

  unlock:
    _cairo_scaled_font_map_unlock ();

    /* If we pulled an item from the holdovers array, (while the font
     * map lock was held, of course), then there is no way that anyone
     * else could have acquired a reference to it. So we can now
     * safely call fini on it without any lock held. This is desirable
     * as we never want to call into any backend function with a lock
     * held. */
    if (lru != NULL) {
	_cairo_scaled_font_fini_internal (lru);
	free (lru);
    }
}
slim_hidden_def (cairo_scaled_font_destroy);

/**
 * cairo_scaled_font_get_reference_count:
 * @scaled_font: a #cairo_scaled_font_t
 *
 * Returns the current reference count of @scaled_font.
 *
 * Return value: the current reference count of @scaled_font.  If the
 * object is a nil object, 0 will be returned.
 *
 * Since: 1.4
 **/
unsigned int
cairo_scaled_font_get_reference_count (cairo_scaled_font_t *scaled_font)
{
    if (scaled_font == NULL ||
	    CAIRO_REFERENCE_COUNT_IS_INVALID (&scaled_font->ref_count))
	return 0;

    return CAIRO_REFERENCE_COUNT_GET_VALUE (&scaled_font->ref_count);
}

/**
 * cairo_scaled_font_get_user_data:
 * @scaled_font: a #cairo_scaled_font_t
 * @key: the address of the #cairo_user_data_key_t the user data was
 * attached to
 *
 * Return user data previously attached to @scaled_font using the
 * specified key.  If no user data has been attached with the given
 * key this function returns %NULL.
 *
 * Return value: the user data previously attached or %NULL.
 *
 * Since: 1.4
 **/
void *
cairo_scaled_font_get_user_data (cairo_scaled_font_t	     *scaled_font,
				 const cairo_user_data_key_t *key)
{
    return _cairo_user_data_array_get_data (&scaled_font->user_data,
					    key);
}
slim_hidden_def (cairo_scaled_font_get_user_data);

/**
 * cairo_scaled_font_set_user_data:
 * @scaled_font: a #cairo_scaled_font_t
 * @key: the address of a #cairo_user_data_key_t to attach the user data to
 * @user_data: the user data to attach to the #cairo_scaled_font_t
 * @destroy: a #cairo_destroy_func_t which will be called when the
 * #cairo_t is destroyed or when new user data is attached using the
 * same key.
 *
 * Attach user data to @scaled_font.  To remove user data from a surface,
 * call this function with the key that was used to set it and %NULL
 * for @data.
 *
 * Return value: %CAIRO_STATUS_SUCCESS or %CAIRO_STATUS_NO_MEMORY if a
 * slot could not be allocated for the user data.
 *
 * Since: 1.4
 **/
cairo_status_t
cairo_scaled_font_set_user_data (cairo_scaled_font_t	     *scaled_font,
				 const cairo_user_data_key_t *key,
				 void			     *user_data,
				 cairo_destroy_func_t	      destroy)
{
    if (CAIRO_REFERENCE_COUNT_IS_INVALID (&scaled_font->ref_count))
	return scaled_font->status;

    return _cairo_user_data_array_set_data (&scaled_font->user_data,
					    key, user_data, destroy);
}
slim_hidden_def (cairo_scaled_font_set_user_data);

/* Public font API follows. */

/**
 * cairo_scaled_font_extents:
 * @scaled_font: a #cairo_scaled_font_t
 * @extents: a #cairo_font_extents_t which to store the retrieved extents.
 *
 * Gets the metrics for a #cairo_scaled_font_t.
 *
 * Since: 1.0
 **/
void
cairo_scaled_font_extents (cairo_scaled_font_t  *scaled_font,
			   cairo_font_extents_t *extents)
{
    if (scaled_font->status) {
	extents->ascent  = 0.0;
	extents->descent = 0.0;
	extents->height  = 0.0;
	extents->max_x_advance = 0.0;
	extents->max_y_advance = 0.0;
	return;
    }

    *extents = scaled_font->extents;
}
slim_hidden_def (cairo_scaled_font_extents);

/**
 * cairo_scaled_font_text_extents:
 * @scaled_font: a #cairo_scaled_font_t
 * @utf8: a NUL-terminated string of text, encoded in UTF-8
 * @extents: a #cairo_text_extents_t which to store the retrieved extents.
 *
 * Gets the extents for a string of text. The extents describe a
 * user-space rectangle that encloses the "inked" portion of the text
 * drawn at the origin (0,0) (as it would be drawn by cairo_show_text()
 * if the cairo graphics state were set to the same font_face,
 * font_matrix, ctm, and font_options as @scaled_font).  Additionally,
 * the x_advance and y_advance values indicate the amount by which the
 * current point would be advanced by cairo_show_text().
 *
 * Note that whitespace characters do not directly contribute to the
 * size of the rectangle (extents.width and extents.height). They do
 * contribute indirectly by changing the position of non-whitespace
 * characters. In particular, trailing whitespace characters are
 * likely to not affect the size of the rectangle, though they will
 * affect the x_advance and y_advance values.
 *
 * Since: 1.2
 **/
void
cairo_scaled_font_text_extents (cairo_scaled_font_t   *scaled_font,
				const char            *utf8,
				cairo_text_extents_t  *extents)
{
    cairo_status_t status;
    cairo_glyph_t *glyphs = NULL;
    int num_glyphs;

    if (scaled_font->status)
	goto ZERO_EXTENTS;

    if (utf8 == NULL)
	goto ZERO_EXTENTS;

    status = cairo_scaled_font_text_to_glyphs (scaled_font, 0., 0.,
					       utf8, -1,
					       &glyphs, &num_glyphs,
					       NULL, NULL,
					       NULL);
    if (unlikely (status)) {
	status = _cairo_scaled_font_set_error (scaled_font, status);
	goto ZERO_EXTENTS;
    }

    cairo_scaled_font_glyph_extents (scaled_font, glyphs, num_glyphs, extents);
    free (glyphs);

    return;

ZERO_EXTENTS:
    extents->x_bearing = 0.0;
    extents->y_bearing = 0.0;
    extents->width  = 0.0;
    extents->height = 0.0;
    extents->x_advance = 0.0;
    extents->y_advance = 0.0;
}

/**
 * cairo_scaled_font_glyph_extents:
 * @scaled_font: a #cairo_scaled_font_t
 * @glyphs: an array of glyph IDs with X and Y offsets.
 * @num_glyphs: the number of glyphs in the @glyphs array
 * @extents: a #cairo_text_extents_t which to store the retrieved extents.
 *
 * Gets the extents for an array of glyphs. The extents describe a
 * user-space rectangle that encloses the "inked" portion of the
 * glyphs, (as they would be drawn by cairo_show_glyphs() if the cairo
 * graphics state were set to the same font_face, font_matrix, ctm,
 * and font_options as @scaled_font).  Additionally, the x_advance and
 * y_advance values indicate the amount by which the current point
 * would be advanced by cairo_show_glyphs().
 *
 * Note that whitespace glyphs do not contribute to the size of the
 * rectangle (extents.width and extents.height).
 *
 * Since: 1.0
 **/
void
cairo_scaled_font_glyph_extents (cairo_scaled_font_t   *scaled_font,
				 const cairo_glyph_t   *glyphs,
				 int                    num_glyphs,
				 cairo_text_extents_t  *extents)
{
    cairo_status_t status;
    int i;
    double min_x = 0.0, min_y = 0.0, max_x = 0.0, max_y = 0.0;
    cairo_bool_t visible = FALSE;
    cairo_scaled_glyph_t *scaled_glyph = NULL;

    extents->x_bearing = 0.0;
    extents->y_bearing = 0.0;
    extents->width  = 0.0;
    extents->height = 0.0;
    extents->x_advance = 0.0;
    extents->y_advance = 0.0;

    if (unlikely (scaled_font->status))
	goto ZERO_EXTENTS;

    if (num_glyphs == 0)
	goto ZERO_EXTENTS;

    if (unlikely (num_glyphs < 0)) {
	_cairo_error_throw (CAIRO_STATUS_NEGATIVE_COUNT);
	/* XXX Can't propagate error */
	goto ZERO_EXTENTS;
    }

    if (unlikely (glyphs == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NULL_POINTER);
	/* XXX Can't propagate error */
	goto ZERO_EXTENTS;
    }

    _cairo_scaled_font_freeze_cache (scaled_font);

    for (i = 0; i < num_glyphs; i++) {
	double			left, top, right, bottom;

	status = _cairo_scaled_glyph_lookup (scaled_font,
					     glyphs[i].index,
					     CAIRO_SCALED_GLYPH_INFO_METRICS,
					     NULL, /* foreground color */
					     &scaled_glyph);
	if (unlikely (status)) {
	    status = _cairo_scaled_font_set_error (scaled_font, status);
	    goto UNLOCK;
	}

	/* "Ink" extents should skip "invisible" glyphs */
	if (scaled_glyph->metrics.width == 0 || scaled_glyph->metrics.height == 0)
	    continue;

	left = scaled_glyph->metrics.x_bearing + glyphs[i].x;
	right = left + scaled_glyph->metrics.width;
	top = scaled_glyph->metrics.y_bearing + glyphs[i].y;
	bottom = top + scaled_glyph->metrics.height;

	if (!visible) {
	    visible = TRUE;
	    min_x = left;
	    max_x = right;
	    min_y = top;
	    max_y = bottom;
	} else {
	    if (left < min_x) min_x = left;
	    if (right > max_x) max_x = right;
	    if (top < min_y) min_y = top;
	    if (bottom > max_y) max_y = bottom;
	}
    }

    if (visible) {
	extents->x_bearing = min_x - glyphs[0].x;
	extents->y_bearing = min_y - glyphs[0].y;
	extents->width = max_x - min_x;
	extents->height = max_y - min_y;
    } else {
	extents->x_bearing = 0.0;
	extents->y_bearing = 0.0;
	extents->width = 0.0;
	extents->height = 0.0;
    }

    if (num_glyphs) {
        double x0, y0, x1, y1;

	x0 = glyphs[0].x;
	y0 = glyphs[0].y;

	/* scaled_glyph contains the glyph for num_glyphs - 1 already. */
	x1 = glyphs[num_glyphs - 1].x + scaled_glyph->metrics.x_advance;
	y1 = glyphs[num_glyphs - 1].y + scaled_glyph->metrics.y_advance;

	extents->x_advance = x1 - x0;
	extents->y_advance = y1 - y0;
    } else {
	extents->x_advance = 0.0;
	extents->y_advance = 0.0;
    }

 UNLOCK:
    _cairo_scaled_font_thaw_cache (scaled_font);
    return;

ZERO_EXTENTS:
    extents->x_bearing = 0.0;
    extents->y_bearing = 0.0;
    extents->width  = 0.0;
    extents->height = 0.0;
    extents->x_advance = 0.0;
    extents->y_advance = 0.0;
}
slim_hidden_def (cairo_scaled_font_glyph_extents);

#define GLYPH_LUT_SIZE 64
static cairo_status_t
cairo_scaled_font_text_to_glyphs_internal_cached (cairo_scaled_font_t		 *scaled_font,
						    double			  x,
						    double			  y,
						    const char			 *utf8,
						    cairo_glyph_t		 *glyphs,
						    cairo_text_cluster_t	**clusters,
						    int				  num_chars)
{
    struct glyph_lut_elt {
	unsigned long index;
	double x_advance;
	double y_advance;
    } glyph_lut[GLYPH_LUT_SIZE];
    uint32_t glyph_lut_unicode[GLYPH_LUT_SIZE];
    cairo_status_t status;
    const char *p;
    int i;

    for (i = 0; i < GLYPH_LUT_SIZE; i++)
	glyph_lut_unicode[i] = ~0U;

    p = utf8;
    for (i = 0; i < num_chars; i++) {
	int idx, num_bytes;
	uint32_t unicode;
	cairo_scaled_glyph_t *scaled_glyph;
	struct glyph_lut_elt *glyph_slot;

	num_bytes = _cairo_utf8_get_char_validated (p, &unicode);
	p += num_bytes;

	glyphs[i].x = x;
	glyphs[i].y = y;

	idx = unicode % ARRAY_LENGTH (glyph_lut);
	glyph_slot = &glyph_lut[idx];
	if (glyph_lut_unicode[idx] == unicode) {
	    glyphs[i].index = glyph_slot->index;
	    x += glyph_slot->x_advance;
	    y += glyph_slot->y_advance;
	} else {
	    unsigned long g;

	    g = scaled_font->backend->ucs4_to_index (scaled_font, unicode);
	    status = _cairo_scaled_glyph_lookup (scaled_font,
						 g,
						 CAIRO_SCALED_GLYPH_INFO_METRICS,
						 NULL, /* foreground color */
						 &scaled_glyph);
	    if (unlikely (status))
		return status;

	    x += scaled_glyph->metrics.x_advance;
	    y += scaled_glyph->metrics.y_advance;

	    glyph_lut_unicode[idx] = unicode;
	    glyph_slot->index = g;
	    glyph_slot->x_advance = scaled_glyph->metrics.x_advance;
	    glyph_slot->y_advance = scaled_glyph->metrics.y_advance;

	    glyphs[i].index = g;
	}

	if (clusters) {
	    (*clusters)[i].num_bytes  = num_bytes;
	    (*clusters)[i].num_glyphs = 1;
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_scaled_font_text_to_glyphs_internal_uncached (cairo_scaled_font_t	 *scaled_font,
						  double		  x,
						  double		  y,
						  const char		 *utf8,
						  cairo_glyph_t		 *glyphs,
						  cairo_text_cluster_t	**clusters,
						  int			  num_chars)
{
    const char *p;
    int i;

    p = utf8;
    for (i = 0; i < num_chars; i++) {
	unsigned long g;
	int num_bytes;
	uint32_t unicode;
	cairo_scaled_glyph_t *scaled_glyph;
	cairo_status_t status;

	num_bytes = _cairo_utf8_get_char_validated (p, &unicode);
	p += num_bytes;

	glyphs[i].x = x;
	glyphs[i].y = y;

	g = scaled_font->backend->ucs4_to_index (scaled_font, unicode);

	/*
	 * No advance needed for a single character string. So, let's speed up
	 * one-character strings by skipping glyph lookup.
	 */
	if (num_chars > 1) {
	    status = _cairo_scaled_glyph_lookup (scaled_font,
					     g,
					     CAIRO_SCALED_GLYPH_INFO_METRICS,
					     NULL, /* foreground color */
					     &scaled_glyph);
	    if (unlikely (status))
		return status;

	    x += scaled_glyph->metrics.x_advance;
	    y += scaled_glyph->metrics.y_advance;
	}

	glyphs[i].index = g;

	if (clusters) {
	    (*clusters)[i].num_bytes  = num_bytes;
	    (*clusters)[i].num_glyphs = 1;
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

/**
 * cairo_scaled_font_text_to_glyphs:
 * @scaled_font: a #cairo_scaled_font_t
 * @x: X position to place first glyph
 * @y: Y position to place first glyph
 * @utf8: a string of text encoded in UTF-8
 * @utf8_len: length of @utf8 in bytes, or -1 if it is NUL-terminated
 * @glyphs: pointer to array of glyphs to fill
 * @num_glyphs: pointer to number of glyphs
 * @clusters: pointer to array of cluster mapping information to fill, or %NULL
 * @num_clusters: pointer to number of clusters, or %NULL
 * @cluster_flags: pointer to location to store cluster flags corresponding to the
 *                 output @clusters, or %NULL
 *
 * Converts UTF-8 text to an array of glyphs, optionally with cluster
 * mapping, that can be used to render later using @scaled_font.
 *
 * If @glyphs initially points to a non-%NULL value, that array is used
 * as a glyph buffer, and @num_glyphs should point to the number of glyph
 * entries available there.  If the provided glyph array is too short for
 * the conversion, a new glyph array is allocated using cairo_glyph_allocate()
 * and placed in @glyphs.  Upon return, @num_glyphs always contains the
 * number of generated glyphs.  If the value @glyphs points to has changed
 * after the call, the user is responsible for freeing the allocated glyph
 * array using cairo_glyph_free().  This may happen even if the provided
 * array was large enough.
 *
 * If @clusters is not %NULL, @num_clusters and @cluster_flags should not be %NULL,
 * and cluster mapping will be computed.
 * The semantics of how cluster array allocation works is similar to the glyph
 * array.  That is,
 * if @clusters initially points to a non-%NULL value, that array is used
 * as a cluster buffer, and @num_clusters should point to the number of cluster
 * entries available there.  If the provided cluster array is too short for
 * the conversion, a new cluster array is allocated using cairo_text_cluster_allocate()
 * and placed in @clusters.  Upon return, @num_clusters always contains the
 * number of generated clusters.  If the value @clusters points at has changed
 * after the call, the user is responsible for freeing the allocated cluster
 * array using cairo_text_cluster_free().  This may happen even if the provided
 * array was large enough.
 *
 * In the simplest case, @glyphs and @clusters can point to %NULL initially
 * and a suitable array will be allocated.  In code:
 * <informalexample><programlisting>
 * cairo_status_t status;
 *
 * cairo_glyph_t *glyphs = NULL;
 * int num_glyphs;
 * cairo_text_cluster_t *clusters = NULL;
 * int num_clusters;
 * cairo_text_cluster_flags_t cluster_flags;
 *
 * status = cairo_scaled_font_text_to_glyphs (scaled_font,
 *                                            x, y,
 *                                            utf8, utf8_len,
 *                                            &amp;glyphs, &amp;num_glyphs,
 *                                            &amp;clusters, &amp;num_clusters, &amp;cluster_flags);
 *
 * if (status == CAIRO_STATUS_SUCCESS) {
 *     cairo_show_text_glyphs (cr,
 *                             utf8, utf8_len,
 *                             glyphs, num_glyphs,
 *                             clusters, num_clusters, cluster_flags);
 *
 *     cairo_glyph_free (glyphs);
 *     cairo_text_cluster_free (clusters);
 * }
 * </programlisting></informalexample>
 *
 * If no cluster mapping is needed:
 * <informalexample><programlisting>
 * cairo_status_t status;
 *
 * cairo_glyph_t *glyphs = NULL;
 * int num_glyphs;
 *
 * status = cairo_scaled_font_text_to_glyphs (scaled_font,
 *                                            x, y,
 *                                            utf8, utf8_len,
 *                                            &amp;glyphs, &amp;num_glyphs,
 *                                            NULL, NULL,
 *                                            NULL);
 *
 * if (status == CAIRO_STATUS_SUCCESS) {
 *     cairo_show_glyphs (cr, glyphs, num_glyphs);
 *     cairo_glyph_free (glyphs);
 * }
 * </programlisting></informalexample>
 *
 * If stack-based glyph and cluster arrays are to be used for small
 * arrays:
 * <informalexample><programlisting>
 * cairo_status_t status;
 *
 * cairo_glyph_t stack_glyphs[40];
 * cairo_glyph_t *glyphs = stack_glyphs;
 * int num_glyphs = sizeof (stack_glyphs) / sizeof (stack_glyphs[0]);
 * cairo_text_cluster_t stack_clusters[40];
 * cairo_text_cluster_t *clusters = stack_clusters;
 * int num_clusters = sizeof (stack_clusters) / sizeof (stack_clusters[0]);
 * cairo_text_cluster_flags_t cluster_flags;
 *
 * status = cairo_scaled_font_text_to_glyphs (scaled_font,
 *                                            x, y,
 *                                            utf8, utf8_len,
 *                                            &amp;glyphs, &amp;num_glyphs,
 *                                            &amp;clusters, &amp;num_clusters, &amp;cluster_flags);
 *
 * if (status == CAIRO_STATUS_SUCCESS) {
 *     cairo_show_text_glyphs (cr,
 *                             utf8, utf8_len,
 *                             glyphs, num_glyphs,
 *                             clusters, num_clusters, cluster_flags);
 *
 *     if (glyphs != stack_glyphs)
 *         cairo_glyph_free (glyphs);
 *     if (clusters != stack_clusters)
 *         cairo_text_cluster_free (clusters);
 * }
 * </programlisting></informalexample>
 *
 * For details of how @clusters, @num_clusters, and @cluster_flags map input
 * UTF-8 text to the output glyphs see cairo_show_text_glyphs().
 *
 * The output values can be readily passed to cairo_show_text_glyphs()
 * cairo_show_glyphs(), or related functions, assuming that the exact
 * same @scaled_font is used for the operation.
 *
 * Return value: %CAIRO_STATUS_SUCCESS upon success, or an error status
 * if the input values are wrong or if conversion failed.  If the input
 * values are correct but the conversion failed, the error status is also
 * set on @scaled_font.
 *
 * Since: 1.8
 **/
#define CACHING_THRESHOLD 16
cairo_status_t
cairo_scaled_font_text_to_glyphs (cairo_scaled_font_t   *scaled_font,
				  double		 x,
				  double		 y,
				  const char	        *utf8,
				  int		         utf8_len,
				  cairo_glyph_t	       **glyphs,
				  int		        *num_glyphs,
				  cairo_text_cluster_t **clusters,
				  int		        *num_clusters,
				  cairo_text_cluster_flags_t *cluster_flags)
{
    int num_chars = 0;
    cairo_int_status_t status;
    cairo_glyph_t *orig_glyphs;
    cairo_text_cluster_t *orig_clusters;

    status = scaled_font->status;
    if (unlikely (status))
	return status;

    /* A slew of sanity checks */

    /* glyphs and num_glyphs can't be NULL */
    if (glyphs     == NULL ||
	num_glyphs == NULL) {
	status = _cairo_error (CAIRO_STATUS_NULL_POINTER);
	goto BAIL;
    }

    /* Special case for NULL and -1 */
    if (utf8 == NULL && utf8_len == -1)
	utf8_len = 0;

    /* No NULLs for non-NULLs! */
    if ((utf8_len && utf8          == NULL) ||
	(clusters && num_clusters  == NULL) ||
	(clusters && cluster_flags == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NULL_POINTER);
	goto BAIL;
    }

    /* A -1 for utf8_len means NUL-terminated */
    if (utf8_len == -1)
	utf8_len = strlen (utf8);

    /* A NULL *glyphs means no prealloced glyphs array */
    if (glyphs && *glyphs == NULL)
	*num_glyphs = 0;

    /* A NULL *clusters means no prealloced clusters array */
    if (clusters && *clusters == NULL)
	*num_clusters = 0;

    if (!clusters && num_clusters) {
	num_clusters = NULL;
    }

    if (cluster_flags) {
	*cluster_flags = FALSE;
    }

    if (!clusters && cluster_flags) {
	cluster_flags = NULL;
    }

    /* Apart from that, no negatives */
    if (utf8_len < 0 ||
	*num_glyphs < 0 ||
	(num_clusters && *num_clusters < 0)) {
	status = _cairo_error (CAIRO_STATUS_NEGATIVE_COUNT);
	goto BAIL;
    }

    if (utf8_len == 0) {
	status = CAIRO_STATUS_SUCCESS;
	goto BAIL;
    }

    /* validate input so backend does not have to */
    status = _cairo_utf8_to_ucs4 (utf8, utf8_len, NULL, &num_chars);
    if (unlikely (status))
	goto BAIL;

    _cairo_scaled_font_freeze_cache (scaled_font);

    orig_glyphs = *glyphs;
    orig_clusters = clusters ? *clusters : NULL;

    if (scaled_font->backend->text_to_glyphs) {
	status = scaled_font->backend->text_to_glyphs (scaled_font, x, y,
						       utf8, utf8_len,
						       glyphs, num_glyphs,
						       clusters, num_clusters,
						       cluster_flags);
        if (status != CAIRO_INT_STATUS_UNSUPPORTED) {
	    if (status == CAIRO_INT_STATUS_SUCCESS) {
	        /* The checks here are crude; we only should do them in
		 * user-font backend, but they don't hurt here.  This stuff
		 * can be hard to get right. */

	        if (*num_glyphs < 0) {
		    status = _cairo_error (CAIRO_STATUS_NEGATIVE_COUNT);
		    goto DONE;
		}
		if (*num_glyphs != 0 && *glyphs == NULL) {
		    status = _cairo_error (CAIRO_STATUS_NULL_POINTER);
		    goto DONE;
		}

		if (clusters) {
		    if (*num_clusters < 0) {
			status = _cairo_error (CAIRO_STATUS_NEGATIVE_COUNT);
			goto DONE;
		    }
		    if (*num_clusters != 0 && *clusters == NULL) {
			status = _cairo_error (CAIRO_STATUS_NULL_POINTER);
			goto DONE;
		    }

		    /* Don't trust the backend, validate clusters! */
		    status =
			_cairo_validate_text_clusters (utf8, utf8_len,
						       *glyphs, *num_glyphs,
						       *clusters, *num_clusters,
						       *cluster_flags);
		}
	    }

            goto DONE;
	}
    }

    if (*num_glyphs < num_chars) {
	*glyphs = cairo_glyph_allocate (num_chars);
	if (unlikely (*glyphs == NULL)) {
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    goto DONE;
	}
    }
    *num_glyphs = num_chars;

    if (clusters) {
	if (*num_clusters < num_chars) {
	    *clusters = cairo_text_cluster_allocate (num_chars);
	    if (unlikely (*clusters == NULL)) {
		status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
		goto DONE;
	    }
	}
	*num_clusters = num_chars;
    }

    if (num_chars > CACHING_THRESHOLD)
	status = cairo_scaled_font_text_to_glyphs_internal_cached (scaled_font,
								     x, y,
								     utf8,
								     *glyphs,
								     clusters,
								     num_chars);
    else
	status = cairo_scaled_font_text_to_glyphs_internal_uncached (scaled_font,
								   x, y,
								   utf8,
								   *glyphs,
								   clusters,
								   num_chars);

 DONE: /* error that should be logged on scaled_font happened */
    _cairo_scaled_font_thaw_cache (scaled_font);

    if (unlikely (status)) {
	*num_glyphs = 0;
	if (*glyphs != orig_glyphs) {
	    cairo_glyph_free (*glyphs);
	    *glyphs = orig_glyphs;
	}

	if (clusters) {
	    *num_clusters = 0;
	    if (*clusters != orig_clusters) {
		cairo_text_cluster_free (*clusters);
		*clusters = orig_clusters;
	    }
	}
    }

    return _cairo_scaled_font_set_error (scaled_font, status);

 BAIL: /* error with input arguments */

    if (num_glyphs)
	*num_glyphs = 0;

    if (num_clusters)
	*num_clusters = 0;

    return status;
}
slim_hidden_def (cairo_scaled_font_text_to_glyphs);

static inline cairo_bool_t
_range_contains_glyph (const cairo_box_t *extents,
		       cairo_fixed_t left,
		       cairo_fixed_t top,
		       cairo_fixed_t right,
		       cairo_fixed_t bottom)
{
    if (left == right || top == bottom)
	return FALSE;

    return right > extents->p1.x &&
	   left < extents->p2.x &&
	   bottom > extents->p1.y &&
	   top < extents->p2.y;
}

static cairo_status_t
_cairo_scaled_font_single_glyph_device_extents (cairo_scaled_font_t	 *scaled_font,
						const cairo_glyph_t	 *glyph,
						cairo_rectangle_int_t   *extents)
{
    cairo_scaled_glyph_t *scaled_glyph;
    cairo_status_t status;

    _cairo_scaled_font_freeze_cache (scaled_font);
    status = _cairo_scaled_glyph_lookup (scaled_font,
					 glyph->index,
					 CAIRO_SCALED_GLYPH_INFO_METRICS,
					 NULL, /* foreground color */
					 &scaled_glyph);
    if (likely (status == CAIRO_STATUS_SUCCESS)) {
	cairo_bool_t round_xy = _cairo_font_options_get_round_glyph_positions (&scaled_font->options) == CAIRO_ROUND_GLYPH_POS_ON;
	cairo_box_t box;
	cairo_fixed_t v;

	if (round_xy)
	    v = _cairo_fixed_from_int (_cairo_lround (glyph->x));
	else
	    v = _cairo_fixed_from_double (glyph->x);
	box.p1.x = v + scaled_glyph->bbox.p1.x;
	box.p2.x = v + scaled_glyph->bbox.p2.x;

	if (round_xy)
	    v = _cairo_fixed_from_int (_cairo_lround (glyph->y));
	else
	    v = _cairo_fixed_from_double (glyph->y);
	box.p1.y = v + scaled_glyph->bbox.p1.y;
	box.p2.y = v + scaled_glyph->bbox.p2.y;

	_cairo_box_round_to_rectangle (&box, extents);
    }
    _cairo_scaled_font_thaw_cache (scaled_font);
    return status;
}

/*
 * Compute a device-space bounding box for the glyphs.
 */
cairo_status_t
_cairo_scaled_font_glyph_device_extents (cairo_scaled_font_t	 *scaled_font,
					 const cairo_glyph_t	 *glyphs,
					 int                      num_glyphs,
					 cairo_rectangle_int_t   *extents,
					 cairo_bool_t *overlap_out)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;
    cairo_box_t box = { { INT_MAX, INT_MAX }, { INT_MIN, INT_MIN }};
    cairo_scaled_glyph_t *glyph_cache[64];
    cairo_bool_t overlap = overlap_out ? FALSE : TRUE;
    cairo_round_glyph_positions_t round_glyph_positions = _cairo_font_options_get_round_glyph_positions (&scaled_font->options);
    int i;

    if (unlikely (scaled_font->status))
	return scaled_font->status;

    if (num_glyphs == 1) {
	if (overlap_out)
	    *overlap_out = FALSE;
	return _cairo_scaled_font_single_glyph_device_extents (scaled_font,
							       glyphs,
							       extents);
    }

    _cairo_scaled_font_freeze_cache (scaled_font);

    memset (glyph_cache, 0, sizeof (glyph_cache));

    for (i = 0; i < num_glyphs; i++) {
	cairo_scaled_glyph_t	*scaled_glyph;
	cairo_fixed_t x, y, x1, y1, x2, y2;
	int cache_index = glyphs[i].index % ARRAY_LENGTH (glyph_cache);

	scaled_glyph = glyph_cache[cache_index];
	if (scaled_glyph == NULL ||
	    _cairo_scaled_glyph_index (scaled_glyph) != glyphs[i].index)
	{
	    status = _cairo_scaled_glyph_lookup (scaled_font,
						 glyphs[i].index,
						 CAIRO_SCALED_GLYPH_INFO_METRICS,
						 NULL, /* foreground color */
						 &scaled_glyph);
	    if (unlikely (status))
		break;

	    glyph_cache[cache_index] = scaled_glyph;
	}

	if (round_glyph_positions == CAIRO_ROUND_GLYPH_POS_ON)
	    x = _cairo_fixed_from_int (_cairo_lround (glyphs[i].x));
	else
	    x = _cairo_fixed_from_double (glyphs[i].x);
	x1 = x + scaled_glyph->bbox.p1.x;
	x2 = x + scaled_glyph->bbox.p2.x;

	if (round_glyph_positions == CAIRO_ROUND_GLYPH_POS_ON)
	    y = _cairo_fixed_from_int (_cairo_lround (glyphs[i].y));
	else
	    y = _cairo_fixed_from_double (glyphs[i].y);
	y1 = y + scaled_glyph->bbox.p1.y;
	y2 = y + scaled_glyph->bbox.p2.y;

	if (overlap == FALSE)
	    overlap = _range_contains_glyph (&box, x1, y1, x2, y2);

	if (x1 < box.p1.x) box.p1.x = x1;
	if (x2 > box.p2.x) box.p2.x = x2;
	if (y1 < box.p1.y) box.p1.y = y1;
	if (y2 > box.p2.y) box.p2.y = y2;
    }

    _cairo_scaled_font_thaw_cache (scaled_font);
    if (unlikely (status))
	return _cairo_scaled_font_set_error (scaled_font, status);

    if (box.p1.x < box.p2.x) {
	_cairo_box_round_to_rectangle (&box, extents);
    } else {
	extents->x = extents->y = 0;
	extents->width = extents->height = 0;
    }

    if (overlap_out != NULL)
	*overlap_out = overlap;

    return CAIRO_STATUS_SUCCESS;
}

cairo_bool_t
_cairo_scaled_font_glyph_approximate_extents (cairo_scaled_font_t	 *scaled_font,
					      const cairo_glyph_t	 *glyphs,
					      int                      num_glyphs,
					      cairo_rectangle_int_t   *extents)
{
    double x0, x1, y0, y1, pad;
    int i;

    /* If any of the factors are suspect (i.e. the font is broken), bail */
    if (scaled_font->fs_extents.max_x_advance == 0 ||
	scaled_font->fs_extents.height == 0 ||
	scaled_font->max_scale == 0)
    {
	return FALSE;
    }

    assert (num_glyphs);

    x0 = x1 = glyphs[0].x;
    y0 = y1 = glyphs[0].y;
    for (i = 1; i < num_glyphs; i++) {
	double g;

	g = glyphs[i].x;
	if (g < x0) x0 = g;
	if (g > x1) x1 = g;

	g = glyphs[i].y;
	if (g < y0) y0 = g;
	if (g > y1) y1 = g;
    }

    pad = MAX(scaled_font->fs_extents.max_x_advance,
	      scaled_font->fs_extents.height);
    pad *= scaled_font->max_scale;

    extents->x = floor (x0 - pad);
    extents->width = ceil (x1 + pad) - extents->x;
    extents->y = floor (y0 - pad);
    extents->height = ceil (y1 + pad) - extents->y;
    return TRUE;
}

/* Add a single-device-unit rectangle to a path. */
static cairo_status_t
_add_unit_rectangle_to_path (cairo_path_fixed_t *path,
			     cairo_fixed_t x,
			     cairo_fixed_t y)
{
    cairo_status_t status;

    status = _cairo_path_fixed_move_to (path, x, y);
    if (unlikely (status))
	return status;

    status = _cairo_path_fixed_rel_line_to (path,
					    _cairo_fixed_from_int (1),
					    _cairo_fixed_from_int (0));
    if (unlikely (status))
	return status;

    status = _cairo_path_fixed_rel_line_to (path,
					    _cairo_fixed_from_int (0),
					    _cairo_fixed_from_int (1));
    if (unlikely (status))
	return status;

    status = _cairo_path_fixed_rel_line_to (path,
					    _cairo_fixed_from_int (-1),
					    _cairo_fixed_from_int (0));
    if (unlikely (status))
	return status;

    return _cairo_path_fixed_close_path (path);
}

/**
 * _trace_mask_to_path:
 * @bitmap: An alpha mask (either %CAIRO_FORMAT_A1 or %CAIRO_FORMAT_A8)
 * @path: An initialized path to hold the result
 *
 * Given a mask surface, (an alpha image), fill out the provided path
 * so that when filled it would result in something that approximates
 * the mask.
 *
 * Note: The current tracing code here is extremely primitive. It
 * operates only on an A1 surface, (converting an A8 surface to A1 if
 * necessary), and performs the tracing by drawing a little square
 * around each pixel that is on in the mask. We do not pretend that
 * this is a high-quality result. But we are leaving it up to someone
 * who cares enough about getting a better result to implement
 * something more sophisticated.
 **/
static cairo_status_t
_trace_mask_to_path (cairo_image_surface_t *mask,
		     cairo_path_fixed_t *path,
		     double tx, double ty)
{
    const uint8_t *row;
    int rows, cols, bytes_per_row;
    int x, y, bit;
    double xoff, yoff;
    cairo_fixed_t x0, y0;
    cairo_fixed_t px, py;
    cairo_status_t status;

    mask = _cairo_image_surface_coerce_to_format (mask, CAIRO_FORMAT_A1);
    status = mask->base.status;
    if (unlikely (status))
	return status;

    cairo_surface_get_device_offset (&mask->base, &xoff, &yoff);
    x0 = _cairo_fixed_from_double (tx - xoff);
    y0 = _cairo_fixed_from_double (ty - yoff);

    bytes_per_row = (mask->width + 7) / 8;
    row = mask->data;
    for (y = 0, rows = mask->height; rows--; row += mask->stride, y++) {
	const uint8_t *byte_ptr = row;
	x = 0;
	py = _cairo_fixed_from_int (y);
	for (cols = bytes_per_row; cols--; ) {
	    uint8_t byte = *byte_ptr++;
	    if (byte == 0) {
		x += 8;
		continue;
	    }

	    byte = CAIRO_BITSWAP8_IF_LITTLE_ENDIAN (byte);
	    for (bit = 1 << 7; bit && x < mask->width; bit >>= 1, x++) {
		if (byte & bit) {
		    px = _cairo_fixed_from_int (x);
		    status = _add_unit_rectangle_to_path (path,
							  px + x0,
							  py + y0);
		    if (unlikely (status))
			goto BAIL;
		}
	    }
	}
    }

BAIL:
    cairo_surface_destroy (&mask->base);

    return status;
}

cairo_status_t
_cairo_scaled_font_glyph_path (cairo_scaled_font_t *scaled_font,
			       const cairo_glyph_t *glyphs,
			       int		    num_glyphs,
			       cairo_path_fixed_t  *path)
{
    cairo_int_status_t status;
    int	i;

    status = scaled_font->status;
    if (unlikely (status))
	return status;

    _cairo_scaled_font_freeze_cache (scaled_font);
    for (i = 0; i < num_glyphs; i++) {
	cairo_scaled_glyph_t *scaled_glyph;

	status = _cairo_scaled_glyph_lookup (scaled_font,
					     glyphs[i].index,
					     CAIRO_SCALED_GLYPH_INFO_PATH,
					     NULL, /* foreground color */
					     &scaled_glyph);
	if (status == CAIRO_INT_STATUS_SUCCESS) {
	    status = _cairo_path_fixed_append (path,
					       scaled_glyph->path,
					       _cairo_fixed_from_double (glyphs[i].x),
					       _cairo_fixed_from_double (glyphs[i].y));

	} else if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	    /* If the font is incapable of providing a path, then we'll
	     * have to trace our own from a surface.
	     */
	    status = _cairo_scaled_glyph_lookup (scaled_font,
						 glyphs[i].index,
						 CAIRO_SCALED_GLYPH_INFO_SURFACE,
						 NULL, /* foreground color */
						 &scaled_glyph);
	    if (unlikely (status))
		goto BAIL;

	    status = _trace_mask_to_path (scaled_glyph->surface, path,
					  glyphs[i].x, glyphs[i].y);
	}

	if (unlikely (status))
	    goto BAIL;
    }
  BAIL:
    _cairo_scaled_font_thaw_cache (scaled_font);

    return _cairo_scaled_font_set_error (scaled_font, status);
}

/**
 * _cairo_scaled_glyph_set_metrics:
 * @scaled_glyph: a #cairo_scaled_glyph_t
 * @scaled_font: a #cairo_scaled_font_t
 * @fs_metrics: a #cairo_text_extents_t in font space
 *
 * _cairo_scaled_glyph_set_metrics() stores user space metrics
 * for the specified glyph given font space metrics. It is
 * called by the font backend when initializing a glyph with
 * %CAIRO_SCALED_GLYPH_INFO_METRICS.
 **/
void
_cairo_scaled_glyph_set_metrics (cairo_scaled_glyph_t *scaled_glyph,
				 cairo_scaled_font_t *scaled_font,
				 cairo_text_extents_t *fs_metrics)
{
    cairo_bool_t first = TRUE;
    double hm, wm;
    double min_user_x = 0.0, max_user_x = 0.0, min_user_y = 0.0, max_user_y = 0.0;
    double min_device_x = 0.0, max_device_x = 0.0, min_device_y = 0.0, max_device_y = 0.0;
    double device_x_advance, device_y_advance;

    scaled_glyph->fs_metrics = *fs_metrics;

    for (hm = 0.0; hm <= 1.0; hm += 1.0)
	for (wm = 0.0; wm <= 1.0; wm += 1.0) {
	    double x, y;

	    /* Transform this corner to user space */
	    x = fs_metrics->x_bearing + fs_metrics->width * wm;
	    y = fs_metrics->y_bearing + fs_metrics->height * hm;
	    cairo_matrix_transform_point (&scaled_font->font_matrix,
					  &x, &y);
	    if (first) {
		min_user_x = max_user_x = x;
		min_user_y = max_user_y = y;
	    } else {
		if (x < min_user_x) min_user_x = x;
		if (x > max_user_x) max_user_x = x;
		if (y < min_user_y) min_user_y = y;
		if (y > max_user_y) max_user_y = y;
	    }

	    /* Transform this corner to device space from glyph origin */
	    x = fs_metrics->x_bearing + fs_metrics->width * wm;
	    y = fs_metrics->y_bearing + fs_metrics->height * hm;
	    cairo_matrix_transform_distance (&scaled_font->scale,
					     &x, &y);

	    if (first) {
		min_device_x = max_device_x = x;
		min_device_y = max_device_y = y;
	    } else {
		if (x < min_device_x) min_device_x = x;
		if (x > max_device_x) max_device_x = x;
		if (y < min_device_y) min_device_y = y;
		if (y > max_device_y) max_device_y = y;
	    }
	    first = FALSE;
	}
    scaled_glyph->metrics.x_bearing = min_user_x;
    scaled_glyph->metrics.y_bearing = min_user_y;
    scaled_glyph->metrics.width = max_user_x - min_user_x;
    scaled_glyph->metrics.height = max_user_y - min_user_y;

    scaled_glyph->metrics.x_advance = fs_metrics->x_advance;
    scaled_glyph->metrics.y_advance = fs_metrics->y_advance;
    cairo_matrix_transform_distance (&scaled_font->font_matrix,
				     &scaled_glyph->metrics.x_advance,
				     &scaled_glyph->metrics.y_advance);

    device_x_advance = fs_metrics->x_advance;
    device_y_advance = fs_metrics->y_advance;
    cairo_matrix_transform_distance (&scaled_font->scale,
				     &device_x_advance,
				     &device_y_advance);

    scaled_glyph->bbox.p1.x = _cairo_fixed_from_double (min_device_x);
    scaled_glyph->bbox.p1.y = _cairo_fixed_from_double (min_device_y);
    scaled_glyph->bbox.p2.x = _cairo_fixed_from_double (max_device_x);
    scaled_glyph->bbox.p2.y = _cairo_fixed_from_double (max_device_y);

    scaled_glyph->x_advance = _cairo_lround (device_x_advance);
    scaled_glyph->y_advance = _cairo_lround (device_y_advance);

    scaled_glyph->has_info |= CAIRO_SCALED_GLYPH_INFO_METRICS;
}

void
_cairo_scaled_glyph_set_surface (cairo_scaled_glyph_t *scaled_glyph,
				 cairo_scaled_font_t *scaled_font,
				 cairo_image_surface_t *surface)
{
    if (scaled_glyph->surface != NULL)
	cairo_surface_destroy (&scaled_glyph->surface->base);

    /* sanity check the backend glyph contents */
    _cairo_debug_check_image_surface_is_defined (&surface->base);
    scaled_glyph->surface = surface;

    if (surface != NULL)
	scaled_glyph->has_info |= CAIRO_SCALED_GLYPH_INFO_SURFACE;
    else
	scaled_glyph->has_info &= ~CAIRO_SCALED_GLYPH_INFO_SURFACE;
}

void
_cairo_scaled_glyph_set_path (cairo_scaled_glyph_t *scaled_glyph,
			      cairo_scaled_font_t *scaled_font,
			      cairo_path_fixed_t *path)
{
    if (scaled_glyph->path != NULL)
	_cairo_path_fixed_destroy (scaled_glyph->path);

    scaled_glyph->path = path;

    if (path != NULL)
	scaled_glyph->has_info |= CAIRO_SCALED_GLYPH_INFO_PATH;
    else
	scaled_glyph->has_info &= ~CAIRO_SCALED_GLYPH_INFO_PATH;
}

/**
 * _cairo_scaled_glyph_set_recording_surface:
 * @scaled_glyph: a #cairo_scaled_glyph_t
 * @scaled_font: a #cairo_scaled_font_t
 * @recording_surface: The recording surface
 * @foreground_color: The foreground color that was used to record the
 * glyph, or NULL if foreground color not required.
 */
void
_cairo_scaled_glyph_set_recording_surface (cairo_scaled_glyph_t *scaled_glyph,
					   cairo_scaled_font_t  *scaled_font,
					   cairo_surface_t      *recording_surface,
					   const cairo_color_t * foreground_color)
{
    if (scaled_glyph->recording_surface != NULL) {
	cairo_surface_finish (scaled_glyph->recording_surface);
	cairo_surface_destroy (scaled_glyph->recording_surface);
    }

    scaled_glyph->recording_surface = recording_surface;
    scaled_glyph->recording_uses_foreground_color = foreground_color != NULL;
    if (foreground_color)
	scaled_glyph->foreground_color = *foreground_color;

    if (recording_surface != NULL)
	scaled_glyph->has_info |= CAIRO_SCALED_GLYPH_INFO_RECORDING_SURFACE;
    else
	scaled_glyph->has_info &= ~CAIRO_SCALED_GLYPH_INFO_RECORDING_SURFACE;
}

/**
 * _cairo_scaled_glyph_set_color_surface:
 * @scaled_glyph: a #cairo_scaled_glyph_t
 * @scaled_font: a #cairo_scaled_font_t
 * @surface: The image surface
 * @foreground_marker_color: The foreground color that was used to
 * substitute the foreground_marker, or NULL if foreground_marker not
 * used when rendering the surface color.
 */
void
_cairo_scaled_glyph_set_color_surface (cairo_scaled_glyph_t  *scaled_glyph,
	                               cairo_scaled_font_t   *scaled_font,
	                               cairo_image_surface_t *surface,
				       const cairo_color_t   *foreground_marker_color)
{
    if (scaled_glyph->color_surface != NULL)
	cairo_surface_destroy (&scaled_glyph->color_surface->base);

    /* sanity check the backend glyph contents */
    _cairo_debug_check_image_surface_is_defined (&surface->base);
    scaled_glyph->color_surface = surface;
    scaled_glyph->recording_uses_foreground_marker = foreground_marker_color != NULL;
    if (foreground_marker_color)
	scaled_glyph->foreground_color = *foreground_marker_color;

    if (surface != NULL)
	scaled_glyph->has_info |= CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE;
    else
	scaled_glyph->has_info &= ~CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE;
}

/* _cairo_hash_table_random_entry () predicate. To avoid race conditions,
 * the font is locked when tested. The font is unlocked in
 * _cairo_scaled_glyph_page_pluck. */
static cairo_bool_t
_cairo_scaled_glyph_page_can_remove (const void *closure)
{
    const cairo_scaled_glyph_page_t *page = closure;
    cairo_scaled_font_t *scaled_font;

    scaled_font = page->scaled_font;

    if (!CAIRO_MUTEX_TRY_LOCK (scaled_font->mutex))
       return FALSE;

    if (scaled_font->cache_frozen != 0) {
       CAIRO_MUTEX_UNLOCK (scaled_font->mutex);
       return FALSE;
    }

    return TRUE;
}

static cairo_status_t
_cairo_scaled_font_allocate_glyph (cairo_scaled_font_t *scaled_font,
				   cairo_scaled_glyph_t **scaled_glyph)
{
    cairo_scaled_glyph_page_t *page;
    cairo_status_t status;

    assert (scaled_font->cache_frozen);

    /* only the first page in the list may contain available slots */
    if (! cairo_list_is_empty (&scaled_font->glyph_pages)) {
        page = cairo_list_last_entry (&scaled_font->glyph_pages,
                                      cairo_scaled_glyph_page_t,
                                      link);
        if (page->num_glyphs < CAIRO_SCALED_GLYPH_PAGE_SIZE) {
            *scaled_glyph = &page->glyphs[page->num_glyphs++];
            return CAIRO_STATUS_SUCCESS;
        }
    }

    page = _cairo_malloc (sizeof (cairo_scaled_glyph_page_t));
    if (unlikely (page == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    page->cache_entry.hash = (uintptr_t) scaled_font;
    page->scaled_font = scaled_font;
    page->cache_entry.size = 1; /* XXX occupancy weighting? */
    page->num_glyphs = 0;

    CAIRO_MUTEX_LOCK (_cairo_scaled_glyph_page_cache_mutex);
    if (scaled_font->global_cache_frozen == FALSE) {
	if (unlikely (cairo_scaled_glyph_page_cache.hash_table == NULL)) {
	    status = _cairo_cache_init (&cairo_scaled_glyph_page_cache,
					NULL,
					_cairo_scaled_glyph_page_can_remove,
					_cairo_scaled_glyph_page_pluck,
					MAX_GLYPH_PAGES_CACHED);
	    if (unlikely (status)) {
		CAIRO_MUTEX_UNLOCK (_cairo_scaled_glyph_page_cache_mutex);
		free (page);
		return status;
	    }
	}

	_cairo_cache_freeze (&cairo_scaled_glyph_page_cache);
	scaled_font->global_cache_frozen = TRUE;
    }

    status = _cairo_cache_insert (&cairo_scaled_glyph_page_cache,
				  &page->cache_entry);
    CAIRO_MUTEX_UNLOCK (_cairo_scaled_glyph_page_cache_mutex);
    if (unlikely (status)) {
	free (page);
	return status;
    }

    cairo_list_add_tail (&page->link, &scaled_font->glyph_pages);

    *scaled_glyph = &page->glyphs[page->num_glyphs++];
    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_scaled_font_free_last_glyph (cairo_scaled_font_t *scaled_font,
			           cairo_scaled_glyph_t *scaled_glyph)
{
    cairo_scaled_glyph_page_t *page;

    assert (scaled_font->cache_frozen);
    assert (! cairo_list_is_empty (&scaled_font->glyph_pages));
    page = cairo_list_last_entry (&scaled_font->glyph_pages,
                                  cairo_scaled_glyph_page_t,
                                  link);
    assert (scaled_glyph == &page->glyphs[page->num_glyphs-1]);

    _cairo_scaled_glyph_fini (scaled_font, scaled_glyph);

    if (--page->num_glyphs == 0) {
	_cairo_scaled_font_thaw_cache (scaled_font);
	CAIRO_MUTEX_LOCK (scaled_font->mutex);

	CAIRO_MUTEX_LOCK (_cairo_scaled_glyph_page_cache_mutex);
	/* Temporarily disconnect callback to avoid recursive locking */
	cairo_scaled_glyph_page_cache.entry_destroy = NULL;
	_cairo_cache_remove (&cairo_scaled_glyph_page_cache,
		             &page->cache_entry);
	_cairo_scaled_glyph_page_destroy (scaled_font, page);
	cairo_scaled_glyph_page_cache.entry_destroy = _cairo_scaled_glyph_page_pluck;
	CAIRO_MUTEX_UNLOCK (_cairo_scaled_glyph_page_cache_mutex);

	CAIRO_MUTEX_UNLOCK (scaled_font->mutex);
	_cairo_scaled_font_freeze_cache (scaled_font);
    }
}

/**
 * _cairo_scaled_glyph_lookup:
 * @scaled_font: a #cairo_scaled_font_t
 * @index: the glyph to create
 * @info: a #cairo_scaled_glyph_info_t marking which portions of
 * the glyph should be filled in.
 * @foreground_color - foreground color to use when rendering color
 * fonts. Use NULL if not requesting
 * CAIRO_SCALED_GLYPH_INFO_RECORDING_SURFACE or
 * CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE, or foreground color is
 * unknown.
 * @scaled_glyph_ret: a #cairo_scaled_glyph_t where the glyph
 * is returned.
 *
 * If the desired info is not available, (for example, when trying to
 * get INFO_PATH with a bitmapped font), this function will return
 * %CAIRO_INT_STATUS_UNSUPPORTED.
 *
 * Note: This function must be called with the scaled font frozen, and it must
 * remain frozen for as long as the @scaled_glyph_ret is alive. (If the scaled
 * font was not frozen, then there is no guarantee that the glyph would not be
 * evicted before you tried to access it.) See
 * _cairo_scaled_font_freeze_cache() and _cairo_scaled_font_thaw_cache().
 *
 * Returns: a glyph with the requested portions filled in. Glyph
 * lookup is cached and glyph will be automatically freed along
 * with the scaled_font so no explicit free is required.
 * @info can be one or more of:
 *  %CAIRO_SCALED_GLYPH_INFO_METRICS - glyph metrics and bounding box
 *  %CAIRO_SCALED_GLYPH_INFO_SURFACE - surface holding glyph image
 *  %CAIRO_SCALED_GLYPH_INFO_PATH - path holding glyph outline in device space
 *  %CAIRO_SCALED_GLYPH_INFO_RECORDING_SURFACE - surface holding recording of glyph
 *  %CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE - surface holding color glyph image
 **/
cairo_int_status_t
_cairo_scaled_glyph_lookup (cairo_scaled_font_t *scaled_font,
			    unsigned long index,
			    cairo_scaled_glyph_info_t info,
			    const cairo_color_t   *foreground_color,
			    cairo_scaled_glyph_t **scaled_glyph_ret)
{
    cairo_int_status_t		 status = CAIRO_INT_STATUS_SUCCESS;
    cairo_scaled_glyph_t	*scaled_glyph;
    cairo_scaled_glyph_info_t	 need_info;
    cairo_hash_entry_t           key;

    *scaled_glyph_ret = NULL;

    if (unlikely (scaled_font->status))
	return scaled_font->status;

    assert (CAIRO_MUTEX_IS_LOCKED(scaled_font->mutex));
    assert (scaled_font->cache_frozen);

    if (CAIRO_INJECT_FAULT ())
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    if (foreground_color == NULL)
	foreground_color = CAIRO_COLOR_BLACK;

    /*
     * Check cache for glyph
     */
    key.hash = index;
    scaled_glyph = _cairo_hash_table_lookup (scaled_font->glyphs, &key);
    if (scaled_glyph == NULL) {
	status = _cairo_scaled_font_allocate_glyph (scaled_font, &scaled_glyph);
	if (unlikely (status))
	    goto err;

	memset (scaled_glyph, 0, sizeof (cairo_scaled_glyph_t));
	_cairo_scaled_glyph_set_index (scaled_glyph, index);
	cairo_list_init (&scaled_glyph->dev_privates);

	/* ask backend to initialize metrics and shape fields */
	status =
	    scaled_font->backend->scaled_glyph_init (scaled_font,
						     scaled_glyph,
						     info | CAIRO_SCALED_GLYPH_INFO_METRICS,
						     foreground_color);
	if (unlikely (status)) {
	    _cairo_scaled_font_free_last_glyph (scaled_font, scaled_glyph);
	    goto err;
	}

	status = _cairo_hash_table_insert (scaled_font->glyphs,
					   &scaled_glyph->hash_entry);
	if (unlikely (status)) {
	    _cairo_scaled_font_free_last_glyph (scaled_font, scaled_glyph);
	    goto err;
	}
    }

    /*
     * Check and see if the glyph, as provided,
     * already has the requested data and amend it if not
     */
    need_info = info & ~scaled_glyph->has_info;

    /* If this is not a color glyph, don't try loading the color surface again. */
    if ((need_info & CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE) &&
	scaled_glyph->color_glyph_set && !scaled_glyph->color_glyph)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    /* If requesting a color surface or recording for a glyph that has
     * used the foreground color to render the recording, and the
     * foreground color has changed, request a new  recording. */
    if ((info & (CAIRO_SCALED_GLYPH_INFO_RECORDING_SURFACE | CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE)) &&
	scaled_glyph->recording_uses_foreground_color &&
	!_cairo_color_equal (foreground_color, &scaled_glyph->foreground_color))
    {
	need_info |= CAIRO_SCALED_GLYPH_INFO_RECORDING_SURFACE;
    }

    /* If requesting a color surface for a glyph that has
     * used the foreground color to render the color_surface, and the
     * foreground color has changed, request a new image. */
    if (info & CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE &&
	(scaled_glyph->recording_uses_foreground_marker || scaled_glyph->recording_uses_foreground_color) &&
	!_cairo_color_equal (foreground_color, &scaled_glyph->foreground_color))
    {
	    need_info |= CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE;
    }

    if (need_info) {
	status = scaled_font->backend->scaled_glyph_init (scaled_font,
							  scaled_glyph,
							  need_info,
							  foreground_color);
	if (unlikely (status))
	    goto err;

	/* Don't trust the scaled_glyph_init() return value, the font
	 * backend may not even know about some of the info.  For example,
	 * no backend other than the user-fonts knows about recording-surface
	 * glyph info. */
	if (info & ~scaled_glyph->has_info)
	    return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    *scaled_glyph_ret = scaled_glyph;
    return CAIRO_STATUS_SUCCESS;

err:
    /* It's not an error for the backend to not support the info we want. */
    if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	status = _cairo_scaled_font_set_error (scaled_font, status);
    return status;
}

double
_cairo_scaled_font_get_max_scale (cairo_scaled_font_t *scaled_font)
{
    return scaled_font->max_scale;
}


/**
 * cairo_scaled_font_get_font_face:
 * @scaled_font: a #cairo_scaled_font_t
 *
 * Gets the font face that this scaled font uses.  This might be the
 * font face passed to cairo_scaled_font_create(), but this does not
 * hold true for all possible cases.
 *
 * Return value: The #cairo_font_face_t with which @scaled_font was
 * created.  This object is owned by cairo. To keep a reference to it,
 * you must call cairo_scaled_font_reference().
 *
 * Since: 1.2
 **/
cairo_font_face_t *
cairo_scaled_font_get_font_face (cairo_scaled_font_t *scaled_font)
{
    if (scaled_font->status)
	return (cairo_font_face_t*) &_cairo_font_face_nil;

    if (scaled_font->original_font_face != NULL)
	return scaled_font->original_font_face;

    return scaled_font->font_face;
}
slim_hidden_def (cairo_scaled_font_get_font_face);

/**
 * cairo_scaled_font_get_font_matrix:
 * @scaled_font: a #cairo_scaled_font_t
 * @font_matrix: return value for the matrix
 *
 * Stores the font matrix with which @scaled_font was created into
 * @matrix.
 *
 * Since: 1.2
 **/
void
cairo_scaled_font_get_font_matrix (cairo_scaled_font_t	*scaled_font,
				   cairo_matrix_t	*font_matrix)
{
    if (scaled_font->status) {
	cairo_matrix_init_identity (font_matrix);
	return;
    }

    *font_matrix = scaled_font->font_matrix;
}
slim_hidden_def (cairo_scaled_font_get_font_matrix);

/**
 * cairo_scaled_font_get_ctm:
 * @scaled_font: a #cairo_scaled_font_t
 * @ctm: return value for the CTM
 *
 * Stores the CTM with which @scaled_font was created into @ctm.
 * Note that the translation offsets (x0, y0) of the CTM are ignored
 * by cairo_scaled_font_create().  So, the matrix this
 * function returns always has 0,0 as x0,y0.
 *
 * Since: 1.2
 **/
void
cairo_scaled_font_get_ctm (cairo_scaled_font_t	*scaled_font,
			   cairo_matrix_t	*ctm)
{
    if (scaled_font->status) {
	cairo_matrix_init_identity (ctm);
	return;
    }

    *ctm = scaled_font->ctm;
}
slim_hidden_def (cairo_scaled_font_get_ctm);

/**
 * cairo_scaled_font_get_scale_matrix:
 * @scaled_font: a #cairo_scaled_font_t
 * @scale_matrix: return value for the matrix
 *
 * Stores the scale matrix of @scaled_font into @matrix.
 * The scale matrix is product of the font matrix and the ctm
 * associated with the scaled font, and hence is the matrix mapping from
 * font space to device space.
 *
 * Since: 1.8
 **/
void
cairo_scaled_font_get_scale_matrix (cairo_scaled_font_t	*scaled_font,
				    cairo_matrix_t	*scale_matrix)
{
    if (scaled_font->status) {
	cairo_matrix_init_identity (scale_matrix);
	return;
    }

    *scale_matrix = scaled_font->scale;
}

/**
 * cairo_scaled_font_get_font_options:
 * @scaled_font: a #cairo_scaled_font_t
 * @options: return value for the font options
 *
 * Stores the font options with which @scaled_font was created into
 * @options.
 *
 * Since: 1.2
 **/
void
cairo_scaled_font_get_font_options (cairo_scaled_font_t		*scaled_font,
				    cairo_font_options_t	*options)
{
    if (cairo_font_options_status (options))
	return;

    if (scaled_font->status) {
	_cairo_font_options_init_default (options);
	return;
    }

    _cairo_font_options_fini (options);
    _cairo_font_options_init_copy (options, &scaled_font->options);
}
slim_hidden_def (cairo_scaled_font_get_font_options);

cairo_bool_t
_cairo_scaled_font_has_color_glyphs (cairo_scaled_font_t *scaled_font)
{
    if (scaled_font->backend != NULL && scaled_font->backend->has_color_glyphs != NULL)
        return scaled_font->backend->has_color_glyphs (scaled_font);
    else
       return FALSE;
}
