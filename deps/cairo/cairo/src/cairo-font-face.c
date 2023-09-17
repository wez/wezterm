/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2005 Red Hat Inc.
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
 *	Carl D. Worth <cworth@cworth.org>
 *      Graydon Hoare <graydon@redhat.com>
 *      Owen Taylor <otaylor@redhat.com>
 */

#include "cairoint.h"
#include "cairo-error-private.h"

/**
 * SECTION:cairo-font-face
 * @Title: cairo_font_face_t
 * @Short_Description: Base class for font faces
 * @See_Also: #cairo_scaled_font_t
 *
 * #cairo_font_face_t represents a particular font at a particular weight,
 * slant, and other characteristic but no size, transformation, or size.
 *
 * Font faces are created using <firstterm>font-backend</firstterm>-specific
 * constructors, typically of the form
 * <function>cairo_<emphasis>backend</emphasis>_font_face_create(<!-- -->)</function>,
 * or implicitly using the <firstterm>toy</firstterm> text API by way of
 * cairo_select_font_face().  The resulting face can be accessed using
 * cairo_get_font_face().
 **/

/* #cairo_font_face_t */

const cairo_font_face_t _cairo_font_face_nil = {
    { 0 },				/* hash_entry */
    CAIRO_STATUS_NO_MEMORY,		/* status */
    CAIRO_REFERENCE_COUNT_INVALID,	/* ref_count */
    { 0, 0, 0, NULL },			/* user_data */
    NULL
};
const cairo_font_face_t _cairo_font_face_nil_file_not_found = {
    { 0 },				/* hash_entry */
    CAIRO_STATUS_FILE_NOT_FOUND,	/* status */
    CAIRO_REFERENCE_COUNT_INVALID,	/* ref_count */
    { 0, 0, 0, NULL },			/* user_data */
    NULL
};

cairo_status_t
_cairo_font_face_set_error (cairo_font_face_t *font_face,
	                    cairo_status_t     status)
{
    if (status == CAIRO_STATUS_SUCCESS)
	return status;

    /* Don't overwrite an existing error. This preserves the first
     * error, which is the most significant. */
    _cairo_status_set_error (&font_face->status, status);

    return _cairo_error (status);
}

void
_cairo_font_face_init (cairo_font_face_t               *font_face,
		       const cairo_font_face_backend_t *backend)
{
    CAIRO_MUTEX_INITIALIZE ();

    font_face->status = CAIRO_STATUS_SUCCESS;
    CAIRO_REFERENCE_COUNT_INIT (&font_face->ref_count, 1);
    font_face->backend = backend;

    _cairo_user_data_array_init (&font_face->user_data);
}

/**
 * cairo_font_face_reference:
 * @font_face: a #cairo_font_face_t, (may be %NULL in which case this
 * function does nothing).
 *
 * Increases the reference count on @font_face by one. This prevents
 * @font_face from being destroyed until a matching call to
 * cairo_font_face_destroy() is made.
 *
 * Use cairo_font_face_get_reference_count() to get the number of
 * references to a #cairo_font_face_t.
 *
 * Return value: the referenced #cairo_font_face_t.
 *
 * Since: 1.0
 **/
cairo_font_face_t *
cairo_font_face_reference (cairo_font_face_t *font_face)
{
    if (font_face == NULL ||
	CAIRO_REFERENCE_COUNT_IS_INVALID (&font_face->ref_count))
	return font_face;

    /* We would normally assert that we have a reference here but we
     * can't get away with that due to the zombie case as documented
     * in _cairo_ft_font_face_destroy. */

    _cairo_reference_count_inc (&font_face->ref_count);

    return font_face;
}
slim_hidden_def (cairo_font_face_reference);

static inline cairo_bool_t
__put(cairo_reference_count_t *v)
{
    int c, old;

    c = CAIRO_REFERENCE_COUNT_GET_VALUE(v);
    while (c != 1 && (old = _cairo_atomic_int_cmpxchg_return_old(&v->ref_count, c, c - 1)) != c)
	c = old;

    return c != 1;
}

cairo_bool_t
_cairo_font_face_destroy (void *abstract_face)
{
#if 0 /* Nothing needs to be done, we can just drop the last reference */
    cairo_font_face_t *font_face = abstract_face;
    return _cairo_reference_count_dec_and_test (&font_face->ref_count);
#endif
    return TRUE;
}

/**
 * cairo_font_face_destroy:
 * @font_face: a #cairo_font_face_t
 *
 * Decreases the reference count on @font_face by one. If the result
 * is zero, then @font_face and all associated resources are freed.
 * See cairo_font_face_reference().
 *
 * Since: 1.0
 **/
void
cairo_font_face_destroy (cairo_font_face_t *font_face)
{
    if (font_face == NULL ||
	CAIRO_REFERENCE_COUNT_IS_INVALID (&font_face->ref_count))
	return;

    assert (CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&font_face->ref_count));

    /* We allow resurrection to deal with some memory management for the
     * FreeType backend where cairo_ft_font_face_t and cairo_ft_unscaled_font_t
     * need to effectively mutually reference each other
     */
    if (__put (&font_face->ref_count))
	return;

    if (! font_face->backend->destroy (font_face))
	return;

    _cairo_user_data_array_fini (&font_face->user_data);

    free (font_face);
}
slim_hidden_def (cairo_font_face_destroy);

/**
 * cairo_font_face_get_type:
 * @font_face: a font face
 *
 * This function returns the type of the backend used to create
 * a font face. See #cairo_font_type_t for available types.
 *
 * Return value: The type of @font_face.
 *
 * Since: 1.2
 **/
cairo_font_type_t
cairo_font_face_get_type (cairo_font_face_t *font_face)
{
    if (CAIRO_REFERENCE_COUNT_IS_INVALID (&font_face->ref_count))
	return CAIRO_FONT_TYPE_TOY;

    return font_face->backend->type;
}

/**
 * cairo_font_face_get_reference_count:
 * @font_face: a #cairo_font_face_t
 *
 * Returns the current reference count of @font_face.
 *
 * Return value: the current reference count of @font_face.  If the
 * object is a nil object, 0 will be returned.
 *
 * Since: 1.4
 **/
unsigned int
cairo_font_face_get_reference_count (cairo_font_face_t *font_face)
{
    if (font_face == NULL ||
	CAIRO_REFERENCE_COUNT_IS_INVALID (&font_face->ref_count))
	return 0;

    return CAIRO_REFERENCE_COUNT_GET_VALUE (&font_face->ref_count);
}

/**
 * cairo_font_face_status:
 * @font_face: a #cairo_font_face_t
 *
 * Checks whether an error has previously occurred for this
 * font face
 *
 * Return value: %CAIRO_STATUS_SUCCESS or another error such as
 *   %CAIRO_STATUS_NO_MEMORY.
 *
 * Since: 1.0
 **/
cairo_status_t
cairo_font_face_status (cairo_font_face_t *font_face)
{
    return font_face->status;
}

/**
 * cairo_font_face_get_user_data:
 * @font_face: a #cairo_font_face_t
 * @key: the address of the #cairo_user_data_key_t the user data was
 * attached to
 *
 * Return user data previously attached to @font_face using the specified
 * key.  If no user data has been attached with the given key this
 * function returns %NULL.
 *
 * Return value: the user data previously attached or %NULL.
 *
 * Since: 1.0
 **/
void *
cairo_font_face_get_user_data (cairo_font_face_t	   *font_face,
			       const cairo_user_data_key_t *key)
{
    return _cairo_user_data_array_get_data (&font_face->user_data,
					    key);
}
slim_hidden_def (cairo_font_face_get_user_data);

/**
 * cairo_font_face_set_user_data:
 * @font_face: a #cairo_font_face_t
 * @key: the address of a #cairo_user_data_key_t to attach the user data to
 * @user_data: the user data to attach to the font face
 * @destroy: a #cairo_destroy_func_t which will be called when the
 * font face is destroyed or when new user data is attached using the
 * same key.
 *
 * Attach user data to @font_face.  To remove user data from a font face,
 * call this function with the key that was used to set it and %NULL
 * for @data.
 *
 * Return value: %CAIRO_STATUS_SUCCESS or %CAIRO_STATUS_NO_MEMORY if a
 * slot could not be allocated for the user data.
 *
 * Since: 1.0
 **/
cairo_status_t
cairo_font_face_set_user_data (cairo_font_face_t	   *font_face,
			       const cairo_user_data_key_t *key,
			       void			   *user_data,
			       cairo_destroy_func_t	    destroy)
{
    if (CAIRO_REFERENCE_COUNT_IS_INVALID (&font_face->ref_count))
	return font_face->status;

    return _cairo_user_data_array_set_data (&font_face->user_data,
					    key, user_data, destroy);
}
slim_hidden_def (cairo_font_face_set_user_data);

void
_cairo_unscaled_font_init (cairo_unscaled_font_t               *unscaled_font,
			   const cairo_unscaled_font_backend_t *backend)
{
    CAIRO_REFERENCE_COUNT_INIT (&unscaled_font->ref_count, 1);
    unscaled_font->backend = backend;
}

cairo_unscaled_font_t *
_cairo_unscaled_font_reference (cairo_unscaled_font_t *unscaled_font)
{
    if (unscaled_font == NULL)
	return NULL;

    assert (CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&unscaled_font->ref_count));

    _cairo_reference_count_inc (&unscaled_font->ref_count);

    return unscaled_font;
}

void
_cairo_unscaled_font_destroy (cairo_unscaled_font_t *unscaled_font)
{
    if (unscaled_font == NULL)
	return;

    assert (CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&unscaled_font->ref_count));

    if (__put (&unscaled_font->ref_count))
	return;

    if (! unscaled_font->backend->destroy (unscaled_font))
	return;

    free (unscaled_font);
}
