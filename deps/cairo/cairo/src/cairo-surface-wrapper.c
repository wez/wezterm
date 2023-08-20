/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2005 Red Hat, Inc
 * Copyright © 2007 Adrian Johnson
 * Copyright © 2009 Chris Wilson
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
 *      Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-clip-inline.h"
#include "cairo-error-private.h"
#include "cairo-pattern-private.h"
#include "cairo-surface-wrapper-private.h"

/* A collection of routines to facilitate surface wrapping */

static void
_copy_transformed_pattern (cairo_pattern_t *pattern,
			   const cairo_pattern_t *original,
			   const cairo_matrix_t  *ctm_inverse,
			   unsigned int region_id)
{
    _cairo_pattern_init_static_copy (pattern, original);

    if (! _cairo_matrix_is_identity (ctm_inverse))
	_cairo_pattern_transform (pattern, ctm_inverse);

    if (pattern->type == CAIRO_PATTERN_TYPE_SURFACE) {
	cairo_surface_pattern_t *surface_pattern = (cairo_surface_pattern_t *) pattern;
	surface_pattern->region_array_id = region_id;
    }
}

cairo_status_t
_cairo_surface_wrapper_acquire_source_image (cairo_surface_wrapper_t *wrapper,
					     cairo_image_surface_t  **image_out,
					     void                   **image_extra)
{
    if (unlikely (wrapper->target->status))
	return wrapper->target->status;

    return _cairo_surface_acquire_source_image (wrapper->target,
						image_out, image_extra);
}

void
_cairo_surface_wrapper_release_source_image (cairo_surface_wrapper_t *wrapper,
					     cairo_image_surface_t  *image,
					     void                   *image_extra)
{
    _cairo_surface_release_source_image (wrapper->target, image, image_extra);
}

static void
_cairo_surface_wrapper_get_transform (cairo_surface_wrapper_t *wrapper,
				      cairo_matrix_t *m)
{
    cairo_matrix_init_identity (m);

    if (! _cairo_matrix_is_identity (&wrapper->transform))
	cairo_matrix_multiply (m, &wrapper->transform, m);

    if (! _cairo_matrix_is_identity (&wrapper->target->device_transform))
	cairo_matrix_multiply (m, &wrapper->target->device_transform, m);
}

static void
_cairo_surface_wrapper_get_inverse_transform (cairo_surface_wrapper_t *wrapper,
					      cairo_matrix_t *m)
{
    cairo_matrix_init_identity (m);

    if (! _cairo_matrix_is_identity (&wrapper->target->device_transform_inverse))
	cairo_matrix_multiply (m, &wrapper->target->device_transform_inverse, m);

    if (! _cairo_matrix_is_identity (&wrapper->transform)) {
	cairo_matrix_t inv;
	cairo_status_t status;

	inv = wrapper->transform;
	status = cairo_matrix_invert (&inv);
	assert (status == CAIRO_STATUS_SUCCESS);
	cairo_matrix_multiply (m, &inv, m);
    }
}

static cairo_clip_t *
_cairo_surface_wrapper_get_clip (cairo_surface_wrapper_t *wrapper,
				 const cairo_clip_t *clip)
{
    cairo_clip_t *copy;
    cairo_matrix_t m;

    copy = _cairo_clip_copy (clip);
    if (wrapper->has_extents) {
	copy = _cairo_clip_intersect_rectangle (copy, &wrapper->extents);
    }
    _cairo_surface_wrapper_get_transform (wrapper, &m);
    copy = _cairo_clip_transform (copy, &m);
    if (wrapper->clip)
	copy = _cairo_clip_intersect_clip (copy, wrapper->clip);

    return copy;
}

cairo_status_t
_cairo_surface_wrapper_paint (cairo_surface_wrapper_t *wrapper,
			      cairo_operator_t	       op,
			      const cairo_pattern_t   *source,
			      unsigned int             source_region_id,
			      const cairo_clip_t      *clip)
{
    cairo_status_t status;
    cairo_clip_t *dev_clip;
    cairo_pattern_union_t source_copy;

    if (unlikely (wrapper->target->status))
	return wrapper->target->status;

    dev_clip = _cairo_surface_wrapper_get_clip (wrapper, clip);
    if (_cairo_clip_is_all_clipped (dev_clip))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (wrapper->needs_transform || source_region_id != 0) {
	cairo_matrix_t m;

	_cairo_surface_wrapper_get_transform (wrapper, &m);

	status = cairo_matrix_invert (&m);
	assert (status == CAIRO_STATUS_SUCCESS);

	_copy_transformed_pattern (&source_copy.base, source, &m, source_region_id);
	source = &source_copy.base;
    }

    status = _cairo_surface_paint (wrapper->target, op, source, dev_clip);

    _cairo_clip_destroy (dev_clip);
    return status;
}

cairo_status_t
_cairo_surface_wrapper_mask (cairo_surface_wrapper_t *wrapper,
			     cairo_operator_t	      op,
			     const cairo_pattern_t   *source,
                             unsigned int             source_region_id,
			     const cairo_pattern_t   *mask,
                             unsigned int             mask_region_id,
			     const cairo_clip_t	     *clip)
{
    cairo_status_t status;
    cairo_clip_t *dev_clip;
    cairo_pattern_union_t source_copy;
    cairo_pattern_union_t mask_copy;

    if (unlikely (wrapper->target->status))
	return wrapper->target->status;

    dev_clip = _cairo_surface_wrapper_get_clip (wrapper, clip);
    if (_cairo_clip_is_all_clipped (dev_clip))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (wrapper->needs_transform || source_region_id != 0 || mask_region_id != 0) {
	cairo_matrix_t m;

	_cairo_surface_wrapper_get_transform (wrapper, &m);

	status = cairo_matrix_invert (&m);
	assert (status == CAIRO_STATUS_SUCCESS);

	_copy_transformed_pattern (&source_copy.base, source, &m, source_region_id);
	source = &source_copy.base;

	_copy_transformed_pattern (&mask_copy.base, mask, &m, mask_region_id);
	mask = &mask_copy.base;
    }

    status = _cairo_surface_mask (wrapper->target, op, source, mask, dev_clip);

    _cairo_clip_destroy (dev_clip);
    return status;
}

cairo_status_t
_cairo_surface_wrapper_stroke (cairo_surface_wrapper_t    *wrapper,
			       cairo_operator_t		   op,
			       const cairo_pattern_t	  *source,
			       unsigned int                source_region_id,
			       const cairo_path_fixed_t	  *path,
			       const cairo_stroke_style_t *stroke_style,
			       const cairo_matrix_t	  *ctm,
			       const cairo_matrix_t	  *ctm_inverse,
			       double			   tolerance,
			       cairo_antialias_t	   antialias,
			       const cairo_clip_t	  *clip)
{
    cairo_status_t status;
    cairo_path_fixed_t path_copy, *dev_path = (cairo_path_fixed_t *) path;
    cairo_clip_t *dev_clip;
    cairo_matrix_t dev_ctm = *ctm;
    cairo_matrix_t dev_ctm_inverse = *ctm_inverse;
    cairo_pattern_union_t source_copy;

    if (unlikely (wrapper->target->status))
	return wrapper->target->status;

    dev_clip = _cairo_surface_wrapper_get_clip (wrapper, clip);
    if (_cairo_clip_is_all_clipped (dev_clip))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (wrapper->needs_transform || source_region_id != 0) {
	cairo_matrix_t m;

	_cairo_surface_wrapper_get_transform (wrapper, &m);

	status = _cairo_path_fixed_init_copy (&path_copy, dev_path);
	if (unlikely (status))
	    goto FINISH;

	_cairo_path_fixed_transform (&path_copy, &m);
	dev_path = &path_copy;

	cairo_matrix_multiply (&dev_ctm, &dev_ctm, &m);

	status = cairo_matrix_invert (&m);
	assert (status == CAIRO_STATUS_SUCCESS);

	cairo_matrix_multiply (&dev_ctm_inverse, &m, &dev_ctm_inverse);

	_copy_transformed_pattern (&source_copy.base, source, &m, source_region_id);
	source = &source_copy.base;
    }

    status = _cairo_surface_stroke (wrapper->target, op, source,
				    dev_path, stroke_style,
				    &dev_ctm, &dev_ctm_inverse,
				    tolerance, antialias,
				    dev_clip);

 FINISH:
    if (dev_path != path)
	_cairo_path_fixed_fini (dev_path);
    _cairo_clip_destroy (dev_clip);
    return status;
}

cairo_status_t
_cairo_surface_wrapper_fill_stroke (cairo_surface_wrapper_t    *wrapper,
				    cairo_operator_t	        fill_op,
				    const cairo_pattern_t      *fill_source,
				    unsigned int                fill_region_id,
				    cairo_fill_rule_t	        fill_rule,
				    double		        fill_tolerance,
				    cairo_antialias_t	        fill_antialias,
				    const cairo_path_fixed_t   *path,
				    cairo_operator_t	        stroke_op,
				    const cairo_pattern_t      *stroke_source,
				    unsigned int                stroke_region_id,
				    const cairo_stroke_style_t *stroke_style,
				    const cairo_matrix_t       *stroke_ctm,
				    const cairo_matrix_t       *stroke_ctm_inverse,
				    double		        stroke_tolerance,
				    cairo_antialias_t	        stroke_antialias,
				    const cairo_clip_t	       *clip)
{
    cairo_status_t status;
    cairo_path_fixed_t path_copy, *dev_path = (cairo_path_fixed_t *)path;
    cairo_matrix_t dev_ctm = *stroke_ctm;
    cairo_matrix_t dev_ctm_inverse = *stroke_ctm_inverse;
    cairo_clip_t *dev_clip;
    cairo_pattern_union_t stroke_source_copy;
    cairo_pattern_union_t fill_source_copy;

    if (unlikely (wrapper->target->status))
	return wrapper->target->status;

    dev_clip = _cairo_surface_wrapper_get_clip (wrapper, clip);
    if (_cairo_clip_is_all_clipped (dev_clip))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (wrapper->needs_transform || fill_region_id != 0 || stroke_region_id != 0) {
	cairo_matrix_t m;

	_cairo_surface_wrapper_get_transform (wrapper, &m);

	status = _cairo_path_fixed_init_copy (&path_copy, dev_path);
	if (unlikely (status))
	    goto FINISH;

	_cairo_path_fixed_transform (&path_copy, &m);
	dev_path = &path_copy;

	cairo_matrix_multiply (&dev_ctm, &dev_ctm, &m);

	status = cairo_matrix_invert (&m);
	assert (status == CAIRO_STATUS_SUCCESS);

	cairo_matrix_multiply (&dev_ctm_inverse, &m, &dev_ctm_inverse);

	_copy_transformed_pattern (&stroke_source_copy.base, stroke_source, &m, fill_region_id);
	stroke_source = &stroke_source_copy.base;

	_copy_transformed_pattern (&fill_source_copy.base, fill_source, &m, stroke_region_id);
	fill_source = &fill_source_copy.base;
    }

    status = _cairo_surface_fill_stroke (wrapper->target,
					 fill_op, fill_source, fill_rule,
					 fill_tolerance, fill_antialias,
					 dev_path,
					 stroke_op, stroke_source,
					 stroke_style,
					 &dev_ctm, &dev_ctm_inverse,
					 stroke_tolerance, stroke_antialias,
					 dev_clip);

  FINISH:
    if (dev_path != path)
	_cairo_path_fixed_fini (dev_path);
    _cairo_clip_destroy (dev_clip);
    return status;
}

cairo_status_t
_cairo_surface_wrapper_fill (cairo_surface_wrapper_t  *wrapper,
			     cairo_operator_t	       op,
			     const cairo_pattern_t    *source,
			     unsigned int              source_region_id,
			     const cairo_path_fixed_t *path,
			     cairo_fill_rule_t	       fill_rule,
			     double		       tolerance,
			     cairo_antialias_t	       antialias,
			     const cairo_clip_t	      *clip)
{
    cairo_status_t status;
    cairo_path_fixed_t path_copy, *dev_path = (cairo_path_fixed_t *) path;
    cairo_pattern_union_t source_copy;
    cairo_clip_t *dev_clip;

    if (unlikely (wrapper->target->status))
	return wrapper->target->status;

    dev_clip = _cairo_surface_wrapper_get_clip (wrapper, clip);
    if (_cairo_clip_is_all_clipped (dev_clip))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (wrapper->needs_transform || source_region_id != 0) {
	cairo_matrix_t m;

	_cairo_surface_wrapper_get_transform (wrapper, &m);

	status = _cairo_path_fixed_init_copy (&path_copy, dev_path);
	if (unlikely (status))
	    goto FINISH;

	_cairo_path_fixed_transform (&path_copy, &m);
	dev_path = &path_copy;

	status = cairo_matrix_invert (&m);
	assert (status == CAIRO_STATUS_SUCCESS);

	_copy_transformed_pattern (&source_copy.base, source, &m, source_region_id);
	source = &source_copy.base;
    }

    status = _cairo_surface_fill (wrapper->target, op, source,
				  dev_path, fill_rule,
				  tolerance, antialias,
				  dev_clip);

 FINISH:
    if (dev_path != path)
	_cairo_path_fixed_fini (dev_path);
    _cairo_clip_destroy (dev_clip);
    return status;
}

cairo_status_t
_cairo_surface_wrapper_show_text_glyphs (cairo_surface_wrapper_t    *wrapper,
					 cairo_operator_t	     op,
					 const cairo_pattern_t	    *source,
					 unsigned int                source_region_id,
					 const char		    *utf8,
					 int			     utf8_len,
					 const cairo_glyph_t	    *glyphs,
					 int			     num_glyphs,
					 const cairo_text_cluster_t *clusters,
					 int			     num_clusters,
					 cairo_text_cluster_flags_t  cluster_flags,
					 cairo_scaled_font_t	    *scaled_font,
					 const cairo_clip_t	    *clip)
{
    cairo_status_t status;
    cairo_clip_t *dev_clip;
    cairo_glyph_t stack_glyphs [CAIRO_STACK_ARRAY_LENGTH(cairo_glyph_t)];
    cairo_glyph_t *dev_glyphs = stack_glyphs;
    cairo_scaled_font_t *dev_scaled_font = scaled_font;
    cairo_pattern_union_t source_copy;
    cairo_font_options_t options;

    if (unlikely (wrapper->target->status))
	return wrapper->target->status;

    dev_clip = _cairo_surface_wrapper_get_clip (wrapper, clip);
    if (_cairo_clip_is_all_clipped (dev_clip))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    cairo_surface_get_font_options (wrapper->target, &options);
    cairo_font_options_merge (&options, &scaled_font->options);

    if (wrapper->needs_transform || source_region_id != 0) {
	cairo_matrix_t m;
	int i;

	_cairo_surface_wrapper_get_transform (wrapper, &m);

	if (! _cairo_matrix_is_translation (&m)) {
	    cairo_matrix_t ctm;

	    _cairo_matrix_multiply (&ctm,
				    &m,
				    &scaled_font->ctm);
	    dev_scaled_font = cairo_scaled_font_create (scaled_font->font_face,
							&scaled_font->font_matrix,
							&ctm, &options);
	}

	if (num_glyphs > ARRAY_LENGTH (stack_glyphs)) {
	    dev_glyphs = _cairo_malloc_ab (num_glyphs, sizeof (cairo_glyph_t));
	    if (unlikely (dev_glyphs == NULL)) {
		status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
		goto FINISH;
	    }
	}

	for (i = 0; i < num_glyphs; i++) {
	    dev_glyphs[i] = glyphs[i];
	    cairo_matrix_transform_point (&m,
					  &dev_glyphs[i].x,
					  &dev_glyphs[i].y);
	}

	status = cairo_matrix_invert (&m);
	assert (status == CAIRO_STATUS_SUCCESS);

	_copy_transformed_pattern (&source_copy.base, source, &m, source_region_id);
	source = &source_copy.base;
    } else {
	if (! cairo_font_options_equal (&options, &scaled_font->options)) {
	    dev_scaled_font = cairo_scaled_font_create (scaled_font->font_face,
							&scaled_font->font_matrix,
							&scaled_font->ctm,
							&options);
	}

	/* show_text_glyphs is special because _cairo_surface_show_text_glyphs is allowed
	 * to modify the glyph array that's passed in.  We must always
	 * copy the array before handing it to the backend.
	 */
	if (num_glyphs > ARRAY_LENGTH (stack_glyphs)) {
	    dev_glyphs = _cairo_malloc_ab (num_glyphs, sizeof (cairo_glyph_t));
	    if (unlikely (dev_glyphs == NULL)) {
		status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
		goto FINISH;
	    }
	}

	memcpy (dev_glyphs, glyphs, sizeof (cairo_glyph_t) * num_glyphs);
    }

    status = _cairo_surface_show_text_glyphs (wrapper->target, op, source,
					      utf8, utf8_len,
					      dev_glyphs, num_glyphs,
					      clusters, num_clusters,
					      cluster_flags,
					      dev_scaled_font,
					      dev_clip);
 FINISH:
    _cairo_clip_destroy (dev_clip);
    if (dev_glyphs != stack_glyphs)
	free (dev_glyphs);
    if (dev_scaled_font != scaled_font)
	cairo_scaled_font_destroy (dev_scaled_font);
    return status;
}

cairo_status_t
_cairo_surface_wrapper_tag (cairo_surface_wrapper_t     *wrapper,
			    cairo_bool_t                 begin,
			    const char                  *tag_name,
			    const char                  *attributes)
{
    if (unlikely (wrapper->target->status))
	return wrapper->target->status;


    return _cairo_surface_tag (wrapper->target, begin, tag_name, attributes);
}

cairo_surface_t *
_cairo_surface_wrapper_create_similar (cairo_surface_wrapper_t *wrapper,
				       cairo_content_t	content,
				       int		width,
				       int		height)
{
    return _cairo_surface_create_scratch (wrapper->target,
					  content, width, height, NULL);
}

cairo_bool_t
_cairo_surface_wrapper_get_extents (cairo_surface_wrapper_t *wrapper,
				    cairo_rectangle_int_t   *extents)
{
    if (wrapper->has_extents) {
	if (_cairo_surface_get_extents (wrapper->target, extents))
	    _cairo_rectangle_intersect (extents, &wrapper->extents);
	else
	    *extents = wrapper->extents;

	return TRUE;
    } else {
	return _cairo_surface_get_extents (wrapper->target, extents);
    }
}

static cairo_bool_t
_cairo_surface_wrapper_needs_device_transform (cairo_surface_wrapper_t *wrapper)
{
    return
	(wrapper->has_extents && (wrapper->extents.x | wrapper->extents.y)) ||
	! _cairo_matrix_is_identity (&wrapper->transform) ||
	! _cairo_matrix_is_identity (&wrapper->target->device_transform);
}

void
_cairo_surface_wrapper_intersect_extents (cairo_surface_wrapper_t *wrapper,
					  const cairo_rectangle_int_t *extents)
{
    if (! wrapper->has_extents) {
	wrapper->extents = *extents;
	wrapper->has_extents = TRUE;
    } else
	_cairo_rectangle_intersect (&wrapper->extents, extents);

    wrapper->needs_transform =
	_cairo_surface_wrapper_needs_device_transform (wrapper);
}

void
_cairo_surface_wrapper_set_inverse_transform (cairo_surface_wrapper_t *wrapper,
					      const cairo_matrix_t *transform)
{
    cairo_status_t status;

    if (transform == NULL || _cairo_matrix_is_identity (transform)) {
	cairo_matrix_init_identity (&wrapper->transform);

	wrapper->needs_transform =
	    _cairo_surface_wrapper_needs_device_transform (wrapper);
    } else {
	wrapper->transform = *transform;
	status = cairo_matrix_invert (&wrapper->transform);
	/* should always be invertible unless given pathological input */
	assert (status == CAIRO_STATUS_SUCCESS);

	wrapper->needs_transform = TRUE;
    }
}

void
_cairo_surface_wrapper_set_clip (cairo_surface_wrapper_t *wrapper,
				 const cairo_clip_t *clip)
{
    wrapper->clip = clip;
}

void
_cairo_surface_wrapper_get_font_options (cairo_surface_wrapper_t    *wrapper,
					 cairo_font_options_t	    *options)
{
    cairo_surface_get_font_options (wrapper->target, options);
}

cairo_surface_t *
_cairo_surface_wrapper_snapshot (cairo_surface_wrapper_t *wrapper)
{
    if (wrapper->target->backend->snapshot)
	return wrapper->target->backend->snapshot (wrapper->target);

    return NULL;
}

cairo_bool_t
_cairo_surface_wrapper_has_show_text_glyphs (cairo_surface_wrapper_t *wrapper)
{
    return cairo_surface_has_show_text_glyphs (wrapper->target);
}

void
_cairo_surface_wrapper_init (cairo_surface_wrapper_t *wrapper,
			     cairo_surface_t *target)
{
    wrapper->target = cairo_surface_reference (target);
    cairo_matrix_init_identity (&wrapper->transform);
    wrapper->has_extents = FALSE;
    wrapper->extents.x = wrapper->extents.y = 0;
    wrapper->clip = NULL;
    wrapper->source_region_id = 0;
    wrapper->mask_region_id = 0;

    wrapper->needs_transform = FALSE;
    if (target) {
	wrapper->needs_transform =
	    ! _cairo_matrix_is_identity (&target->device_transform);
    }
}

void
_cairo_surface_wrapper_fini (cairo_surface_wrapper_t *wrapper)
{
    cairo_surface_destroy (wrapper->target);
}

cairo_bool_t
_cairo_surface_wrapper_get_target_extents (cairo_surface_wrapper_t *wrapper,
					   cairo_bool_t surface_is_unbounded,
					   cairo_rectangle_int_t *extents)
{
    cairo_rectangle_int_t clip;
    cairo_bool_t has_clip = FALSE;

    if (!surface_is_unbounded)
	has_clip = _cairo_surface_get_extents (wrapper->target, &clip);

    if (wrapper->clip) {
	if (has_clip) {
	    if (! _cairo_rectangle_intersect (&clip,
					      _cairo_clip_get_extents (wrapper->clip)))
		return FALSE;
	} else {
	    has_clip = TRUE;
	    clip = *_cairo_clip_get_extents (wrapper->clip);
	}
    }

    if (has_clip && wrapper->needs_transform) {
	cairo_matrix_t m;
	double x1, y1, x2, y2;

	_cairo_surface_wrapper_get_inverse_transform (wrapper, &m);

	x1 = clip.x;
	y1 = clip.y;
	x2 = clip.x + clip.width;
	y2 = clip.y + clip.height;

	_cairo_matrix_transform_bounding_box (&m, &x1, &y1, &x2, &y2, NULL);

	clip.x = floor (x1);
	clip.y = floor (y1);
	clip.width  = ceil (x2) - clip.x;
	clip.height = ceil (y2) - clip.y;
    }

    if (has_clip) {
	if (wrapper->has_extents) {
	    *extents = wrapper->extents;
	    return _cairo_rectangle_intersect (extents, &clip);
	} else {
	    *extents = clip;
	    return TRUE;
	}
    } else if (wrapper->has_extents) {
	*extents = wrapper->extents;
	return TRUE;
    } else {
	_cairo_unbounded_rectangle_init (extents);
	return TRUE;
    }
}
