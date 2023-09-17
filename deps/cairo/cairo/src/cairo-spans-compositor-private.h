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

#ifndef CAIRO_SPANS_COMPOSITOR_PRIVATE_H
#define CAIRO_SPANS_COMPOSITOR_PRIVATE_H

#include "cairo-compositor-private.h"
#include "cairo-types-private.h"
#include "cairo-spans-private.h"

CAIRO_BEGIN_DECLS

typedef struct _cairo_abstract_span_renderer {
    cairo_span_renderer_t base;
    char data[4096];
} cairo_abstract_span_renderer_t;

struct cairo_spans_compositor {
    cairo_compositor_t base;

    unsigned int flags;
#define CAIRO_SPANS_COMPOSITOR_HAS_LERP 0x1

    /* pixel-aligned fast paths */
    cairo_int_status_t (*fill_boxes)	(void			*surface,
					 cairo_operator_t	 op,
					 const cairo_color_t	*color,
					 cairo_boxes_t		*boxes);

    cairo_int_status_t (*draw_image_boxes) (void *surface,
					    cairo_image_surface_t *image,
					    cairo_boxes_t *boxes,
					    int dx, int dy);

    cairo_int_status_t (*copy_boxes) (void *surface,
				      cairo_surface_t *src,
				      cairo_boxes_t *boxes,
				      const cairo_rectangle_int_t *extents,
				      int dx, int dy);

    cairo_surface_t * (*pattern_to_surface) (cairo_surface_t *dst,
					     const cairo_pattern_t *pattern,
					     cairo_bool_t is_mask,
					     const cairo_rectangle_int_t *extents,
					     const cairo_rectangle_int_t *sample,
					     int *src_x, int *src_y);

    cairo_int_status_t (*composite_boxes) (void			*surface,
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

    /* general shape masks using a span renderer */
    cairo_int_status_t (*renderer_init) (cairo_abstract_span_renderer_t *renderer,
					 const cairo_composite_rectangles_t *extents,
					 cairo_antialias_t antialias,
					 cairo_bool_t	 needs_clip);

    void (*renderer_fini) (cairo_abstract_span_renderer_t *renderer,
			   cairo_int_status_t status);
};

cairo_private void
_cairo_spans_compositor_init (cairo_spans_compositor_t *compositor,
			      const cairo_compositor_t  *delegate);

CAIRO_END_DECLS

#endif /* CAIRO_SPANS_COMPOSITOR_PRIVATE_H */
