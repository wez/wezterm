/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/*
 * Copyright © 2006 Keith Packard
 * Copyright © 2007 Adrian Johnson
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
 *      Adrian Johnson <ajohnson@redneon.com>
 */

#include "cairoint.h"

#include "cairo-analysis-surface-private.h"
#include "cairo-box-inline.h"
#include "cairo-default-context-private.h"
#include "cairo-error-private.h"
#include "cairo-paginated-private.h"
#include "cairo-recording-surface-inline.h"
#include "cairo-surface-snapshot-inline.h"
#include "cairo-surface-subsurface-inline.h"
#include "cairo-region-private.h"

typedef struct {
    cairo_surface_t base;

    cairo_surface_t *target;

    cairo_bool_t first_op;
    cairo_bool_t has_supported;
    cairo_bool_t has_unsupported;

    cairo_region_t supported_region;
    cairo_region_t fallback_region;
    cairo_box_t page_bbox;

    cairo_bool_t create_region_ids;
    unsigned source_region_id;
    unsigned mask_region_id;

    cairo_bool_t has_ctm;
    cairo_matrix_t ctm;

} cairo_analysis_surface_t;

cairo_int_status_t
_cairo_analysis_surface_merge_status (cairo_int_status_t status_a,
				      cairo_int_status_t status_b)
{
    /* fatal errors should be checked and propagated at source */
    assert (! _cairo_int_status_is_error (status_a));
    assert (! _cairo_int_status_is_error (status_b));

    /* return the most important status */
    if (status_a == CAIRO_INT_STATUS_UNSUPPORTED ||
	status_b == CAIRO_INT_STATUS_UNSUPPORTED)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    if (status_a == CAIRO_INT_STATUS_IMAGE_FALLBACK ||
	status_b == CAIRO_INT_STATUS_IMAGE_FALLBACK)
	return CAIRO_INT_STATUS_IMAGE_FALLBACK;

    if (status_a == CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN ||
	status_b == CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN)
	return CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN;

    if (status_a == CAIRO_INT_STATUS_FLATTEN_TRANSPARENCY ||
	status_b == CAIRO_INT_STATUS_FLATTEN_TRANSPARENCY)
	return CAIRO_INT_STATUS_FLATTEN_TRANSPARENCY;

    /* at this point we have checked all the valid internal codes, so... */
    assert (status_a == CAIRO_INT_STATUS_SUCCESS &&
	    status_b == CAIRO_INT_STATUS_SUCCESS);

    return CAIRO_INT_STATUS_SUCCESS;
}

struct proxy {
    cairo_surface_t base;
    cairo_surface_t *target;
};

static cairo_status_t
proxy_finish (void *abstract_surface)
{
    return CAIRO_STATUS_SUCCESS;
}

static const cairo_surface_backend_t proxy_backend  = {
    CAIRO_INTERNAL_SURFACE_TYPE_NULL,
    proxy_finish,
};

static cairo_surface_t *
attach_proxy (cairo_surface_t *source,
	      cairo_surface_t *target)
{
    struct proxy *proxy;

    proxy = _cairo_malloc (sizeof (*proxy));
    if (unlikely (proxy == NULL))
	return _cairo_surface_create_in_error (CAIRO_STATUS_NO_MEMORY);

    _cairo_surface_init (&proxy->base, &proxy_backend, NULL, target->content, target->is_vector);

    proxy->target = target;
    _cairo_surface_attach_snapshot (source, &proxy->base, NULL);

    return &proxy->base;
}

static void
detach_proxy (cairo_surface_t *proxy)
{
    cairo_surface_finish (proxy);
    cairo_surface_destroy (proxy);
}

static cairo_int_status_t
_add_operation (cairo_analysis_surface_t *surface,
		cairo_rectangle_int_t    *rect,
		cairo_int_status_t        backend_status)
{
    cairo_int_status_t status;
    cairo_box_t bbox;

    if (rect->width == 0 || rect->height == 0) {
	/* Even though the operation is not visible we must be careful
	 * to not allow unsupported operations to be replayed to the
	 * backend during CAIRO_PAGINATED_MODE_RENDER */
	if (backend_status == CAIRO_INT_STATUS_SUCCESS ||
	    backend_status == CAIRO_INT_STATUS_FLATTEN_TRANSPARENCY ||
	    backend_status == CAIRO_INT_STATUS_NOTHING_TO_DO)
	{
	    return CAIRO_INT_STATUS_SUCCESS;
	}
	else
	{
	    return CAIRO_INT_STATUS_IMAGE_FALLBACK;
	}
    }

    _cairo_box_from_rectangle (&bbox, rect);

    if (surface->has_ctm) {
	int tx, ty;

	if (_cairo_matrix_is_integer_translation (&surface->ctm, &tx, &ty)) {
	    rect->x += tx;
	    rect->y += ty;

	    tx = _cairo_fixed_from_int (tx);
	    bbox.p1.x += tx;
	    bbox.p2.x += tx;

	    ty = _cairo_fixed_from_int (ty);
	    bbox.p1.y += ty;
	    bbox.p2.y += ty;
	} else {
	    _cairo_matrix_transform_bounding_box_fixed (&surface->ctm,
							&bbox, NULL);

	    if (bbox.p1.x == bbox.p2.x || bbox.p1.y == bbox.p2.y) {
		/* Even though the operation is not visible we must be
		 * careful to not allow unsupported operations to be
		 * replayed to the backend during
		 * CAIRO_PAGINATED_MODE_RENDER */
		if (backend_status == CAIRO_INT_STATUS_SUCCESS ||
		    backend_status == CAIRO_INT_STATUS_FLATTEN_TRANSPARENCY ||
		    backend_status == CAIRO_INT_STATUS_NOTHING_TO_DO)
		{
		    return CAIRO_INT_STATUS_SUCCESS;
		}
		else
		{
		    return CAIRO_INT_STATUS_IMAGE_FALLBACK;
		}
	    }

	    _cairo_box_round_to_rectangle (&bbox, rect);
	}
    }

    if (surface->first_op) {
	surface->first_op = FALSE;
	surface->page_bbox = bbox;
    } else
	_cairo_box_add_box(&surface->page_bbox, &bbox);

    /* If the operation is completely enclosed within the fallback
     * region there is no benefit in emitting a native operation as
     * the fallback image will be painted on top.
     */
    if (cairo_region_contains_rectangle (&surface->fallback_region, rect) == CAIRO_REGION_OVERLAP_IN)
	return CAIRO_INT_STATUS_IMAGE_FALLBACK;

    if (backend_status == CAIRO_INT_STATUS_FLATTEN_TRANSPARENCY) {
	/* A status of CAIRO_INT_STATUS_FLATTEN_TRANSPARENCY indicates
	 * that the backend only supports this operation if the
	 * transparency removed. If the extents of this operation does
	 * not intersect any other native operation, the operation is
	 * natively supported and the backend will blend the
	 * transparency into the white background.
	 */
	if (cairo_region_contains_rectangle (&surface->supported_region, rect) == CAIRO_REGION_OVERLAP_OUT)
	    backend_status = CAIRO_INT_STATUS_SUCCESS;
    }

    if (backend_status == CAIRO_INT_STATUS_SUCCESS) {
	/* Add the operation to the supported region. Operations in
	 * this region will be emitted as native operations.
	 */
	surface->has_supported = TRUE;
	return cairo_region_union_rectangle (&surface->supported_region, rect);
    }

    /* Add the operation to the unsupported region. This region will
     * be painted as an image after all native operations have been
     * emitted.
     */
    surface->has_unsupported = TRUE;
    status = cairo_region_union_rectangle (&surface->fallback_region, rect);

    /* The status CAIRO_INT_STATUS_IMAGE_FALLBACK is used to indicate
     * unsupported operations to the recording surface as using
     * CAIRO_INT_STATUS_UNSUPPORTED would cause cairo-surface to
     * invoke the cairo-surface-fallback path then return
     * CAIRO_STATUS_SUCCESS.
     */
    if (status == CAIRO_INT_STATUS_SUCCESS)
	return CAIRO_INT_STATUS_IMAGE_FALLBACK;
    else
	return status;
}

static cairo_int_status_t
_analyze_recording_surface_pattern (cairo_analysis_surface_t *surface,
				    const cairo_pattern_t    *pattern,
				    cairo_rectangle_int_t    *extents,
				    unsigned int             *regions_id)
{
    const cairo_surface_pattern_t *surface_pattern;
    cairo_analysis_surface_t *tmp;
    cairo_surface_t *source, *proxy;
    cairo_matrix_t p2d;
    cairo_int_status_t status;
    cairo_int_status_t analysis_status = CAIRO_INT_STATUS_SUCCESS;
    cairo_bool_t surface_is_unbounded;
    cairo_bool_t unused;

    assert (pattern->type == CAIRO_PATTERN_TYPE_SURFACE);
    surface_pattern = (const cairo_surface_pattern_t *) pattern;
    assert (surface_pattern->surface->type == CAIRO_SURFACE_TYPE_RECORDING);
    source = surface_pattern->surface;

    proxy = _cairo_surface_has_snapshot (source, &proxy_backend);
    if (proxy != NULL) {
	/* nothing untoward found so far */
	return CAIRO_STATUS_SUCCESS;
    }

    tmp = (cairo_analysis_surface_t *)
	_cairo_analysis_surface_create (surface->target, surface->create_region_ids);
    if (unlikely (tmp->base.status)) {
	status =tmp->base.status;
	goto cleanup1;
    }
    proxy = attach_proxy (source, &tmp->base);

    p2d = pattern->matrix;
    status = cairo_matrix_invert (&p2d);
    assert (status == CAIRO_INT_STATUS_SUCCESS);
    _cairo_analysis_surface_set_ctm (&tmp->base, &p2d);


    source = _cairo_surface_get_source (source, NULL);
    surface_is_unbounded = (pattern->extend == CAIRO_EXTEND_REPEAT
			    || pattern->extend == CAIRO_EXTEND_REFLECT);

    if (surface->create_region_ids) {
	status = _cairo_recording_surface_region_array_attach (source, regions_id);
	if (unlikely (status))
	    goto cleanup2;

	status = _cairo_recording_surface_replay_and_create_regions (source,
								     *regions_id,
								     &pattern->matrix,
								     &tmp->base,
								     surface_is_unbounded);
	if (unlikely (status))
	    goto cleanup2;
    } else {
	status = _cairo_recording_surface_replay_with_clip (source,
							    &pattern->matrix,
							    &tmp->base,
							    NULL, /* target clip */
							    surface_is_unbounded);
	if (unlikely (status))
	    goto cleanup2;
    }

    /* black background or mime data fills entire extents */
    if (!(source->content & CAIRO_CONTENT_ALPHA) || _cairo_surface_has_mime_image (source)) {
	cairo_rectangle_int_t rect;

	if (_cairo_surface_get_extents (source, &rect)) {
	    cairo_box_t bbox;

	    _cairo_box_from_rectangle (&bbox, &rect);
	    _cairo_matrix_transform_bounding_box_fixed (&p2d, &bbox, NULL);
	    _cairo_box_round_to_rectangle (&bbox, &rect);
	    status = _add_operation (tmp, &rect, CAIRO_INT_STATUS_SUCCESS);
	    if (status == CAIRO_INT_STATUS_IMAGE_FALLBACK)
		status = CAIRO_INT_STATUS_SUCCESS;
	    if (unlikely (status))
		goto cleanup2;
	}
    }

    if (tmp->has_supported) {
	surface->has_supported = TRUE;
	unused = cairo_region_union (&surface->supported_region, &tmp->supported_region);
    }

    if (tmp->has_unsupported) {
	surface->has_unsupported = TRUE;
	unused = cairo_region_union (&surface->fallback_region, &tmp->fallback_region);
    }

    analysis_status = tmp->has_unsupported ? CAIRO_INT_STATUS_IMAGE_FALLBACK : CAIRO_INT_STATUS_SUCCESS;
    if (pattern->extend != CAIRO_EXTEND_NONE) {
	_cairo_unbounded_rectangle_init (extents);
    } else {
	status = cairo_matrix_invert (&tmp->ctm);
	_cairo_matrix_transform_bounding_box_fixed (&tmp->ctm,
						    &tmp->page_bbox, NULL);
	_cairo_box_round_to_rectangle (&tmp->page_bbox, extents);
    }

  cleanup2:
    detach_proxy (proxy);
  cleanup1:
    cairo_surface_destroy (&tmp->base);

    if (unlikely (status))
	return status;
    else
	return analysis_status;
}

static cairo_status_t
_cairo_analysis_surface_finish (void *abstract_surface)
{
    cairo_analysis_surface_t	*surface = (cairo_analysis_surface_t *) abstract_surface;

    _cairo_region_fini (&surface->supported_region);
    _cairo_region_fini (&surface->fallback_region);

    cairo_surface_destroy (surface->target);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
_cairo_analysis_surface_get_extents (void			*abstract_surface,
				     cairo_rectangle_int_t	*rectangle)
{
    cairo_analysis_surface_t *surface = abstract_surface;

    return _cairo_surface_get_extents (surface->target, rectangle);
}

static void
_rectangle_intersect_clip (cairo_rectangle_int_t *extents, const cairo_clip_t *clip)
{
    if (clip != NULL)
	_cairo_rectangle_intersect (extents, _cairo_clip_get_extents (clip));
}

static void
_cairo_analysis_surface_operation_extents (cairo_analysis_surface_t *surface,
					   cairo_operator_t op,
					   const cairo_pattern_t *source,
					   const cairo_clip_t *clip,
					   cairo_rectangle_int_t *extents)
{
    cairo_bool_t is_empty;

    is_empty = _cairo_surface_get_extents (&surface->base, extents);

    if (_cairo_operator_bounded_by_source (op)) {
	cairo_rectangle_int_t source_extents;

	_cairo_pattern_get_extents (source, &source_extents, surface->target->is_vector);
	_cairo_rectangle_intersect (extents, &source_extents);
    }

    _rectangle_intersect_clip (extents, clip);
}

static cairo_int_status_t
_cairo_analysis_surface_paint (void			*abstract_surface,
			       cairo_operator_t		op,
			       const cairo_pattern_t	*source,
			       const cairo_clip_t	*clip)
{
    cairo_analysis_surface_t *surface = abstract_surface;
    cairo_int_status_t	     backend_status;
    cairo_rectangle_int_t  extents;

    surface->source_region_id = 0;
    surface->mask_region_id = 0;
    if (surface->target->backend->paint == NULL) {
	backend_status = CAIRO_INT_STATUS_UNSUPPORTED;
    } else {
	backend_status =
	    surface->target->backend->paint (surface->target,
					     op, source, clip);
	if (_cairo_int_status_is_error (backend_status))
	    return backend_status;
    }

    _cairo_analysis_surface_operation_extents (surface,
					       op, source, clip,
					       &extents);
    if (backend_status == CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN) {
	cairo_rectangle_int_t rec_extents;
	backend_status = _analyze_recording_surface_pattern (surface,
							     source,
							     &rec_extents,
							     &surface->source_region_id);
	_cairo_rectangle_intersect (&extents, &rec_extents);
    }

    return _add_operation (surface, &extents, backend_status);
}

static cairo_int_status_t
_cairo_analysis_surface_mask (void			*abstract_surface,
			      cairo_operator_t		 op,
			      const cairo_pattern_t	*source,
			      const cairo_pattern_t	*mask,
			      const cairo_clip_t	*clip)
{
    cairo_analysis_surface_t *surface = abstract_surface;
    cairo_int_status_t	      backend_status;
    cairo_rectangle_int_t   extents;

    surface->source_region_id = 0;
    surface->mask_region_id = 0;
    if (surface->target->backend->mask == NULL) {
	backend_status = CAIRO_INT_STATUS_UNSUPPORTED;
    } else {
	backend_status =
	    surface->target->backend->mask (surface->target,
					    op, source, mask, clip);
	if (_cairo_int_status_is_error (backend_status))
	    return backend_status;
    }

    _cairo_analysis_surface_operation_extents (surface,
					       op, source, clip,
					       &extents);
    if (backend_status == CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN) {
	cairo_int_status_t backend_source_status = CAIRO_STATUS_SUCCESS;
	cairo_int_status_t backend_mask_status = CAIRO_STATUS_SUCCESS;
	cairo_rectangle_int_t rec_extents;

	if (source->type == CAIRO_PATTERN_TYPE_SURFACE) {
	    cairo_surface_t *src_surface = ((cairo_surface_pattern_t *)source)->surface;
	    src_surface = _cairo_surface_get_source (src_surface, NULL);
	    if (_cairo_surface_is_recording (src_surface)) {
		backend_source_status =
		    _analyze_recording_surface_pattern (surface,
							source,
							&rec_extents,
							&surface->source_region_id);
		if (_cairo_int_status_is_error (backend_source_status))
		    return backend_source_status;

		_cairo_rectangle_intersect (&extents, &rec_extents);
	    }
	}

	if (mask->type == CAIRO_PATTERN_TYPE_SURFACE) {
	    cairo_surface_t *mask_surface = ((cairo_surface_pattern_t *)mask)->surface;
	    mask_surface = _cairo_surface_get_source (mask_surface, NULL);
	    if (_cairo_surface_is_recording (mask_surface)) {
		backend_mask_status =
		    _analyze_recording_surface_pattern (surface,
							mask,
							&rec_extents,
							&surface->mask_region_id);
		if (_cairo_int_status_is_error (backend_mask_status))
		    return backend_mask_status;

		_cairo_rectangle_intersect (&extents, &rec_extents);
	    }
	}

	backend_status =
	    _cairo_analysis_surface_merge_status (backend_source_status,
						  backend_mask_status);
    }

    if (_cairo_operator_bounded_by_mask (op)) {
	cairo_rectangle_int_t mask_extents;

	_cairo_pattern_get_extents (mask, &mask_extents, surface->target->is_vector);
	_cairo_rectangle_intersect (&extents, &mask_extents);
    }

    return _add_operation (surface, &extents, backend_status);
}

static cairo_int_status_t
_cairo_analysis_surface_stroke (void			   *abstract_surface,
				cairo_operator_t	    op,
				const cairo_pattern_t	   *source,
				const cairo_path_fixed_t   *path,
				const cairo_stroke_style_t *style,
				const cairo_matrix_t	   *ctm,
				const cairo_matrix_t	   *ctm_inverse,
				double			    tolerance,
				cairo_antialias_t	    antialias,
				const cairo_clip_t	   *clip)
{
    cairo_analysis_surface_t *surface = abstract_surface;
    cairo_int_status_t	     backend_status;
    cairo_rectangle_int_t    extents;

    surface->source_region_id = 0;
    surface->mask_region_id = 0;
    if (surface->target->backend->stroke == NULL) {
	backend_status = CAIRO_INT_STATUS_UNSUPPORTED;
    } else {
	backend_status =
	    surface->target->backend->stroke (surface->target, op,
					      source, path, style,
					      ctm, ctm_inverse,
					      tolerance, antialias,
					      clip);
	if (_cairo_int_status_is_error (backend_status))
	    return backend_status;
    }

    _cairo_analysis_surface_operation_extents (surface,
					       op, source, clip,
					       &extents);
    if (backend_status == CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN) {
	cairo_rectangle_int_t rec_extents;
	backend_status = _analyze_recording_surface_pattern (surface,
							     source,
							     &rec_extents,
							     &surface->source_region_id);
	_cairo_rectangle_intersect (&extents, &rec_extents);
    }

    if (_cairo_operator_bounded_by_mask (op)) {
	cairo_rectangle_int_t mask_extents;
	cairo_int_status_t status;

	status = _cairo_path_fixed_stroke_extents (path, style,
						   ctm, ctm_inverse,
						   tolerance,
						   &mask_extents);
	if (unlikely (status))
	    return status;

	_cairo_rectangle_intersect (&extents, &mask_extents);
    }

    return _add_operation (surface, &extents, backend_status);
}

static cairo_int_status_t
_cairo_analysis_surface_fill (void			*abstract_surface,
			      cairo_operator_t		 op,
			      const cairo_pattern_t	*source,
			      const cairo_path_fixed_t	*path,
			      cairo_fill_rule_t		 fill_rule,
			      double			 tolerance,
			      cairo_antialias_t		 antialias,
			      const cairo_clip_t	*clip)
{
    cairo_analysis_surface_t *surface = abstract_surface;
    cairo_int_status_t	     backend_status;
    cairo_rectangle_int_t    extents;

    if (surface->target->backend->fill == NULL) {
	backend_status = CAIRO_INT_STATUS_UNSUPPORTED;
    } else {
	backend_status =
	    surface->target->backend->fill (surface->target, op,
					    source, path, fill_rule,
					    tolerance, antialias,
					    clip);
	if (_cairo_int_status_is_error (backend_status))
	    return backend_status;
    }

    _cairo_analysis_surface_operation_extents (surface,
					       op, source, clip,
					       &extents);
    if (backend_status == CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN) {
	cairo_rectangle_int_t rec_extents;
	backend_status = _analyze_recording_surface_pattern (surface,
							     source,
							     &rec_extents,
							     &surface->source_region_id);
	_cairo_rectangle_intersect (&extents, &rec_extents);
    }

    if (_cairo_operator_bounded_by_mask (op)) {
	cairo_rectangle_int_t mask_extents;

	_cairo_path_fixed_fill_extents (path, fill_rule, tolerance,
					&mask_extents);

	_cairo_rectangle_intersect (&extents, &mask_extents);
    }

    return _add_operation (surface, &extents, backend_status);
}

static cairo_int_status_t
_cairo_analysis_surface_show_glyphs (void		  *abstract_surface,
				     cairo_operator_t	   op,
				     const cairo_pattern_t *source,
				     cairo_glyph_t	  *glyphs,
				     int		   num_glyphs,
				     cairo_scaled_font_t  *scaled_font,
				     const cairo_clip_t         *clip)
{
    cairo_analysis_surface_t *surface = abstract_surface;
    cairo_int_status_t	     status, backend_status;
    cairo_rectangle_int_t    extents, glyph_extents;

    surface->source_region_id = 0;
    surface->mask_region_id = 0;

    /* Adapted from _cairo_surface_show_glyphs */
    if (surface->target->backend->show_glyphs != NULL) {
	backend_status =
	    surface->target->backend->show_glyphs (surface->target, op,
						   source,
						   glyphs, num_glyphs,
						   scaled_font,
						   clip);
	if (_cairo_int_status_is_error (backend_status))
	    return backend_status;
    }
    else if (surface->target->backend->show_text_glyphs != NULL)
    {
	backend_status =
	    surface->target->backend->show_text_glyphs (surface->target, op,
							source,
							NULL, 0,
							glyphs, num_glyphs,
							NULL, 0,
							FALSE,
							scaled_font,
							clip);
	if (_cairo_int_status_is_error (backend_status))
	    return backend_status;
    }
    else
    {
	backend_status = CAIRO_INT_STATUS_UNSUPPORTED;
    }

    _cairo_analysis_surface_operation_extents (surface,
					       op, source, clip,
					       &extents);
    if (backend_status == CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN) {
	cairo_rectangle_int_t rec_extents;
	backend_status = _analyze_recording_surface_pattern (surface,
							     source,
							     &rec_extents,
							     &surface->source_region_id);
	_cairo_rectangle_intersect (&extents, &rec_extents);
    }

    if (_cairo_operator_bounded_by_mask (op)) {
	status = _cairo_scaled_font_glyph_device_extents (scaled_font,
							  glyphs,
							  num_glyphs,
							  &glyph_extents,
							  NULL);
	if (unlikely (status))
	    return status;

	_cairo_rectangle_intersect (&extents, &glyph_extents);
    }

    return _add_operation (surface, &extents, backend_status);
}

static cairo_bool_t
_cairo_analysis_surface_has_show_text_glyphs (void *abstract_surface)
{
    cairo_analysis_surface_t *surface = abstract_surface;

    return cairo_surface_has_show_text_glyphs (surface->target);
}

static cairo_int_status_t
_cairo_analysis_surface_show_text_glyphs (void			    *abstract_surface,
					  cairo_operator_t	     op,
					  const cairo_pattern_t	    *source,
					  const char		    *utf8,
					  int			     utf8_len,
					  cairo_glyph_t		    *glyphs,
					  int			     num_glyphs,
					  const cairo_text_cluster_t *clusters,
					  int			     num_clusters,
					  cairo_text_cluster_flags_t cluster_flags,
					  cairo_scaled_font_t	    *scaled_font,
					  const cairo_clip_t		    *clip)
{
    cairo_analysis_surface_t *surface = abstract_surface;
    cairo_int_status_t	     status, backend_status;
    cairo_rectangle_int_t    extents, glyph_extents;

    surface->source_region_id = 0;
    surface->mask_region_id = 0;

    /* Adapted from _cairo_surface_show_glyphs */
    backend_status = CAIRO_INT_STATUS_UNSUPPORTED;
    if (surface->target->backend->show_text_glyphs != NULL) {
	backend_status =
	    surface->target->backend->show_text_glyphs (surface->target, op,
							source,
							utf8, utf8_len,
							glyphs, num_glyphs,
							clusters, num_clusters,
							cluster_flags,
							scaled_font,
							clip);
	if (_cairo_int_status_is_error (backend_status))
	    return backend_status;
    }
    if (backend_status == CAIRO_INT_STATUS_UNSUPPORTED &&
	surface->target->backend->show_glyphs != NULL)
    {
	backend_status =
	    surface->target->backend->show_glyphs (surface->target, op,
						   source,
						   glyphs, num_glyphs,
						   scaled_font,
						   clip);
	if (_cairo_int_status_is_error (backend_status))
	    return backend_status;
    }

    _cairo_analysis_surface_operation_extents (surface,
					       op, source, clip,
					       &extents);
    if (backend_status == CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN) {
	cairo_rectangle_int_t rec_extents;
	_analyze_recording_surface_pattern (surface,
					    source,
					    &rec_extents,
					    &surface->source_region_id);
	_cairo_rectangle_intersect (&extents, &rec_extents);
    }

    if (_cairo_operator_bounded_by_mask (op)) {
	status = _cairo_scaled_font_glyph_device_extents (scaled_font,
							  glyphs,
							  num_glyphs,
							  &glyph_extents,
							  NULL);
	if (unlikely (status))
	    return status;

	_cairo_rectangle_intersect (&extents, &glyph_extents);
    }

    return _add_operation (surface, &extents, backend_status);
}

static cairo_int_status_t
_cairo_analysis_surface_tag (void	                *abstract_surface,
			     cairo_bool_t                begin,
			     const char                 *tag_name,
			     const char                 *attributes)
{
    cairo_analysis_surface_t *surface = abstract_surface;
    cairo_int_status_t	     backend_status;

    surface->source_region_id = 0;
    surface->mask_region_id = 0;
    backend_status = CAIRO_INT_STATUS_SUCCESS;
    if (surface->target->backend->tag != NULL) {
	backend_status =
	    surface->target->backend->tag (surface->target,
					   begin,
					   tag_name,
					   attributes);
        if (backend_status == CAIRO_INT_STATUS_SUCCESS)
            surface->has_supported = TRUE;
    }

    return backend_status;
}

static const cairo_surface_backend_t cairo_analysis_surface_backend = {
    CAIRO_INTERNAL_SURFACE_TYPE_ANALYSIS,

    _cairo_analysis_surface_finish,
    NULL,

    NULL, /* create_similar */
    NULL, /* create_similar_image */
    NULL, /* map_to_image */
    NULL, /* unmap */

    NULL, /* source */
    NULL, /* acquire_source_image */
    NULL, /* release_source_image */
    NULL, /* snapshot */

    NULL, /* copy_page */
    NULL, /* show_page */

    _cairo_analysis_surface_get_extents,
    NULL, /* get_font_options */

    NULL, /* flush */
    NULL, /* mark_dirty_rectangle */

    _cairo_analysis_surface_paint,
    _cairo_analysis_surface_mask,
    _cairo_analysis_surface_stroke,
    _cairo_analysis_surface_fill,
    NULL, /* fill_stroke */
    _cairo_analysis_surface_show_glyphs,
    _cairo_analysis_surface_has_show_text_glyphs,
    _cairo_analysis_surface_show_text_glyphs,
    NULL, /* get_supported_mime_types */
    _cairo_analysis_surface_tag
};

cairo_surface_t *
_cairo_analysis_surface_create (cairo_surface_t		*target,
				cairo_bool_t             create_region_ids)
{
    cairo_analysis_surface_t *surface;
    cairo_status_t status;

    status = target->status;
    if (unlikely (status))
	return _cairo_surface_create_in_error (status);

    surface = _cairo_malloc (sizeof (cairo_analysis_surface_t));
    if (unlikely (surface == NULL))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    /* I believe the content type here is truly arbitrary. I'm quite
     * sure nothing will ever use this value. */
    _cairo_surface_init (&surface->base,
			 &cairo_analysis_surface_backend,
			 NULL, /* device */
			 CAIRO_CONTENT_COLOR_ALPHA,
			 target->is_vector);

    cairo_matrix_init_identity (&surface->ctm);
    surface->has_ctm = FALSE;

    surface->target = cairo_surface_reference (target);
    surface->first_op  = TRUE;
    surface->has_supported = FALSE;
    surface->has_unsupported = FALSE;

    surface->create_region_ids = create_region_ids;
    surface->source_region_id = 0;
    surface->mask_region_id = 0;

    _cairo_region_init (&surface->supported_region);
    _cairo_region_init (&surface->fallback_region);

    surface->page_bbox.p1.x = 0;
    surface->page_bbox.p1.y = 0;
    surface->page_bbox.p2.x = 0;
    surface->page_bbox.p2.y = 0;

    return &surface->base;
}

void
_cairo_analysis_surface_set_ctm (cairo_surface_t *abstract_surface,
				 const cairo_matrix_t  *ctm)
{
    cairo_analysis_surface_t	*surface;

    if (abstract_surface->status)
	return;

    surface = (cairo_analysis_surface_t *) abstract_surface;

    surface->ctm = *ctm;
    surface->has_ctm = ! _cairo_matrix_is_identity (&surface->ctm);
}

void
_cairo_analysis_surface_get_ctm (cairo_surface_t *abstract_surface,
				 cairo_matrix_t  *ctm)
{
    cairo_analysis_surface_t	*surface = (cairo_analysis_surface_t *) abstract_surface;

    *ctm = surface->ctm;
}


cairo_region_t *
_cairo_analysis_surface_get_supported (cairo_surface_t *abstract_surface)
{
    cairo_analysis_surface_t	*surface = (cairo_analysis_surface_t *) abstract_surface;

    return &surface->supported_region;
}

cairo_region_t *
_cairo_analysis_surface_get_unsupported (cairo_surface_t *abstract_surface)
{
    cairo_analysis_surface_t	*surface = (cairo_analysis_surface_t *) abstract_surface;

    return &surface->fallback_region;
}

cairo_bool_t
_cairo_analysis_surface_has_supported (cairo_surface_t *abstract_surface)
{
    cairo_analysis_surface_t	*surface = (cairo_analysis_surface_t *) abstract_surface;

    return surface->has_supported;
}

cairo_bool_t
_cairo_analysis_surface_has_unsupported (cairo_surface_t *abstract_surface)
{
    cairo_analysis_surface_t	*surface = (cairo_analysis_surface_t *) abstract_surface;

    return surface->has_unsupported;
}

void
_cairo_analysis_surface_get_bounding_box (cairo_surface_t *abstract_surface,
					  cairo_box_t     *bbox)
{
    cairo_analysis_surface_t	*surface = (cairo_analysis_surface_t *) abstract_surface;

    *bbox = surface->page_bbox;
}

unsigned int
_cairo_analysis_surface_get_source_region_id (cairo_surface_t *abstract_surface)
{
    cairo_analysis_surface_t	*surface = (cairo_analysis_surface_t *) abstract_surface;

    return surface->source_region_id;
}

unsigned int
_cairo_analysis_surface_get_mask_region_id (cairo_surface_t *abstract_surface)
{
    cairo_analysis_surface_t	*surface = (cairo_analysis_surface_t *) abstract_surface;

    return surface->mask_region_id;
}


/* null surface type: a surface that does nothing (has no side effects, yay!) */

static cairo_int_status_t
_paint_return_success (void			*surface,
		       cairo_operator_t		 op,
		       const cairo_pattern_t	*source,
		       const cairo_clip_t	*clip)
{
    if (source->type == CAIRO_PATTERN_TYPE_SURFACE) {
        cairo_surface_pattern_t *surface_pattern = (cairo_surface_pattern_t *) source;
        if (surface_pattern->surface->type == CAIRO_SURFACE_TYPE_RECORDING)
            return CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN;
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_mask_return_success (void			*surface,
		      cairo_operator_t		 op,
		      const cairo_pattern_t	*source,
		      const cairo_pattern_t	*mask,
		      const cairo_clip_t	*clip)
{
    if (source->type == CAIRO_PATTERN_TYPE_SURFACE) {
        cairo_surface_pattern_t *surface_pattern = (cairo_surface_pattern_t *) source;
        if (surface_pattern->surface->type == CAIRO_SURFACE_TYPE_RECORDING)
            return CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN;
    }

    if (mask->type == CAIRO_PATTERN_TYPE_SURFACE) {
        cairo_surface_pattern_t *surface_pattern = (cairo_surface_pattern_t *) mask;
        if (surface_pattern->surface->type == CAIRO_SURFACE_TYPE_RECORDING)
            return CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN;
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_stroke_return_success (void				*surface,
			cairo_operator_t		 op,
			const cairo_pattern_t		*source,
			const cairo_path_fixed_t	*path,
			const cairo_stroke_style_t	*style,
			const cairo_matrix_t		*ctm,
			const cairo_matrix_t		*ctm_inverse,
			double				 tolerance,
			cairo_antialias_t		 antialias,
			const cairo_clip_t		*clip)
{
    if (source->type == CAIRO_PATTERN_TYPE_SURFACE) {
        cairo_surface_pattern_t *surface_pattern = (cairo_surface_pattern_t *) source;
        if (surface_pattern->surface->type == CAIRO_SURFACE_TYPE_RECORDING)
            return CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN;
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_fill_return_success (void			*surface,
		      cairo_operator_t		 op,
		      const cairo_pattern_t	*source,
		      const cairo_path_fixed_t	*path,
		      cairo_fill_rule_t		 fill_rule,
		      double			 tolerance,
		      cairo_antialias_t		 antialias,
		      const cairo_clip_t	*clip)
{
    if (source->type == CAIRO_PATTERN_TYPE_SURFACE) {
        cairo_surface_pattern_t *surface_pattern = (cairo_surface_pattern_t *) source;
        if (surface_pattern->surface->type == CAIRO_SURFACE_TYPE_RECORDING)
            return CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN;
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_show_glyphs_return_success (void			*surface,
			     cairo_operator_t		 op,
			     const cairo_pattern_t	*source,
			     cairo_glyph_t		*glyphs,
			     int			 num_glyphs,
			     cairo_scaled_font_t	*scaled_font,
			     const cairo_clip_t		*clip)
{
    if (source->type == CAIRO_PATTERN_TYPE_SURFACE) {
        cairo_surface_pattern_t *surface_pattern = (cairo_surface_pattern_t *) source;
        if (surface_pattern->surface->type == CAIRO_SURFACE_TYPE_RECORDING)
            return CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN;
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

static const cairo_surface_backend_t cairo_null_surface_backend = {
    CAIRO_INTERNAL_SURFACE_TYPE_NULL,
    NULL, /* finish */

    NULL, /* only accessed through the surface functions */

    NULL, /* create_similar */
    NULL, /* create similar image */
    NULL, /* map to image */
    NULL, /* unmap image*/

    NULL, /* source */
    NULL, /* acquire_source_image */
    NULL, /* release_source_image */
    NULL, /* snapshot */

    NULL, /* copy_page */
    NULL, /* show_page */

    NULL, /* get_extents */
    NULL, /* get_font_options */

    NULL, /* flush */
    NULL, /* mark_dirty_rectangle */

    _paint_return_success,	    /* paint */
    _mask_return_success,	    /* mask */
    _stroke_return_success,	    /* stroke */
    _fill_return_success,	    /* fill */
    NULL, /* fill_stroke */
    _show_glyphs_return_success,    /* show_glyphs */
    NULL, /* has_show_text_glyphs */
    NULL  /* show_text_glyphs */
};

cairo_surface_t *
_cairo_null_surface_create (cairo_content_t content)
{
    cairo_surface_t *surface;

    surface = _cairo_malloc (sizeof (cairo_surface_t));
    if (unlikely (surface == NULL)) {
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));
    }

    _cairo_surface_init (surface,
			 &cairo_null_surface_backend,
			 NULL, /* device */
			 content,
			 TRUE); /* is_vector */

    return surface;
}
