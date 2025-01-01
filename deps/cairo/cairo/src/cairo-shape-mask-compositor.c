/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2012 Intel Corporation
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
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-compositor-private.h"
#include "cairo-clip-private.h"
#include "cairo-pattern-private.h"
#include "cairo-surface-private.h"
#include "cairo-surface-offset-private.h"

static cairo_int_status_t
_cairo_shape_mask_compositor_stroke (const cairo_compositor_t *_compositor,
				     cairo_composite_rectangles_t *extents,
				     const cairo_path_fixed_t	*path,
				     const cairo_stroke_style_t	*style,
				     const cairo_matrix_t	*ctm,
				     const cairo_matrix_t	*ctm_inverse,
				     double		 tolerance,
				     cairo_antialias_t	 antialias)
{
    cairo_surface_t *mask;
    cairo_surface_pattern_t pattern;
    cairo_int_status_t status;
    cairo_clip_t *clip;

    if (! extents->is_bounded)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    TRACE ((stderr, "%s\n", __FUNCTION__));
    mask = _cairo_surface_create_scratch (extents->surface,
					  CAIRO_CONTENT_ALPHA,
					  extents->bounded.width,
					  extents->bounded.height,
					  NULL);
    if (unlikely (mask->status))
	return mask->status;

    clip = extents->clip;
    if (! _cairo_clip_is_region (clip))
	clip = _cairo_clip_copy_region (clip);

    if (! mask->is_clear) {
	status = _cairo_surface_offset_paint (mask,
					      extents->bounded.x,
					      extents->bounded.y,
					      CAIRO_OPERATOR_CLEAR,
					      &_cairo_pattern_clear.base,
					      clip);
	if (unlikely (status))
	    goto error;
    }

    status = _cairo_surface_offset_stroke (mask,
					   extents->bounded.x,
					   extents->bounded.y,
					   CAIRO_OPERATOR_ADD,
					   &_cairo_pattern_white.base,
					   path, style, ctm, ctm_inverse,
					   tolerance, antialias,
					   clip);
    if (unlikely (status))
	goto error;

    if (clip != extents->clip) {
	status = _cairo_clip_combine_with_surface (extents->clip, mask,
						   extents->bounded.x,
						   extents->bounded.y);
	if (unlikely (status))
	    goto error;
    }

    _cairo_pattern_init_for_surface (&pattern, mask);
    cairo_matrix_init_translate (&pattern.base.matrix,
				 -extents->bounded.x,
				 -extents->bounded.y);
    pattern.base.filter = CAIRO_FILTER_NEAREST;
    pattern.base.extend = CAIRO_EXTEND_NONE;
    if (extents->op == CAIRO_OPERATOR_SOURCE) {
	status = _cairo_surface_mask (extents->surface,
				      CAIRO_OPERATOR_DEST_OUT,
				      &_cairo_pattern_white.base,
				      &pattern.base,
				      clip);
	if ((status == CAIRO_INT_STATUS_SUCCESS)) {
	    status = _cairo_surface_mask (extents->surface,
					  CAIRO_OPERATOR_ADD,
					  &extents->source_pattern.base,
					  &pattern.base,
					  clip);
	}
    } else {
	status = _cairo_surface_mask (extents->surface,
				      extents->op,
				      &extents->source_pattern.base,
				      &pattern.base,
				      clip);
    }
    _cairo_pattern_fini (&pattern.base);

error:
    cairo_surface_destroy (mask);
    if (clip != extents->clip)
	_cairo_clip_destroy (clip);
    return status;
}

static cairo_int_status_t
_cairo_shape_mask_compositor_fill (const cairo_compositor_t *_compositor,
				   cairo_composite_rectangles_t *extents,
				   const cairo_path_fixed_t	*path,
				   cairo_fill_rule_t	 fill_rule,
				   double			 tolerance,
				   cairo_antialias_t	 antialias)
{
    cairo_surface_t *mask;
    cairo_surface_pattern_t pattern;
    cairo_int_status_t status;
    cairo_clip_t *clip;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (! extents->is_bounded)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    mask = _cairo_surface_create_scratch (extents->surface,
					  CAIRO_CONTENT_ALPHA,
					  extents->bounded.width,
					  extents->bounded.height,
					  NULL);
    if (unlikely (mask->status))
	return mask->status;

    clip = extents->clip;
    if (! _cairo_clip_is_region (clip))
	clip = _cairo_clip_copy_region (clip);

    if (! mask->is_clear) {
	status = _cairo_surface_offset_paint (mask,
					      extents->bounded.x,
					      extents->bounded.y,
					      CAIRO_OPERATOR_CLEAR,
					      &_cairo_pattern_clear.base,
					      clip);
	if (unlikely (status))
	    goto error;
    }

    status = _cairo_surface_offset_fill (mask,
					 extents->bounded.x,
					 extents->bounded.y,
					 CAIRO_OPERATOR_ADD,
					 &_cairo_pattern_white.base,
					 path, fill_rule, tolerance, antialias,
					 clip);
    if (unlikely (status))
	goto error;

    if (clip != extents->clip) {
	status = _cairo_clip_combine_with_surface (extents->clip, mask,
						   extents->bounded.x,
						   extents->bounded.y);
	if (unlikely (status))
	    goto error;
    }

    _cairo_pattern_init_for_surface (&pattern, mask);
    cairo_matrix_init_translate (&pattern.base.matrix,
				 -extents->bounded.x,
				 -extents->bounded.y);
    pattern.base.filter = CAIRO_FILTER_NEAREST;
    pattern.base.extend = CAIRO_EXTEND_NONE;
    if (extents->op == CAIRO_OPERATOR_SOURCE) {
	status = _cairo_surface_mask (extents->surface,
				      CAIRO_OPERATOR_DEST_OUT,
				      &_cairo_pattern_white.base,
				      &pattern.base,
				      clip);
	if ((status == CAIRO_INT_STATUS_SUCCESS)) {
	    status = _cairo_surface_mask (extents->surface,
					  CAIRO_OPERATOR_ADD,
					  &extents->source_pattern.base,
					  &pattern.base,
					  clip);
	}
    } else {
	status = _cairo_surface_mask (extents->surface,
				      extents->op,
				      &extents->source_pattern.base,
				      &pattern.base,
				      clip);
    }
    _cairo_pattern_fini (&pattern.base);

error:
    if (clip != extents->clip)
	_cairo_clip_destroy (clip);
    cairo_surface_destroy (mask);
    return status;
}

static cairo_int_status_t
_cairo_shape_mask_compositor_glyphs (const cairo_compositor_t *_compositor,
				     cairo_composite_rectangles_t *extents,
				     cairo_scaled_font_t	*scaled_font,
				     cairo_glyph_t		*glyphs,
				     int			 num_glyphs,
				     cairo_bool_t		 overlap)
{
    cairo_surface_t *mask;
    cairo_surface_pattern_t pattern;
    cairo_int_status_t status;
    cairo_clip_t *clip;

    if (! extents->is_bounded)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    TRACE ((stderr, "%s\n", __FUNCTION__));
    mask = _cairo_surface_create_scratch (extents->surface,
					  CAIRO_CONTENT_ALPHA,
					  extents->bounded.width,
					  extents->bounded.height,
					  NULL);
    if (unlikely (mask->status))
	return mask->status;

    clip = extents->clip;
    if (! _cairo_clip_is_region (clip))
	clip = _cairo_clip_copy_region (clip);

    if (! mask->is_clear) {
	status = _cairo_surface_offset_paint (mask,
					      extents->bounded.x,
					      extents->bounded.y,
					      CAIRO_OPERATOR_CLEAR,
					      &_cairo_pattern_clear.base,
					      clip);
	if (unlikely (status))
	    goto error;
    }

    status = _cairo_surface_offset_glyphs (mask,
					   extents->bounded.x,
					   extents->bounded.y,
					   CAIRO_OPERATOR_ADD,
					   &_cairo_pattern_white.base,
					   scaled_font, glyphs, num_glyphs,
					   clip);
    if (unlikely (status))
	goto error;

    if (clip != extents->clip) {
	status = _cairo_clip_combine_with_surface (extents->clip, mask,
						   extents->bounded.x,
						   extents->bounded.y);
	if (unlikely (status))
	    goto error;
    }

    _cairo_pattern_init_for_surface (&pattern, mask);
    cairo_matrix_init_translate (&pattern.base.matrix,
				 -extents->bounded.x,
				 -extents->bounded.y);
    pattern.base.filter = CAIRO_FILTER_NEAREST;
    pattern.base.extend = CAIRO_EXTEND_NONE;
    if (extents->op == CAIRO_OPERATOR_SOURCE) {
	status = _cairo_surface_mask (extents->surface,
				      CAIRO_OPERATOR_DEST_OUT,
				      &_cairo_pattern_white.base,
				      &pattern.base,
				      clip);
	if ((status == CAIRO_INT_STATUS_SUCCESS)) {
	    status = _cairo_surface_mask (extents->surface,
					  CAIRO_OPERATOR_ADD,
					  &extents->source_pattern.base,
					  &pattern.base,
					  clip);
	}
    } else {
	status = _cairo_surface_mask (extents->surface,
				      extents->op,
				      &extents->source_pattern.base,
				      &pattern.base,
				      clip);
    }
    _cairo_pattern_fini (&pattern.base);

error:
    if (clip != extents->clip)
	_cairo_clip_destroy (clip);
    cairo_surface_destroy (mask);
    return status;
}

void
_cairo_shape_mask_compositor_init (cairo_compositor_t *compositor,
				   const cairo_compositor_t  *delegate)
{
    compositor->delegate = delegate;

    compositor->paint  = NULL;
    compositor->mask   = NULL;
    compositor->fill   = _cairo_shape_mask_compositor_fill;
    compositor->stroke = _cairo_shape_mask_compositor_stroke;
    compositor->glyphs = _cairo_shape_mask_compositor_glyphs;
}
