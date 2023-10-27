/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright � 2008 Mozilla Corporation
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
 * The Initial Developer of the Original Code is Mozilla Foundation.
 *
 * Contributor(s):
 *	Vladimir Vukicevic <vladimir@mozilla.com>
 */

#include "cairoint.h"

#include "cairo-image-surface-inline.h"
#include "cairo-quartz-image.h"
#include "cairo-quartz-private.h"
#include "cairo-surface-backend-private.h"

#include "cairo-error-private.h"
#include "cairo-default-context-private.h"

#define SURFACE_ERROR_NO_MEMORY (_cairo_surface_create_in_error(_cairo_error(CAIRO_STATUS_NO_MEMORY)))
#define SURFACE_ERROR_TYPE_MISMATCH (_cairo_surface_create_in_error(_cairo_error(CAIRO_STATUS_SURFACE_TYPE_MISMATCH)))
#define SURFACE_ERROR_INVALID_SIZE (_cairo_surface_create_in_error(_cairo_error(CAIRO_STATUS_INVALID_SIZE)))
#define SURFACE_ERROR_INVALID_FORMAT (_cairo_surface_create_in_error(_cairo_error(CAIRO_STATUS_INVALID_FORMAT)))

static void
DataProviderReleaseCallback (void *image_info, const void *data, size_t size)
{
    free (image_info);
}

static cairo_surface_t *
_cairo_quartz_image_surface_create_similar (void *asurface,
					    cairo_content_t content,
					    int width,
					    int height)
{
    cairo_surface_t *isurf =
	_cairo_image_surface_create_with_content (content, width, height);
    cairo_surface_t *result = cairo_quartz_image_surface_create (isurf);
    cairo_surface_destroy (isurf);

    return result;
}

static cairo_surface_t *
_cairo_quartz_image_surface_create_similar_image (void *asurface,
						  cairo_format_t format,
						  int width,
						  int height)
{
    cairo_surface_t *isurf = cairo_image_surface_create (format, width, height);
    cairo_surface_t *result = cairo_quartz_image_surface_create (isurf);
    cairo_surface_destroy (isurf);

    return result;
}

static cairo_status_t
_cairo_quartz_image_surface_finish (void *asurface)
{
    cairo_quartz_image_surface_t *surface = (cairo_quartz_image_surface_t *) asurface;

    CGImageRelease (surface->image);
    cairo_surface_destroy ( (cairo_surface_t*) surface->imageSurface);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_quartz_image_surface_acquire_source_image (void *asurface,
						  cairo_image_surface_t **image_out,
						  void **image_extra)
{
    cairo_quartz_image_surface_t *surface = (cairo_quartz_image_surface_t *) asurface;

    *image_out = surface->imageSurface;
    *image_extra = NULL;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_image_surface_t *
_cairo_quartz_image_surface_map_to_image (void *asurface,
					  const cairo_rectangle_int_t *extents)
{
    cairo_quartz_image_surface_t *surface = (cairo_quartz_image_surface_t *) asurface;
    return _cairo_surface_map_to_image (&surface->imageSurface->base, extents);
}

static cairo_int_status_t
_cairo_quartz_image_surface_unmap_image (void *asurface,
					 cairo_image_surface_t *image)
{
    cairo_quartz_image_surface_t *surface = (cairo_quartz_image_surface_t *) asurface;
    return _cairo_surface_unmap_image (&surface->imageSurface->base, image);
}

static cairo_bool_t
_cairo_quartz_image_surface_get_extents (void *asurface,
					 cairo_rectangle_int_t *extents)
{
    cairo_quartz_image_surface_t *surface = (cairo_quartz_image_surface_t *) asurface;

    extents->x = 0;
    extents->y = 0;
    extents->width  = surface->width;
    extents->height = surface->height;
    return TRUE;
}

/* we assume some drawing happened to the image buffer; make sure it's
 * represented in the CGImage on flush()
 */

static cairo_status_t
_cairo_quartz_image_surface_flush (void *asurface,
				   unsigned flags)
{
    cairo_quartz_image_surface_t *surface = (cairo_quartz_image_surface_t *) asurface;
    CGImageRef oldImage = surface->image;
    CGImageRef newImage = NULL;
    void *image_data;
    const unsigned int size = surface->imageSurface->height * surface->imageSurface->stride;
    if (flags)
	return CAIRO_STATUS_SUCCESS;

    /* XXX only flush if the image has been modified. */

    image_data = _cairo_malloc_ab ( surface->imageSurface->height,
				    surface->imageSurface->stride);
    if (unlikely (!image_data))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    memcpy (image_data, surface->imageSurface->data,
	    surface->imageSurface->height * surface->imageSurface->stride);
    newImage = CairoQuartzCreateCGImage (surface->imageSurface->format,
					 surface->imageSurface->width,
					 surface->imageSurface->height,
					 surface->imageSurface->stride,
					 image_data,
					 TRUE,
					 NULL,
					 DataProviderReleaseCallback,
					 image_data);

    surface->image = newImage;
    CGImageRelease (oldImage);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_quartz_image_surface_paint (void			*abstract_surface,
				   cairo_operator_t		 op,
				   const cairo_pattern_t	*source,
				   const cairo_clip_t		*clip)
{
    cairo_quartz_image_surface_t *surface = abstract_surface;
    return _cairo_surface_paint (&surface->imageSurface->base,
				 op, source, clip);
}

static cairo_int_status_t
_cairo_quartz_image_surface_mask (void				*abstract_surface,
				  cairo_operator_t		 op,
				  const cairo_pattern_t		*source,
				  const cairo_pattern_t		*mask,
				  const cairo_clip_t		*clip)
{
    cairo_quartz_image_surface_t *surface = abstract_surface;
    return _cairo_surface_mask (&surface->imageSurface->base,
				op, source, mask, clip);
}

static cairo_int_status_t
_cairo_quartz_image_surface_stroke (void			*abstract_surface,
				    cairo_operator_t		 op,
				    const cairo_pattern_t	*source,
				    const cairo_path_fixed_t	*path,
				    const cairo_stroke_style_t	*style,
				    const cairo_matrix_t	*ctm,
				    const cairo_matrix_t	*ctm_inverse,
				    double			 tolerance,
				    cairo_antialias_t		 antialias,
				    const cairo_clip_t		*clip)
{
    cairo_quartz_image_surface_t *surface = abstract_surface;
    return _cairo_surface_stroke (&surface->imageSurface->base,
				  op, source, path,
				  style, ctm, ctm_inverse,
				  tolerance, antialias, clip);
}

static cairo_int_status_t
_cairo_quartz_image_surface_fill (void				*abstract_surface,
			   cairo_operator_t		 op,
			   const cairo_pattern_t	*source,
			   const cairo_path_fixed_t	*path,
			   cairo_fill_rule_t		 fill_rule,
			   double			 tolerance,
			   cairo_antialias_t		 antialias,
			   const cairo_clip_t		*clip)
{
    cairo_quartz_image_surface_t *surface = abstract_surface;
    return _cairo_surface_fill (&surface->imageSurface->base,
				op, source, path,
				fill_rule, tolerance, antialias,
				clip);
}

static cairo_int_status_t
_cairo_quartz_image_surface_glyphs (void			*abstract_surface,
				    cairo_operator_t		 op,
				    const cairo_pattern_t	*source,
				    cairo_glyph_t		*glyphs,
				    int				 num_glyphs,
				    cairo_scaled_font_t		*scaled_font,
				    const cairo_clip_t		*clip)
{
    cairo_quartz_image_surface_t *surface = abstract_surface;
    return _cairo_surface_show_text_glyphs (&surface->imageSurface->base,
					    op, source,
					    NULL, 0,
					    glyphs, num_glyphs,
					    NULL, 0, 0,
					    scaled_font, clip);
}


static const cairo_surface_backend_t cairo_quartz_image_surface_backend = {
    CAIRO_SURFACE_TYPE_QUARTZ_IMAGE,
    _cairo_quartz_image_surface_finish,

    _cairo_default_context_create,

    _cairo_quartz_image_surface_create_similar,
    _cairo_quartz_image_surface_create_similar_image,
    _cairo_quartz_image_surface_map_to_image,
    _cairo_quartz_image_surface_unmap_image,

    _cairo_surface_default_source,
    _cairo_quartz_image_surface_acquire_source_image,
    NULL, /* release_source_image */
    NULL, /* snapshot */

    NULL, /* copy_page */
    NULL, /* show_page */

    _cairo_quartz_image_surface_get_extents,
    NULL, /* get_font_options */

    _cairo_quartz_image_surface_flush,
    NULL, /* mark_dirty_rectangle */

    _cairo_quartz_image_surface_paint,
    _cairo_quartz_image_surface_mask,
    _cairo_quartz_image_surface_stroke,
    _cairo_quartz_image_surface_fill,
    NULL,  /* fill-stroke */
    _cairo_quartz_image_surface_glyphs,
};

/**
 * cairo_quartz_image_surface_create:
 * @image_surface: a cairo image surface to wrap with a quartz image surface
 *
 * Creates a Quartz surface backed by a CGImageRef that references the
 * given image surface. The resulting surface can be rendered quickly
 * when used as a source when rendering to a #cairo_quartz_surface.  If
 * the data in the image surface is ever updated, cairo_surface_flush()
 * must be called on the #cairo_quartz_image_surface to ensure that the
 * CGImageRef refers to the updated data.
 *
 * Return value: the newly created surface.
 *
 * Since: 1.6
 **/
cairo_surface_t *
cairo_quartz_image_surface_create (cairo_surface_t *surface)
{
    cairo_quartz_image_surface_t *qisurf;

    CGImageRef image;

    cairo_image_surface_t *image_surface;
    int width, height, stride;
    cairo_format_t format;
    void *image_data;

    if (surface->status)
	return surface;

    if (! _cairo_surface_is_image (surface))
	return SURFACE_ERROR_TYPE_MISMATCH;

    image_surface = (cairo_image_surface_t*) surface;
    width = image_surface->width;
    height = image_surface->height;
    stride = image_surface->stride;
    format = image_surface->format;

    if (!_cairo_quartz_verify_surface_size(width, height))
	return SURFACE_ERROR_INVALID_SIZE;

    if (width == 0 || height == 0)
	return SURFACE_ERROR_INVALID_SIZE;

    if (format != CAIRO_FORMAT_ARGB32 && format != CAIRO_FORMAT_RGB24)
	return SURFACE_ERROR_INVALID_FORMAT;

    qisurf = _cairo_malloc (sizeof(cairo_quartz_image_surface_t));
    if (qisurf == NULL)
	return SURFACE_ERROR_NO_MEMORY;

    memset (qisurf, 0, sizeof(cairo_quartz_image_surface_t));

    image_data = _cairo_malloc_ab (height, stride);
    if (unlikely (!image_data)) {
	free(qisurf);
	return SURFACE_ERROR_NO_MEMORY;
    }

    memcpy (image_data, image_surface->data, height * stride);
    image = CairoQuartzCreateCGImage (format,
				      width, height,
				      stride,
				      image_data,
				      TRUE,
				      NULL,
				      DataProviderReleaseCallback,
				      image_data);

    if (!image) {
	free (qisurf);
	return SURFACE_ERROR_NO_MEMORY;
    }

    _cairo_surface_init (&qisurf->base,
			 &cairo_quartz_image_surface_backend,
			 NULL, /* device */
			 _cairo_content_from_format (format),
			 FALSE); /* is_vector */

    qisurf->width = width;
    qisurf->height = height;

    qisurf->image = image;
    qisurf->imageSurface = (cairo_image_surface_t*) cairo_surface_reference(surface);

    return &qisurf->base;
}


cairo_surface_t *
cairo_quartz_image_surface_get_image (cairo_surface_t *asurface)
{
    cairo_quartz_image_surface_t *surface = (cairo_quartz_image_surface_t*) asurface;

    /* Throw an error for a non-quartz surface */
    if (! _cairo_surface_is_quartz (asurface)) {
        return SURFACE_ERROR_TYPE_MISMATCH;
    }

    return (cairo_surface_t*) surface->imageSurface;
}
