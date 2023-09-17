/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
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
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_COMPOSITOR_PRIVATE_H
#define CAIRO_COMPOSITOR_PRIVATE_H

#include "cairo-composite-rectangles-private.h"

CAIRO_BEGIN_DECLS

typedef struct {
    cairo_scaled_font_t *font;
    cairo_glyph_t *glyphs;
    int num_glyphs;
    cairo_bool_t use_mask;
    cairo_rectangle_int_t extents;
} cairo_composite_glyphs_info_t;

struct cairo_compositor {
    const cairo_compositor_t *delegate;

    cairo_warn cairo_int_status_t
    (*paint)			(const cairo_compositor_t	*compositor,
				 cairo_composite_rectangles_t	*extents);

    cairo_warn cairo_int_status_t
    (*mask)			(const cairo_compositor_t	*compositor,
				 cairo_composite_rectangles_t	*extents);

    cairo_warn cairo_int_status_t
    (*stroke)			(const cairo_compositor_t	*compositor,
				 cairo_composite_rectangles_t	*extents,
				 const cairo_path_fixed_t	*path,
				 const cairo_stroke_style_t	*style,
				 const cairo_matrix_t		*ctm,
				 const cairo_matrix_t		*ctm_inverse,
				 double				 tolerance,
				 cairo_antialias_t		 antialias);

    cairo_warn cairo_int_status_t
    (*fill)			(const cairo_compositor_t	*compositor,
				 cairo_composite_rectangles_t	*extents,
				 const cairo_path_fixed_t	*path,
				 cairo_fill_rule_t		 fill_rule,
				 double				 tolerance,
				 cairo_antialias_t		 antialias);

    cairo_warn cairo_int_status_t
    (*glyphs)			(const cairo_compositor_t	 *compositor,
				 cairo_composite_rectangles_t	*extents,
				 cairo_scaled_font_t		*scaled_font,
				 cairo_glyph_t			*glyphs,
				 int				 num_glyphs,
				 cairo_bool_t			 overlap);
};

struct cairo_mask_compositor {
    cairo_compositor_t base;

    cairo_int_status_t (*acquire) (void *surface);
    cairo_int_status_t (*release) (void *surface);

    cairo_int_status_t (*set_clip_region) (void		 *surface,
					   cairo_region_t	*clip_region);

    cairo_surface_t * (*pattern_to_surface) (cairo_surface_t *dst,
					     const cairo_pattern_t *pattern,
					     cairo_bool_t is_mask,
					     const cairo_rectangle_int_t *extents,
					     const cairo_rectangle_int_t *sample,
					     int *src_x, int *src_y);

    cairo_int_status_t (*draw_image_boxes) (void *surface,
					    cairo_image_surface_t *image,
					    cairo_boxes_t *boxes,
					    int dx, int dy);

    cairo_int_status_t (*copy_boxes) (void *surface,
				      cairo_surface_t *src,
				      cairo_boxes_t *boxes,
				      const cairo_rectangle_int_t *extents,
				      int dx, int dy);

    cairo_int_status_t
	(*fill_rectangles)	(void			 *surface,
				 cairo_operator_t	  op,
				 const cairo_color_t     *color,
				 cairo_rectangle_int_t   *rectangles,
				 int			  num_rects);

    cairo_int_status_t
	(*fill_boxes)		(void			*surface,
				 cairo_operator_t	 op,
				 const cairo_color_t	*color,
				 cairo_boxes_t		*boxes);

    cairo_int_status_t
	(*check_composite) (const cairo_composite_rectangles_t *extents);

    cairo_int_status_t
	(*composite)		(void			*dst,
				 cairo_operator_t	 op,
				 cairo_surface_t	*src,
				 cairo_surface_t	*mask,
				 int			 src_x,
				 int			 src_y,
				 int			 mask_x,
				 int			 mask_y,
				 int			 dst_x,
				 int			 dst_y,
				 unsigned int		 width,
				 unsigned int		 height);

    cairo_int_status_t
	(*composite_boxes)	(void			*surface,
				 cairo_operator_t	 op,
				 cairo_surface_t	*source,
				 cairo_surface_t	*mask,
				 int			 src_x,
				 int			 src_y,
				 int			 mask_x,
				 int			 mask_y,
				 int			 dst_x,
				 int			 dst_y,
				 cairo_boxes_t		*boxes,
				 const cairo_rectangle_int_t  *extents);

    cairo_int_status_t
	(*check_composite_glyphs) (const cairo_composite_rectangles_t *extents,
				   cairo_scaled_font_t *scaled_font,
				   cairo_glyph_t *glyphs,
				   int *num_glyphs);
    cairo_int_status_t
	(*composite_glyphs)	(void				*surface,
				 cairo_operator_t		 op,
				 cairo_surface_t		*src,
				 int				 src_x,
				 int				 src_y,
				 int				 dst_x,
				 int				 dst_y,
				 cairo_composite_glyphs_info_t  *info);
};

struct cairo_traps_compositor {
    cairo_compositor_t base;

    cairo_int_status_t
	(*acquire) (void *surface);

    cairo_int_status_t
	(*release) (void *surface);

    cairo_int_status_t
	(*set_clip_region) (void		 *surface,
			    cairo_region_t	*clip_region);

    cairo_surface_t *
	(*pattern_to_surface) (cairo_surface_t *dst,
			       const cairo_pattern_t *pattern,
			       cairo_bool_t is_mask,
			       const cairo_rectangle_int_t *extents,
			       const cairo_rectangle_int_t *sample,
			       int *src_x, int *src_y);

    cairo_int_status_t (*draw_image_boxes) (void *surface,
					    cairo_image_surface_t *image,
					    cairo_boxes_t *boxes,
					    int dx, int dy);

    cairo_int_status_t (*copy_boxes) (void *surface,
				      cairo_surface_t *src,
				      cairo_boxes_t *boxes,
				      const cairo_rectangle_int_t *extents,
				      int dx, int dy);

    cairo_int_status_t
	(*fill_boxes)		(void			*surface,
				 cairo_operator_t	 op,
				 const cairo_color_t	*color,
				 cairo_boxes_t		*boxes);

    cairo_int_status_t
	(*check_composite) (const cairo_composite_rectangles_t *extents);

    cairo_int_status_t
	(*composite)		(void			*dst,
				 cairo_operator_t	 op,
				 cairo_surface_t	*src,
				 cairo_surface_t	*mask,
				 int			 src_x,
				 int			 src_y,
				 int			 mask_x,
				 int			 mask_y,
				 int			 dst_x,
				 int			 dst_y,
				 unsigned int		 width,
				 unsigned int		 height);
    cairo_int_status_t
	    (*lerp)		(void			*_dst,
				 cairo_surface_t	*abstract_src,
				 cairo_surface_t	*abstract_mask,
				 int			src_x,
				 int			src_y,
				 int			mask_x,
				 int			mask_y,
				 int			dst_x,
				 int			dst_y,
				 unsigned int		width,
				 unsigned int		height);

    cairo_int_status_t
	(*composite_boxes)	(void			*surface,
				 cairo_operator_t	 op,
				 cairo_surface_t	*source,
				 cairo_surface_t	*mask,
				 int			 src_x,
				 int			 src_y,
				 int			 mask_x,
				 int			 mask_y,
				 int			 dst_x,
				 int			 dst_y,
				 cairo_boxes_t		*boxes,
				 const cairo_rectangle_int_t  *extents);

    cairo_int_status_t
	(*composite_traps)	(void			*dst,
				 cairo_operator_t	 op,
				 cairo_surface_t	*source,
				 int			 src_x,
				 int			 src_y,
				 int			 dst_x,
				 int			 dst_y,
				 const cairo_rectangle_int_t *extents,
				 cairo_antialias_t	 antialias,
				 cairo_traps_t		*traps);

    cairo_int_status_t
	(*composite_tristrip)	(void			*dst,
				 cairo_operator_t	 op,
				 cairo_surface_t	*source,
				 int			 src_x,
				 int			 src_y,
				 int			 dst_x,
				 int			 dst_y,
				 const cairo_rectangle_int_t *extents,
				 cairo_antialias_t	 antialias,
				 cairo_tristrip_t	*tristrip);

    cairo_int_status_t
	(*check_composite_glyphs) (const cairo_composite_rectangles_t *extents,
				   cairo_scaled_font_t *scaled_font,
				   cairo_glyph_t *glyphs,
				   int *num_glyphs);
    cairo_int_status_t
	(*composite_glyphs)	(void				*surface,
				 cairo_operator_t		 op,
				 cairo_surface_t		*src,
				 int				 src_x,
				 int				 src_y,
				 int				 dst_x,
				 int				 dst_y,
				 cairo_composite_glyphs_info_t  *info);
};

cairo_private extern const cairo_compositor_t __cairo_no_compositor;
cairo_private extern const cairo_compositor_t _cairo_fallback_compositor;

cairo_private void
_cairo_mask_compositor_init (cairo_mask_compositor_t *compositor,
			     const cairo_compositor_t *delegate);

cairo_private void
_cairo_shape_mask_compositor_init (cairo_compositor_t *compositor,
				   const cairo_compositor_t  *delegate);

cairo_private void
_cairo_traps_compositor_init (cairo_traps_compositor_t *compositor,
			      const cairo_compositor_t *delegate);

cairo_private cairo_int_status_t
_cairo_compositor_paint (const cairo_compositor_t	*compositor,
			 cairo_surface_t		*surface,
			 cairo_operator_t		 op,
			 const cairo_pattern_t		*source,
			 const cairo_clip_t		*clip);

cairo_private cairo_int_status_t
_cairo_compositor_mask (const cairo_compositor_t	*compositor,
			cairo_surface_t			*surface,
			cairo_operator_t		 op,
			const cairo_pattern_t		*source,
			const cairo_pattern_t		*mask,
			const cairo_clip_t		*clip);

cairo_private cairo_int_status_t
_cairo_compositor_stroke (const cairo_compositor_t	*compositor,
			  cairo_surface_t		*surface,
			  cairo_operator_t		 op,
			  const cairo_pattern_t		*source,
			  const cairo_path_fixed_t	*path,
			  const cairo_stroke_style_t	*style,
			  const cairo_matrix_t		*ctm,
			  const cairo_matrix_t		*ctm_inverse,
			  double			 tolerance,
			  cairo_antialias_t		 antialias,
			  const cairo_clip_t		*clip);

cairo_private cairo_int_status_t
_cairo_compositor_fill (const cairo_compositor_t	*compositor,
			cairo_surface_t			*surface,
			cairo_operator_t		 op,
			const cairo_pattern_t		*source,
			const cairo_path_fixed_t	*path,
			cairo_fill_rule_t		 fill_rule,
			double				 tolerance,
			cairo_antialias_t		 antialias,
			const cairo_clip_t		*clip);

cairo_private cairo_int_status_t
_cairo_compositor_glyphs (const cairo_compositor_t		*compositor,
			  cairo_surface_t			*surface,
			  cairo_operator_t			 op,
			  const cairo_pattern_t			*source,
			  cairo_glyph_t				*glyphs,
			  int					 num_glyphs,
			  cairo_scaled_font_t			*scaled_font,
			  const cairo_clip_t			*clip);

CAIRO_END_DECLS

#endif /* CAIRO_COMPOSITOR_PRIVATE_H */
