/* cairo - a vector graphics library with display and print output
 *
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
 * The Initial Developer of the Original Code is Intel Corporation.
 *
 * Contributor(s):
 *      Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-clip-inline.h"
#include "cairo-error-private.h"
#include "cairo-image-surface-private.h"
#include "cairo-recording-surface-private.h"
#include "cairo-surface-offset-private.h"
#include "cairo-surface-snapshot-private.h"
#include "cairo-surface-subsurface-private.h"

static const cairo_surface_backend_t _cairo_surface_subsurface_backend;

static cairo_status_t
_cairo_surface_subsurface_finish (void *abstract_surface)
{
    cairo_surface_subsurface_t *surface = abstract_surface;

    cairo_surface_destroy (surface->target);
    cairo_surface_destroy (surface->snapshot);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_surface_t *
_cairo_surface_subsurface_create_similar (void *other,
					  cairo_content_t content,
					  int width, int height)
{
    cairo_surface_subsurface_t *surface = other;

    if (surface->target->backend->create_similar == NULL)
	return NULL;

    return surface->target->backend->create_similar (surface->target, content, width, height);
}

static cairo_surface_t *
_cairo_surface_subsurface_create_similar_image (void *other,
						cairo_format_t format,
						int width, int height)
{
    cairo_surface_subsurface_t *surface = other;

    if (surface->target->backend->create_similar_image == NULL)
	return NULL;

    return surface->target->backend->create_similar_image (surface->target,
							   format,
							   width, height);
}

static cairo_image_surface_t *
_cairo_surface_subsurface_map_to_image (void *abstract_surface,
					const cairo_rectangle_int_t *extents)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    cairo_rectangle_int_t target_extents;

    target_extents.x = extents->x + surface->extents.x;
    target_extents.y = extents->y + surface->extents.y;
    target_extents.width  = extents->width;
    target_extents.height = extents->height;

    return _cairo_surface_map_to_image (surface->target, &target_extents);
}

static cairo_int_status_t
_cairo_surface_subsurface_unmap_image (void *abstract_surface,
				       cairo_image_surface_t *image)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    return _cairo_surface_unmap_image (surface->target, image);
}

static cairo_int_status_t
_cairo_surface_subsurface_paint (void *abstract_surface,
				 cairo_operator_t op,
				 const cairo_pattern_t *source,
				 const cairo_clip_t *clip)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    cairo_rectangle_int_t rect = { 0, 0, surface->extents.width, surface->extents.height };
    cairo_status_t status;
    cairo_clip_t *target_clip;

    target_clip = _cairo_clip_copy_intersect_rectangle (clip, &rect);
    status = _cairo_surface_offset_paint (surface->target,
					 -surface->extents.x, -surface->extents.y,
					  op, source, target_clip);
    _cairo_clip_destroy (target_clip);
    return status;
}

static cairo_int_status_t
_cairo_surface_subsurface_mask (void *abstract_surface,
				cairo_operator_t op,
				const cairo_pattern_t *source,
				const cairo_pattern_t *mask,
				const cairo_clip_t *clip)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    cairo_rectangle_int_t rect = { 0, 0, surface->extents.width, surface->extents.height };
    cairo_status_t status;
    cairo_clip_t *target_clip;

    target_clip = _cairo_clip_copy_intersect_rectangle (clip, &rect);
    status = _cairo_surface_offset_mask (surface->target,
					 -surface->extents.x, -surface->extents.y,
					 op, source, mask, target_clip);
    _cairo_clip_destroy (target_clip);
    return status;
}

static cairo_int_status_t
_cairo_surface_subsurface_fill (void			*abstract_surface,
				cairo_operator_t	 op,
				const cairo_pattern_t	*source,
				const cairo_path_fixed_t	*path,
				cairo_fill_rule_t	 fill_rule,
				double			 tolerance,
				cairo_antialias_t	 antialias,
				const cairo_clip_t		*clip)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    cairo_rectangle_int_t rect = { 0, 0, surface->extents.width, surface->extents.height };
    cairo_status_t status;
    cairo_clip_t *target_clip;

    target_clip = _cairo_clip_copy_intersect_rectangle (clip, &rect);
    status = _cairo_surface_offset_fill (surface->target,
					 -surface->extents.x, -surface->extents.y,
					 op, source, path, fill_rule, tolerance, antialias,
					 target_clip);
    _cairo_clip_destroy (target_clip);
    return status;
}

static cairo_int_status_t
_cairo_surface_subsurface_stroke (void				*abstract_surface,
				  cairo_operator_t		 op,
				  const cairo_pattern_t		*source,
				  const cairo_path_fixed_t		*path,
				  const cairo_stroke_style_t	*stroke_style,
				  const cairo_matrix_t		*ctm,
				  const cairo_matrix_t		*ctm_inverse,
				  double			 tolerance,
				  cairo_antialias_t		 antialias,
				  const cairo_clip_t			*clip)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    cairo_rectangle_int_t rect = { 0, 0, surface->extents.width, surface->extents.height };
    cairo_status_t status;
    cairo_clip_t *target_clip;

    target_clip = _cairo_clip_copy_intersect_rectangle (clip, &rect);
    status = _cairo_surface_offset_stroke (surface->target,
					   -surface->extents.x, -surface->extents.y,
					   op, source, path, stroke_style, ctm, ctm_inverse,
					   tolerance, antialias,
					   target_clip);
    _cairo_clip_destroy (target_clip);
    return status;
}

static cairo_int_status_t
_cairo_surface_subsurface_glyphs (void			*abstract_surface,
				  cairo_operator_t	 op,
				  const cairo_pattern_t	*source,
				  cairo_glyph_t		*glyphs,
				  int			 num_glyphs,
				  cairo_scaled_font_t	*scaled_font,
				  const cairo_clip_t	*clip)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    cairo_rectangle_int_t rect = { 0, 0, surface->extents.width, surface->extents.height };
    cairo_status_t status;
    cairo_clip_t *target_clip;

    target_clip = _cairo_clip_copy_intersect_rectangle (clip, &rect);
    status = _cairo_surface_offset_glyphs (surface->target,
					   -surface->extents.x, -surface->extents.y,
					   op, source,
					   scaled_font, glyphs, num_glyphs,
					   target_clip);
    _cairo_clip_destroy (target_clip);
    return status;
}

static cairo_status_t
_cairo_surface_subsurface_flush (void *abstract_surface, unsigned flags)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    return _cairo_surface_flush (surface->target, flags);
}

static cairo_status_t
_cairo_surface_subsurface_mark_dirty (void *abstract_surface,
				      int x, int y,
				      int width, int height)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    cairo_status_t status;

    status = CAIRO_STATUS_SUCCESS;
    if (surface->target->backend->mark_dirty_rectangle != NULL) {
	cairo_rectangle_int_t rect, extents;

	rect.x = x;
	rect.y = y;
	rect.width  = width;
	rect.height = height;

	extents.x = extents.y = 0;
	extents.width  = surface->extents.width;
	extents.height = surface->extents.height;

	if (_cairo_rectangle_intersect (&rect, &extents)) {
	    status = surface->target->backend->mark_dirty_rectangle (surface->target,
								     rect.x + surface->extents.x,
								     rect.y + surface->extents.y,
								     rect.width, rect.height);
	}
    }

    return status;
}

static cairo_bool_t
_cairo_surface_subsurface_get_extents (void *abstract_surface,
				       cairo_rectangle_int_t *extents)
{
    cairo_surface_subsurface_t *surface = abstract_surface;

    extents->x = 0;
    extents->y = 0;
    extents->width  = surface->extents.width;
    extents->height = surface->extents.height;

    return TRUE;
}

static void
_cairo_surface_subsurface_get_font_options (void *abstract_surface,
					    cairo_font_options_t *options)
{
    cairo_surface_subsurface_t *surface = abstract_surface;

    if (surface->target->backend->get_font_options != NULL)
	surface->target->backend->get_font_options (surface->target, options);
}

static cairo_surface_t *
_cairo_surface_subsurface_source (void *abstract_surface,
				  cairo_rectangle_int_t *extents)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    cairo_surface_t *source;

    source = _cairo_surface_get_source (surface->target, extents);
    if (extents)
	*extents = surface->extents;

    return source;
}

static cairo_status_t
_cairo_surface_subsurface_acquire_source_image (void                    *abstract_surface,
						cairo_image_surface_t  **image_out,
						void                   **extra_out)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    cairo_surface_pattern_t pattern;
    cairo_surface_t *image;
    cairo_status_t status;

    image = _cairo_image_surface_create_with_content (surface->base.content,
						      surface->extents.width,
						      surface->extents.height);
    if (unlikely (image->status))
	return image->status;

    _cairo_pattern_init_for_surface (&pattern, surface->target);
    cairo_matrix_init_translate (&pattern.base.matrix,
				 surface->extents.x,
				 surface->extents.y);
    pattern.base.filter = CAIRO_FILTER_NEAREST;
    status = _cairo_surface_paint (image,
				   CAIRO_OPERATOR_SOURCE,
				   &pattern.base, NULL);
    _cairo_pattern_fini (&pattern.base);
    if (unlikely (status)) {
	cairo_surface_destroy (image);
	return status;
    }

    *image_out = (cairo_image_surface_t *)image;
    *extra_out = NULL;
    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_surface_subsurface_release_source_image (void                   *abstract_surface,
						cairo_image_surface_t  *image,
						void                   *abstract_extra)
{
    cairo_surface_destroy (&image->base);
}

static cairo_surface_t *
_cairo_surface_subsurface_snapshot (void *abstract_surface)
{
    cairo_surface_subsurface_t *surface = abstract_surface;
    cairo_surface_pattern_t pattern;
    cairo_surface_t *clone;
    cairo_status_t status;

    TRACE ((stderr, "%s: target=%d\n", __FUNCTION__, surface->target->unique_id));

    clone = _cairo_surface_create_scratch (surface->target,
					   surface->target->content,
					   surface->extents.width,
					   surface->extents.height,
					   NULL);
    if (unlikely (clone->status))
	return clone;

    _cairo_pattern_init_for_surface (&pattern, surface->target);
    cairo_matrix_init_translate (&pattern.base.matrix,
				 surface->extents.x, surface->extents.y);
    pattern.base.filter = CAIRO_FILTER_NEAREST;
    status = _cairo_surface_paint (clone,
				   CAIRO_OPERATOR_SOURCE,
				   &pattern.base, NULL);
    _cairo_pattern_fini (&pattern.base);

    if (unlikely (status)) {
	cairo_surface_destroy (clone);
	clone = _cairo_surface_create_in_error (status);
    }

    return clone;
}

static cairo_t *
_cairo_surface_subsurface_create_context(void *target)
{
    cairo_surface_subsurface_t *surface = target;
    return surface->target->backend->create_context (&surface->base);
}

static const cairo_surface_backend_t _cairo_surface_subsurface_backend = {
    CAIRO_SURFACE_TYPE_SUBSURFACE,
    _cairo_surface_subsurface_finish,

    _cairo_surface_subsurface_create_context,

    _cairo_surface_subsurface_create_similar,
    _cairo_surface_subsurface_create_similar_image,
    _cairo_surface_subsurface_map_to_image,
    _cairo_surface_subsurface_unmap_image,

    _cairo_surface_subsurface_source,
    _cairo_surface_subsurface_acquire_source_image,
    _cairo_surface_subsurface_release_source_image,
    _cairo_surface_subsurface_snapshot,

    NULL, /* copy_page */
    NULL, /* show_page */

    _cairo_surface_subsurface_get_extents,
    _cairo_surface_subsurface_get_font_options,

    _cairo_surface_subsurface_flush,
    _cairo_surface_subsurface_mark_dirty,

    _cairo_surface_subsurface_paint,
    _cairo_surface_subsurface_mask,
    _cairo_surface_subsurface_stroke,
    _cairo_surface_subsurface_fill,
    NULL, /* fill/stroke */
    _cairo_surface_subsurface_glyphs,
};

/**
 * cairo_surface_create_for_rectangle:
 * @target: an existing surface for which the sub-surface will point to
 * @x: the x-origin of the sub-surface from the top-left of the target surface (in device-space units)
 * @y: the y-origin of the sub-surface from the top-left of the target surface (in device-space units)
 * @width: width of the sub-surface (in device-space units)
 * @height: height of the sub-surface (in device-space units)
 *
 * Create a new surface that is a rectangle within the target surface.
 * All operations drawn to this surface are then clipped and translated
 * onto the target surface. Nothing drawn via this sub-surface outside of
 * its bounds is drawn onto the target surface, making this a useful method
 * for passing constrained child surfaces to library routines that draw
 * directly onto the parent surface, i.e. with no further backend allocations,
 * double buffering or copies.
 *
 * <note><para>The semantics of subsurfaces have not been finalized yet
 * unless the rectangle is in full device units, is contained within
 * the extents of the target surface, and the target or subsurface's
 * device transforms are not changed.</para></note>
 *
 * Return value: a pointer to the newly allocated surface. The caller
 * owns the surface and should call cairo_surface_destroy() when done
 * with it.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" surface if @other is already in an error state
 * or any other error occurs.
 *
 * Since: 1.10
 **/
cairo_surface_t *
cairo_surface_create_for_rectangle (cairo_surface_t *target,
				    double x, double y,
				    double width, double height)
{
    cairo_surface_subsurface_t *surface;

    if (unlikely (width < 0 || height < 0))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_INVALID_SIZE));

    if (unlikely (target->status))
	return _cairo_surface_create_in_error (target->status);
    if (unlikely (target->finished))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    surface = _cairo_malloc (sizeof (cairo_surface_subsurface_t));
    if (unlikely (surface == NULL))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    x *= target->device_transform.xx;
    y *= target->device_transform.yy;

    width *= target->device_transform.xx;
    height *= target->device_transform.yy;

    x += target->device_transform.x0;
    y += target->device_transform.y0;

    _cairo_surface_init (&surface->base,
			 &_cairo_surface_subsurface_backend,
			 NULL, /* device */
			 target->content,
			 target->is_vector);

    /* XXX forced integer alignment */
    surface->extents.x = ceil (x);
    surface->extents.y = ceil (y);
    surface->extents.width = floor (x + width) - surface->extents.x;
    surface->extents.height = floor (y + height) - surface->extents.y;
    if ((surface->extents.width | surface->extents.height) < 0)
	surface->extents.width = surface->extents.height = 0;

    if (target->backend->type == CAIRO_SURFACE_TYPE_SUBSURFACE) {
	/* Maintain subsurfaces as 1-depth */
	cairo_surface_subsurface_t *sub = (cairo_surface_subsurface_t *) target;
	surface->extents.x += sub->extents.x;
	surface->extents.y += sub->extents.y;
	target = sub->target;
    }

    surface->target = cairo_surface_reference (target);
    surface->base.type = surface->target->type;

    surface->snapshot = NULL;

    cairo_surface_set_device_scale (&surface->base,
                                    target->device_transform.xx,
                                    target->device_transform.yy);

    return &surface->base;
}

cairo_surface_t *
_cairo_surface_create_for_rectangle_int (cairo_surface_t *target,
					 const cairo_rectangle_int_t *extents)
{
    cairo_surface_subsurface_t *surface;

    if (unlikely (target->status))
	return _cairo_surface_create_in_error (target->status);
    if (unlikely (target->finished))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    assert (target->backend->type != CAIRO_SURFACE_TYPE_SUBSURFACE);

    surface = _cairo_malloc (sizeof (cairo_surface_subsurface_t));
    if (unlikely (surface == NULL))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    _cairo_surface_init (&surface->base,
			 &_cairo_surface_subsurface_backend,
			 NULL, /* device */
			 target->content,
			 target->is_vector);

    surface->extents = *extents;
    surface->extents.x *= target->device_transform.xx;
    surface->extents.y *= target->device_transform.yy;
    surface->extents.width *= target->device_transform.xx;
    surface->extents.height *= target->device_transform.yy;
    surface->extents.x += target->device_transform.x0;
    surface->extents.y += target->device_transform.y0;

    surface->target = cairo_surface_reference (target);
    surface->base.type = surface->target->type;

    surface->snapshot = NULL;

    cairo_surface_set_device_scale (&surface->base,
                                    target->device_transform.xx,
                                    target->device_transform.yy);

    return &surface->base;
}
/* XXX observe mark-dirty */

static void
_cairo_surface_subsurface_detach_snapshot (cairo_surface_t *surface)
{
    cairo_surface_subsurface_t *ss = (cairo_surface_subsurface_t *) surface;

    TRACE ((stderr, "%s: target=%d\n", __FUNCTION__, ss->target->unique_id));

    cairo_surface_destroy (ss->snapshot);
    ss->snapshot = NULL;
}

void
_cairo_surface_subsurface_set_snapshot (cairo_surface_t *surface,
					cairo_surface_t *snapshot)
{
    cairo_surface_subsurface_t *ss = (cairo_surface_subsurface_t *) surface;

    TRACE ((stderr, "%s: target=%d, snapshot=%d\n", __FUNCTION__,
	    ss->target->unique_id, snapshot->unique_id));

    /* FIXME: attaching the subsurface as a snapshot to its target creates
     * a reference cycle.  Let's make this call as a no-op until that bug
     * is fixed.
     */
    return;

    if (ss->snapshot)
	_cairo_surface_detach_snapshot (ss->snapshot);

    ss->snapshot = cairo_surface_reference (snapshot);

    _cairo_surface_attach_snapshot (ss->target, &ss->base,
				    _cairo_surface_subsurface_detach_snapshot);
}
