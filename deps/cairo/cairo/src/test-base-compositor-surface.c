/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2005 Red Hat, Inc.
 * Copyright © 2011 Intel Corporation
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
 *      Joonas Pihlaja <jpihlaja@cc.helsinki.fi>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "test-compositor-surface-private.h"

#include "cairo-clip-private.h"
#include "cairo-composite-rectangles-private.h"
#include "cairo-compositor-private.h"
#include "cairo-error-private.h"
#include "cairo-image-surface-private.h"
#include "cairo-region-private.h"
#include "cairo-traps-private.h"

/* The intention is that this is a surface that just works, and most
 * important of all does not try to be clever!
 */

typedef cairo_int_status_t
(*draw_func_t) (cairo_image_surface_t		*dst,
		void				*closure,
		cairo_operator_t		 op,
		const cairo_pattern_t		*pattern,
		int				 dst_x,
		int				 dst_y,
		const cairo_rectangle_int_t	*extents);

static pixman_op_t
_pixman_operator (cairo_operator_t op)
{
    switch ((int) op) {
    case CAIRO_OPERATOR_CLEAR:
	return PIXMAN_OP_CLEAR;

    case CAIRO_OPERATOR_SOURCE:
	return PIXMAN_OP_SRC;
    case CAIRO_OPERATOR_OVER:
	return PIXMAN_OP_OVER;
    case CAIRO_OPERATOR_IN:
	return PIXMAN_OP_IN;
    case CAIRO_OPERATOR_OUT:
	return PIXMAN_OP_OUT;
    case CAIRO_OPERATOR_ATOP:
	return PIXMAN_OP_ATOP;

    case CAIRO_OPERATOR_DEST:
	return PIXMAN_OP_DST;
    case CAIRO_OPERATOR_DEST_OVER:
	return PIXMAN_OP_OVER_REVERSE;
    case CAIRO_OPERATOR_DEST_IN:
	return PIXMAN_OP_IN_REVERSE;
    case CAIRO_OPERATOR_DEST_OUT:
	return PIXMAN_OP_OUT_REVERSE;
    case CAIRO_OPERATOR_DEST_ATOP:
	return PIXMAN_OP_ATOP_REVERSE;

    case CAIRO_OPERATOR_XOR:
	return PIXMAN_OP_XOR;
    case CAIRO_OPERATOR_ADD:
	return PIXMAN_OP_ADD;
    case CAIRO_OPERATOR_SATURATE:
	return PIXMAN_OP_SATURATE;

    case CAIRO_OPERATOR_MULTIPLY:
	return PIXMAN_OP_MULTIPLY;
    case CAIRO_OPERATOR_SCREEN:
	return PIXMAN_OP_SCREEN;
    case CAIRO_OPERATOR_OVERLAY:
	return PIXMAN_OP_OVERLAY;
    case CAIRO_OPERATOR_DARKEN:
	return PIXMAN_OP_DARKEN;
    case CAIRO_OPERATOR_LIGHTEN:
	return PIXMAN_OP_LIGHTEN;
    case CAIRO_OPERATOR_COLOR_DODGE:
	return PIXMAN_OP_COLOR_DODGE;
    case CAIRO_OPERATOR_COLOR_BURN:
	return PIXMAN_OP_COLOR_BURN;
    case CAIRO_OPERATOR_HARD_LIGHT:
	return PIXMAN_OP_HARD_LIGHT;
    case CAIRO_OPERATOR_SOFT_LIGHT:
	return PIXMAN_OP_SOFT_LIGHT;
    case CAIRO_OPERATOR_DIFFERENCE:
	return PIXMAN_OP_DIFFERENCE;
    case CAIRO_OPERATOR_EXCLUSION:
	return PIXMAN_OP_EXCLUSION;
    case CAIRO_OPERATOR_HSL_HUE:
	return PIXMAN_OP_HSL_HUE;
    case CAIRO_OPERATOR_HSL_SATURATION:
	return PIXMAN_OP_HSL_SATURATION;
    case CAIRO_OPERATOR_HSL_COLOR:
	return PIXMAN_OP_HSL_COLOR;
    case CAIRO_OPERATOR_HSL_LUMINOSITY:
	return PIXMAN_OP_HSL_LUMINOSITY;

    default:
	ASSERT_NOT_REACHED;
	return PIXMAN_OP_OVER;
    }
}

static cairo_image_surface_t *
create_composite_mask (cairo_image_surface_t	*dst,
		       void			*draw_closure,
		       draw_func_t		 draw_func,
		       const cairo_composite_rectangles_t *extents)
{
    cairo_image_surface_t *surface;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    surface = (cairo_image_surface_t *)
	_cairo_image_surface_create_with_pixman_format (NULL, PIXMAN_a8,
							extents->bounded.width,
							extents->bounded.height,
							0);
    if (unlikely (surface->base.status))
	return surface;

    status = draw_func (surface, draw_closure,
			CAIRO_OPERATOR_ADD, &_cairo_pattern_white.base,
			extents->bounded.x, extents->bounded.y,
			&extents->bounded);
    if (unlikely (status))
	goto error;

    status = _cairo_clip_combine_with_surface (extents->clip,
					       &surface->base,
					       extents->bounded.x,
					       extents->bounded.y);
    if (unlikely (status))
	goto error;

    return surface;

error:
    cairo_surface_destroy (&surface->base);
    return (cairo_image_surface_t *)_cairo_surface_create_in_error (status);
}

/* Handles compositing with a clip surface when the operator allows
 * us to combine the clip with the mask
 */
static cairo_status_t
clip_and_composite_with_mask (const cairo_composite_rectangles_t*extents,
			      draw_func_t		 draw_func,
			      void			*draw_closure)
{
    cairo_image_surface_t *dst = (cairo_image_surface_t *)extents->surface;
    cairo_image_surface_t *mask;
    pixman_image_t *src;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;
    int src_x, src_y;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    mask = create_composite_mask (dst, draw_closure, draw_func, extents);
    if (unlikely (mask->base.status))
	return mask->base.status;

    src = _pixman_image_for_pattern (dst,
				     &extents->source_pattern.base, FALSE,
				     &extents->bounded,
				     &extents->source_sample_area,
				     &src_x, &src_y);
    if (unlikely (src == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto error;
    }

    pixman_image_composite32 (_pixman_operator (extents->op),
			      src, mask->pixman_image, dst->pixman_image,
			      extents->bounded.x + src_x,
			      extents->bounded.y + src_y,
			      0, 0,
			      extents->bounded.x,      extents->bounded.y,
			      extents->bounded.width,  extents->bounded.height);

    pixman_image_unref (src);
error:
    cairo_surface_destroy (&mask->base);
    return status;
}

/* Handles compositing with a clip surface when we have to do the operation
 * in two pieces and combine them together.
 */
static cairo_status_t
clip_and_composite_combine (const cairo_composite_rectangles_t*extents,
			    draw_func_t			 draw_func,
			    void			*draw_closure)
{
    cairo_image_surface_t *dst = (cairo_image_surface_t *)extents->surface;
    cairo_image_surface_t *tmp, *clip;
    int clip_x, clip_y;
    cairo_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    tmp = (cairo_image_surface_t *)
	_cairo_image_surface_create_with_pixman_format (NULL,
							dst->pixman_format,
							extents->bounded.width,
							extents->bounded.height,
							0);
    if (unlikely (tmp->base.status))
	return tmp->base.status;

    pixman_image_composite32 (PIXMAN_OP_SRC,
			      dst->pixman_image, NULL, tmp->pixman_image,
			      extents->bounded.x,      extents->bounded.y,
			      0, 0,
			      0, 0,
			      extents->bounded.width,  extents->bounded.height);

    status = draw_func (tmp, draw_closure,
			extents->op, &extents->source_pattern.base,
			extents->bounded.x, extents->bounded.y,
			&extents->bounded);
    if (unlikely (status))
	goto error;

    clip = (cairo_image_surface_t *)
	_cairo_clip_get_surface (extents->clip, &dst->base, &clip_x, &clip_y);
    if (unlikely (clip->base.status))
	goto error;

    pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
			      clip->pixman_image, NULL, dst->pixman_image,
			      extents->bounded.x - clip_x, extents->bounded.y - clip_y,
			      0,      0,
			      extents->bounded.x, extents->bounded.y,
			      extents->bounded.width, extents->bounded.height);
    pixman_image_composite32 (PIXMAN_OP_ADD,
			      tmp->pixman_image, clip->pixman_image, dst->pixman_image,
			      0,  0,
			      extents->bounded.x - clip_x, extents->bounded.y - clip_y,
			      extents->bounded.x, extents->bounded.y,
			      extents->bounded.width, extents->bounded.height);

    cairo_surface_destroy (&clip->base);

 error:
    cairo_surface_destroy (&tmp->base);

    return status;
}

/* Handles compositing for %CAIRO_OPERATOR_SOURCE, which is special; it's
 * defined as (src IN mask IN clip) ADD (dst OUT (mask IN clip))
 */
static cairo_status_t
clip_and_composite_source (const cairo_composite_rectangles_t	*extents,
			   draw_func_t				 draw_func,
			   void					*draw_closure)
{
    cairo_image_surface_t *dst = (cairo_image_surface_t *)extents->surface;
    cairo_image_surface_t *mask;
    pixman_image_t *src;
    int src_x, src_y;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    mask = create_composite_mask (dst, draw_closure, draw_func, extents);
    if (unlikely (mask->base.status))
	return mask->base.status;

    pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
			      mask->pixman_image, NULL, dst->pixman_image,
			      0,      0,
			      0,      0,
			      extents->bounded.x, extents->bounded.y,
			      extents->bounded.width, extents->bounded.height);

    src = _pixman_image_for_pattern (dst,
				     &extents->source_pattern.base, FALSE,
				     &extents->bounded,
				     &extents->source_sample_area,
				     &src_x, &src_y);
    if (unlikely (src == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto error;
    }

    pixman_image_composite32 (PIXMAN_OP_ADD,
			      src, mask->pixman_image, dst->pixman_image,
			      extents->bounded.x + src_x,  extents->bounded.y + src_y,
			      0, 0,
			      extents->bounded.x, extents->bounded.y,
			      extents->bounded.width, extents->bounded.height);

    pixman_image_unref (src);

error:
    cairo_surface_destroy (&mask->base);
    return status;
}

static cairo_status_t
fixup_unbounded (const cairo_composite_rectangles_t *extents)
{
    cairo_image_surface_t *dst = (cairo_image_surface_t *)extents->surface;
    pixman_image_t *mask;
    int mask_x, mask_y;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (! _cairo_clip_is_region (extents->clip)) {
	cairo_image_surface_t *clip;

	clip = (cairo_image_surface_t *)
	    _cairo_clip_get_surface (extents->clip, &dst->base,
				     &mask_x, &mask_y);
	if (unlikely (clip->base.status))
	    return clip->base.status;

	mask = pixman_image_ref (clip->pixman_image);
	cairo_surface_destroy (&clip->base);
    } else {
	mask_x = mask_y = 0;
	mask = _pixman_image_for_color (CAIRO_COLOR_WHITE);
	if (unlikely (mask == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    /* top */
    if (extents->bounded.y != extents->unbounded.y) {
	int x = extents->unbounded.x;
	int y = extents->unbounded.y;
	int width = extents->unbounded.width;
	int height = extents->bounded.y - y;

	pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
				  mask, NULL, dst->pixman_image,
				  x - mask_x, y - mask_y,
				  0, 0,
				  x, y,
				  width, height);
    }

    /* left */
    if (extents->bounded.x != extents->unbounded.x) {
	int x = extents->unbounded.x;
	int y = extents->bounded.y;
	int width = extents->bounded.x - x;
	int height = extents->bounded.height;

	pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
				  mask, NULL, dst->pixman_image,
				  x - mask_x, y - mask_y,
				  0, 0,
				  x, y,
				  width, height);
    }

    /* right */
    if (extents->bounded.x + extents->bounded.width != extents->unbounded.x + extents->unbounded.width) {
	int x = extents->bounded.x + extents->bounded.width;
	int y = extents->bounded.y;
	int width = extents->unbounded.x + extents->unbounded.width - x;
	int height = extents->bounded.height;

	pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
				  mask, NULL, dst->pixman_image,
				  x - mask_x, y - mask_y,
				  0, 0,
				  x, y,
				  width, height);
    }

    /* bottom */
    if (extents->bounded.y + extents->bounded.height != extents->unbounded.y + extents->unbounded.height) {
	int x = extents->unbounded.x;
	int y = extents->bounded.y + extents->bounded.height;
	int width = extents->unbounded.width;
	int height = extents->unbounded.y + extents->unbounded.height - y;

	pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
				  mask, NULL, dst->pixman_image,
				  x - mask_x, y - mask_y,
				  0, 0,
				  x, y,
				  width, height);
    }

    pixman_image_unref (mask);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
set_clip_region (cairo_composite_rectangles_t *extents)
{
    cairo_image_surface_t *dst = (cairo_image_surface_t *) extents->surface;
    cairo_region_t *region = _cairo_clip_get_region (extents->clip);
    pixman_region32_t *rgn = region ? &region->rgn : NULL;
    if (! pixman_image_set_clip_region32 (dst->pixman_image, rgn))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
clip_and_composite (cairo_composite_rectangles_t *extents,
		    draw_func_t		 draw_func,
		    void		*draw_closure)
{
    cairo_status_t status;

    status = set_clip_region (extents);
    if (unlikely (status))
	return status;

    if (extents->op == CAIRO_OPERATOR_SOURCE) {
	status = clip_and_composite_source (extents, draw_func, draw_closure);
    } else {
	if (extents->op == CAIRO_OPERATOR_CLEAR) {
	    extents->source_pattern.solid = _cairo_pattern_white;
	    extents->op = CAIRO_OPERATOR_DEST_OUT;
	}
	if (! _cairo_clip_is_region (extents->clip)) {
	    if (extents->is_bounded)
		status = clip_and_composite_with_mask (extents, draw_func, draw_closure);
	    else
		status = clip_and_composite_combine (extents, draw_func, draw_closure);
	} else {
	    status = draw_func ((cairo_image_surface_t *) extents->surface,
				draw_closure,
				extents->op,
				&extents->source_pattern.base,
				0, 0,
				&extents->bounded);
	}
    }

    if (status == CAIRO_STATUS_SUCCESS && ! extents->is_bounded)
	status = fixup_unbounded (extents);

    return status;
}

/* high-level compositor interface */

static cairo_int_status_t
composite_paint (cairo_image_surface_t		*dst,
		 void				*closure,
		 cairo_operator_t		 op,
		 const cairo_pattern_t		*pattern,
		 int				 dst_x,
		 int				 dst_y,
		 const cairo_rectangle_int_t	*extents)
{
    cairo_rectangle_int_t sample;
    pixman_image_t *src;
    int src_x, src_y;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    _cairo_pattern_sampled_area (pattern, extents, &sample);
    src = _pixman_image_for_pattern (dst,
				     pattern, FALSE,
				     extents, &sample,
				     &src_x, &src_y);
    if (unlikely (src == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    TRACE ((stderr, "%s: src=(%d, %d), dst=(%d, %d) size=%dx%d\n", __FUNCTION__,
	    extents->x + src_x, extents->y + src_y,
	    extents->x - dst_x, extents->y - dst_y,
	    extents->width, extents->height));

    pixman_image_composite32 (_pixman_operator (op),
			      src, NULL, dst->pixman_image,
			      extents->x + src_x, extents->y + src_y,
			      0, 0,
			      extents->x - dst_x, extents->y - dst_y,
			      extents->width, extents->height);

    pixman_image_unref (src);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
base_compositor_paint (const cairo_compositor_t *_compositor,
		       cairo_composite_rectangles_t *extents)
{
    TRACE ((stderr, "%s\n", __FUNCTION__));
    return clip_and_composite (extents, composite_paint, NULL);
}

static cairo_int_status_t
composite_mask (cairo_image_surface_t		*dst,
		void				*closure,
		cairo_operator_t		 op,
		const cairo_pattern_t		*pattern,
		int				 dst_x,
		int				 dst_y,
		const cairo_rectangle_int_t	 *extents)
{
    cairo_rectangle_int_t sample;
    pixman_image_t *src, *mask;
    int src_x, src_y;
    int mask_x, mask_y;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    _cairo_pattern_sampled_area (pattern, extents, &sample);
    src = _pixman_image_for_pattern (dst, pattern, FALSE,
				     extents, &sample,
				     &src_x, &src_y);
    if (unlikely (src == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    _cairo_pattern_sampled_area (closure, extents, &sample);
    mask = _pixman_image_for_pattern (dst, closure, TRUE,
				      extents, &sample,
				      &mask_x, &mask_y);
    if (unlikely (mask == NULL)) {
	pixman_image_unref (src);
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    pixman_image_composite32 (_pixman_operator (op),
			      src, mask, dst->pixman_image,
			      extents->x + src_x, extents->y + src_y,
			      extents->x + mask_x, extents->y + mask_y,
			      extents->x - dst_x, extents->y - dst_y,
			      extents->width, extents->height);

    pixman_image_unref (mask);
    pixman_image_unref (src);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
base_compositor_mask (const cairo_compositor_t *_compositor,
		      cairo_composite_rectangles_t *extents)
{
    TRACE ((stderr, "%s\n", __FUNCTION__));
    return clip_and_composite (extents, composite_mask, &extents->mask_pattern.base);
}

typedef struct {
    cairo_traps_t traps;
    cairo_antialias_t antialias;
} composite_traps_info_t;

static cairo_int_status_t
composite_traps (cairo_image_surface_t	*dst,
		 void			*closure,
		 cairo_operator_t	 op,
		 const cairo_pattern_t	*pattern,
		 int			 dst_x,
		 int			 dst_y,
		 const cairo_rectangle_int_t *extents)
{
    composite_traps_info_t *info = closure;
    cairo_rectangle_int_t sample;
    pixman_image_t *src, *mask;
    int src_x, src_y;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    _cairo_pattern_sampled_area (pattern, extents, &sample);
    src = _pixman_image_for_pattern (dst, pattern, FALSE,
				     extents, &sample,
				     &src_x, &src_y);
    if (unlikely (src == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    mask = pixman_image_create_bits (info->antialias == CAIRO_ANTIALIAS_NONE ? PIXMAN_a1 : PIXMAN_a8,
				     extents->width, extents->height,
				     NULL, 0);
    if (unlikely (mask == NULL)) {
	pixman_image_unref (src);
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    _pixman_image_add_traps (mask, extents->x, extents->y, &info->traps);
    pixman_image_composite32 (_pixman_operator (op),
                              src, mask, dst->pixman_image,
                              extents->x + src_x - dst_x, extents->y + src_y - dst_y,
                              0, 0,
                              extents->x - dst_x, extents->y - dst_y,
                              extents->width, extents->height);

    pixman_image_unref (mask);
    pixman_image_unref (src);

    return  CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
trim_extents_to_traps (cairo_composite_rectangles_t *extents,
		       cairo_traps_t *traps)
{
    cairo_box_t box;

    /* X trims the affected area to the extents of the trapezoids, so
     * we need to compensate when fixing up the unbounded area.
    */
    _cairo_traps_extents (traps, &box);
    return _cairo_composite_rectangles_intersect_mask_extents (extents, &box);
}

static cairo_int_status_t
base_compositor_stroke (const cairo_compositor_t *_compositor,
			cairo_composite_rectangles_t *extents,
			const cairo_path_fixed_t *path,
			const cairo_stroke_style_t *style,
			const cairo_matrix_t	*ctm,
			const cairo_matrix_t	*ctm_inverse,
			double			 tolerance,
			cairo_antialias_t	 antialias)
{
    composite_traps_info_t info;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    info.antialias = antialias;
    _cairo_traps_init_with_clip (&info.traps, extents->clip);
    status = _cairo_path_fixed_stroke_polygon_to_traps (path, style,
							ctm, ctm_inverse,
							tolerance,
							&info.traps);
    if (likely (status == CAIRO_INT_STATUS_SUCCESS))
	status = trim_extents_to_traps (extents, &info.traps);
    if (likely (status == CAIRO_INT_STATUS_SUCCESS))
	status = clip_and_composite (extents, composite_traps, &info);
    _cairo_traps_fini (&info.traps);

    return status;
}

static cairo_int_status_t
base_compositor_fill (const cairo_compositor_t *_compositor,
		      cairo_composite_rectangles_t *extents,
		      const cairo_path_fixed_t	*path,
		      cairo_fill_rule_t		 fill_rule,
		      double			 tolerance,
		      cairo_antialias_t		 antialias)
{
    composite_traps_info_t info;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    info.antialias = antialias;
    _cairo_traps_init_with_clip (&info.traps, extents->clip);
    status = _cairo_path_fixed_fill_to_traps (path,
					      fill_rule, tolerance,
					      &info.traps);
    if (likely (status == CAIRO_INT_STATUS_SUCCESS))
	status = trim_extents_to_traps (extents, &info.traps);
    if (likely (status == CAIRO_INT_STATUS_SUCCESS))
	status = clip_and_composite (extents, composite_traps, &info);
    _cairo_traps_fini (&info.traps);

    return status;
}

static cairo_int_status_t
composite_glyphs (cairo_image_surface_t	*dst,
		  void			 *closure,
		  cairo_operator_t	 op,
		  const cairo_pattern_t	*pattern,
		  int			 dst_x,
		  int			 dst_y,
		  const cairo_rectangle_int_t *extents)
{
    cairo_composite_glyphs_info_t *info = closure;
    pixman_image_t *mask;
    cairo_status_t status;
    int i;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    mask = pixman_image_create_bits (PIXMAN_a8,
				     extents->width, extents->height,
				     NULL, 0);
    if (unlikely (mask == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    status = CAIRO_STATUS_SUCCESS;
    _cairo_scaled_font_freeze_cache (info->font);
    for (i = 0; i < info->num_glyphs; i++) {
	cairo_image_surface_t *glyph_surface;
	cairo_scaled_glyph_t *scaled_glyph;
	unsigned long glyph_index = info->glyphs[i].index;
	int x, y;

	status = _cairo_scaled_glyph_lookup (info->font, glyph_index,
					     CAIRO_SCALED_GLYPH_INFO_SURFACE,
					     NULL, /* foreground color */
					     &scaled_glyph);

	if (unlikely (status))
	    break;

	glyph_surface = scaled_glyph->surface;
	if (glyph_surface->width && glyph_surface->height) {
	    /* round glyph locations to the nearest pixel */
	    /* XXX: FRAGILE: We're ignoring device_transform scaling here. A bug? */
	    x = _cairo_lround (info->glyphs[i].x -
			       glyph_surface->base.device_transform.x0);
	    y = _cairo_lround (info->glyphs[i].y -
			       glyph_surface->base.device_transform.y0);

	    pixman_image_composite32 (PIXMAN_OP_ADD,
				      glyph_surface->pixman_image, NULL, mask,
				      0, 0,
                                      0, 0,
                                      x - extents->x, y - extents->y,
				      glyph_surface->width,
				      glyph_surface->height);
	}
    }
    _cairo_scaled_font_thaw_cache (info->font);

    if (status == CAIRO_STATUS_SUCCESS) {
	cairo_rectangle_int_t sample;
	pixman_image_t *src;
	int src_x, src_y;

	_cairo_pattern_sampled_area (pattern, extents, &sample);
	src = _pixman_image_for_pattern (dst, pattern, FALSE,
					 extents, &sample,
					 &src_x, &src_y);
	if (src != NULL) {
	    dst_x = extents->x - dst_x;
	    dst_y = extents->y - dst_y;
	    pixman_image_composite32 (_pixman_operator (op),
				      src, mask, dst->pixman_image,
				      src_x + dst_x,  src_y + dst_y,
				      0, 0,
				      dst_x, dst_y,
				      extents->width, extents->height);
	    pixman_image_unref (src);
	} else
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }
    pixman_image_unref (mask);

    return status;
}

static cairo_int_status_t
base_compositor_glyphs (const cairo_compositor_t	*_compositor,
			cairo_composite_rectangles_t	*extents,
			cairo_scaled_font_t		*scaled_font,
			cairo_glyph_t			*glyphs,
			int				 num_glyphs,
			cairo_bool_t			 overlap)
{
    cairo_composite_glyphs_info_t info;

    info.font = scaled_font;
    info.glyphs = glyphs;
    info.num_glyphs = num_glyphs;

    TRACE ((stderr, "%s\n", __FUNCTION__));
    return clip_and_composite (extents, composite_glyphs, &info);
}

static const cairo_compositor_t base_compositor = {
    &__cairo_no_compositor,

    base_compositor_paint,
    base_compositor_mask,
    base_compositor_stroke,
    base_compositor_fill,
    base_compositor_glyphs,
};

cairo_surface_t *
_cairo_test_base_compositor_surface_create (cairo_content_t	content,
					    int		width,
					    int		height)
{
    return test_compositor_surface_create (&base_compositor,
					   content, width, height);
}
