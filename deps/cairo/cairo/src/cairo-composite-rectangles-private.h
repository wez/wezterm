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
 * The Initial Developer of the Original Code is University of Southern
 * California.
 *
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilson.co.u>
 */

#ifndef CAIRO_COMPOSITE_RECTANGLES_PRIVATE_H
#define CAIRO_COMPOSITE_RECTANGLES_PRIVATE_H

#include "cairo-types-private.h"
#include "cairo-error-private.h"
#include "cairo-pattern-private.h"

CAIRO_BEGIN_DECLS

/* Rectangles that take part in a composite operation.
 *
 * The source and mask track the extents of the respective patterns in device
 * space. The unbounded rectangle is essentially the clip rectangle. And the
 * intersection of all is the bounded rectangle, which is the minimum extents
 * the operation may require. Whether or not the operation is actually bounded
 * is tracked in the is_bounded boolean.
 *
 */
struct _cairo_composite_rectangles {
    cairo_surface_t *surface;
    cairo_operator_t op;

    cairo_rectangle_int_t source;
    cairo_rectangle_int_t mask;
    cairo_rectangle_int_t destination;

    cairo_rectangle_int_t bounded; /* source? IN mask? IN unbounded */
    cairo_rectangle_int_t unbounded; /* destination IN clip */
    uint32_t is_bounded;

    cairo_rectangle_int_t source_sample_area;
    cairo_rectangle_int_t mask_sample_area;

    cairo_pattern_union_t source_pattern;
    cairo_pattern_union_t mask_pattern;
    const cairo_pattern_t *original_source_pattern;
    const cairo_pattern_t *original_mask_pattern;

    cairo_clip_t *clip; /* clip will be reduced to the minimal container */
};

cairo_private cairo_int_status_t
_cairo_composite_rectangles_init_for_paint (cairo_composite_rectangles_t *extents,
					    cairo_surface_t *surface,
					    cairo_operator_t	 op,
					    const cairo_pattern_t	*source,
					    const cairo_clip_t		*clip);

cairo_private cairo_int_status_t
_cairo_composite_rectangles_init_for_mask (cairo_composite_rectangles_t *extents,
					   cairo_surface_t *surface,
					   cairo_operator_t	 op,
					   const cairo_pattern_t	*source,
					   const cairo_pattern_t	*mask,
					   const cairo_clip_t		*clip);

cairo_private cairo_int_status_t
_cairo_composite_rectangles_init_for_stroke (cairo_composite_rectangles_t *extents,
					     cairo_surface_t *surface,
					     cairo_operator_t	 op,
					     const cairo_pattern_t	*source,
					     const cairo_path_fixed_t	*path,
					     const cairo_stroke_style_t	*style,
					     const cairo_matrix_t	*ctm,
					     const cairo_clip_t		*clip);

cairo_private cairo_int_status_t
_cairo_composite_rectangles_init_for_fill (cairo_composite_rectangles_t *extents,
					   cairo_surface_t *surface,
					   cairo_operator_t	 op,
					   const cairo_pattern_t	*source,
					   const cairo_path_fixed_t	*path,
					   const cairo_clip_t		*clip);

cairo_private cairo_int_status_t
_cairo_composite_rectangles_init_for_boxes (cairo_composite_rectangles_t *extents,
					      cairo_surface_t		*surface,
					      cairo_operator_t		 op,
					      const cairo_pattern_t	*source,
					      const cairo_boxes_t	*boxes,
					      const cairo_clip_t		*clip);

cairo_private cairo_int_status_t
_cairo_composite_rectangles_init_for_polygon (cairo_composite_rectangles_t *extents,
					      cairo_surface_t		*surface,
					      cairo_operator_t		 op,
					      const cairo_pattern_t	*source,
					      const cairo_polygon_t	*polygon,
					      const cairo_clip_t		*clip);

cairo_private cairo_int_status_t
_cairo_composite_rectangles_init_for_glyphs (cairo_composite_rectangles_t *extents,
					     cairo_surface_t *surface,
					     cairo_operator_t		 op,
					     const cairo_pattern_t	*source,
					     cairo_scaled_font_t	*scaled_font,
					     cairo_glyph_t		*glyphs,
					     int			 num_glyphs,
					     const cairo_clip_t		*clip,
					     cairo_bool_t		*overlap);

cairo_private cairo_int_status_t
_cairo_composite_rectangles_intersect_source_extents (cairo_composite_rectangles_t *extents,
						      const cairo_box_t *box);

cairo_private cairo_int_status_t
_cairo_composite_rectangles_intersect_mask_extents (cairo_composite_rectangles_t *extents,
						    const cairo_box_t *box);

cairo_private cairo_bool_t
_cairo_composite_rectangles_can_reduce_clip (cairo_composite_rectangles_t *composite,
					     cairo_clip_t *clip);

cairo_private cairo_int_status_t
_cairo_composite_rectangles_add_to_damage (cairo_composite_rectangles_t *composite,
					   cairo_boxes_t *damage);

cairo_private void
_cairo_composite_rectangles_fini (cairo_composite_rectangles_t *extents);

CAIRO_END_DECLS

#endif /* CAIRO_COMPOSITE_RECTANGLES_PRIVATE_H */
