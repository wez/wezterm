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
 * The Initial Developer of the Original Code is Red Hat, Inc.
 *
 * Contributor(s):
 *      Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-clip-inline.h"
#include "cairo-error-private.h"
#include "cairo-composite-rectangles-private.h"
#include "cairo-pattern-private.h"

/* A collection of routines to facilitate writing compositors. */

void _cairo_composite_rectangles_fini (cairo_composite_rectangles_t *extents)
{
    /* If adding further free() code here, make sure those fields are inited by
     * _cairo_composite_rectangles_init IN ALL CASES
     */
    _cairo_clip_destroy (extents->clip);
    extents->clip = NULL;
}

static void
_cairo_composite_reduce_pattern (const cairo_pattern_t *src,
				 cairo_pattern_union_t *dst)
{
    int tx, ty;

    _cairo_pattern_init_static_copy (&dst->base, src);
    if (dst->base.type == CAIRO_PATTERN_TYPE_SOLID)
	return;

    dst->base.filter = _cairo_pattern_analyze_filter (&dst->base);

    tx = ty = 0;
    if (_cairo_matrix_is_pixman_translation (&dst->base.matrix,
					     dst->base.filter,
					     &tx, &ty))
    {
	dst->base.matrix.x0 = tx;
	dst->base.matrix.y0 = ty;
    }
}

static inline cairo_bool_t
_cairo_composite_rectangles_init (cairo_composite_rectangles_t *extents,
				  cairo_surface_t *surface,
				  cairo_operator_t op,
				  const cairo_pattern_t *source,
				  const cairo_clip_t *clip)
{
    /* Always set the clip so that a _cairo_composite_rectangles_init can ALWAYS be
     * balanced by a _cairo_composite_rectangles_fini */
    extents->clip = NULL;

    if (_cairo_clip_is_all_clipped (clip))
	return FALSE;
    extents->surface = surface;
    extents->op = op;

    _cairo_surface_get_extents (surface, &extents->destination);

    extents->unbounded = extents->destination;
    if (clip && ! _cairo_rectangle_intersect (&extents->unbounded,
					      _cairo_clip_get_extents (clip)))
	return FALSE;

    extents->bounded = extents->unbounded;
    extents->is_bounded = _cairo_operator_bounded_by_either (op);

    extents->original_source_pattern = source;
    _cairo_composite_reduce_pattern (source, &extents->source_pattern);

    _cairo_pattern_get_extents (&extents->source_pattern.base,
				&extents->source,
				surface->is_vector);
    if (extents->is_bounded & CAIRO_OPERATOR_BOUND_BY_SOURCE) {
	if (! _cairo_rectangle_intersect (&extents->bounded, &extents->source))
	    return FALSE;
    }

    extents->original_mask_pattern = NULL;
    extents->mask_pattern.base.type = CAIRO_PATTERN_TYPE_SOLID;
    extents->mask_pattern.solid.color.alpha = 1.; /* XXX full initialisation? */
    extents->mask_pattern.solid.color.alpha_short = 0xffff;

    return TRUE;
}

cairo_int_status_t
_cairo_composite_rectangles_init_for_paint (cairo_composite_rectangles_t *extents,
					    cairo_surface_t *surface,
					    cairo_operator_t		 op,
					    const cairo_pattern_t	*source,
					    const cairo_clip_t		*clip)
{
    if (! _cairo_composite_rectangles_init (extents,
					    surface, op, source, clip))
    {
	goto NOTHING_TO_DO;
    }

    extents->mask = extents->destination;

    extents->clip = _cairo_clip_reduce_for_composite (clip, extents);
    if (_cairo_clip_is_all_clipped (extents->clip))
	goto NOTHING_TO_DO;

    if (! _cairo_rectangle_intersect (&extents->unbounded,
				      _cairo_clip_get_extents (extents->clip)))
	goto NOTHING_TO_DO;

    if (extents->source_pattern.base.type != CAIRO_PATTERN_TYPE_SOLID)
	_cairo_pattern_sampled_area (&extents->source_pattern.base,
				     &extents->bounded,
				     &extents->source_sample_area);

    return CAIRO_INT_STATUS_SUCCESS;
NOTHING_TO_DO:
    _cairo_composite_rectangles_fini(extents);
    return CAIRO_INT_STATUS_NOTHING_TO_DO;
}

static cairo_int_status_t
_cairo_composite_rectangles_intersect (cairo_composite_rectangles_t *extents,
				       const cairo_clip_t *clip)
{
    if ((!_cairo_rectangle_intersect (&extents->bounded, &extents->mask)) &&
        (extents->is_bounded & CAIRO_OPERATOR_BOUND_BY_MASK))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (extents->is_bounded == (CAIRO_OPERATOR_BOUND_BY_MASK | CAIRO_OPERATOR_BOUND_BY_SOURCE)) {
	extents->unbounded = extents->bounded;
    } else if (extents->is_bounded & CAIRO_OPERATOR_BOUND_BY_MASK) {
	if (!_cairo_rectangle_intersect (&extents->unbounded, &extents->mask))
	    return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    extents->clip = _cairo_clip_reduce_for_composite (clip, extents);
    if (_cairo_clip_is_all_clipped (extents->clip))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (! _cairo_rectangle_intersect (&extents->unbounded,
				      _cairo_clip_get_extents (extents->clip)))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (! _cairo_rectangle_intersect (&extents->bounded,
				      _cairo_clip_get_extents (extents->clip)) &&
	extents->is_bounded & CAIRO_OPERATOR_BOUND_BY_MASK)
    {
	return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    if (extents->source_pattern.base.type != CAIRO_PATTERN_TYPE_SOLID)
	_cairo_pattern_sampled_area (&extents->source_pattern.base,
				     &extents->bounded,
				     &extents->source_sample_area);
    if (extents->mask_pattern.base.type != CAIRO_PATTERN_TYPE_SOLID) {
	_cairo_pattern_sampled_area (&extents->mask_pattern.base,
				     &extents->bounded,
				     &extents->mask_sample_area);
	if (extents->mask_sample_area.width == 0 ||
	    extents->mask_sample_area.height == 0) {
	    _cairo_composite_rectangles_fini (extents);
	    return CAIRO_INT_STATUS_NOTHING_TO_DO;
	}
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

cairo_int_status_t
_cairo_composite_rectangles_intersect_source_extents (cairo_composite_rectangles_t *extents,
						      const cairo_box_t *box)
{
    cairo_rectangle_int_t rect;
    cairo_clip_t *clip;

    _cairo_box_round_to_rectangle (box, &rect);
    if (rect.x == extents->source.x &&
	rect.y == extents->source.y &&
	rect.width  == extents->source.width &&
	rect.height == extents->source.height)
    {
	return CAIRO_INT_STATUS_SUCCESS;
    }

    _cairo_rectangle_intersect (&extents->source, &rect);

    rect = extents->bounded;
    if (! _cairo_rectangle_intersect (&extents->bounded, &extents->source) &&
	extents->is_bounded & CAIRO_OPERATOR_BOUND_BY_SOURCE)
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (rect.width  == extents->bounded.width &&
	rect.height == extents->bounded.height)
	return CAIRO_INT_STATUS_SUCCESS;

    if (extents->is_bounded == (CAIRO_OPERATOR_BOUND_BY_MASK | CAIRO_OPERATOR_BOUND_BY_SOURCE)) {
	extents->unbounded = extents->bounded;
    } else if (extents->is_bounded & CAIRO_OPERATOR_BOUND_BY_MASK) {
	if (!_cairo_rectangle_intersect (&extents->unbounded, &extents->mask))
	    return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    clip = extents->clip;
    extents->clip = _cairo_clip_reduce_for_composite (clip, extents);
    if (clip != extents->clip)
	_cairo_clip_destroy (clip);

    if (_cairo_clip_is_all_clipped (extents->clip))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (! _cairo_rectangle_intersect (&extents->unbounded,
				      _cairo_clip_get_extents (extents->clip)))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (extents->source_pattern.base.type != CAIRO_PATTERN_TYPE_SOLID)
	_cairo_pattern_sampled_area (&extents->source_pattern.base,
				     &extents->bounded,
				     &extents->source_sample_area);
    if (extents->mask_pattern.base.type != CAIRO_PATTERN_TYPE_SOLID) {
	_cairo_pattern_sampled_area (&extents->mask_pattern.base,
				     &extents->bounded,
				     &extents->mask_sample_area);
	if (extents->mask_sample_area.width == 0 ||
	    extents->mask_sample_area.height == 0)
	    return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

cairo_int_status_t
_cairo_composite_rectangles_intersect_mask_extents (cairo_composite_rectangles_t *extents,
						    const cairo_box_t *box)
{
    cairo_rectangle_int_t mask;
    cairo_clip_t *clip;

    _cairo_box_round_to_rectangle (box, &mask);
    if (mask.x == extents->mask.x &&
	mask.y == extents->mask.y &&
	mask.width  == extents->mask.width &&
	mask.height == extents->mask.height)
    {
	return CAIRO_INT_STATUS_SUCCESS;
    }

    _cairo_rectangle_intersect (&extents->mask, &mask);

    mask = extents->bounded;
    if (! _cairo_rectangle_intersect (&extents->bounded, &extents->mask) &&
	extents->is_bounded & CAIRO_OPERATOR_BOUND_BY_MASK)
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (mask.width  == extents->bounded.width &&
	mask.height == extents->bounded.height)
	return CAIRO_INT_STATUS_SUCCESS;

    if (extents->is_bounded == (CAIRO_OPERATOR_BOUND_BY_MASK | CAIRO_OPERATOR_BOUND_BY_SOURCE)) {
	extents->unbounded = extents->bounded;
    } else if (extents->is_bounded & CAIRO_OPERATOR_BOUND_BY_MASK) {
	if (!_cairo_rectangle_intersect (&extents->unbounded, &extents->mask))
	    return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    clip = extents->clip;
    extents->clip = _cairo_clip_reduce_for_composite (clip, extents);
    if (clip != extents->clip)
	_cairo_clip_destroy (clip);

    if (_cairo_clip_is_all_clipped (extents->clip))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (! _cairo_rectangle_intersect (&extents->unbounded,
				      _cairo_clip_get_extents (extents->clip)))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    if (extents->source_pattern.base.type != CAIRO_PATTERN_TYPE_SOLID)
	_cairo_pattern_sampled_area (&extents->source_pattern.base,
				     &extents->bounded,
				     &extents->source_sample_area);
    if (extents->mask_pattern.base.type != CAIRO_PATTERN_TYPE_SOLID) {
	_cairo_pattern_sampled_area (&extents->mask_pattern.base,
				     &extents->bounded,
				     &extents->mask_sample_area);
	if (extents->mask_sample_area.width == 0 ||
	    extents->mask_sample_area.height == 0)
	    return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

cairo_int_status_t
_cairo_composite_rectangles_init_for_mask (cairo_composite_rectangles_t *extents,
					   cairo_surface_t              *surface,
					   cairo_operator_t		 op,
					   const cairo_pattern_t	*source,
					   const cairo_pattern_t	*mask,
					   const cairo_clip_t		*clip)
{
    cairo_int_status_t status;
    if (! _cairo_composite_rectangles_init (extents,
					    surface, op, source, clip))
    {
	_cairo_composite_rectangles_fini(extents);
	return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    extents->original_mask_pattern = mask;
    _cairo_composite_reduce_pattern (mask, &extents->mask_pattern);
    _cairo_pattern_get_extents (&extents->mask_pattern.base, &extents->mask, surface->is_vector);

    status = _cairo_composite_rectangles_intersect (extents, clip);
    if(status == CAIRO_INT_STATUS_NOTHING_TO_DO) {
	_cairo_composite_rectangles_fini(extents);
    }
    return status;
}

cairo_int_status_t
_cairo_composite_rectangles_init_for_stroke (cairo_composite_rectangles_t *extents,
					     cairo_surface_t *surface,
					     cairo_operator_t		 op,
					     const cairo_pattern_t	*source,
					     const cairo_path_fixed_t		*path,
					     const cairo_stroke_style_t	*style,
					     const cairo_matrix_t	*ctm,
					     const cairo_clip_t		*clip)
{
    cairo_int_status_t status;
    if (! _cairo_composite_rectangles_init (extents,
					    surface, op, source, clip))
    {
	_cairo_composite_rectangles_fini(extents);
	return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    _cairo_path_fixed_approximate_stroke_extents (path, style, ctm, surface->is_vector, &extents->mask);

    status = _cairo_composite_rectangles_intersect (extents, clip);
    if(status == CAIRO_INT_STATUS_NOTHING_TO_DO) {
	_cairo_composite_rectangles_fini(extents);
    }
    return status;
}

cairo_int_status_t
_cairo_composite_rectangles_init_for_fill (cairo_composite_rectangles_t *extents,
					   cairo_surface_t *surface,
					   cairo_operator_t		 op,
					   const cairo_pattern_t	*source,
					   const cairo_path_fixed_t		*path,
					   const cairo_clip_t		*clip)
{
    cairo_int_status_t status;
    if (! _cairo_composite_rectangles_init (extents,
					    surface, op, source, clip))
    {
	_cairo_composite_rectangles_fini(extents);
	return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    _cairo_path_fixed_approximate_fill_extents (path, &extents->mask);

    status = _cairo_composite_rectangles_intersect (extents, clip);
        if(status == CAIRO_INT_STATUS_NOTHING_TO_DO) {
    _cairo_composite_rectangles_fini(extents);
    }
    return status;
}

cairo_int_status_t
_cairo_composite_rectangles_init_for_polygon (cairo_composite_rectangles_t *extents,
					      cairo_surface_t		*surface,
					      cairo_operator_t		 op,
					      const cairo_pattern_t	*source,
					      const cairo_polygon_t	*polygon,
					      const cairo_clip_t		*clip)
{
    cairo_int_status_t status;
    if (! _cairo_composite_rectangles_init (extents,
					    surface, op, source, clip))
    {
	_cairo_composite_rectangles_fini(extents);
	return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    _cairo_box_round_to_rectangle (&polygon->extents, &extents->mask);
    status = _cairo_composite_rectangles_intersect (extents, clip);
    if(status == CAIRO_INT_STATUS_NOTHING_TO_DO) {
	_cairo_composite_rectangles_fini(extents);
    }
    return status;
}

cairo_int_status_t
_cairo_composite_rectangles_init_for_boxes (cairo_composite_rectangles_t *extents,
					      cairo_surface_t		*surface,
					      cairo_operator_t		 op,
					      const cairo_pattern_t	*source,
					      const cairo_boxes_t	*boxes,
					      const cairo_clip_t		*clip)
{
    cairo_box_t box;
    cairo_int_status_t status;

    if (! _cairo_composite_rectangles_init (extents,
					    surface, op, source, clip))
    {
	_cairo_composite_rectangles_fini(extents);
	return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    _cairo_boxes_extents (boxes, &box);
    _cairo_box_round_to_rectangle (&box, &extents->mask);
    status = _cairo_composite_rectangles_intersect (extents, clip);
    if(status == CAIRO_INT_STATUS_NOTHING_TO_DO) {
	_cairo_composite_rectangles_fini(extents);
    }
    return status;
}

cairo_int_status_t
_cairo_composite_rectangles_init_for_glyphs (cairo_composite_rectangles_t *extents,
					     cairo_surface_t *surface,
					     cairo_operator_t		 op,
					     const cairo_pattern_t	*source,
					     cairo_scaled_font_t	*scaled_font,
					     cairo_glyph_t		*glyphs,
					     int			 num_glyphs,
					     const cairo_clip_t		*clip,
					     cairo_bool_t		*overlap)
{
    cairo_status_t status;
    cairo_int_status_t int_status;

    if (! _cairo_composite_rectangles_init (extents, surface, op, source, clip)) {
	_cairo_composite_rectangles_fini(extents);
	return CAIRO_INT_STATUS_NOTHING_TO_DO;
    }

    status = _cairo_scaled_font_glyph_device_extents (scaled_font,
						      glyphs, num_glyphs,
						      &extents->mask,
						      overlap);
    if (unlikely (status)) {
	_cairo_composite_rectangles_fini(extents);
	return status;
    }
    if (overlap && *overlap &&
	scaled_font->options.antialias == CAIRO_ANTIALIAS_NONE &&
	_cairo_pattern_is_opaque_solid (&extents->source_pattern.base))
    {
	*overlap = FALSE;
    }

    int_status = _cairo_composite_rectangles_intersect (extents, clip);
    if (int_status == CAIRO_INT_STATUS_NOTHING_TO_DO) {
	_cairo_composite_rectangles_fini(extents);
    }
    return int_status;
}

cairo_bool_t
_cairo_composite_rectangles_can_reduce_clip (cairo_composite_rectangles_t *composite,
					     cairo_clip_t *clip)
{
    cairo_rectangle_int_t extents;
    cairo_box_t box;

    if (clip == NULL)
	return TRUE;

    extents = composite->destination;
    if (composite->is_bounded & CAIRO_OPERATOR_BOUND_BY_SOURCE)
	_cairo_rectangle_intersect (&extents, &composite->source);
    if (composite->is_bounded & CAIRO_OPERATOR_BOUND_BY_MASK)
	_cairo_rectangle_intersect (&extents, &composite->mask);

    _cairo_box_from_rectangle (&box, &extents);
    return _cairo_clip_contains_box (clip, &box);
}

cairo_int_status_t
_cairo_composite_rectangles_add_to_damage (cairo_composite_rectangles_t *composite,
					   cairo_boxes_t *damage)
{
    cairo_int_status_t status;
    int n;

    for (n = 0; n < composite->clip->num_boxes; n++) {
	status = _cairo_boxes_add (damage,
			  CAIRO_ANTIALIAS_NONE,
			  &composite->clip->boxes[n]);
	if (unlikely (status))
	    return status;
    }

    return CAIRO_INT_STATUS_SUCCESS;
}
