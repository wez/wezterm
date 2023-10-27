/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
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
 * The Initial Developer of the Original Code is University of Southern
 * California.
 *
 * Contributor(s):
 *	Carl D. Worth <cworth@cworth.org>
 */

#include "cairoint.h"

#include "cairo-array-private.h"
#include "cairo-clip-inline.h"
#include "cairo-clip-private.h"
#include "cairo-damage-private.h"
#include "cairo-device-private.h"
#include "cairo-error-private.h"
#include "cairo-list-inline.h"
#include "cairo-image-surface-inline.h"
#include "cairo-recording-surface-private.h"
#include "cairo-region-private.h"
#include "cairo-surface-inline.h"
#include "cairo-tee-surface-private.h"

/**
 * SECTION:cairo-surface
 * @Title: cairo_surface_t
 * @Short_Description: Base class for surfaces
 * @See_Also: #cairo_t, #cairo_pattern_t
 *
 * #cairo_surface_t is the abstract type representing all different drawing
 * targets that cairo can render to.  The actual drawings are
 * performed using a cairo <firstterm>context</firstterm>.
 *
 * A cairo surface is created by using <firstterm>backend</firstterm>-specific
 * constructors, typically of the form
 * <function>cairo_<emphasis>backend</emphasis>_surface_create(<!-- -->)</function>.
 *
 * Most surface types allow accessing the surface without using Cairo
 * functions. If you do this, keep in mind that it is mandatory that you call
 * cairo_surface_flush() before reading from or writing to the surface and that
 * you must use cairo_surface_mark_dirty() after modifying it.
 * <example>
 * <title>Directly modifying an image surface</title>
 * <programlisting>
 * void
 * modify_image_surface (cairo_surface_t *surface)
 * {
 *   unsigned char *data;
 *   int width, height, stride;
 *
 *   // flush to ensure all writing to the image was done
 *   cairo_surface_flush (surface);
 *
 *   // modify the image
 *   data = cairo_image_surface_get_data (surface);
 *   width = cairo_image_surface_get_width (surface);
 *   height = cairo_image_surface_get_height (surface);
 *   stride = cairo_image_surface_get_stride (surface);
 *   modify_image_data (data, width, height, stride);
 *
 *   // mark the image dirty so Cairo clears its caches.
 *   cairo_surface_mark_dirty (surface);
 * }
 * </programlisting>
 * </example>
 * Note that for other surface types it might be necessary to acquire the
 * surface's device first. See cairo_device_acquire() for a discussion of
 * devices.
 **/

#define DEFINE_NIL_SURFACE(status, name)			\
const cairo_surface_t name = {					\
    NULL,				/* backend */		\
    NULL,				/* device */		\
    CAIRO_SURFACE_TYPE_IMAGE,		/* type */		\
    CAIRO_CONTENT_COLOR,		/* content */		\
    CAIRO_REFERENCE_COUNT_INVALID,	/* ref_count */		\
    status,				/* status */		\
    0,					/* unique id */		\
    0,					/* serial */		\
    NULL,				/* damage */		\
    FALSE,				/* _finishing */	\
    FALSE,				/* finished */		\
    TRUE,				/* is_clear */		\
    FALSE,				/* has_font_options */	\
    FALSE,				/* owns_device */ \
    FALSE,                              /* is_vector */ \
    { 0, 0, 0, NULL, },			/* user_data */		\
    { 0, 0, 0, NULL, },			/* mime_data */         \
    { 1.0, 0.0, 0.0, 1.0, 0.0, 0.0 },   /* device_transform */	\
    { 1.0, 0.0,	0.0, 1.0, 0.0, 0.0 },	/* device_transform_inverse */	\
    { NULL, NULL },			/* device_transform_observers */ \
    0.0,				/* x_resolution */	\
    0.0,				/* y_resolution */	\
    0.0,				/* x_fallback_resolution */	\
    0.0,				/* y_fallback_resolution */	\
    NULL,				/* snapshot_of */	\
    NULL,				/* snapshot_detach */	\
    { NULL, NULL },			/* snapshots */		\
    { NULL, NULL },			/* snapshot */		\
    { CAIRO_ANTIALIAS_DEFAULT,		/* antialias */		\
      CAIRO_SUBPIXEL_ORDER_DEFAULT,	/* subpixel_order */	\
      CAIRO_LCD_FILTER_DEFAULT,		/* lcd_filter */	\
      CAIRO_HINT_STYLE_DEFAULT,		/* hint_style */	\
      CAIRO_HINT_METRICS_DEFAULT,	/* hint_metrics */	\
      CAIRO_ROUND_GLYPH_POS_DEFAULT	/* round_glyph_positions */	\
    },					/* font_options */		\
    NULL,                               /* foreground_source */		\
    FALSE,                              /* foreground_used */   \
}

/* XXX error object! */

static DEFINE_NIL_SURFACE(CAIRO_STATUS_NO_MEMORY, _cairo_surface_nil);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_SURFACE_TYPE_MISMATCH, _cairo_surface_nil_surface_type_mismatch);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_INVALID_STATUS, _cairo_surface_nil_invalid_status);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_INVALID_CONTENT, _cairo_surface_nil_invalid_content);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_INVALID_FORMAT, _cairo_surface_nil_invalid_format);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_INVALID_VISUAL, _cairo_surface_nil_invalid_visual);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_FILE_NOT_FOUND, _cairo_surface_nil_file_not_found);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_TEMP_FILE_ERROR, _cairo_surface_nil_temp_file_error);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_READ_ERROR, _cairo_surface_nil_read_error);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_WRITE_ERROR, _cairo_surface_nil_write_error);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_INVALID_STRIDE, _cairo_surface_nil_invalid_stride);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_INVALID_SIZE, _cairo_surface_nil_invalid_size);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_DEVICE_TYPE_MISMATCH, _cairo_surface_nil_device_type_mismatch);
static DEFINE_NIL_SURFACE(CAIRO_STATUS_DEVICE_ERROR, _cairo_surface_nil_device_error);

static DEFINE_NIL_SURFACE(CAIRO_INT_STATUS_UNSUPPORTED, _cairo_surface_nil_unsupported);
static DEFINE_NIL_SURFACE(CAIRO_INT_STATUS_NOTHING_TO_DO, _cairo_surface_nil_nothing_to_do);

static void _cairo_surface_finish_snapshots (cairo_surface_t *surface);
static void _cairo_surface_finish (cairo_surface_t *surface);

/**
 * _cairo_surface_set_error:
 * @surface: a surface
 * @status: a status value indicating an error
 *
 * Atomically sets surface->status to @status and calls _cairo_error;
 * Does nothing if status is %CAIRO_STATUS_SUCCESS or any of the internal
 * status values.
 *
 * All assignments of an error status to surface->status should happen
 * through _cairo_surface_set_error(). Note that due to the nature of
 * the atomic operation, it is not safe to call this function on the
 * nil objects.
 *
 * The purpose of this function is to allow the user to set a
 * breakpoint in _cairo_error() to generate a stack trace for when the
 * user causes cairo to detect an error.
 *
 * Return value: the error status.
 **/
cairo_int_status_t
_cairo_surface_set_error (cairo_surface_t *surface,
			  cairo_int_status_t status)
{
    /* NOTHING_TO_DO is magic. We use it to break out of the inner-most
     * surface function, but anything higher just sees "success".
     */
    if (status == CAIRO_INT_STATUS_NOTHING_TO_DO)
	status = CAIRO_INT_STATUS_SUCCESS;

    if (status == CAIRO_INT_STATUS_SUCCESS ||
        status >= (int)CAIRO_INT_STATUS_LAST_STATUS)
        return status;

    /* Don't overwrite an existing error. This preserves the first
     * error, which is the most significant. */
    _cairo_status_set_error (&surface->status, (cairo_status_t)status);

    return _cairo_error (status);
}

/**
 * cairo_surface_get_type:
 * @surface: a #cairo_surface_t
 *
 * This function returns the type of the backend used to create
 * a surface. See #cairo_surface_type_t for available types.
 *
 * Return value: The type of @surface.
 *
 * Since: 1.2
 **/
cairo_surface_type_t
cairo_surface_get_type (cairo_surface_t *surface)
{
    /* We don't use surface->backend->type here so that some of the
     * special "wrapper" surfaces such as cairo_paginated_surface_t
     * can override surface->type with the type of the "child"
     * surface. */
    return surface->type;
}

/**
 * cairo_surface_get_content:
 * @surface: a #cairo_surface_t
 *
 * This function returns the content type of @surface which indicates
 * whether the surface contains color and/or alpha information. See
 * #cairo_content_t.
 *
 * Return value: The content type of @surface.
 *
 * Since: 1.2
 **/
cairo_content_t
cairo_surface_get_content (cairo_surface_t *surface)
{
    return surface->content;
}

/**
 * cairo_surface_status:
 * @surface: a #cairo_surface_t
 *
 * Checks whether an error has previously occurred for this
 * surface.
 *
 * Return value: %CAIRO_STATUS_SUCCESS, %CAIRO_STATUS_NULL_POINTER,
 * %CAIRO_STATUS_NO_MEMORY, %CAIRO_STATUS_READ_ERROR,
 * %CAIRO_STATUS_INVALID_CONTENT, %CAIRO_STATUS_INVALID_FORMAT, or
 * %CAIRO_STATUS_INVALID_VISUAL.
 *
 * Since: 1.0
 **/
cairo_status_t
cairo_surface_status (cairo_surface_t *surface)
{
    return surface->status;
}
slim_hidden_def (cairo_surface_status);

static unsigned int
_cairo_surface_allocate_unique_id (void)
{
    static cairo_atomic_int_t unique_id;

#if CAIRO_NO_MUTEX
    if (++unique_id == 0)
	unique_id = 1;
    return unique_id;
#else
    cairo_atomic_int_t old, id;

    do {
	old = _cairo_atomic_uint_get (&unique_id);
	id = old + 1;
	if (id == 0)
	    id = 1;
    } while (! _cairo_atomic_uint_cmpxchg (&unique_id, old, id));

    return id;
#endif
}

/**
 * cairo_surface_get_device:
 * @surface: a #cairo_surface_t
 *
 * This function returns the device for a @surface.
 * See #cairo_device_t.
 *
 * Return value: The device for @surface or %NULL if the surface does
 *               not have an associated device.
 *
 * Since: 1.10
 **/
cairo_device_t *
cairo_surface_get_device (cairo_surface_t *surface)
{
    if (unlikely (surface->status))
	return _cairo_device_create_in_error (surface->status);

    return surface->device;
}

static cairo_bool_t
_cairo_surface_has_snapshots (cairo_surface_t *surface)
{
    return ! cairo_list_is_empty (&surface->snapshots);
}

static cairo_bool_t
_cairo_surface_has_mime_data (cairo_surface_t *surface)
{
    return surface->mime_data.num_elements != 0;
}

static void
_cairo_surface_detach_mime_data (cairo_surface_t *surface)
{
    if (! _cairo_surface_has_mime_data (surface))
	return;

    _cairo_user_data_array_fini (&surface->mime_data);
    _cairo_user_data_array_init (&surface->mime_data);
}

static void
_cairo_surface_detach_snapshots (cairo_surface_t *surface)
{
    while (_cairo_surface_has_snapshots (surface)) {
	_cairo_surface_detach_snapshot (cairo_list_first_entry (&surface->snapshots,
								cairo_surface_t,
								snapshot));
    }
}

void
_cairo_surface_detach_snapshot (cairo_surface_t *snapshot)
{
    assert (snapshot->snapshot_of != NULL);

    snapshot->snapshot_of = NULL;
    cairo_list_del (&snapshot->snapshot);

    if (snapshot->snapshot_detach != NULL)
	snapshot->snapshot_detach (snapshot);

    cairo_surface_destroy (snapshot);
}

void
_cairo_surface_attach_snapshot (cairo_surface_t *surface,
				 cairo_surface_t *snapshot,
				 cairo_surface_func_t detach_func)
{
    assert (surface != snapshot);
    assert (snapshot->snapshot_of != surface);

    cairo_surface_reference (snapshot);

    if (snapshot->snapshot_of != NULL)
	_cairo_surface_detach_snapshot (snapshot);

    snapshot->snapshot_of = surface;
    snapshot->snapshot_detach = detach_func;

    cairo_list_add (&snapshot->snapshot, &surface->snapshots);

    assert (_cairo_surface_has_snapshot (surface, snapshot->backend) == snapshot);
}

cairo_surface_t *
_cairo_surface_has_snapshot (cairo_surface_t *surface,
			     const cairo_surface_backend_t *backend)
{
    cairo_surface_t *snapshot;

    cairo_list_foreach_entry (snapshot, cairo_surface_t,
			      &surface->snapshots, snapshot)
    {
	if (snapshot->backend == backend)
	    return snapshot;
    }

    return NULL;
}

cairo_status_t
_cairo_surface_begin_modification (cairo_surface_t *surface)
{
    assert (surface->status == CAIRO_STATUS_SUCCESS);
    assert (! surface->finished);

    return _cairo_surface_flush (surface, 1);
}

void
_cairo_surface_init (cairo_surface_t			*surface,
		     const cairo_surface_backend_t	*backend,
		     cairo_device_t			*device,
		     cairo_content_t			 content,
		     cairo_bool_t                        is_vector)
{
    CAIRO_MUTEX_INITIALIZE ();

    surface->backend = backend;
    surface->device = cairo_device_reference (device);
    surface->content = content;
    surface->type = backend->type;
    surface->is_vector = is_vector;

    CAIRO_REFERENCE_COUNT_INIT (&surface->ref_count, 1);
    surface->status = CAIRO_STATUS_SUCCESS;
    surface->unique_id = _cairo_surface_allocate_unique_id ();
    surface->finished = FALSE;
    surface->_finishing = FALSE;
    surface->is_clear = FALSE;
    surface->serial = 0;
    surface->damage = NULL;
    surface->owns_device = (device != NULL);

    _cairo_user_data_array_init (&surface->user_data);
    _cairo_user_data_array_init (&surface->mime_data);

    cairo_matrix_init_identity (&surface->device_transform);
    cairo_matrix_init_identity (&surface->device_transform_inverse);
    cairo_list_init (&surface->device_transform_observers);

    surface->x_resolution = CAIRO_SURFACE_RESOLUTION_DEFAULT;
    surface->y_resolution = CAIRO_SURFACE_RESOLUTION_DEFAULT;

    surface->x_fallback_resolution = CAIRO_SURFACE_FALLBACK_RESOLUTION_DEFAULT;
    surface->y_fallback_resolution = CAIRO_SURFACE_FALLBACK_RESOLUTION_DEFAULT;

    cairo_list_init (&surface->snapshots);
    surface->snapshot_of = NULL;

    surface->has_font_options = FALSE;

    surface->foreground_source = NULL;
    surface->foreground_used = FALSE;
}

static void
_cairo_surface_copy_similar_properties (cairo_surface_t *surface,
					cairo_surface_t *other)
{
    if (other->has_font_options || other->backend != surface->backend) {
	cairo_font_options_t options;

	cairo_surface_get_font_options (other, &options);
	_cairo_surface_set_font_options (surface, &options);
    }

    cairo_surface_set_fallback_resolution (surface,
					   other->x_fallback_resolution,
					   other->y_fallback_resolution);
}

/**
 * cairo_surface_create_similar:
 * @other: an existing surface used to select the backend of the new surface
 * @content: the content for the new surface
 * @width: width of the new surface, (in device-space units)
 * @height: height of the new surface (in device-space units)
 *
 * Create a new surface that is as compatible as possible with an
 * existing surface. For example the new surface will have the same
 * device scale, fallback resolution and font options as
 * @other. Generally, the new surface will also use the same backend
 * as @other, unless that is not possible for some reason. The type of
 * the returned surface may be examined with
 * cairo_surface_get_type().
 *
 * Initially the surface contents are all 0 (transparent if contents
 * have transparency, black otherwise.)
 *
 * Use cairo_surface_create_similar_image() if you need an image surface
 * which can be painted quickly to the target surface.
 *
 * Return value: a pointer to the newly allocated surface. The caller
 * owns the surface and should call cairo_surface_destroy() when done
 * with it.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" surface if @other is already in an error state
 * or any other error occurs.
 *
 * Since: 1.0
 **/
cairo_surface_t *
cairo_surface_create_similar (cairo_surface_t  *other,
			      cairo_content_t	content,
			      int		width,
			      int		height)
{
    cairo_surface_t *surface;
    cairo_status_t status;
    cairo_solid_pattern_t pattern;

    if (unlikely (other->status))
	return _cairo_surface_create_in_error (other->status);
    if (unlikely (other->finished))
	return _cairo_surface_create_in_error (CAIRO_STATUS_SURFACE_FINISHED);
    if (unlikely (width < 0 || height < 0))
	return _cairo_surface_create_in_error (CAIRO_STATUS_INVALID_SIZE);
    if (unlikely (! CAIRO_CONTENT_VALID (content)))
	return _cairo_surface_create_in_error (CAIRO_STATUS_INVALID_CONTENT);

    /* We inherit the device scale, so create a larger surface */
    width = width * other->device_transform.xx;
    height = height * other->device_transform.yy;

    surface = NULL;
    if (other->backend->create_similar)
	surface = other->backend->create_similar (other, content, width, height);
    if (surface == NULL)
	surface = cairo_surface_create_similar_image (other,
						      _cairo_format_from_content (content),
						      width, height);

    if (unlikely (surface->status))
	return surface;

    _cairo_surface_copy_similar_properties (surface, other);
    cairo_surface_set_device_scale (surface,
				    other->device_transform.xx,
				    other->device_transform.yy);

    if (unlikely (surface->status))
	return surface;

    _cairo_pattern_init_solid (&pattern, CAIRO_COLOR_TRANSPARENT);
    status = _cairo_surface_paint (surface,
				   CAIRO_OPERATOR_CLEAR,
				   &pattern.base, NULL);
    if (unlikely (status)) {
	cairo_surface_destroy (surface);
	surface = _cairo_surface_create_in_error (status);
    }

    assert (surface->is_clear);

    return surface;
}

/**
 * cairo_surface_create_similar_image:
 * @other: an existing surface used to select the preference of the new surface
 * @format: the format for the new surface
 * @width: width of the new surface, (in pixels)
 * @height: height of the new surface (in pixels)
 *
 * Create a new image surface that is as compatible as possible for uploading
 * to and the use in conjunction with an existing surface. However, this surface
 * can still be used like any normal image surface. Unlike
 * cairo_surface_create_similar() the new image surface won't inherit
 * the device scale from @other.
 *
 * Initially the surface contents are all 0 (transparent if contents
 * have transparency, black otherwise.)
 *
 * Use cairo_surface_create_similar() if you don't need an image surface.
 *
 * Return value: a pointer to the newly allocated image surface. The caller
 * owns the surface and should call cairo_surface_destroy() when done
 * with it.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" surface if @other is already in an error state
 * or any other error occurs.
 *
 * Since: 1.12
 **/
cairo_surface_t *
cairo_surface_create_similar_image (cairo_surface_t  *other,
				    cairo_format_t    format,
				    int		width,
				    int		height)
{
    cairo_surface_t *image;

    if (unlikely (other->status))
	return _cairo_surface_create_in_error (other->status);
    if (unlikely (other->finished))
	return _cairo_surface_create_in_error (CAIRO_STATUS_SURFACE_FINISHED);

    if (unlikely (width < 0 || height < 0))
	return _cairo_surface_create_in_error (CAIRO_STATUS_INVALID_SIZE);
    if (unlikely (! CAIRO_FORMAT_VALID (format)))
	return _cairo_surface_create_in_error (CAIRO_STATUS_INVALID_FORMAT);

    image = NULL;
    if (other->backend->create_similar_image)
	image = other->backend->create_similar_image (other,
						      format, width, height);
    if (image == NULL)
	image = cairo_image_surface_create (format, width, height);

    assert (image->is_clear);

    return image;
}
slim_hidden_def (cairo_surface_create_similar_image);

/**
 * _cairo_surface_map_to_image:
 * @surface: an existing surface used to extract the image from
 * @extents: limit the extraction to an rectangular region
 *
 * Returns an image surface that is the most efficient mechanism for
 * modifying the backing store of the target surface. The region
 * retrieved is limited to @extents.
 *
 * Note, the use of the original surface as a target or source whilst
 * it is mapped is undefined. The result of mapping the surface
 * multiple times is undefined. Calling cairo_surface_destroy() or
 * cairo_surface_finish() on the resulting image surface results in
 * undefined behavior. Changing the device transform of the image
 * surface or of @surface before the image surface is unmapped results
 * in undefined behavior.
 *
 * Assumes that @surface is valid (CAIRO_STATUS_SUCCESS,
 * non-finished).
 *
 * Return value: a pointer to the newly allocated image surface. The
 * caller must use _cairo_surface_unmap_image() to destroy this image
 * surface.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" surface if @other is already in an error state
 * or any other error occurs.
 *
 * The returned image might have a %CAIRO_FORMAT_INVALID format.
 **/
cairo_image_surface_t *
_cairo_surface_map_to_image (cairo_surface_t  *surface,
			     const cairo_rectangle_int_t *extents)
{
    cairo_image_surface_t *image = NULL;

    assert (extents != NULL);

    /* TODO: require map_to_image != NULL */
    if (surface->backend->map_to_image)
	image = surface->backend->map_to_image (surface, extents);

    if (image == NULL)
	image = _cairo_image_surface_clone_subimage (surface, extents);

    return image;
}

/**
 * _cairo_surface_unmap_image:
 * @surface: the surface passed to _cairo_surface_map_to_image().
 * @image: the currently mapped image
 *
 * Unmaps the image surface as returned from
 * _cairo_surface_map_to_image().
 *
 * The content of the image will be uploaded to the target surface.
 * Afterwards, the image is destroyed.
 *
 * Using an image surface which wasn't returned by
 * _cairo_surface_map_to_image() results in undefined behavior.
 *
 * An image surface in error status can be passed to
 * _cairo_surface_unmap_image().
 *
 * Return value: the unmap status.
 *
 * Even if the unmap status is not successful, @image is destroyed.
 **/
cairo_int_status_t
_cairo_surface_unmap_image (cairo_surface_t       *surface,
			    cairo_image_surface_t *image)
{
    cairo_surface_pattern_t pattern;
    cairo_rectangle_int_t extents;
    cairo_clip_t *clip;
    cairo_int_status_t status;

    /* map_to_image can return error surfaces */
    if (unlikely (image->base.status)) {
	status = image->base.status;
	goto destroy;
    }

    /* If the image is untouched just skip the update */
    if (image->base.serial == 0) {
	status = CAIRO_STATUS_SUCCESS;
	goto destroy;
    }

    /* TODO: require unmap_image != NULL */
    if (surface->backend->unmap_image &&
	! _cairo_image_surface_is_clone (image))
    {
	status = surface->backend->unmap_image (surface, image);
	if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	    return status;
    }

    _cairo_pattern_init_for_surface (&pattern, &image->base);
    pattern.base.filter = CAIRO_FILTER_NEAREST;

    /* We have to apply the translate from map_to_image's extents.x and .y */
    cairo_matrix_init_translate (&pattern.base.matrix,
				 image->base.device_transform.x0,
				 image->base.device_transform.y0);

    /* And we also have to clip the operation to the image's extents */
    extents.x = image->base.device_transform_inverse.x0;
    extents.y = image->base.device_transform_inverse.y0;
    extents.width  = image->width;
    extents.height = image->height;
    clip = _cairo_clip_intersect_rectangle (NULL, &extents);

    status = _cairo_surface_paint (surface,
				   CAIRO_OPERATOR_SOURCE,
				   &pattern.base,
				   clip);

    _cairo_pattern_fini (&pattern.base);
    _cairo_clip_destroy (clip);

destroy:
    cairo_surface_finish (&image->base);
    cairo_surface_destroy (&image->base);

    return status;
}

/**
 * cairo_surface_map_to_image:
 * @surface: an existing surface used to extract the image from
 * @extents: limit the extraction to an rectangular region
 *
 * Returns an image surface that is the most efficient mechanism for
 * modifying the backing store of the target surface. The region retrieved
 * may be limited to the @extents or %NULL for the whole surface
 *
 * Note, the use of the original surface as a target or source whilst
 * it is mapped is undefined. The result of mapping the surface
 * multiple times is undefined. Calling cairo_surface_destroy() or
 * cairo_surface_finish() on the resulting image surface results in
 * undefined behavior. Changing the device transform of the image
 * surface or of @surface before the image surface is unmapped results
 * in undefined behavior.
 *
 * Return value: a pointer to the newly allocated image surface. The caller
 * must use cairo_surface_unmap_image() to destroy this image surface.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" surface if @other is already in an error state
 * or any other error occurs. If the returned pointer does not have an
 * error status, it is guaranteed to be an image surface whose format
 * is not %CAIRO_FORMAT_INVALID.
 *
 * Since: 1.12
 **/
cairo_surface_t *
cairo_surface_map_to_image (cairo_surface_t  *surface,
			    const cairo_rectangle_int_t *extents)
{
    cairo_rectangle_int_t rect;
    cairo_image_surface_t *image;
    cairo_status_t status;

    if (unlikely (surface->status))
	return _cairo_surface_create_in_error (surface->status);
    if (unlikely (surface->finished))
	return _cairo_surface_create_in_error (CAIRO_STATUS_SURFACE_FINISHED);

    if (extents == NULL) {
	if (unlikely (! surface->backend->get_extents (surface, &rect)))
	    return _cairo_surface_create_in_error (CAIRO_STATUS_INVALID_SIZE);

	extents = &rect;
    } else {
	cairo_rectangle_int_t surface_extents;

	/* If this surface is bounded, we can't map parts
	 * that are outside of it. */
	if (likely (surface->backend->get_extents (surface, &surface_extents))) {
	    if (unlikely (! _cairo_rectangle_contains_rectangle (&surface_extents, extents)))
		return _cairo_surface_create_in_error (CAIRO_STATUS_INVALID_SIZE);
	}
    }

    image = _cairo_surface_map_to_image (surface, extents);

    status = image->base.status;
    if (unlikely (status)) {
	cairo_surface_destroy (&image->base);
	return _cairo_surface_create_in_error (status);
    }

    if (image->format == CAIRO_FORMAT_INVALID) {
	cairo_surface_destroy (&image->base);
	image = _cairo_image_surface_clone_subimage (surface, extents);
    }

    return &image->base;
}

/**
 * cairo_surface_unmap_image:
 * @surface: the surface passed to cairo_surface_map_to_image().
 * @image: the currently mapped image
 *
 * Unmaps the image surface as returned from #cairo_surface_map_to_image().
 *
 * The content of the image will be uploaded to the target surface.
 * Afterwards, the image is destroyed.
 *
 * Using an image surface which wasn't returned by cairo_surface_map_to_image()
 * results in undefined behavior.
 *
 * Since: 1.12
 **/
void
cairo_surface_unmap_image (cairo_surface_t *surface,
			   cairo_surface_t *image)
{
    cairo_int_status_t status = CAIRO_STATUS_SUCCESS;

    if (unlikely (surface->status)) {
	status = surface->status;
	goto error;
    }
    if (unlikely (surface->finished)) {
	status = _cairo_error (CAIRO_STATUS_SURFACE_FINISHED);
	goto error;
    }
    if (unlikely (image->status)) {
	status = image->status;
	goto error;
    }
    if (unlikely (image->finished)) {
	status = _cairo_error (CAIRO_STATUS_SURFACE_FINISHED);
	goto error;
    }
    if (unlikely (! _cairo_surface_is_image (image))) {
	status = _cairo_error (CAIRO_STATUS_SURFACE_TYPE_MISMATCH);
	goto error;
    }

    status = _cairo_surface_unmap_image (surface,
					 (cairo_image_surface_t *) image);
    if (unlikely (status))
	_cairo_surface_set_error (surface, status);

    return;

error:
    _cairo_surface_set_error (surface, status);
    cairo_surface_finish (image);
    cairo_surface_destroy (image);
}

cairo_surface_t *
_cairo_surface_create_scratch (cairo_surface_t	 *other,
			       cairo_content_t	  content,
			       int		  width,
			       int		  height,
			       const cairo_color_t *color)
{
    cairo_surface_t *surface;
    cairo_status_t status;
    cairo_solid_pattern_t pattern;

    if (unlikely (other->status))
	return _cairo_surface_create_in_error (other->status);

    surface = NULL;
    if (other->backend->create_similar)
	surface = other->backend->create_similar (other, content, width, height);
    if (surface == NULL)
	surface = cairo_surface_create_similar_image (other,
						      _cairo_format_from_content (content),
						      width, height);

    if (unlikely (surface->status))
	return surface;

    _cairo_surface_copy_similar_properties (surface, other);

    if (unlikely (surface->status))
	return surface;

    if (color) {
	_cairo_pattern_init_solid (&pattern, color);
	status = _cairo_surface_paint (surface,
				       color == CAIRO_COLOR_TRANSPARENT ?
				       CAIRO_OPERATOR_CLEAR : CAIRO_OPERATOR_SOURCE,
				       &pattern.base, NULL);
	if (unlikely (status)) {
	    cairo_surface_destroy (surface);
	    surface = _cairo_surface_create_in_error (status);
	}
    }

    return surface;
}

/**
 * cairo_surface_reference:
 * @surface: a #cairo_surface_t
 *
 * Increases the reference count on @surface by one. This prevents
 * @surface from being destroyed until a matching call to
 * cairo_surface_destroy() is made.
 *
 * Use cairo_surface_get_reference_count() to get the number of
 * references to a #cairo_surface_t.
 *
 * Return value: the referenced #cairo_surface_t.
 *
 * Since: 1.0
 **/
cairo_surface_t *
cairo_surface_reference (cairo_surface_t *surface)
{
    if (surface == NULL ||
	    CAIRO_REFERENCE_COUNT_IS_INVALID (&surface->ref_count))
	return surface;

    assert (CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&surface->ref_count));

    _cairo_reference_count_inc (&surface->ref_count);

    return surface;
}
slim_hidden_def (cairo_surface_reference);

/**
 * cairo_surface_destroy:
 * @surface: a #cairo_surface_t
 *
 * Decreases the reference count on @surface by one. If the result is
 * zero, then @surface and all associated resources are freed.  See
 * cairo_surface_reference().
 *
 * Since: 1.0
 **/
void
cairo_surface_destroy (cairo_surface_t *surface)
{
    if (surface == NULL ||
	    CAIRO_REFERENCE_COUNT_IS_INVALID (&surface->ref_count))
	return;

    assert (CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&surface->ref_count));

    if (! _cairo_reference_count_dec_and_test (&surface->ref_count))
	return;

    assert (surface->snapshot_of == NULL);

    if (! surface->finished) {
	_cairo_surface_finish_snapshots (surface);
	/* We may have been referenced by a snapshot prior to have
	 * detaching it with the copy-on-write.
	 */
	if (CAIRO_REFERENCE_COUNT_GET_VALUE (&surface->ref_count))
	    return;

	_cairo_surface_finish (surface);
    }

    if (surface->damage)
	_cairo_damage_destroy (surface->damage);

    _cairo_user_data_array_fini (&surface->user_data);
    _cairo_user_data_array_fini (&surface->mime_data);

    if (surface->foreground_source)
	cairo_pattern_destroy (surface->foreground_source);

    if (surface->owns_device)
        cairo_device_destroy (surface->device);

    assert (surface->snapshot_of == NULL);
    assert (! _cairo_surface_has_snapshots (surface));
    /* paranoid check that nobody took a reference whilst finishing */
    assert (! CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&surface->ref_count));

    free (surface);
}
slim_hidden_def(cairo_surface_destroy);

/**
 * cairo_surface_get_reference_count:
 * @surface: a #cairo_surface_t
 *
 * Returns the current reference count of @surface.
 *
 * Return value: the current reference count of @surface.  If the
 * object is a nil object, 0 will be returned.
 *
 * Since: 1.4
 **/
unsigned int
cairo_surface_get_reference_count (cairo_surface_t *surface)
{
    if (surface == NULL ||
	    CAIRO_REFERENCE_COUNT_IS_INVALID (&surface->ref_count))
	return 0;

    return CAIRO_REFERENCE_COUNT_GET_VALUE (&surface->ref_count);
}

static void
_cairo_surface_finish_snapshots (cairo_surface_t *surface)
{
    cairo_status_t status;

    /* update the snapshots *before* we declare the surface as finished */
    surface->_finishing = TRUE;
    status = _cairo_surface_flush (surface, 0);
    (void) status;
}

static void
_cairo_surface_finish (cairo_surface_t *surface)
{
    cairo_status_t status;

    /* call finish even if in error mode */
    if (surface->backend->finish) {
	status = surface->backend->finish (surface);
	if (unlikely (status))
	    _cairo_surface_set_error (surface, status);
    }

    surface->finished = TRUE;

    assert (surface->snapshot_of == NULL);
    assert (!_cairo_surface_has_snapshots (surface));
}

/**
 * cairo_surface_finish:
 * @surface: the #cairo_surface_t to finish
 *
 * This function finishes the surface and drops all references to
 * external resources.  For example, for the Xlib backend it means
 * that cairo will no longer access the drawable, which can be freed.
 * After calling cairo_surface_finish() the only valid operations on a
 * surface are checking status, getting and setting user, referencing
 * and destroying, and flushing and finishing it.
 * Further drawing to the surface will not affect the
 * surface but will instead trigger a %CAIRO_STATUS_SURFACE_FINISHED
 * error.
 *
 * When the last call to cairo_surface_destroy() decreases the
 * reference count to zero, cairo will call cairo_surface_finish() if
 * it hasn't been called already, before freeing the resources
 * associated with the surface.
 *
 * Since: 1.0
 **/
void
cairo_surface_finish (cairo_surface_t *surface)
{
    if (surface == NULL)
	return;

    if (CAIRO_REFERENCE_COUNT_IS_INVALID (&surface->ref_count))
	return;

    if (surface->finished)
	return;

    /* We have to be careful when decoupling potential reference cycles */
    cairo_surface_reference (surface);

    _cairo_surface_finish_snapshots (surface);
    /* XXX need to block and wait for snapshot references */
    _cairo_surface_finish (surface);

    cairo_surface_destroy (surface);
}
slim_hidden_def (cairo_surface_finish);

/**
 * _cairo_surface_release_device_reference:
 * @surface: a #cairo_surface_t
 *
 * This function makes @surface release the reference to its device. The
 * function is intended to be used for avoiding cycling references for
 * surfaces that are owned by their device, for example cache surfaces.
 * Note that the @surface will still assume that the device is available.
 * So it is the caller's responsibility to ensure the device stays around
 * until the @surface is destroyed. Just calling cairo_surface_finish() is
 * not enough.
 **/
void
_cairo_surface_release_device_reference (cairo_surface_t *surface)
{
    assert (surface->owns_device);

    cairo_device_destroy (surface->device);
    surface->owns_device = FALSE;
}

/**
 * cairo_surface_get_user_data:
 * @surface: a #cairo_surface_t
 * @key: the address of the #cairo_user_data_key_t the user data was
 * attached to
 *
 * Return user data previously attached to @surface using the specified
 * key.  If no user data has been attached with the given key this
 * function returns %NULL.
 *
 * Return value: the user data previously attached or %NULL.
 *
 * Since: 1.0
 **/
void *
cairo_surface_get_user_data (cairo_surface_t		 *surface,
			     const cairo_user_data_key_t *key)
{
    /* Prevent reads of the array during teardown */
    if (! CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&surface->ref_count))
	return NULL;

    return _cairo_user_data_array_get_data (&surface->user_data, key);
}

/**
 * cairo_surface_set_user_data:
 * @surface: a #cairo_surface_t
 * @key: the address of a #cairo_user_data_key_t to attach the user data to
 * @user_data: the user data to attach to the surface
 * @destroy: a #cairo_destroy_func_t which will be called when the
 * surface is destroyed or when new user data is attached using the
 * same key.
 *
 * Attach user data to @surface.  To remove user data from a surface,
 * call this function with the key that was used to set it and %NULL
 * for @data.
 *
 * Return value: %CAIRO_STATUS_SUCCESS or %CAIRO_STATUS_NO_MEMORY if a
 * slot could not be allocated for the user data.
 *
 * Since: 1.0
 **/
cairo_status_t
cairo_surface_set_user_data (cairo_surface_t		 *surface,
			     const cairo_user_data_key_t *key,
			     void			 *user_data,
			     cairo_destroy_func_t	 destroy)
{
    if (CAIRO_REFERENCE_COUNT_IS_INVALID (&surface->ref_count))
	return surface->status;

    if (! CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&surface->ref_count))
	return _cairo_error (CAIRO_STATUS_SURFACE_FINISHED);

    return _cairo_user_data_array_set_data (&surface->user_data,
					    key, user_data, destroy);
}

/**
 * cairo_surface_get_mime_data:
 * @surface: a #cairo_surface_t
 * @mime_type: the mime type of the image data
 * @data: the image data to attached to the surface
 * @length: the length of the image data
 *
 * Return mime data previously attached to @surface using the
 * specified mime type.  If no data has been attached with the given
 * mime type, @data is set %NULL.
 *
 * Since: 1.10
 **/
void
cairo_surface_get_mime_data (cairo_surface_t		*surface,
                             const char			*mime_type,
                             const unsigned char       **data,
                             unsigned long		*length)
{
    cairo_user_data_slot_t *slots;
    int i, num_slots;

    *data = NULL;
    *length = 0;

    /* Prevent reads of the array during teardown */
    if (! CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&surface->ref_count))
	return;

    /* The number of mime-types attached to a surface is usually small,
     * typically zero. Therefore it is quicker to do a strcmp() against
     * each key than it is to intern the string (i.e. compute a hash,
     * search the hash table, and do a final strcmp).
     */
    num_slots = surface->mime_data.num_elements;
    slots = _cairo_array_index (&surface->mime_data, 0);
    for (i = 0; i < num_slots; i++) {
	if (slots[i].key != NULL && strcmp ((char *) slots[i].key, mime_type) == 0) {
	    cairo_mime_data_t *mime_data = slots[i].user_data;

	    *data = mime_data->data;
	    *length = mime_data->length;
	    return;
	}
    }
}
slim_hidden_def (cairo_surface_get_mime_data);

static void
_cairo_mime_data_destroy (void *ptr)
{
    cairo_mime_data_t *mime_data = ptr;

    if (! _cairo_reference_count_dec_and_test (&mime_data->ref_count))
	return;

    if (mime_data->destroy && mime_data->closure)
	mime_data->destroy (mime_data->closure);

    free (mime_data);
}


static const char *_cairo_surface_image_mime_types[] = {
    CAIRO_MIME_TYPE_JPEG,
    CAIRO_MIME_TYPE_PNG,
    CAIRO_MIME_TYPE_JP2,
    CAIRO_MIME_TYPE_JBIG2,
    CAIRO_MIME_TYPE_CCITT_FAX,
};

cairo_bool_t
_cairo_surface_has_mime_image (cairo_surface_t *surface)
{
    cairo_user_data_slot_t *slots;
    int i, j, num_slots;

    /* Prevent reads of the array during teardown */
    if (! CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&surface->ref_count))
	return FALSE;

    /* The number of mime-types attached to a surface is usually small,
     * typically zero. Therefore it is quicker to do a strcmp() against
     * each key than it is to intern the string (i.e. compute a hash,
     * search the hash table, and do a final strcmp).
     */
    num_slots = surface->mime_data.num_elements;
    slots = _cairo_array_index (&surface->mime_data, 0);
    for (i = 0; i < num_slots; i++) {
	if (slots[i].key != NULL) {
	    for (j = 0; j < ARRAY_LENGTH (_cairo_surface_image_mime_types); j++) {
		if (strcmp ((char *) slots[i].key, _cairo_surface_image_mime_types[j]) == 0)
		    return TRUE;
	    }
	}
    }

    return FALSE;
}

/**
 * CAIRO_MIME_TYPE_CCITT_FAX:
 *
 * Group 3 or Group 4 CCITT facsimile encoding (International
 * Telecommunication Union, Recommendations T.4 and T.6.)
 *
 * Since: 1.16
 **/

/**
 * CAIRO_MIME_TYPE_CCITT_FAX_PARAMS:
 *
 * Decode parameters for Group 3 or Group 4 CCITT facsimile encoding.
 * See [CCITT Fax Images][ccitt].
 *
 * Since: 1.16
 **/

/**
 * CAIRO_MIME_TYPE_EPS:
 *
 * Encapsulated PostScript file.
 * [Encapsulated PostScript File Format Specification](http://wwwimages.adobe.com/content/dam/Adobe/endevnet/postscript/pdfs/5002.EPSF_Spec.pdf)
 *
 * Since: 1.16
 **/

/**
 * CAIRO_MIME_TYPE_EPS_PARAMS:
 *
 * Embedding parameters Encapsulated PostScript data.
 * See [Embedding EPS files][eps].
 *
 * Since: 1.16
 **/

/**
 * CAIRO_MIME_TYPE_JBIG2:
 *
 * Joint Bi-level Image Experts Group image coding standard (ISO/IEC 11544).
 *
 * Since: 1.14
 **/

/**
 * CAIRO_MIME_TYPE_JBIG2_GLOBAL:
 *
 * Joint Bi-level Image Experts Group image coding standard (ISO/IEC 11544) global segment.
 *
 * Since: 1.14
 **/

/**
 * CAIRO_MIME_TYPE_JBIG2_GLOBAL_ID:
 *
 * An unique identifier shared by a JBIG2 global segment and all JBIG2 images
 * that depend on the global segment.
 *
 * Since: 1.14
 **/

/**
 * CAIRO_MIME_TYPE_JP2:
 *
 * The Joint Photographic Experts Group (JPEG) 2000 image coding standard (ISO/IEC 15444-1).
 *
 * Since: 1.10
 **/

/**
 * CAIRO_MIME_TYPE_JPEG:
 *
 * The Joint Photographic Experts Group (JPEG) image coding standard (ISO/IEC 10918-1).
 *
 * Since: 1.10
 **/

/**
 * CAIRO_MIME_TYPE_PNG:
 *
 * The Portable Network Graphics image file format (ISO/IEC 15948).
 *
 * Since: 1.10
 **/

/**
 * CAIRO_MIME_TYPE_URI:
 *
 * URI for an image file (unofficial MIME type).
 *
 * Since: 1.10
 **/

/**
 * CAIRO_MIME_TYPE_UNIQUE_ID:
 *
 * Unique identifier for a surface (cairo specific MIME type). All surfaces with
 * the same unique identifier will only be embedded once.
 *
 * Since: 1.12
 **/

/**
 * cairo_surface_set_mime_data:
 * @surface: a #cairo_surface_t
 * @mime_type: the MIME type of the image data
 * @data: the image data to attach to the surface
 * @length: the length of the image data
 * @destroy: a #cairo_destroy_func_t which will be called when the
 * surface is destroyed or when new image data is attached using the
 * same mime type.
 * @closure: the data to be passed to the @destroy notifier
 *
 * Attach an image in the format @mime_type to @surface. To remove
 * the data from a surface, call this function with same mime type
 * and %NULL for @data.
 *
 * The attached image (or filename) data can later be used by backends
 * which support it (currently: PDF, PS, SVG and Win32 Printing
 * surfaces) to emit this data instead of making a snapshot of the
 * @surface.  This approach tends to be faster and requires less
 * memory and disk space.
 *
 * The recognized MIME types are the following: %CAIRO_MIME_TYPE_JPEG,
 * %CAIRO_MIME_TYPE_PNG, %CAIRO_MIME_TYPE_JP2, %CAIRO_MIME_TYPE_URI,
 * %CAIRO_MIME_TYPE_UNIQUE_ID, %CAIRO_MIME_TYPE_JBIG2,
 * %CAIRO_MIME_TYPE_JBIG2_GLOBAL, %CAIRO_MIME_TYPE_JBIG2_GLOBAL_ID,
 * %CAIRO_MIME_TYPE_CCITT_FAX, %CAIRO_MIME_TYPE_CCITT_FAX_PARAMS.
 *
 * See corresponding backend surface docs for details about which MIME
 * types it can handle. Caution: the associated MIME data will be
 * discarded if you draw on the surface afterwards. Use this function
 * with care.
 *
 * Even if a backend supports a MIME type, that does not mean cairo
 * will always be able to use the attached MIME data. For example, if
 * the backend does not natively support the compositing operation used
 * to apply the MIME data to the backend. In that case, the MIME data
 * will be ignored. Therefore, to apply an image in all cases, it is best
 * to create an image surface which contains the decoded image data and
 * then attach the MIME data to that. This ensures the image will always
 * be used while still allowing the MIME data to be used whenever
 * possible.
 *
 * Return value: %CAIRO_STATUS_SUCCESS or %CAIRO_STATUS_NO_MEMORY if a
 * slot could not be allocated for the user data.
 *
 * Since: 1.10
 **/
cairo_status_t
cairo_surface_set_mime_data (cairo_surface_t		*surface,
                             const char			*mime_type,
                             const unsigned char	*data,
                             unsigned long		 length,
			     cairo_destroy_func_t	 destroy,
			     void			*closure)
{
    cairo_status_t status;
    cairo_mime_data_t *mime_data;

    if (CAIRO_REFERENCE_COUNT_IS_INVALID (&surface->ref_count))
	return surface->status;

    if (! CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&surface->ref_count))
	return _cairo_error (CAIRO_STATUS_SURFACE_FINISHED);

    if (unlikely (surface->status))
	return surface->status;
    if (unlikely (surface->finished))
	return _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    status = _cairo_intern_string (&mime_type, -1);
    if (unlikely (status))
	return _cairo_surface_set_error (surface, status);

    if (data != NULL) {
	mime_data = _cairo_malloc (sizeof (cairo_mime_data_t));
	if (unlikely (mime_data == NULL))
	    return _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_NO_MEMORY));

	CAIRO_REFERENCE_COUNT_INIT (&mime_data->ref_count, 1);

	mime_data->data = (unsigned char *) data;
	mime_data->length = length;
	mime_data->destroy = destroy;
	mime_data->closure = closure;
    } else
	mime_data = NULL;

    status = _cairo_user_data_array_set_data (&surface->mime_data,
					      (cairo_user_data_key_t *) mime_type,
					      mime_data,
					      _cairo_mime_data_destroy);
    if (unlikely (status)) {
	free (mime_data);

	return _cairo_surface_set_error (surface, status);
    }

    surface->is_clear = FALSE;

    return CAIRO_STATUS_SUCCESS;
}
slim_hidden_def (cairo_surface_set_mime_data);

/**
 * cairo_surface_supports_mime_type:
 * @surface: a #cairo_surface_t
 * @mime_type: the mime type
 *
 * Return whether @surface supports @mime_type.
 *
 * Return value: %TRUE if @surface supports
 *               @mime_type, %FALSE otherwise
 *
 * Since: 1.12
 **/
cairo_bool_t
cairo_surface_supports_mime_type (cairo_surface_t		*surface,
				  const char			*mime_type)
{
    const char **types;

    if (unlikely (surface->status))
	return FALSE;
    if (unlikely (surface->finished)) {
	_cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return FALSE;
    }

    if (surface->backend->get_supported_mime_types) {
	types = surface->backend->get_supported_mime_types (surface);
	if (types) {
	    while (*types) {
		if (strcmp (*types, mime_type) == 0)
		    return TRUE;
		types++;
	    }
	}
    }

    return FALSE;
}
slim_hidden_def (cairo_surface_supports_mime_type);

static void
_cairo_mime_data_reference (const void *key, void *elt, void *closure)
{
    cairo_mime_data_t *mime_data = elt;

    _cairo_reference_count_inc (&mime_data->ref_count);
}

cairo_status_t
_cairo_surface_copy_mime_data (cairo_surface_t *dst,
			       cairo_surface_t *src)
{
    cairo_status_t status;

    if (dst->status)
	return dst->status;

    if (src->status)
	return _cairo_surface_set_error (dst, src->status);

    /* first copy the mime-data, discarding any already set on dst */
    status = _cairo_user_data_array_copy (&dst->mime_data, &src->mime_data);
    if (unlikely (status))
	return _cairo_surface_set_error (dst, status);

    /* now increment the reference counters for the copies */
    _cairo_user_data_array_foreach (&dst->mime_data,
				    _cairo_mime_data_reference,
				    NULL);

    dst->is_clear = FALSE;

    return CAIRO_STATUS_SUCCESS;
}

/**
 * _cairo_surface_set_font_options:
 * @surface: a #cairo_surface_t
 * @options: a #cairo_font_options_t object that contains the
 *   options to use for this surface instead of backend's default
 *   font options.
 *
 * Sets the default font rendering options for the surface.
 * This is useful to correctly propagate default font options when
 * falling back to an image surface in a backend implementation.
 * This affects the options returned in cairo_surface_get_font_options().
 *
 * If @options is %NULL the surface options are reset to those of
 * the backend default.
 **/
void
_cairo_surface_set_font_options (cairo_surface_t       *surface,
				 cairo_font_options_t  *options)
{
    if (surface->status)
	return;

    assert (surface->snapshot_of == NULL);

    if (surface->finished) {
	_cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return;
    }

    if (options) {
	surface->has_font_options = TRUE;
	_cairo_font_options_init_copy (&surface->font_options, options);
    } else {
	surface->has_font_options = FALSE;
    }
}

/**
 * cairo_surface_get_font_options:
 * @surface: a #cairo_surface_t
 * @options: a #cairo_font_options_t object into which to store
 *   the retrieved options. All existing values are overwritten
 *
 * Retrieves the default font rendering options for the surface.
 * This allows display surfaces to report the correct subpixel order
 * for rendering on them, print surfaces to disable hinting of
 * metrics and so forth. The result can then be used with
 * cairo_scaled_font_create().
 *
 * Since: 1.0
 **/
void
cairo_surface_get_font_options (cairo_surface_t       *surface,
				cairo_font_options_t  *options)
{
    if (cairo_font_options_status (options))
	return;

    if (surface->status) {
	_cairo_font_options_init_default (options);
	return;
    }

    if (! surface->has_font_options) {
	surface->has_font_options = TRUE;

	_cairo_font_options_init_default (&surface->font_options);

	if (!surface->finished && surface->backend->get_font_options) {
	    surface->backend->get_font_options (surface, &surface->font_options);
	}
    }

    _cairo_font_options_init_copy (options, &surface->font_options);
}
slim_hidden_def (cairo_surface_get_font_options);

cairo_status_t
_cairo_surface_flush (cairo_surface_t *surface, unsigned flags)
{
    /* update the current snapshots *before* the user updates the surface */
    _cairo_surface_detach_snapshots (surface);
    if (surface->snapshot_of != NULL)
	_cairo_surface_detach_snapshot (surface);
    _cairo_surface_detach_mime_data (surface);

    return __cairo_surface_flush (surface, flags);
}

/**
 * cairo_surface_flush:
 * @surface: a #cairo_surface_t
 *
 * Do any pending drawing for the surface and also restore any temporary
 * modifications cairo has made to the surface's state. This function
 * must be called before switching from drawing on the surface with
 * cairo to drawing on it directly with native APIs, or accessing its
 * memory outside of Cairo. If the surface doesn't support direct
 * access, then this function does nothing.
 *
 * Since: 1.0
 **/
void
cairo_surface_flush (cairo_surface_t *surface)
{
    cairo_status_t status;

    if (surface->status)
	return;

    if (surface->finished)
	return;

    status = _cairo_surface_flush (surface, 0);
    if (unlikely (status))
	_cairo_surface_set_error (surface, status);
}
slim_hidden_def (cairo_surface_flush);

/**
 * cairo_surface_mark_dirty:
 * @surface: a #cairo_surface_t
 *
 * Tells cairo that drawing has been done to surface using means other
 * than cairo, and that cairo should reread any cached areas. Note
 * that you must call cairo_surface_flush() before doing such drawing.
 *
 * Since: 1.0
 **/
void
cairo_surface_mark_dirty (cairo_surface_t *surface)
{
    cairo_rectangle_int_t extents;

    if (unlikely (surface->status))
	return;
    if (unlikely (surface->finished)) {
	_cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return;
    }

    _cairo_surface_get_extents (surface, &extents);
    cairo_surface_mark_dirty_rectangle (surface,
					extents.x, extents.y,
					extents.width, extents.height);
}
slim_hidden_def (cairo_surface_mark_dirty);

/**
 * cairo_surface_mark_dirty_rectangle:
 * @surface: a #cairo_surface_t
 * @x: X coordinate of dirty rectangle
 * @y: Y coordinate of dirty rectangle
 * @width: width of dirty rectangle
 * @height: height of dirty rectangle
 *
 * Like cairo_surface_mark_dirty(), but drawing has been done only to
 * the specified rectangle, so that cairo can retain cached contents
 * for other parts of the surface.
 *
 * Any cached clip set on the surface will be reset by this function,
 * to make sure that future cairo calls have the clip set that they
 * expect.
 *
 * Since: 1.0
 **/
void
cairo_surface_mark_dirty_rectangle (cairo_surface_t *surface,
				    int              x,
				    int              y,
				    int              width,
				    int              height)
{
    cairo_status_t status;

    if (unlikely (surface->status))
	return;

    assert (surface->snapshot_of == NULL);

    if (unlikely (surface->finished)) {
	_cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return;
    }

    /* The application *should* have called cairo_surface_flush() before
     * modifying the surface independently of cairo (and thus having to
     * call mark_dirty()). */
    assert (! _cairo_surface_has_snapshots (surface));
    assert (! _cairo_surface_has_mime_data (surface));

    surface->is_clear = FALSE;
    surface->serial++;

    if (surface->damage) {
	cairo_box_t box;

	box.p1.x = x;
	box.p1.y = y;
	box.p2.x = x + width;
	box.p2.y = y + height;

	surface->damage = _cairo_damage_add_box (surface->damage, &box);
    }

    if (surface->backend->mark_dirty_rectangle != NULL) {
	/* XXX: FRAGILE: We're ignoring the scaling component of
	 * device_transform here. I don't know what the right thing to
	 * do would actually be if there were some scaling here, but
	 * we avoid this since device_transfom scaling is not exported
	 * publicly and mark_dirty is not used internally. */
	status = surface->backend->mark_dirty_rectangle (surface,
                                                         x + surface->device_transform.x0,
                                                         y + surface->device_transform.y0,
							 width, height);

	if (unlikely (status))
	    _cairo_surface_set_error (surface, status);
    }
}
slim_hidden_def (cairo_surface_mark_dirty_rectangle);

/**
 * cairo_surface_set_device_scale:
 * @surface: a #cairo_surface_t
 * @x_scale: a scale factor in the X direction
 * @y_scale: a scale factor in the Y direction
 *
 * Sets a scale that is multiplied to the device coordinates determined
 * by the CTM when drawing to @surface. One common use for this is to
 * render to very high resolution display devices at a scale factor, so
 * that code that assumes 1 pixel will be a certain size will still work.
 * Setting a transformation via cairo_scale() isn't
 * sufficient to do this, since functions like
 * cairo_device_to_user() will expose the hidden scale.
 *
 * Note that the scale affects drawing to the surface as well as
 * using the surface in a source pattern.
 *
 * Since: 1.14
 **/
void
cairo_surface_set_device_scale (cairo_surface_t *surface,
				double		 x_scale,
				double		 y_scale)
{
    cairo_status_t status;

    if (unlikely (surface->status))
	return;

    assert (surface->snapshot_of == NULL);

    if (unlikely (surface->finished)) {
	_cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return;
    }

    status = _cairo_surface_begin_modification (surface);
    if (unlikely (status)) {
	_cairo_surface_set_error (surface, status);
	return;
    }

    surface->device_transform.xx = x_scale;
    surface->device_transform.yy = y_scale;
    surface->device_transform.xy = 0.0;
    surface->device_transform.yx = 0.0;

    surface->device_transform_inverse = surface->device_transform;
    status = cairo_matrix_invert (&surface->device_transform_inverse);
    /* should always be invertible unless given pathological input */
    assert (status == CAIRO_STATUS_SUCCESS);

    _cairo_observers_notify (&surface->device_transform_observers, surface);
}
slim_hidden_def (cairo_surface_set_device_scale);

/**
 * cairo_surface_get_device_scale:
 * @surface: a #cairo_surface_t
 * @x_scale: the scale in the X direction, in device units
 * @y_scale: the scale in the Y direction, in device units
 *
 * This function returns the previous device scale set by
 * cairo_surface_set_device_scale().
 *
 * Since: 1.14
 **/
void
cairo_surface_get_device_scale (cairo_surface_t *surface,
				double          *x_scale,
				double          *y_scale)
{
    if (x_scale)
	*x_scale = surface->device_transform.xx;
    if (y_scale)
	*y_scale = surface->device_transform.yy;
}
slim_hidden_def (cairo_surface_get_device_scale);

/**
 * cairo_surface_set_device_offset:
 * @surface: a #cairo_surface_t
 * @x_offset: the offset in the X direction, in device units
 * @y_offset: the offset in the Y direction, in device units
 *
 * Sets an offset that is added to the device coordinates determined
 * by the CTM when drawing to @surface. One use case for this function
 * is when we want to create a #cairo_surface_t that redirects drawing
 * for a portion of an onscreen surface to an offscreen surface in a
 * way that is completely invisible to the user of the cairo
 * API. Setting a transformation via cairo_translate() isn't
 * sufficient to do this, since functions like
 * cairo_device_to_user() will expose the hidden offset.
 *
 * Note that the offset affects drawing to the surface as well as
 * using the surface in a source pattern.
 *
 * Since: 1.0
 **/
void
cairo_surface_set_device_offset (cairo_surface_t *surface,
				 double           x_offset,
				 double           y_offset)
{
    cairo_status_t status;

    if (unlikely (surface->status))
	return;

    assert (surface->snapshot_of == NULL);

    if (unlikely (surface->finished)) {
	_cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return;
    }

    status = _cairo_surface_begin_modification (surface);
    if (unlikely (status)) {
	_cairo_surface_set_error (surface, status);
	return;
    }

    surface->device_transform.x0 = x_offset;
    surface->device_transform.y0 = y_offset;

    surface->device_transform_inverse = surface->device_transform;
    status = cairo_matrix_invert (&surface->device_transform_inverse);
    /* should always be invertible unless given pathological input */
    assert (status == CAIRO_STATUS_SUCCESS);

    _cairo_observers_notify (&surface->device_transform_observers, surface);
}
slim_hidden_def (cairo_surface_set_device_offset);

/**
 * cairo_surface_get_device_offset:
 * @surface: a #cairo_surface_t
 * @x_offset: the offset in the X direction, in device units
 * @y_offset: the offset in the Y direction, in device units
 *
 * This function returns the previous device offset set by
 * cairo_surface_set_device_offset().
 *
 * Since: 1.2
 **/
void
cairo_surface_get_device_offset (cairo_surface_t *surface,
				 double          *x_offset,
				 double          *y_offset)
{
    if (x_offset)
	*x_offset = surface->device_transform.x0;
    if (y_offset)
	*y_offset = surface->device_transform.y0;
}
slim_hidden_def (cairo_surface_get_device_offset);

/**
 * cairo_surface_set_fallback_resolution:
 * @surface: a #cairo_surface_t
 * @x_pixels_per_inch: horizontal setting for pixels per inch
 * @y_pixels_per_inch: vertical setting for pixels per inch
 *
 * Set the horizontal and vertical resolution for image fallbacks.
 *
 * When certain operations aren't supported natively by a backend,
 * cairo will fallback by rendering operations to an image and then
 * overlaying that image onto the output. For backends that are
 * natively vector-oriented, this function can be used to set the
 * resolution used for these image fallbacks, (larger values will
 * result in more detailed images, but also larger file sizes).
 *
 * Some examples of natively vector-oriented backends are the ps, pdf,
 * and svg backends.
 *
 * For backends that are natively raster-oriented, image fallbacks are
 * still possible, but they are always performed at the native
 * device resolution. So this function has no effect on those
 * backends.
 *
 * Note: The fallback resolution only takes effect at the time of
 * completing a page (with cairo_show_page() or cairo_copy_page()) so
 * there is currently no way to have more than one fallback resolution
 * in effect on a single page.
 *
 * The default fallback resolution is 300 pixels per inch in both
 * dimensions.
 *
 * Since: 1.2
 **/
void
cairo_surface_set_fallback_resolution (cairo_surface_t	*surface,
				       double		 x_pixels_per_inch,
				       double		 y_pixels_per_inch)
{
    cairo_status_t status;

    if (unlikely (surface->status))
	return;

    assert (surface->snapshot_of == NULL);

    if (unlikely (surface->finished)) {
	_cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return;
    }

    if (x_pixels_per_inch <= 0 || y_pixels_per_inch <= 0) {
	/* XXX Could delay raising the error until we fallback, but throwing
	 * the error here means that we can catch the real culprit.
	 */
	_cairo_surface_set_error (surface, CAIRO_STATUS_INVALID_MATRIX);
	return;
    }

    status = _cairo_surface_begin_modification (surface);
    if (unlikely (status)) {
	_cairo_surface_set_error (surface, status);
	return;
    }

    surface->x_fallback_resolution = x_pixels_per_inch;
    surface->y_fallback_resolution = y_pixels_per_inch;
}
slim_hidden_def (cairo_surface_set_fallback_resolution);

/**
 * cairo_surface_get_fallback_resolution:
 * @surface: a #cairo_surface_t
 * @x_pixels_per_inch: horizontal pixels per inch
 * @y_pixels_per_inch: vertical pixels per inch
 *
 * This function returns the previous fallback resolution set by
 * cairo_surface_set_fallback_resolution(), or default fallback
 * resolution if never set.
 *
 * Since: 1.8
 **/
void
cairo_surface_get_fallback_resolution (cairo_surface_t	*surface,
				       double		*x_pixels_per_inch,
				       double		*y_pixels_per_inch)
{
    if (x_pixels_per_inch)
	*x_pixels_per_inch = surface->x_fallback_resolution;
    if (y_pixels_per_inch)
	*y_pixels_per_inch = surface->y_fallback_resolution;
}

cairo_bool_t
_cairo_surface_has_device_transform (cairo_surface_t *surface)
{
    return ! _cairo_matrix_is_identity (&surface->device_transform);
}

/**
 * _cairo_surface_acquire_source_image:
 * @surface: a #cairo_surface_t
 * @image_out: location to store a pointer to an image surface that
 *    has identical contents to @surface. This surface could be @surface
 *    itself, a surface held internal to @surface, or it could be a new
 *    surface with a copy of the relevant portion of @surface.
 * @image_extra: location to store image specific backend data
 *
 * Gets an image surface to use when drawing as a fallback when drawing with
 * @surface as a source. _cairo_surface_release_source_image() must be called
 * when finished.
 *
 * Return value: %CAIRO_STATUS_SUCCESS if an image was stored in @image_out.
 * %CAIRO_INT_STATUS_UNSUPPORTED if an image cannot be retrieved for the specified
 * surface. Or %CAIRO_STATUS_NO_MEMORY.
 **/
cairo_status_t
_cairo_surface_acquire_source_image (cairo_surface_t         *surface,
				     cairo_image_surface_t  **image_out,
				     void                   **image_extra)
{
    cairo_status_t status;

    if (unlikely (surface->status))
	return surface->status;

    assert (!surface->finished);

    if (surface->backend->acquire_source_image == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    status = surface->backend->acquire_source_image (surface,
						     image_out, image_extra);
    if (unlikely (status))
	return _cairo_surface_set_error (surface, status);

    _cairo_debug_check_image_surface_is_defined (&(*image_out)->base);

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_surface_default_acquire_source_image (void                    *_surface,
					     cairo_image_surface_t  **image_out,
					     void                   **image_extra)
{
    cairo_surface_t *surface = _surface;
    cairo_rectangle_int_t extents;

    if (unlikely (! surface->backend->get_extents (surface, &extents)))
	return _cairo_error (CAIRO_STATUS_INVALID_SIZE);

    *image_out = _cairo_surface_map_to_image (surface, &extents);
    *image_extra = NULL;
    return (*image_out)->base.status;
}

/**
 * _cairo_surface_release_source_image:
 * @surface: a #cairo_surface_t
 * @image_extra: same as return from the matching _cairo_surface_acquire_source_image()
 *
 * Releases any resources obtained with _cairo_surface_acquire_source_image()
 **/
void
_cairo_surface_release_source_image (cairo_surface_t        *surface,
				     cairo_image_surface_t  *image,
				     void                   *image_extra)
{
    assert (!surface->finished);

    if (surface->backend->release_source_image)
	surface->backend->release_source_image (surface, image, image_extra);
}

void
_cairo_surface_default_release_source_image (void                   *surface,
					     cairo_image_surface_t  *image,
					     void                   *image_extra)
{
    cairo_status_t ignored;

    ignored = _cairo_surface_unmap_image (surface, image);
    (void)ignored;
}


cairo_surface_t *
_cairo_surface_get_source (cairo_surface_t *surface,
			   cairo_rectangle_int_t *extents)
{
    assert (surface->backend->source);
    return surface->backend->source (surface, extents);
}

cairo_surface_t *
_cairo_surface_default_source (void *surface,
			       cairo_rectangle_int_t *extents)
{
    if (extents)
	_cairo_surface_get_extents(surface, extents);
    return surface;
}

static cairo_status_t
_pattern_has_error (const cairo_pattern_t *pattern)
{
    const cairo_surface_pattern_t *spattern;

    if (unlikely (pattern->status))
	return pattern->status;

    if (pattern->type != CAIRO_PATTERN_TYPE_SURFACE)
	return CAIRO_STATUS_SUCCESS;

    spattern = (const cairo_surface_pattern_t *) pattern;
    if (unlikely (spattern->surface->status))
	return spattern->surface->status;

    if (unlikely (spattern->surface->finished))
	return _cairo_error (CAIRO_STATUS_SURFACE_FINISHED);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
nothing_to_do (cairo_surface_t *surface,
	       cairo_operator_t op,
	       const cairo_pattern_t *source)
{
    if (_cairo_pattern_is_clear (source)) {
	if (op == CAIRO_OPERATOR_OVER || op == CAIRO_OPERATOR_ADD)
	    return TRUE;

	if (op == CAIRO_OPERATOR_SOURCE)
	    op = CAIRO_OPERATOR_CLEAR;
    }

    if (op == CAIRO_OPERATOR_CLEAR && surface->is_clear)
	return TRUE;

    if (op == CAIRO_OPERATOR_ATOP && (surface->content & CAIRO_CONTENT_COLOR) ==0)
	return TRUE;

    return FALSE;
}

cairo_status_t
_cairo_surface_paint (cairo_surface_t		*surface,
		      cairo_operator_t		 op,
		      const cairo_pattern_t	*source,
		      const cairo_clip_t	*clip)
{
    cairo_int_status_t status;
    cairo_bool_t is_clear;

    TRACE ((stderr, "%s\n", __FUNCTION__));
    if (unlikely (surface->status))
	return surface->status;
    if (unlikely (surface->finished))
	return _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    if (_cairo_clip_is_all_clipped (clip))
	return CAIRO_STATUS_SUCCESS;

    status = _pattern_has_error (source);
    if (unlikely (status))
	return status;

    if (nothing_to_do (surface, op, source))
	return CAIRO_STATUS_SUCCESS;

    status = _cairo_surface_begin_modification (surface);
    if (unlikely (status))
	return status;

    if (source->is_foreground_marker && surface->foreground_source) {
	source = surface->foreground_source;
	surface->foreground_used = TRUE;
    }

    status = surface->backend->paint (surface, op, source, clip);
    is_clear = op == CAIRO_OPERATOR_CLEAR && clip == NULL;
    if (status != CAIRO_INT_STATUS_NOTHING_TO_DO || is_clear) {
	surface->is_clear = is_clear;
	surface->serial++;
    }

    return _cairo_surface_set_error (surface, status);
}

cairo_status_t
_cairo_surface_mask (cairo_surface_t		*surface,
		     cairo_operator_t		 op,
		     const cairo_pattern_t	*source,
		     const cairo_pattern_t	*mask,
		     const cairo_clip_t		*clip)
{
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));
    if (unlikely (surface->status))
	return surface->status;
    if (unlikely (surface->finished))
	return _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    if (_cairo_clip_is_all_clipped (clip))
	return CAIRO_STATUS_SUCCESS;

    /* If the mask is blank, this is just an expensive no-op */
    if (_cairo_pattern_is_clear (mask) &&
	_cairo_operator_bounded_by_mask (op))
    {
	return CAIRO_STATUS_SUCCESS;
    }

    status = _pattern_has_error (source);
    if (unlikely (status))
	return status;

    status = _pattern_has_error (mask);
    if (unlikely (status))
	return status;

    if (nothing_to_do (surface, op, source))
	return CAIRO_STATUS_SUCCESS;

    status = _cairo_surface_begin_modification (surface);
    if (unlikely (status))
	return status;

    if (source->is_foreground_marker && surface->foreground_source) {
	source = surface->foreground_source;
	surface->foreground_used = TRUE;
    }

    status = surface->backend->mask (surface, op, source, mask, clip);
    if (status != CAIRO_INT_STATUS_NOTHING_TO_DO) {
	surface->is_clear = FALSE;
	surface->serial++;
    }

    return _cairo_surface_set_error (surface, status);
}

cairo_status_t
_cairo_surface_fill_stroke (cairo_surface_t	    *surface,
			    cairo_operator_t	     fill_op,
			    const cairo_pattern_t   *fill_source,
			    cairo_fill_rule_t	     fill_rule,
			    double		     fill_tolerance,
			    cairo_antialias_t	     fill_antialias,
			    cairo_path_fixed_t	    *path,
			    cairo_operator_t	     stroke_op,
			    const cairo_pattern_t   *stroke_source,
			    const cairo_stroke_style_t    *stroke_style,
			    const cairo_matrix_t	    *stroke_ctm,
			    const cairo_matrix_t	    *stroke_ctm_inverse,
			    double		     stroke_tolerance,
			    cairo_antialias_t	     stroke_antialias,
			    const cairo_clip_t	    *clip)
{
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));
    if (unlikely (surface->status))
	return surface->status;
    if (unlikely (surface->finished))
	return _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    if (_cairo_clip_is_all_clipped (clip))
	return CAIRO_STATUS_SUCCESS;

    if (surface->is_clear &&
	fill_op == CAIRO_OPERATOR_CLEAR &&
	stroke_op == CAIRO_OPERATOR_CLEAR)
    {
	return CAIRO_STATUS_SUCCESS;
    }

    status = _pattern_has_error (fill_source);
    if (unlikely (status))
	return status;

    status = _pattern_has_error (stroke_source);
    if (unlikely (status))
	return status;

    status = _cairo_surface_begin_modification (surface);
    if (unlikely (status))
	return status;

    if (fill_source->is_foreground_marker && surface->foreground_source) {
	fill_source = surface->foreground_source;
	surface->foreground_used = TRUE;
    }

    if (stroke_source->is_foreground_marker && surface->foreground_source) {
	stroke_source = surface->foreground_source;
	surface->foreground_used = TRUE;
    }

    if (surface->backend->fill_stroke) {
	cairo_matrix_t dev_ctm = *stroke_ctm;
	cairo_matrix_t dev_ctm_inverse = *stroke_ctm_inverse;

	status = surface->backend->fill_stroke (surface,
						fill_op, fill_source, fill_rule,
						fill_tolerance, fill_antialias,
						path,
						stroke_op, stroke_source,
						stroke_style,
						&dev_ctm, &dev_ctm_inverse,
						stroke_tolerance, stroke_antialias,
						clip);

	if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	    goto FINISH;
    }

    status = _cairo_surface_fill (surface, fill_op, fill_source, path,
				  fill_rule, fill_tolerance, fill_antialias,
				  clip);
    if (unlikely (status))
	goto FINISH;

    status = _cairo_surface_stroke (surface, stroke_op, stroke_source, path,
				    stroke_style, stroke_ctm, stroke_ctm_inverse,
				    stroke_tolerance, stroke_antialias,
				    clip);
    if (unlikely (status))
	goto FINISH;

  FINISH:
    if (status != CAIRO_INT_STATUS_NOTHING_TO_DO) {
	surface->is_clear = FALSE;
	surface->serial++;
    }

    return _cairo_surface_set_error (surface, status);
}

cairo_status_t
_cairo_surface_stroke (cairo_surface_t			*surface,
		       cairo_operator_t			 op,
		       const cairo_pattern_t		*source,
		       const cairo_path_fixed_t		*path,
		       const cairo_stroke_style_t	*stroke_style,
		       const cairo_matrix_t		*ctm,
		       const cairo_matrix_t		*ctm_inverse,
		       double				 tolerance,
		       cairo_antialias_t		 antialias,
		       const cairo_clip_t		*clip)
{
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));
    if (unlikely (surface->status))
	return surface->status;
    if (unlikely (surface->finished))
	return _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    if (_cairo_clip_is_all_clipped (clip))
	return CAIRO_STATUS_SUCCESS;

    status = _pattern_has_error (source);
    if (unlikely (status))
	return status;

    if (nothing_to_do (surface, op, source))
	return CAIRO_STATUS_SUCCESS;

    status = _cairo_surface_begin_modification (surface);
    if (unlikely (status))
	return status;

    if (source->is_foreground_marker && surface->foreground_source) {
	source = surface->foreground_source;
	surface->foreground_used = TRUE;
    }

    status = surface->backend->stroke (surface, op, source,
				       path, stroke_style,
				       ctm, ctm_inverse,
				       tolerance, antialias,
				       clip);
    if (status != CAIRO_INT_STATUS_NOTHING_TO_DO) {
	surface->is_clear = FALSE;
	surface->serial++;
    }

    return _cairo_surface_set_error (surface, status);
}

cairo_status_t
_cairo_surface_fill (cairo_surface_t		*surface,
		     cairo_operator_t		 op,
		     const cairo_pattern_t	 *source,
		     const cairo_path_fixed_t	*path,
		     cairo_fill_rule_t		 fill_rule,
		     double			 tolerance,
		     cairo_antialias_t		 antialias,
		     const cairo_clip_t		*clip)
{
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));
    if (unlikely (surface->status))
	return surface->status;
    if (unlikely (surface->finished))
	return _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    if (_cairo_clip_is_all_clipped (clip))
	return CAIRO_STATUS_SUCCESS;

    status = _pattern_has_error (source);
    if (unlikely (status))
	return status;

    if (nothing_to_do (surface, op, source))
	return CAIRO_STATUS_SUCCESS;

    status = _cairo_surface_begin_modification (surface);
    if (unlikely (status))
	return status;

    if (source->is_foreground_marker && surface->foreground_source) {
	source = surface->foreground_source;
	surface->foreground_used = TRUE;
    }

    status = surface->backend->fill (surface, op, source,
				     path, fill_rule,
				     tolerance, antialias,
				     clip);
    if (status != CAIRO_INT_STATUS_NOTHING_TO_DO) {
	surface->is_clear = FALSE;
	surface->serial++;
    }

    return _cairo_surface_set_error (surface, status);
}

/**
 * cairo_surface_copy_page:
 * @surface: a #cairo_surface_t
 *
 * Emits the current page for backends that support multiple pages,
 * but doesn't clear it, so that the contents of the current page will
 * be retained for the next page.  Use cairo_surface_show_page() if you
 * want to get an empty page after the emission.
 *
 * There is a convenience function for this that takes a #cairo_t,
 * namely cairo_copy_page().
 *
 * Since: 1.6
 **/
void
cairo_surface_copy_page (cairo_surface_t *surface)
{
    if (unlikely (surface->status))
	return;

    assert (surface->snapshot_of == NULL);

    if (unlikely (surface->finished)) {
	_cairo_surface_set_error (surface, CAIRO_STATUS_SURFACE_FINISHED);
	return;
    }

    /* It's fine if some backends don't implement copy_page */
    if (surface->backend->copy_page == NULL)
	return;

    _cairo_surface_set_error (surface, surface->backend->copy_page (surface));
}
slim_hidden_def (cairo_surface_copy_page);

/**
 * cairo_surface_show_page:
 * @surface: a #cairo_Surface_t
 *
 * Emits and clears the current page for backends that support multiple
 * pages.  Use cairo_surface_copy_page() if you don't want to clear the page.
 *
 * There is a convenience function for this that takes a #cairo_t,
 * namely cairo_show_page().
 *
 * Since: 1.6
 **/
void
cairo_surface_show_page (cairo_surface_t *surface)
{
    cairo_status_t status;

    if (unlikely (surface->status))
	return;

    if (unlikely (surface->finished)) {
	_cairo_surface_set_error (surface, CAIRO_STATUS_SURFACE_FINISHED);
	return;
    }

    status = _cairo_surface_begin_modification (surface);
    if (unlikely (status)) {
	_cairo_surface_set_error (surface, status);
	return;
    }

    /* It's fine if some backends don't implement show_page */
    if (surface->backend->show_page == NULL)
	return;

    _cairo_surface_set_error (surface, surface->backend->show_page (surface));
}
slim_hidden_def (cairo_surface_show_page);

/**
 * _cairo_surface_get_extents:
 * @surface: the #cairo_surface_t to fetch extents for
 *
 * This function returns a bounding box for the surface.  The surface
 * bounds are defined as a region beyond which no rendering will
 * possibly be recorded, in other words, it is the maximum extent of
 * potentially usable coordinates.
 *
 * For vector surfaces, (PDF, PS, SVG and recording-surfaces), the surface
 * might be conceived as unbounded, but we force the user to provide a
 * maximum size at the time of surface_create. So get_extents uses
 * that size.
 *
 * Note: The coordinates returned are in "backend" space rather than
 * "surface" space. That is, they are relative to the true (0,0)
 * origin rather than the device_transform origin. This might seem a
 * bit inconsistent with other #cairo_surface_t interfaces, but all
 * current callers are within the surface layer where backend space is
 * desired.
 *
 * This behavior would have to be changed is we ever exported a public
 * variant of this function.
 **/
cairo_bool_t
_cairo_surface_get_extents (cairo_surface_t         *surface,
			    cairo_rectangle_int_t   *extents)
{
    cairo_bool_t bounded;

    if (unlikely (surface->status))
	goto zero_extents;
    if (unlikely (surface->finished)) {
	_cairo_surface_set_error(surface, CAIRO_STATUS_SURFACE_FINISHED);
	goto zero_extents;
    }

    bounded = FALSE;
    if (surface->backend->get_extents != NULL)
	bounded = surface->backend->get_extents (surface, extents);

    if (! bounded)
	_cairo_unbounded_rectangle_init (extents);

    return bounded;

zero_extents:
    extents->x = extents->y = 0;
    extents->width = extents->height = 0;
    return TRUE;
}

/**
 * cairo_surface_has_show_text_glyphs:
 * @surface: a #cairo_surface_t
 *
 * Returns whether the surface supports
 * sophisticated cairo_show_text_glyphs() operations.  That is,
 * whether it actually uses the provided text and cluster data
 * to a cairo_show_text_glyphs() call.
 *
 * Note: Even if this function returns %FALSE, a
 * cairo_show_text_glyphs() operation targeted at @surface will
 * still succeed.  It just will
 * act like a cairo_show_glyphs() operation.  Users can use this
 * function to avoid computing UTF-8 text and cluster mapping if the
 * target surface does not use it.
 *
 * Return value: %TRUE if @surface supports
 *               cairo_show_text_glyphs(), %FALSE otherwise
 *
 * Since: 1.8
 **/
cairo_bool_t
cairo_surface_has_show_text_glyphs (cairo_surface_t	    *surface)
{
    if (unlikely (surface->status))
	return FALSE;

    if (unlikely (surface->finished)) {
	_cairo_surface_set_error (surface, CAIRO_STATUS_SURFACE_FINISHED);
	return FALSE;
    }

    if (surface->backend->has_show_text_glyphs)
	return surface->backend->has_show_text_glyphs (surface);
    else
	return surface->backend->show_text_glyphs != NULL;
}
slim_hidden_def (cairo_surface_has_show_text_glyphs);

#define GLYPH_CACHE_SIZE 64

static inline cairo_int_status_t
ensure_scaled_glyph (cairo_scaled_font_t   *scaled_font,
		     cairo_color_t         *foreground_color,
                     cairo_scaled_glyph_t **glyph_cache,
                     cairo_glyph_t         *glyph,
                     cairo_scaled_glyph_t **scaled_glyph)
{
    int cache_index;
    cairo_int_status_t status = CAIRO_INT_STATUS_SUCCESS;

    cache_index = glyph->index % GLYPH_CACHE_SIZE;
    *scaled_glyph = glyph_cache[cache_index];
    if (*scaled_glyph == NULL || _cairo_scaled_glyph_index (*scaled_glyph) != glyph->index) {
        status = _cairo_scaled_glyph_lookup (scaled_font,
                                             glyph->index,
                                             CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE,
                                             foreground_color,
                                             scaled_glyph);
        if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
            /* If the color surface not available, ensure scaled_glyph is not NULL. */
            status = _cairo_scaled_glyph_lookup (scaled_font,
                                                 glyph->index,
                                                 CAIRO_SCALED_GLYPH_INFO_SURFACE,
                                                 NULL, /* foreground color */
                                                 scaled_glyph);
        }
        if (unlikely (status))
            status = _cairo_scaled_font_set_error (scaled_font, status);

        glyph_cache[cache_index] = *scaled_glyph;
    }

    return status;
}

static inline cairo_int_status_t
composite_one_color_glyph (cairo_surface_t       *surface,
                           cairo_operator_t       op,
                           const cairo_pattern_t *source,
                           const cairo_clip_t    *clip,
                           cairo_glyph_t         *glyph,
                           cairo_scaled_glyph_t  *scaled_glyph,
			   double                 x_scale,
			   double                 y_scale)
{
    cairo_int_status_t status;
    cairo_image_surface_t *glyph_surface;
    cairo_pattern_t *pattern;
    cairo_matrix_t matrix;
    int has_color;

    status = CAIRO_INT_STATUS_SUCCESS;

    has_color = scaled_glyph->has_info & CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE;
    if (has_color)
        glyph_surface = scaled_glyph->color_surface;
    else
        glyph_surface = scaled_glyph->surface;

    if (glyph_surface->width && glyph_surface->height) {
        int x, y;
        /* round glyph locations to the nearest pixels */
        /* XXX: FRAGILE: We're ignoring device_transform scaling here. A bug? */
	x = _cairo_lround (glyph->x * x_scale - glyph_surface->base.device_transform.x0);
	y = _cairo_lround (glyph->y * y_scale - glyph_surface->base.device_transform.y0);

        pattern = cairo_pattern_create_for_surface ((cairo_surface_t *)glyph_surface);
        cairo_matrix_init_translate (&matrix, - x, - y);
	cairo_matrix_scale (&matrix, x_scale, y_scale);
        cairo_pattern_set_matrix (pattern, &matrix);
        if (op == CAIRO_OPERATOR_SOURCE || op == CAIRO_OPERATOR_CLEAR || !has_color)
	    status = _cairo_surface_mask (surface, op, pattern, pattern, clip);
        else
	    status = _cairo_surface_paint (surface, op, pattern, clip);
        cairo_pattern_destroy (pattern);
    }

    return status;
}

static cairo_int_status_t
composite_color_glyphs (cairo_surface_t             *surface,
                        cairo_operator_t             op,
                        const cairo_pattern_t       *source,
                        char                        *utf8,
                        int                         *utf8_len,
                        cairo_glyph_t               *glyphs,
                        int                         *num_glyphs,
                        cairo_text_cluster_t        *clusters,
	                int			    *num_clusters,
		        cairo_text_cluster_flags_t   cluster_flags,
                        cairo_scaled_font_t         *scaled_font,
                        const cairo_clip_t          *clip)
{
    cairo_int_status_t status;
    int i, j;
    cairo_scaled_glyph_t *scaled_glyph;
    int remaining_clusters = 0;
    int remaining_glyphs = 0;
    int remaining_bytes = 0;
    int glyph_pos = 0;
    int byte_pos = 0;
    int gp;
    cairo_scaled_glyph_t *glyph_cache[GLYPH_CACHE_SIZE];
    cairo_color_t *foreground_color = NULL;
    double x_scale = 1.0;
    double y_scale = 1.0;

    if (surface->is_vector) {
	cairo_font_face_t *font_face;
	cairo_matrix_t font_matrix;
	cairo_matrix_t ctm;
	cairo_font_options_t font_options;

	x_scale = surface->x_fallback_resolution / surface->x_resolution;
	y_scale = surface->y_fallback_resolution / surface->y_resolution;
	font_face = cairo_scaled_font_get_font_face (scaled_font);
	cairo_scaled_font_get_font_matrix (scaled_font, &font_matrix);
	cairo_scaled_font_get_ctm (scaled_font, &ctm);
	_cairo_font_options_init_default (&font_options);
	cairo_scaled_font_get_font_options (scaled_font, &font_options);
	cairo_matrix_scale (&ctm, x_scale, y_scale);
	scaled_font = cairo_scaled_font_create (font_face,
						&font_matrix,
						&ctm,
						&font_options);
    }

    if (source->type == CAIRO_PATTERN_TYPE_SOLID)
	foreground_color = &((cairo_solid_pattern_t *) source)->color;

    memset (glyph_cache, 0, sizeof (glyph_cache));

    status = CAIRO_INT_STATUS_SUCCESS;

    _cairo_scaled_font_freeze_cache (scaled_font);

    if (clusters) {

        if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
            glyph_pos = *num_glyphs - 1;

        for (i = 0; i < *num_clusters; i++) {
            cairo_bool_t skip_cluster = TRUE;

            for (j = 0; j < clusters[i].num_glyphs; j++) {
                if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
                    gp = glyph_pos - j;
                else
                    gp = glyph_pos + j;

                status = ensure_scaled_glyph (scaled_font, foreground_color, glyph_cache,
                                              &glyphs[gp], &scaled_glyph);
                if (unlikely (status))
                    goto UNLOCK;

                if ((scaled_glyph->has_info & CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE) != 0) {
                    skip_cluster = FALSE;
                    break;
                }
            }

            if (skip_cluster) {
                memmove (utf8 + remaining_bytes, utf8 + byte_pos, clusters[i].num_bytes);
                remaining_bytes += clusters[i].num_bytes;
                byte_pos += clusters[i].num_bytes;
                for (j = 0; j < clusters[i].num_glyphs; j++, remaining_glyphs++) {
                    if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
                        glyphs[*num_glyphs - 1 - remaining_glyphs] = glyphs[glyph_pos--];
                    else
                        glyphs[remaining_glyphs] = glyphs[glyph_pos++];
                }
                clusters[remaining_clusters++] = clusters[i];
                continue;
            }

            for (j = 0; j < clusters[i].num_glyphs; j++) {
                if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
                    gp = glyph_pos - j;
                else
                    gp = glyph_pos + j;

                status = ensure_scaled_glyph (scaled_font, foreground_color, glyph_cache,
                                              &glyphs[gp], &scaled_glyph);
                if (unlikely (status))
                    goto UNLOCK;

                status = composite_one_color_glyph (surface, op, source, clip,
						    &glyphs[gp], scaled_glyph,
						    x_scale, y_scale);
                if (unlikely (status && status != CAIRO_INT_STATUS_NOTHING_TO_DO))
                    goto UNLOCK;
            }

            if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
                glyph_pos -= clusters[i].num_glyphs;
            else
                glyph_pos += clusters[i].num_glyphs;

            byte_pos += clusters[i].num_bytes;
        }

        if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD) {
            memmove (utf8, utf8 + *utf8_len - remaining_bytes, remaining_bytes);
            memmove (glyphs, glyphs + (*num_glyphs - remaining_glyphs), sizeof (cairo_glyph_t) * remaining_glyphs);
        }

        *utf8_len = remaining_bytes;
        *num_glyphs = remaining_glyphs;
        *num_clusters = remaining_clusters;

    } else {

       for (glyph_pos = 0; glyph_pos < *num_glyphs; glyph_pos++) {
           status = ensure_scaled_glyph (scaled_font, foreground_color, glyph_cache,
                                         &glyphs[glyph_pos], &scaled_glyph);
           if (unlikely (status))
               goto UNLOCK;

           if ((scaled_glyph->has_info & CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE) == 0) {
               glyphs[remaining_glyphs++] = glyphs[glyph_pos];
               continue;
           }

           status = composite_one_color_glyph (surface, op, source, clip,
					       &glyphs[glyph_pos], scaled_glyph,
					       x_scale, y_scale);
           if (unlikely (status && status != CAIRO_INT_STATUS_NOTHING_TO_DO))
               goto UNLOCK;
        }

        *num_glyphs = remaining_glyphs;
    }

UNLOCK:
    _cairo_scaled_font_thaw_cache (scaled_font);

    if (surface->is_vector)
	cairo_scaled_font_destroy (scaled_font);

    return status;
}

/* Note: the backends may modify the contents of the glyph array as long as
 * they do not return %CAIRO_INT_STATUS_UNSUPPORTED. This makes it possible to
 * avoid copying the array again and again, and edit it in-place.
 * Backends are in fact free to use the array as a generic buffer as they
 * see fit.
 *
 * For show_glyphs backend method, and NOT for show_text_glyphs method,
 * when they do return UNSUPPORTED, they may adjust remaining_glyphs to notify
 * that they have successfully rendered some of the glyphs (from the beginning
 * of the array), but not all.  If they don't touch remaining_glyphs, it
 * defaults to all glyphs.
 *
 * See commits 5a9642c5746fd677aed35ce620ce90b1029b1a0c and
 * 1781e6018c17909311295a9cc74b70500c6b4d0a for the rationale.
 */
cairo_status_t
_cairo_surface_show_text_glyphs (cairo_surface_t	    *surface,
				 cairo_operator_t	     op,
				 const cairo_pattern_t	    *source,
				 const char		    *utf8,
				 int			     utf8_len,
				 cairo_glyph_t		    *glyphs,
				 int			     num_glyphs,
				 const cairo_text_cluster_t *clusters,
				 int			     num_clusters,
				 cairo_text_cluster_flags_t  cluster_flags,
				 cairo_scaled_font_t	    *scaled_font,
				 const cairo_clip_t	    *clip)
{
    cairo_int_status_t status;
    char *utf8_copy = NULL;

    TRACE ((stderr, "%s\n", __FUNCTION__));
    if (unlikely (surface->status))
	return surface->status;
    if (unlikely (surface->finished))
	return _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    if (num_glyphs == 0 && utf8_len == 0)
	return CAIRO_STATUS_SUCCESS;

    if (_cairo_clip_is_all_clipped (clip))
	return CAIRO_STATUS_SUCCESS;

    status = _pattern_has_error (source);
    if (unlikely (status))
	return status;

    status = cairo_scaled_font_status (scaled_font);
    if (unlikely (status))
	return status;

    if (!(_cairo_scaled_font_has_color_glyphs (scaled_font) &&
	  scaled_font->options.color_mode != CAIRO_COLOR_MODE_NO_COLOR))
    {
        if (nothing_to_do (surface, op, source))
	    return CAIRO_STATUS_SUCCESS;
    }

    status = _cairo_surface_begin_modification (surface);
    if (unlikely (status))
	return status;

    if (source->is_foreground_marker && surface->foreground_source)
	source = surface->foreground_source;

    if (_cairo_scaled_font_has_color_glyphs (scaled_font) &&
	scaled_font->options.color_mode != CAIRO_COLOR_MODE_NO_COLOR)
    {
        utf8_copy = malloc (sizeof (char) * utf8_len);
        memcpy (utf8_copy, utf8, sizeof (char) * utf8_len);
        utf8 = utf8_copy;

        status = composite_color_glyphs (surface, op,
                                         source,
                                         (char *)utf8, &utf8_len,
                                         glyphs, &num_glyphs,
                                         (cairo_text_cluster_t *)clusters, &num_clusters, cluster_flags,
                                         scaled_font,
                                         clip);

        if (unlikely (status && status != CAIRO_INT_STATUS_NOTHING_TO_DO))
            goto DONE;

        if (num_glyphs == 0)
            goto DONE;
    }
    else
      utf8_copy = NULL;

    /* The logic here is duplicated in _cairo_analysis_surface show_glyphs and
     * show_text_glyphs.  Keep in synch. */
    if (clusters) {
        status = CAIRO_INT_STATUS_UNSUPPORTED;
	/* A real show_text_glyphs call.  Try show_text_glyphs backend
	 * method first */
	if (surface->backend->show_text_glyphs != NULL) {
	    status = surface->backend->show_text_glyphs (surface, op,
							 source,
							 utf8, utf8_len,
							 glyphs, num_glyphs,
							 clusters, num_clusters, cluster_flags,
							 scaled_font,
							 clip);
	}
	if (status == CAIRO_INT_STATUS_UNSUPPORTED &&
	    surface->backend->show_glyphs)
	{
	    status = surface->backend->show_glyphs (surface, op,
						    source,
						    glyphs, num_glyphs,
						    scaled_font,
						    clip);
	}
    } else {
	/* A mere show_glyphs call.  Try show_glyphs backend method first */
	if (surface->backend->show_glyphs != NULL) {
	    status = surface->backend->show_glyphs (surface, op,
						    source,
						    glyphs, num_glyphs,
						    scaled_font,
						    clip);
	} else if (surface->backend->show_text_glyphs != NULL) {
	    /* Intentionally only try show_text_glyphs method for show_glyphs
	     * calls if backend does not have show_glyphs.  If backend has
	     * both methods implemented, we don't fallback from show_glyphs to
	     * show_text_glyphs, and hence the backend can assume in its
	     * show_text_glyphs call that clusters is not NULL (which also
	     * implies that UTF-8 is not NULL, unless the text is
	     * zero-length).
	     */
	    status = surface->backend->show_text_glyphs (surface, op,
							 source,
							 utf8, utf8_len,
							 glyphs, num_glyphs,
							 clusters, num_clusters, cluster_flags,
							 scaled_font,
							 clip);
	}
    }

DONE:
    if (status != CAIRO_INT_STATUS_NOTHING_TO_DO) {
	surface->is_clear = FALSE;
	surface->serial++;
    }

    if (utf8_copy)
        free (utf8_copy);

    return _cairo_surface_set_error (surface, status);
}

cairo_status_t
_cairo_surface_tag (cairo_surface_t	        *surface,
		    cairo_bool_t                 begin,
		    const char                  *tag_name,
		    const char                  *attributes)
{
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));
    if (unlikely (surface->status))
	return surface->status;
    if (unlikely (surface->finished))
	return _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    if (surface->backend->tag == NULL)
	return CAIRO_STATUS_SUCCESS;

    status = surface->backend->tag (surface, begin, tag_name, attributes);
    surface->is_clear = FALSE;

    return _cairo_surface_set_error (surface, status);
}


/**
 * _cairo_surface_set_resolution:
 * @surface: the surface
 * @x_res: x resolution, in dpi
 * @y_res: y resolution, in dpi
 *
 * Set the actual surface resolution of @surface to the given x and y DPI.
 * Mainly used for correctly computing the scale factor when fallback
 * rendering needs to take place in the paginated surface.
 **/
void
_cairo_surface_set_resolution (cairo_surface_t *surface,
			       double x_res,
			       double y_res)
{
    if (surface->status)
	return;

    surface->x_resolution = x_res;
    surface->y_resolution = y_res;
}

/**
 * _cairo_surface_create_in_error:
 * @status: the error status
 *
 * Return an appropriate static error surface for the error status.
 * On error, surface creation functions should always return a surface
 * created with _cairo_surface_create_in_error() instead of a new surface
 * in an error state. This simplifies internal code as no refcounting has
 * to be done.
 **/
cairo_surface_t *
_cairo_surface_create_in_error (cairo_status_t status)
{
    assert (status < CAIRO_STATUS_LAST_STATUS);
    switch (status) {
    case CAIRO_STATUS_NO_MEMORY:
	return (cairo_surface_t *) &_cairo_surface_nil;
    case CAIRO_STATUS_SURFACE_TYPE_MISMATCH:
	return (cairo_surface_t *) &_cairo_surface_nil_surface_type_mismatch;
    case CAIRO_STATUS_INVALID_STATUS:
	return (cairo_surface_t *) &_cairo_surface_nil_invalid_status;
    case CAIRO_STATUS_INVALID_CONTENT:
	return (cairo_surface_t *) &_cairo_surface_nil_invalid_content;
    case CAIRO_STATUS_INVALID_FORMAT:
	return (cairo_surface_t *) &_cairo_surface_nil_invalid_format;
    case CAIRO_STATUS_INVALID_VISUAL:
	return (cairo_surface_t *) &_cairo_surface_nil_invalid_visual;
    case CAIRO_STATUS_READ_ERROR:
	return (cairo_surface_t *) &_cairo_surface_nil_read_error;
    case CAIRO_STATUS_WRITE_ERROR:
	return (cairo_surface_t *) &_cairo_surface_nil_write_error;
    case CAIRO_STATUS_FILE_NOT_FOUND:
	return (cairo_surface_t *) &_cairo_surface_nil_file_not_found;
    case CAIRO_STATUS_TEMP_FILE_ERROR:
	return (cairo_surface_t *) &_cairo_surface_nil_temp_file_error;
    case CAIRO_STATUS_INVALID_STRIDE:
	return (cairo_surface_t *) &_cairo_surface_nil_invalid_stride;
    case CAIRO_STATUS_INVALID_SIZE:
	return (cairo_surface_t *) &_cairo_surface_nil_invalid_size;
    case CAIRO_STATUS_DEVICE_TYPE_MISMATCH:
	return (cairo_surface_t *) &_cairo_surface_nil_device_type_mismatch;
    case CAIRO_STATUS_DEVICE_ERROR:
	return (cairo_surface_t *) &_cairo_surface_nil_device_error;
    case CAIRO_STATUS_SUCCESS:
    case CAIRO_STATUS_LAST_STATUS:
	ASSERT_NOT_REACHED;
	/* fall-through */
    case CAIRO_STATUS_INVALID_RESTORE:
    case CAIRO_STATUS_INVALID_POP_GROUP:
    case CAIRO_STATUS_NO_CURRENT_POINT:
    case CAIRO_STATUS_INVALID_MATRIX:
    case CAIRO_STATUS_NULL_POINTER:
    case CAIRO_STATUS_INVALID_STRING:
    case CAIRO_STATUS_INVALID_PATH_DATA:
    case CAIRO_STATUS_SURFACE_FINISHED:
    case CAIRO_STATUS_PATTERN_TYPE_MISMATCH:
    case CAIRO_STATUS_INVALID_DASH:
    case CAIRO_STATUS_INVALID_DSC_COMMENT:
    case CAIRO_STATUS_INVALID_INDEX:
    case CAIRO_STATUS_CLIP_NOT_REPRESENTABLE:
    case CAIRO_STATUS_FONT_TYPE_MISMATCH:
    case CAIRO_STATUS_USER_FONT_IMMUTABLE:
    case CAIRO_STATUS_USER_FONT_ERROR:
    case CAIRO_STATUS_NEGATIVE_COUNT:
    case CAIRO_STATUS_INVALID_CLUSTERS:
    case CAIRO_STATUS_INVALID_SLANT:
    case CAIRO_STATUS_INVALID_WEIGHT:
    case CAIRO_STATUS_USER_FONT_NOT_IMPLEMENTED:
    case CAIRO_STATUS_INVALID_MESH_CONSTRUCTION:
    case CAIRO_STATUS_DEVICE_FINISHED:
    case CAIRO_STATUS_JBIG2_GLOBAL_MISSING:
    case CAIRO_STATUS_PNG_ERROR:
    case CAIRO_STATUS_FREETYPE_ERROR:
    case CAIRO_STATUS_WIN32_GDI_ERROR:
    case CAIRO_INT_STATUS_DWRITE_ERROR:
    case CAIRO_STATUS_TAG_ERROR:
    case CAIRO_STATUS_SVG_FONT_ERROR:
    default:
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_surface_t *) &_cairo_surface_nil;
    }
}

cairo_surface_t *
_cairo_int_surface_create_in_error (cairo_int_status_t status)
{
    if (status < CAIRO_INT_STATUS_LAST_STATUS)
	return _cairo_surface_create_in_error (status);

    switch ((int)status) {
    case CAIRO_INT_STATUS_UNSUPPORTED:
	return (cairo_surface_t *) &_cairo_surface_nil_unsupported;
    case CAIRO_INT_STATUS_NOTHING_TO_DO:
	return (cairo_surface_t *) &_cairo_surface_nil_nothing_to_do;
    default:
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_surface_t *) &_cairo_surface_nil;
    }
}

/*  LocalWords:  rasterized
 */
