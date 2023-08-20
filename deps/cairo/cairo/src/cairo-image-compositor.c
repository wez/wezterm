/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2003 University of Southern California
 * Copyright © 2009,2010,2011 Intel Corporation
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
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

/* The primarily reason for keeping a traps-compositor around is
 * for validating cairo-xlib (which currently also uses traps).
 */

#include "cairoint.h"

#include "cairo-image-surface-private.h"

#include "cairo-compositor-private.h"
#include "cairo-spans-compositor-private.h"

#include "cairo-region-private.h"
#include "cairo-traps-private.h"
#include "cairo-tristrip-private.h"

#include "cairo-pixman-private.h"

static pixman_image_t *
to_pixman_image (cairo_surface_t *s)
{
    return ((cairo_image_surface_t *)s)->pixman_image;
}

static cairo_int_status_t
acquire (void *abstract_dst)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
release (void *abstract_dst)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
set_clip_region (void *_surface,
		 cairo_region_t *region)
{
    cairo_image_surface_t *surface = _surface;
    pixman_region32_t *rgn = region ? &region->rgn : NULL;

    if (! pixman_image_set_clip_region32 (surface->pixman_image, rgn))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
draw_image_boxes (void *_dst,
		  cairo_image_surface_t *image,
		  cairo_boxes_t *boxes,
		  int dx, int dy)
{
    cairo_image_surface_t *dst = _dst;
    struct _cairo_boxes_chunk *chunk;
    int i;

    TRACE ((stderr, "%s x %d\n", __FUNCTION__, boxes->num_boxes));

    for (chunk = &boxes->chunks; chunk; chunk = chunk->next) {
	for (i = 0; i < chunk->count; i++) {
	    cairo_box_t *b = &chunk->base[i];
	    int x = _cairo_fixed_integer_part (b->p1.x);
	    int y = _cairo_fixed_integer_part (b->p1.y);
	    int w = _cairo_fixed_integer_part (b->p2.x) - x;
	    int h = _cairo_fixed_integer_part (b->p2.y) - y;
	    if (dst->pixman_format != image->pixman_format ||
		! pixman_blt ((uint32_t *)image->data, (uint32_t *)dst->data,
			      image->stride / sizeof (uint32_t),
			      dst->stride / sizeof (uint32_t),
			      PIXMAN_FORMAT_BPP (image->pixman_format),
			      PIXMAN_FORMAT_BPP (dst->pixman_format),
			      x + dx, y + dy,
			      x, y,
			      w, h))
	    {
		pixman_image_composite32 (PIXMAN_OP_SRC,
					  image->pixman_image, NULL, dst->pixman_image,
					  x + dx, y + dy,
					  0, 0,
					  x, y,
					  w, h);
	    }
	}
    }
    return CAIRO_STATUS_SUCCESS;
}

static inline uint32_t
color_to_uint32 (const cairo_color_t *color)
{
    return
        ((uint32_t)color->alpha_short >> 8 << 24) |
        (color->red_short >> 8 << 16)   |
        (color->green_short & 0xff00)   |
        (color->blue_short >> 8);
}

static inline cairo_bool_t
color_to_pixel (const cairo_color_t	*color,
                pixman_format_code_t	 format,
                uint32_t		*pixel)
{
    uint32_t c;

    if (!(format == PIXMAN_a8r8g8b8     ||
          format == PIXMAN_x8r8g8b8     ||
          format == PIXMAN_a8b8g8r8     ||
          format == PIXMAN_x8b8g8r8     ||
          format == PIXMAN_b8g8r8a8     ||
          format == PIXMAN_b8g8r8x8     ||
          format == PIXMAN_r5g6b5       ||
          format == PIXMAN_b5g6r5       ||
          format == PIXMAN_a8))
    {
	return FALSE;
    }

    c = color_to_uint32 (color);

    if (PIXMAN_FORMAT_TYPE (format) == PIXMAN_TYPE_ABGR) {
	c = ((c & 0xff000000) >>  0) |
	    ((c & 0x00ff0000) >> 16) |
	    ((c & 0x0000ff00) >>  0) |
	    ((c & 0x000000ff) << 16);
    }

    if (PIXMAN_FORMAT_TYPE (format) == PIXMAN_TYPE_BGRA) {
	c = ((c & 0xff000000) >> 24) |
	    ((c & 0x00ff0000) >>  8) |
	    ((c & 0x0000ff00) <<  8) |
	    ((c & 0x000000ff) << 24);
    }

    if (format == PIXMAN_a8) {
	c = c >> 24;
    } else if (format == PIXMAN_r5g6b5 || format == PIXMAN_b5g6r5) {
	c = ((((c) >> 3) & 0x001f) |
	     (((c) >> 5) & 0x07e0) |
	     (((c) >> 8) & 0xf800));
    }

    *pixel = c;
    return TRUE;
}

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

static cairo_bool_t
__fill_reduces_to_source (cairo_operator_t op,
			  const cairo_color_t *color,
			  const cairo_image_surface_t *dst)
{
    if (op == CAIRO_OPERATOR_SOURCE || op == CAIRO_OPERATOR_CLEAR)
	return TRUE;
    if (op == CAIRO_OPERATOR_OVER && CAIRO_COLOR_IS_OPAQUE (color))
	return TRUE;
    if (dst->base.is_clear)
	return op == CAIRO_OPERATOR_OVER || op == CAIRO_OPERATOR_ADD;

    return FALSE;
}

static cairo_bool_t
fill_reduces_to_source (cairo_operator_t op,
			const cairo_color_t *color,
			const cairo_image_surface_t *dst,
			uint32_t *pixel)
{
    if (__fill_reduces_to_source (op, color, dst)) {
	return color_to_pixel (color, dst->pixman_format, pixel);
    }

    return FALSE;
}

static cairo_int_status_t
fill_rectangles (void			*_dst,
		 cairo_operator_t	 op,
		 const cairo_color_t	*color,
		 cairo_rectangle_int_t	*rects,
		 int			 num_rects)
{
    cairo_image_surface_t *dst = _dst;
    uint32_t pixel;
    int i;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (fill_reduces_to_source (op, color, dst, &pixel)) {
	for (i = 0; i < num_rects; i++) {
	    pixman_fill ((uint32_t *) dst->data, dst->stride / sizeof (uint32_t),
			 PIXMAN_FORMAT_BPP (dst->pixman_format),
			 rects[i].x, rects[i].y,
			 rects[i].width, rects[i].height,
			 pixel);
	}
    } else {
	pixman_image_t *src = _pixman_image_for_color (color);
	if (unlikely (src == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	op = _pixman_operator (op);
	for (i = 0; i < num_rects; i++) {
	    pixman_image_composite32 (op,
				      src, NULL, dst->pixman_image,
				      0, 0,
				      0, 0,
				      rects[i].x, rects[i].y,
				      rects[i].width, rects[i].height);
	}

	pixman_image_unref (src);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
fill_boxes (void		*_dst,
	    cairo_operator_t	 op,
	    const cairo_color_t	*color,
	    cairo_boxes_t	*boxes)
{
    cairo_image_surface_t *dst = _dst;
    struct _cairo_boxes_chunk *chunk;
    uint32_t pixel;
    int i;

    TRACE ((stderr, "%s x %d\n", __FUNCTION__, boxes->num_boxes));

    if (fill_reduces_to_source (op, color, dst, &pixel)) {
	for (chunk = &boxes->chunks; chunk; chunk = chunk->next) {
	    for (i = 0; i < chunk->count; i++) {
		int x = _cairo_fixed_integer_part (chunk->base[i].p1.x);
		int y = _cairo_fixed_integer_part (chunk->base[i].p1.y);
		int w = _cairo_fixed_integer_part (chunk->base[i].p2.x) - x;
		int h = _cairo_fixed_integer_part (chunk->base[i].p2.y) - y;
		pixman_fill ((uint32_t *) dst->data,
			     dst->stride / sizeof (uint32_t),
			     PIXMAN_FORMAT_BPP (dst->pixman_format),
			     x, y, w, h, pixel);
	    }
	}
    }
    else
    {
	pixman_image_t *src = _pixman_image_for_color (color);
	if (unlikely (src == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	op = _pixman_operator (op);
	for (chunk = &boxes->chunks; chunk; chunk = chunk->next) {
	    for (i = 0; i < chunk->count; i++) {
		int x1 = _cairo_fixed_integer_part (chunk->base[i].p1.x);
		int y1 = _cairo_fixed_integer_part (chunk->base[i].p1.y);
		int x2 = _cairo_fixed_integer_part (chunk->base[i].p2.x);
		int y2 = _cairo_fixed_integer_part (chunk->base[i].p2.y);
		pixman_image_composite32 (op,
					  src, NULL, dst->pixman_image,
					  0, 0,
					  0, 0,
					  x1, y1,
					  x2-x1, y2-y1);
	    }
	}

	pixman_image_unref (src);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
composite (void			*_dst,
	   cairo_operator_t	op,
	   cairo_surface_t	*abstract_src,
	   cairo_surface_t	*abstract_mask,
	   int			src_x,
	   int			src_y,
	   int			mask_x,
	   int			mask_y,
	   int			dst_x,
	   int			dst_y,
	   unsigned int		width,
	   unsigned int		height)
{
    cairo_image_source_t *src = (cairo_image_source_t *)abstract_src;
    cairo_image_source_t *mask = (cairo_image_source_t *)abstract_mask;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (mask) {
	pixman_image_composite32 (_pixman_operator (op),
				  src->pixman_image, mask->pixman_image, to_pixman_image (_dst),
				  src_x, src_y,
				  mask_x, mask_y,
				  dst_x, dst_y,
				  width, height);
    } else {
	pixman_image_composite32 (_pixman_operator (op),
				  src->pixman_image, NULL, to_pixman_image (_dst),
				  src_x, src_y,
				  0, 0,
				  dst_x, dst_y,
				  width, height);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
lerp (void			*_dst,
      cairo_surface_t		*abstract_src,
      cairo_surface_t		*abstract_mask,
      int			src_x,
      int			src_y,
      int			mask_x,
      int			mask_y,
      int			dst_x,
      int			dst_y,
      unsigned int		width,
      unsigned int		height)
{
    cairo_image_surface_t *dst = _dst;
    cairo_image_source_t *src = (cairo_image_source_t *)abstract_src;
    cairo_image_source_t *mask = (cairo_image_source_t *)abstract_mask;

    TRACE ((stderr, "%s\n", __FUNCTION__));

#if PIXMAN_HAS_OP_LERP
    pixman_image_composite32 (PIXMAN_OP_LERP_SRC,
			      src->pixman_image, mask->pixman_image, dst->pixman_image,
			      src_x,  src_y,
			      mask_x, mask_y,
			      dst_x,  dst_y,
			      width,  height);
#else
    /* Punch the clip out of the destination */
    TRACE ((stderr, "%s - OUT_REVERSE (mask=%d/%p, dst=%d/%p)\n",
	    __FUNCTION__,
	    mask->base.unique_id, mask->pixman_image,
	    dst->base.unique_id, dst->pixman_image));
    pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
			      mask->pixman_image, NULL, dst->pixman_image,
			      mask_x, mask_y,
			      0,      0,
			      dst_x,  dst_y,
			      width,  height);

    /* Now add the two results together */
    TRACE ((stderr, "%s - ADD (src=%d/%p, mask=%d/%p, dst=%d/%p)\n",
	    __FUNCTION__,
	    src->base.unique_id, src->pixman_image,
	    mask->base.unique_id, mask->pixman_image,
	    dst->base.unique_id, dst->pixman_image));
    pixman_image_composite32 (PIXMAN_OP_ADD,
			      src->pixman_image, mask->pixman_image, dst->pixman_image,
			      src_x,  src_y,
			      mask_x, mask_y,
			      dst_x,  dst_y,
			      width,  height);
#endif

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
composite_boxes (void			*_dst,
		 cairo_operator_t	op,
		 cairo_surface_t	*abstract_src,
		 cairo_surface_t	*abstract_mask,
		 int			src_x,
		 int			src_y,
		 int			mask_x,
		 int			mask_y,
		 int			dst_x,
		 int			dst_y,
		 cairo_boxes_t		*boxes,
		 const cairo_rectangle_int_t  *extents)
{
    pixman_image_t *dst = to_pixman_image (_dst);
    pixman_image_t *src = ((cairo_image_source_t *)abstract_src)->pixman_image;
    pixman_image_t *mask = abstract_mask ? ((cairo_image_source_t *)abstract_mask)->pixman_image : NULL;
    pixman_image_t *free_src = NULL;
    struct _cairo_boxes_chunk *chunk;
    int i;

    /* XXX consider using a region? saves multiple prepare-composite */
    TRACE ((stderr, "%s x %d\n", __FUNCTION__, boxes->num_boxes));

    if (((cairo_surface_t *)_dst)->is_clear &&
	(op == CAIRO_OPERATOR_SOURCE ||
	 op == CAIRO_OPERATOR_OVER ||
	 op == CAIRO_OPERATOR_ADD)) {
	op = PIXMAN_OP_SRC;
    } else if (mask) {
	if (op == CAIRO_OPERATOR_CLEAR) {
#if PIXMAN_HAS_OP_LERP
	    op = PIXMAN_OP_LERP_CLEAR;
#else
	    free_src = src = _pixman_image_for_color (CAIRO_COLOR_WHITE);
	    if (unlikely (src == NULL))
		return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    op = PIXMAN_OP_OUT_REVERSE;
#endif
	} else if (op == CAIRO_OPERATOR_SOURCE) {
#if PIXMAN_HAS_OP_LERP
	    op = PIXMAN_OP_LERP_SRC;
#else
	    return CAIRO_INT_STATUS_UNSUPPORTED;
#endif
	} else {
	    op = _pixman_operator (op);
	}
    } else {
	op = _pixman_operator (op);
    }

    for (chunk = &boxes->chunks; chunk; chunk = chunk->next) {
	for (i = 0; i < chunk->count; i++) {
	    int x1 = _cairo_fixed_integer_part (chunk->base[i].p1.x);
	    int y1 = _cairo_fixed_integer_part (chunk->base[i].p1.y);
	    int x2 = _cairo_fixed_integer_part (chunk->base[i].p2.x);
	    int y2 = _cairo_fixed_integer_part (chunk->base[i].p2.y);

	    pixman_image_composite32 (op, src, mask, dst,
				      x1 + src_x, y1 + src_y,
				      x1 + mask_x, y1 + mask_y,
				      x1 + dst_x, y1 + dst_y,
				      x2 - x1, y2 - y1);
	}
    }

    if (free_src)
	pixman_image_unref (free_src);

    return CAIRO_STATUS_SUCCESS;
}

#define CAIRO_FIXED_16_16_MIN _cairo_fixed_from_int (-32768)
#define CAIRO_FIXED_16_16_MAX _cairo_fixed_from_int (32767)

static cairo_bool_t
line_exceeds_16_16 (const cairo_line_t *line)
{
    return
	line->p1.x <= CAIRO_FIXED_16_16_MIN ||
	line->p1.x >= CAIRO_FIXED_16_16_MAX ||

	line->p2.x <= CAIRO_FIXED_16_16_MIN ||
	line->p2.x >= CAIRO_FIXED_16_16_MAX ||

	line->p1.y <= CAIRO_FIXED_16_16_MIN ||
	line->p1.y >= CAIRO_FIXED_16_16_MAX ||

	line->p2.y <= CAIRO_FIXED_16_16_MIN ||
	line->p2.y >= CAIRO_FIXED_16_16_MAX;
}

static void
project_line_x_onto_16_16 (const cairo_line_t *line,
			   cairo_fixed_t top,
			   cairo_fixed_t bottom,
			   pixman_line_fixed_t *out)
{
    /* XXX use fixed-point arithmetic? */
    cairo_point_double_t p1, p2;
    double m;

    p1.x = _cairo_fixed_to_double (line->p1.x);
    p1.y = _cairo_fixed_to_double (line->p1.y);

    p2.x = _cairo_fixed_to_double (line->p2.x);
    p2.y = _cairo_fixed_to_double (line->p2.y);

    m = (p2.x - p1.x) / (p2.y - p1.y);
    out->p1.x = _cairo_fixed_16_16_from_double (p1.x + m * _cairo_fixed_to_double (top - line->p1.y));
    out->p2.x = _cairo_fixed_16_16_from_double (p1.x + m * _cairo_fixed_to_double (bottom - line->p1.y));
}

void
_pixman_image_add_traps (pixman_image_t *image,
			 int dst_x, int dst_y,
			 cairo_traps_t *traps)
{
    cairo_trapezoid_t *t = traps->traps;
    int num_traps = traps->num_traps;
    while (num_traps--) {
	pixman_trapezoid_t trap;

	/* top/bottom will be clamped to surface bounds */
	trap.top = _cairo_fixed_to_16_16 (t->top);
	trap.bottom = _cairo_fixed_to_16_16 (t->bottom);

	/* However, all the other coordinates will have been left untouched so
	 * as not to introduce numerical error. Recompute them if they
	 * exceed the 16.16 limits.
	 */
	if (unlikely (line_exceeds_16_16 (&t->left))) {
	    project_line_x_onto_16_16 (&t->left, t->top, t->bottom, &trap.left);
	    trap.left.p1.y = trap.top;
	    trap.left.p2.y = trap.bottom;
	} else {
	    trap.left.p1.x = _cairo_fixed_to_16_16 (t->left.p1.x);
	    trap.left.p1.y = _cairo_fixed_to_16_16 (t->left.p1.y);
	    trap.left.p2.x = _cairo_fixed_to_16_16 (t->left.p2.x);
	    trap.left.p2.y = _cairo_fixed_to_16_16 (t->left.p2.y);
	}

	if (unlikely (line_exceeds_16_16 (&t->right))) {
	    project_line_x_onto_16_16 (&t->right, t->top, t->bottom, &trap.right);
	    trap.right.p1.y = trap.top;
	    trap.right.p2.y = trap.bottom;
	} else {
	    trap.right.p1.x = _cairo_fixed_to_16_16 (t->right.p1.x);
	    trap.right.p1.y = _cairo_fixed_to_16_16 (t->right.p1.y);
	    trap.right.p2.x = _cairo_fixed_to_16_16 (t->right.p2.x);
	    trap.right.p2.y = _cairo_fixed_to_16_16 (t->right.p2.y);
	}

	pixman_rasterize_trapezoid (image, &trap, -dst_x, -dst_y);
	t++;
    }
}

static cairo_int_status_t
composite_traps (void			*_dst,
		 cairo_operator_t	op,
		 cairo_surface_t	*abstract_src,
		 int			src_x,
		 int			src_y,
		 int			dst_x,
		 int			dst_y,
		 const cairo_rectangle_int_t *extents,
		 cairo_antialias_t	antialias,
		 cairo_traps_t		*traps)
{
    cairo_image_surface_t *dst = (cairo_image_surface_t *) _dst;
    cairo_image_source_t *src = (cairo_image_source_t *) abstract_src;
    cairo_int_status_t status;
    pixman_image_t *mask;
    pixman_format_code_t format;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    /* pixman doesn't eliminate self-intersecting trapezoids/edges */
    status = _cairo_bentley_ottmann_tessellate_traps (traps,
						      CAIRO_FILL_RULE_WINDING);
    if (status != CAIRO_INT_STATUS_SUCCESS)
	    return status;

    /* Special case adding trapezoids onto a mask surface; we want to avoid
     * creating an intermediate temporary mask unnecessarily.
     *
     * We make the assumption here that the portion of the trapezoids
     * contained within the surface is bounded by [dst_x,dst_y,width,height];
     * the Cairo core code passes bounds based on the trapezoid extents.
     */
    format = antialias == CAIRO_ANTIALIAS_NONE ? PIXMAN_a1 : PIXMAN_a8;
    if (dst->pixman_format == format &&
	(abstract_src == NULL ||
	 (op == CAIRO_OPERATOR_ADD && src->is_opaque_solid)))
    {
	_pixman_image_add_traps (dst->pixman_image, dst_x, dst_y, traps);
	return CAIRO_STATUS_SUCCESS;
    }

    mask = pixman_image_create_bits (format,
				     extents->width, extents->height,
				     NULL, 0);
    if (unlikely (mask == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    _pixman_image_add_traps (mask, extents->x, extents->y, traps);
    pixman_image_composite32 (_pixman_operator (op),
                              src->pixman_image, mask, dst->pixman_image,
                              extents->x + src_x, extents->y + src_y,
                              0, 0,
                              extents->x - dst_x, extents->y - dst_y,
                              extents->width, extents->height);

    pixman_image_unref (mask);

    return  CAIRO_STATUS_SUCCESS;
}

#if PIXMAN_VERSION >= PIXMAN_VERSION_ENCODE(0,22,0)
static void
set_point (pixman_point_fixed_t *p, cairo_point_t *c)
{
    p->x = _cairo_fixed_to_16_16 (c->x);
    p->y = _cairo_fixed_to_16_16 (c->y);
}

void
_pixman_image_add_tristrip (pixman_image_t *image,
			    int dst_x, int dst_y,
			    cairo_tristrip_t *strip)
{
    pixman_triangle_t tri;
    pixman_point_fixed_t *p[3] = {&tri.p1, &tri.p2, &tri.p3 };
    int n;

    set_point (p[0], &strip->points[0]);
    set_point (p[1], &strip->points[1]);
    set_point (p[2], &strip->points[2]);
    pixman_add_triangles (image, -dst_x, -dst_y, 1, &tri);
    for (n = 3; n < strip->num_points; n++) {
	set_point (p[n%3], &strip->points[n]);
	pixman_add_triangles (image, -dst_x, -dst_y, 1, &tri);
    }
}

static cairo_int_status_t
composite_tristrip (void			*_dst,
		    cairo_operator_t	op,
		    cairo_surface_t	*abstract_src,
		    int			src_x,
		    int			src_y,
		    int			dst_x,
		    int			dst_y,
		    const cairo_rectangle_int_t *extents,
		    cairo_antialias_t	antialias,
		    cairo_tristrip_t	*strip)
{
    cairo_image_surface_t *dst = (cairo_image_surface_t *) _dst;
    cairo_image_source_t *src = (cairo_image_source_t *) abstract_src;
    pixman_image_t *mask;
    pixman_format_code_t format;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (strip->num_points < 3)
	return CAIRO_STATUS_SUCCESS;

    if (1) { /* pixman doesn't eliminate self-intersecting triangles/edges */
	    cairo_int_status_t status;
	    cairo_traps_t traps;
	    int n;

	    _cairo_traps_init (&traps);
	    for (n = 0; n < strip->num_points; n++) {
		    cairo_point_t p[4];

		    p[0] = strip->points[0];
		    p[1] = strip->points[1];
		    p[2] = strip->points[2];
		    p[3] = strip->points[0];

		    _cairo_traps_tessellate_convex_quad (&traps, p);
	    }
	    status = composite_traps (_dst, op, abstract_src,
				      src_x, src_y,
				      dst_x, dst_y,
				      extents, antialias, &traps);
	    _cairo_traps_fini (&traps);

	    return status;
    }

    format = antialias == CAIRO_ANTIALIAS_NONE ? PIXMAN_a1 : PIXMAN_a8;
    if (dst->pixman_format == format &&
	(abstract_src == NULL ||
	 (op == CAIRO_OPERATOR_ADD && src->is_opaque_solid)))
    {
	_pixman_image_add_tristrip (dst->pixman_image, dst_x, dst_y, strip);
	return CAIRO_STATUS_SUCCESS;
    }

    mask = pixman_image_create_bits (format,
				     extents->width, extents->height,
				     NULL, 0);
    if (unlikely (mask == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    _pixman_image_add_tristrip (mask, extents->x, extents->y, strip);
    pixman_image_composite32 (_pixman_operator (op),
                              src->pixman_image, mask, dst->pixman_image,
                              extents->x + src_x, extents->y + src_y,
                              0, 0,
                              extents->x - dst_x, extents->y - dst_y,
                              extents->width, extents->height);

    pixman_image_unref (mask);

    return  CAIRO_STATUS_SUCCESS;
}
#endif

static cairo_int_status_t
check_composite_glyphs (const cairo_composite_rectangles_t *extents,
			cairo_scaled_font_t *scaled_font,
			cairo_glyph_t *glyphs,
			int *num_glyphs)
{
    return CAIRO_STATUS_SUCCESS;
}

#if HAS_PIXMAN_GLYPHS
static pixman_glyph_cache_t *global_glyph_cache;

static inline pixman_glyph_cache_t *
get_glyph_cache (void)
{
    if (!global_glyph_cache)
	global_glyph_cache = pixman_glyph_cache_create ();

    return global_glyph_cache;
}

void
_cairo_image_compositor_reset_static_data (void)
{
    CAIRO_MUTEX_LOCK (_cairo_glyph_cache_mutex);

    if (global_glyph_cache)
	pixman_glyph_cache_destroy (global_glyph_cache);
    global_glyph_cache = NULL;

    CAIRO_MUTEX_UNLOCK (_cairo_glyph_cache_mutex);
}

void
_cairo_image_scaled_glyph_fini (cairo_scaled_font_t *scaled_font,
				cairo_scaled_glyph_t *scaled_glyph)
{
    CAIRO_MUTEX_LOCK (_cairo_glyph_cache_mutex);

    if (global_glyph_cache) {
	pixman_glyph_cache_remove (
	    global_glyph_cache, scaled_font,
	    (void *)scaled_glyph->hash_entry.hash);
    }

    CAIRO_MUTEX_UNLOCK (_cairo_glyph_cache_mutex);
}

#define PHASE(x) ((int)(floor (4 * (x + 0.125)) - 4 * floor (x + 0.125)))
#define POSITION(x) ((int) floor (x + 0.125))

static cairo_int_status_t
composite_glyphs (void				*_dst,
		  cairo_operator_t		 op,
		  cairo_surface_t		*_src,
		  int				 src_x,
		  int				 src_y,
		  int				 dst_x,
		  int				 dst_y,
		  cairo_composite_glyphs_info_t *info)
{
    cairo_int_status_t status = CAIRO_INT_STATUS_SUCCESS;
    pixman_glyph_cache_t *glyph_cache;
    pixman_glyph_t pglyphs_stack[CAIRO_STACK_ARRAY_LENGTH (pixman_glyph_t)];
    pixman_glyph_t *pglyphs = pglyphs_stack;
    pixman_glyph_t *pg;
    int i;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    CAIRO_MUTEX_LOCK (_cairo_glyph_cache_mutex);

    glyph_cache = get_glyph_cache();
    if (unlikely (glyph_cache == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto out_unlock;
    }

    pixman_glyph_cache_freeze (glyph_cache);

    if (info->num_glyphs > ARRAY_LENGTH (pglyphs_stack)) {
	pglyphs = _cairo_malloc_ab (info->num_glyphs, sizeof (pixman_glyph_t));
	if (unlikely (pglyphs == NULL)) {
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    goto out_thaw;
	}
    }

    pg = pglyphs;
    for (i = 0; i < info->num_glyphs; i++) {
	unsigned long index = info->glyphs[i].index;
	const void *glyph;
        unsigned long xphase, yphase;

        xphase = PHASE(info->glyphs[i].x);
        yphase = PHASE(info->glyphs[i].y);

	index = index | (xphase << 24) | (yphase << 26);

	glyph = pixman_glyph_cache_lookup (glyph_cache, info->font, (void *)(uintptr_t)index);
	if (!glyph) {
	    cairo_scaled_glyph_t *scaled_glyph;
	    cairo_image_surface_t *glyph_surface;

	    /* This call can actually end up recursing, so we have to
	     * drop the mutex around it.
	     */
	    CAIRO_MUTEX_UNLOCK (_cairo_glyph_cache_mutex);
	    status = _cairo_scaled_glyph_lookup (info->font, index,
						 CAIRO_SCALED_GLYPH_INFO_SURFACE,
						 NULL, /* foreground color */
						 &scaled_glyph);
	    CAIRO_MUTEX_LOCK (_cairo_glyph_cache_mutex);

	    if (unlikely (status))
		goto out_thaw;

	    glyph_surface = scaled_glyph->surface;
	    glyph = pixman_glyph_cache_insert (glyph_cache, info->font, (void *)(uintptr_t)index,
					       glyph_surface->base.device_transform.x0,
					       glyph_surface->base.device_transform.y0,
					       glyph_surface->pixman_image);
	    if (unlikely (!glyph)) {
		status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
		goto out_thaw;
	    }
	}

	pg->x = POSITION (info->glyphs[i].x);
	pg->y = POSITION (info->glyphs[i].y);
	pg->glyph = glyph;
	pg++;
    }

    if (info->use_mask) {
	pixman_format_code_t mask_format;

	mask_format = pixman_glyph_get_mask_format (glyph_cache, pg - pglyphs, pglyphs);

	pixman_composite_glyphs (_pixman_operator (op),
				 ((cairo_image_source_t *)_src)->pixman_image,
				 to_pixman_image (_dst),
				 mask_format,
				 info->extents.x + src_x, info->extents.y + src_y,
				 info->extents.x, info->extents.y,
				 info->extents.x - dst_x, info->extents.y - dst_y,
				 info->extents.width, info->extents.height,
				 glyph_cache, pg - pglyphs, pglyphs);
    } else {
	pixman_composite_glyphs_no_mask (_pixman_operator (op),
					 ((cairo_image_source_t *)_src)->pixman_image,
					 to_pixman_image (_dst),
					 src_x, src_y,
					 - dst_x, - dst_y,
					 glyph_cache, pg - pglyphs, pglyphs);
    }

out_thaw:
    pixman_glyph_cache_thaw (glyph_cache);

    if (pglyphs != pglyphs_stack)
	free(pglyphs);

out_unlock:
    CAIRO_MUTEX_UNLOCK (_cairo_glyph_cache_mutex);
    return status;
}
#else
void
_cairo_image_compositor_reset_static_data (void)
{
}

void
_cairo_image_scaled_glyph_fini (cairo_scaled_font_t *scaled_font,
				cairo_scaled_glyph_t *scaled_glyph)
{
}

static cairo_int_status_t
composite_one_glyph (void				*_dst,
		     cairo_operator_t			 op,
		     cairo_surface_t			*_src,
		     int				 src_x,
		     int				 src_y,
		     int				 dst_x,
		     int				 dst_y,
		     cairo_composite_glyphs_info_t	 *info)
{
    cairo_image_surface_t *glyph_surface;
    cairo_scaled_glyph_t *scaled_glyph;
    cairo_status_t status;
    int x, y;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = _cairo_scaled_glyph_lookup (info->font,
					 info->glyphs[0].index,
					 CAIRO_SCALED_GLYPH_INFO_SURFACE,
					 NULL, /* foreground color */
					 &scaled_glyph);

    if (unlikely (status))
	return status;

    glyph_surface = scaled_glyph->surface;
    if (glyph_surface->width == 0 || glyph_surface->height == 0)
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    /* round glyph locations to the nearest pixel */
    /* XXX: FRAGILE: We're ignoring device_transform scaling here. A bug? */
    x = _cairo_lround (info->glyphs[0].x -
		       glyph_surface->base.device_transform.x0);
    y = _cairo_lround (info->glyphs[0].y -
		       glyph_surface->base.device_transform.y0);

    pixman_image_composite32 (_pixman_operator (op),
			      ((cairo_image_source_t *)_src)->pixman_image,
			      glyph_surface->pixman_image,
			      to_pixman_image (_dst),
			      x + src_x,  y + src_y,
			      0, 0,
			      x - dst_x, y - dst_y,
			      glyph_surface->width,
			      glyph_surface->height);

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
composite_glyphs_via_mask (void				*_dst,
			   cairo_operator_t		 op,
			   cairo_surface_t		*_src,
			   int				 src_x,
			   int				 src_y,
			   int				 dst_x,
			   int				 dst_y,
			   cairo_composite_glyphs_info_t *info)
{
    cairo_scaled_glyph_t *glyph_cache[64];
    pixman_image_t *white = _pixman_image_for_color (CAIRO_COLOR_WHITE);
    cairo_scaled_glyph_t *scaled_glyph;
    uint8_t buf[2048];
    pixman_image_t *mask;
    pixman_format_code_t format;
    cairo_status_t status;
    int i;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (unlikely (white == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    /* XXX convert the glyphs to common formats a8/a8r8g8b8 to hit
     * optimised paths through pixman. Should we increase the bit
     * depth of the target surface, we should reconsider the appropriate
     * mask formats.
     */

    status = _cairo_scaled_glyph_lookup (info->font,
					 info->glyphs[0].index,
					 CAIRO_SCALED_GLYPH_INFO_SURFACE,
					 NULL, /* foreground color */
					 &scaled_glyph);
    if (unlikely (status)) {
	pixman_image_unref (white);
	return status;
    }

    memset (glyph_cache, 0, sizeof (glyph_cache));
    glyph_cache[info->glyphs[0].index % ARRAY_LENGTH (glyph_cache)] = scaled_glyph;

    format = PIXMAN_a8;
    i = (info->extents.width + 3) & ~3;
    if (scaled_glyph->surface->base.content & CAIRO_CONTENT_COLOR) {
	format = PIXMAN_a8r8g8b8;
	i = info->extents.width * 4;
    }

    if (i * info->extents.height > (int) sizeof (buf)) {
	mask = pixman_image_create_bits (format,
					info->extents.width,
					info->extents.height,
					NULL, 0);
    } else {
	memset (buf, 0, i * info->extents.height);
	mask = pixman_image_create_bits (format,
					info->extents.width,
					info->extents.height,
					(uint32_t *)buf, i);
    }
    if (unlikely (mask == NULL)) {
	pixman_image_unref (white);
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    status = CAIRO_STATUS_SUCCESS;
    for (i = 0; i < info->num_glyphs; i++) {
	unsigned long glyph_index = info->glyphs[i].index;
	int cache_index = glyph_index % ARRAY_LENGTH (glyph_cache);
	cairo_image_surface_t *glyph_surface;
	int x, y;

	scaled_glyph = glyph_cache[cache_index];
	if (scaled_glyph == NULL ||
	    _cairo_scaled_glyph_index (scaled_glyph) != glyph_index)
	{
	    status = _cairo_scaled_glyph_lookup (info->font, glyph_index,
						 CAIRO_SCALED_GLYPH_INFO_SURFACE,
						 NULL, /* foreground color */
						 &scaled_glyph);

	    if (unlikely (status)) {
		pixman_image_unref (mask);
		pixman_image_unref (white);
		return status;
	    }

	    glyph_cache[cache_index] = scaled_glyph;
	}

	glyph_surface = scaled_glyph->surface;
	if (glyph_surface->width && glyph_surface->height) {
	    if (glyph_surface->base.content & CAIRO_CONTENT_COLOR &&
		format == PIXMAN_a8) {
		pixman_image_t *ca_mask;

		format = PIXMAN_a8r8g8b8;
		ca_mask = pixman_image_create_bits (format,
						    info->extents.width,
						    info->extents.height,
						    NULL, 0);
		if (unlikely (ca_mask == NULL)) {
		    pixman_image_unref (mask);
		    pixman_image_unref (white);
		    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
		}

		pixman_image_composite32 (PIXMAN_OP_SRC,
					  white, mask, ca_mask,
					  0, 0,
					  0, 0,
					  0, 0,
					  info->extents.width,
					  info->extents.height);
		pixman_image_unref (mask);
		mask = ca_mask;
	    }

	    /* round glyph locations to the nearest pixel */
	    /* XXX: FRAGILE: We're ignoring device_transform scaling here. A bug? */
	    x = _cairo_lround (info->glyphs[i].x -
			       glyph_surface->base.device_transform.x0);
	    y = _cairo_lround (info->glyphs[i].y -
			       glyph_surface->base.device_transform.y0);

	    if (glyph_surface->pixman_format == format) {
		pixman_image_composite32 (PIXMAN_OP_ADD,
					  glyph_surface->pixman_image, NULL, mask,
					  0, 0,
					  0, 0,
					  x - info->extents.x, y - info->extents.y,
					  glyph_surface->width,
					  glyph_surface->height);
	    } else {
		pixman_image_composite32 (PIXMAN_OP_ADD,
					  white, glyph_surface->pixman_image, mask,
					  0, 0,
					  0, 0,
					  x - info->extents.x, y - info->extents.y,
					  glyph_surface->width,
					  glyph_surface->height);
	    }
	}
    }

    if (format == PIXMAN_a8r8g8b8)
	pixman_image_set_component_alpha (mask, TRUE);

    pixman_image_composite32 (_pixman_operator (op),
			      ((cairo_image_source_t *)_src)->pixman_image,
			      mask,
			      to_pixman_image (_dst),
			      info->extents.x + src_x, info->extents.y + src_y,
			      0, 0,
			      info->extents.x - dst_x, info->extents.y - dst_y,
			      info->extents.width, info->extents.height);
    pixman_image_unref (mask);
    pixman_image_unref (white);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
composite_glyphs (void				*_dst,
		  cairo_operator_t		 op,
		  cairo_surface_t		*_src,
		  int				 src_x,
		  int				 src_y,
		  int				 dst_x,
		  int				 dst_y,
		  cairo_composite_glyphs_info_t *info)
{
    cairo_scaled_glyph_t *glyph_cache[64];
    pixman_image_t *dst, *src;
    cairo_status_t status;
    int i;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (info->num_glyphs == 1)
	return composite_one_glyph(_dst, op, _src, src_x, src_y, dst_x, dst_y, info);

    if (info->use_mask)
	return composite_glyphs_via_mask(_dst, op, _src, src_x, src_y, dst_x, dst_y, info);

    op = _pixman_operator (op);
    dst = to_pixman_image (_dst);
    src = ((cairo_image_source_t *)_src)->pixman_image;

    memset (glyph_cache, 0, sizeof (glyph_cache));
    status = CAIRO_STATUS_SUCCESS;

    for (i = 0; i < info->num_glyphs; i++) {
	int x, y;
	cairo_image_surface_t *glyph_surface;
	cairo_scaled_glyph_t *scaled_glyph;
	unsigned long glyph_index = info->glyphs[i].index;
	int cache_index = glyph_index % ARRAY_LENGTH (glyph_cache);

	scaled_glyph = glyph_cache[cache_index];
	if (scaled_glyph == NULL ||
	    _cairo_scaled_glyph_index (scaled_glyph) != glyph_index)
	{
	    status = _cairo_scaled_glyph_lookup (info->font, glyph_index,
						 CAIRO_SCALED_GLYPH_INFO_SURFACE,
						 NULL, /* foreground color */
						 &scaled_glyph);

	    if (unlikely (status))
		break;

	    glyph_cache[cache_index] = scaled_glyph;
	}

	glyph_surface = scaled_glyph->surface;
	if (glyph_surface->width && glyph_surface->height) {
	    /* round glyph locations to the nearest pixel */
	    /* XXX: FRAGILE: We're ignoring device_transform scaling here. A bug? */
	    x = _cairo_lround (info->glyphs[i].x -
			       glyph_surface->base.device_transform.x0);
	    y = _cairo_lround (info->glyphs[i].y -
			       glyph_surface->base.device_transform.y0);

	    pixman_image_composite32 (op, src, glyph_surface->pixman_image, dst,
                                      x + src_x,  y + src_y,
                                      0, 0,
                                      x - dst_x, y - dst_y,
				      glyph_surface->width,
				      glyph_surface->height);
	}
    }

    return status;
}
#endif

static cairo_int_status_t
check_composite (const cairo_composite_rectangles_t *extents)
{
    return CAIRO_STATUS_SUCCESS;
}

const cairo_compositor_t *
_cairo_image_traps_compositor_get (void)
{
    static cairo_atomic_once_t once = CAIRO_ATOMIC_ONCE_INIT;
    static cairo_traps_compositor_t compositor;

    if (_cairo_atomic_init_once_enter(&once)) {
	_cairo_traps_compositor_init(&compositor,
				     &__cairo_no_compositor);
	compositor.acquire = acquire;
	compositor.release = release;
	compositor.set_clip_region = set_clip_region;
	compositor.pattern_to_surface = _cairo_image_source_create_for_pattern;
	compositor.draw_image_boxes = draw_image_boxes;
	//compositor.copy_boxes = copy_boxes;
	compositor.fill_boxes = fill_boxes;
	compositor.check_composite = check_composite;
	compositor.composite = composite;
	compositor.lerp = lerp;
	//compositor.check_composite_boxes = check_composite_boxes;
	compositor.composite_boxes = composite_boxes;
	//compositor.check_composite_traps = check_composite_traps;
	compositor.composite_traps = composite_traps;
	//compositor.check_composite_tristrip = check_composite_traps;
#if PIXMAN_VERSION >= PIXMAN_VERSION_ENCODE(0,22,0)
	compositor.composite_tristrip = composite_tristrip;
#endif
	compositor.check_composite_glyphs = check_composite_glyphs;
	compositor.composite_glyphs = composite_glyphs;

	_cairo_atomic_init_once_leave(&once);
    }

    return &compositor.base;
}

const cairo_compositor_t *
_cairo_image_mask_compositor_get (void)
{
    static cairo_atomic_once_t once = CAIRO_ATOMIC_ONCE_INIT;
    static cairo_mask_compositor_t compositor;

    if (_cairo_atomic_init_once_enter(&once)) {
	_cairo_mask_compositor_init (&compositor,
				     _cairo_image_traps_compositor_get ());
	compositor.acquire = acquire;
	compositor.release = release;
	compositor.set_clip_region = set_clip_region;
	compositor.pattern_to_surface = _cairo_image_source_create_for_pattern;
	compositor.draw_image_boxes = draw_image_boxes;
	compositor.fill_rectangles = fill_rectangles;
	compositor.fill_boxes = fill_boxes;
	compositor.check_composite = check_composite;
	compositor.composite = composite;
	//compositor.lerp = lerp;
	//compositor.check_composite_boxes = check_composite_boxes;
	compositor.composite_boxes = composite_boxes;
	compositor.check_composite_glyphs = check_composite_glyphs;
	compositor.composite_glyphs = composite_glyphs;

	_cairo_atomic_init_once_leave(&once);
    }

    return &compositor.base;
}

#if PIXMAN_HAS_COMPOSITOR
typedef struct _cairo_image_span_renderer {
    cairo_span_renderer_t base;

    pixman_image_compositor_t *compositor;
    pixman_image_t *src, *mask;
    float opacity;
    cairo_rectangle_int_t extents;
} cairo_image_span_renderer_t;
COMPILE_TIME_ASSERT (sizeof (cairo_image_span_renderer_t) <= sizeof (cairo_abstract_span_renderer_t));

static cairo_status_t
_cairo_image_bounded_opaque_spans (void *abstract_renderer,
				   int y, int height,
				   const cairo_half_open_span_t *spans,
				   unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    do {
	if (spans[0].coverage)
	    pixman_image_compositor_blt (r->compositor,
					 spans[0].x, y,
					 spans[1].x - spans[0].x, height,
					 spans[0].coverage);
	spans++;
    } while (--num_spans > 1);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_image_bounded_spans (void *abstract_renderer,
			    int y, int height,
			    const cairo_half_open_span_t *spans,
			    unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    do {
	if (spans[0].coverage) {
	    pixman_image_compositor_blt (r->compositor,
					 spans[0].x, y,
					 spans[1].x - spans[0].x, height,
					 r->opacity * spans[0].coverage);
	}
	spans++;
    } while (--num_spans > 1);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_image_unbounded_spans (void *abstract_renderer,
			      int y, int height,
			      const cairo_half_open_span_t *spans,
			      unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    assert (y + height <= r->extents.height);
    if (y > r->extents.y) {
	pixman_image_compositor_blt (r->compositor,
				     r->extents.x, r->extents.y,
				     r->extents.width, y - r->extents.y,
				     0);
    }

    if (num_spans == 0) {
	pixman_image_compositor_blt (r->compositor,
				     r->extents.x, y,
				     r->extents.width,  height,
				     0);
    } else {
	if (spans[0].x != r->extents.x) {
	    pixman_image_compositor_blt (r->compositor,
					 r->extents.x, y,
					 spans[0].x - r->extents.x,
					 height,
					 0);
	}

	do {
	    assert (spans[0].x < r->extents.x + r->extents.width);
	    pixman_image_compositor_blt (r->compositor,
					 spans[0].x, y,
					 spans[1].x - spans[0].x, height,
					 r->opacity * spans[0].coverage);
	    spans++;
	} while (--num_spans > 1);

	if (spans[0].x != r->extents.x + r->extents.width) {
	    assert (spans[0].x < r->extents.x + r->extents.width);
	    pixman_image_compositor_blt (r->compositor,
					 spans[0].x,     y,
					 r->extents.x + r->extents.width - spans[0].x, height,
					 0);
	}
    }

    r->extents.y = y + height;
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_image_clipped_spans (void *abstract_renderer,
			    int y, int height,
			    const cairo_half_open_span_t *spans,
			    unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    assert (num_spans);

    do {
	if (! spans[0].inverse)
	    pixman_image_compositor_blt (r->compositor,
					 spans[0].x, y,
					 spans[1].x - spans[0].x, height,
					 r->opacity * spans[0].coverage);
	spans++;
    } while (--num_spans > 1);

    r->extents.y = y + height;
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_image_finish_unbounded_spans (void *abstract_renderer)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (r->extents.y < r->extents.height) {
	pixman_image_compositor_blt (r->compositor,
				     r->extents.x, r->extents.y,
				     r->extents.width,
				     r->extents.height - r->extents.y,
				     0);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
span_renderer_init (cairo_abstract_span_renderer_t	*_r,
		    const cairo_composite_rectangles_t *composite,
		    cairo_bool_t			 needs_clip)
{
    cairo_image_span_renderer_t *r = (cairo_image_span_renderer_t *)_r;
    cairo_image_surface_t *dst = (cairo_image_surface_t *)composite->surface;
    const cairo_pattern_t *source = &composite->source_pattern.base;
    cairo_operator_t op = composite->op;
    int src_x, src_y;
    int mask_x, mask_y;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (op == CAIRO_OPERATOR_CLEAR) {
	op = PIXMAN_OP_LERP_CLEAR;
    } else if (dst->base.is_clear &&
	       (op == CAIRO_OPERATOR_SOURCE ||
		op == CAIRO_OPERATOR_OVER ||
		op == CAIRO_OPERATOR_ADD)) {
	op = PIXMAN_OP_SRC;
    } else if (op == CAIRO_OPERATOR_SOURCE) {
	op = PIXMAN_OP_LERP_SRC;
    } else {
	op = _pixman_operator (op);
    }

    r->compositor = NULL;
    r->mask = NULL;
    r->src = _pixman_image_for_pattern (dst, source, FALSE,
					&composite->unbounded,
					&composite->source_sample_area,
					&src_x, &src_y);
    if (unlikely (r->src == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    r->opacity = 1.0;
    if (composite->mask_pattern.base.type == CAIRO_PATTERN_TYPE_SOLID) {
	r->opacity = composite->mask_pattern.solid.color.alpha;
    } else {
	r->mask = _pixman_image_for_pattern (dst,
					     &composite->mask_pattern.base,
					     TRUE,
					     &composite->unbounded,
					     &composite->mask_sample_area,
					     &mask_x, &mask_y);
	if (unlikely (r->mask == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	/* XXX Component-alpha? */
	if ((dst->base.content & CAIRO_CONTENT_COLOR) == 0 &&
	    _cairo_pattern_is_opaque (source, &composite->source_sample_area))
	{
	    pixman_image_unref (r->src);
	    r->src = r->mask;
	    src_x = mask_x;
	    src_y = mask_y;
	    r->mask = NULL;
	}
    }

    if (composite->is_bounded) {
	if (r->opacity == 1.)
	    r->base.render_rows = _cairo_image_bounded_opaque_spans;
	else
	    r->base.render_rows = _cairo_image_bounded_spans;
	r->base.finish = NULL;
    } else {
	if (needs_clip)
	    r->base.render_rows = _cairo_image_clipped_spans;
	else
	    r->base.render_rows = _cairo_image_unbounded_spans;
        r->base.finish = _cairo_image_finish_unbounded_spans;
	r->extents = composite->unbounded;
	r->extents.height += r->extents.y;
    }

    r->compositor =
	pixman_image_create_compositor (op, r->src, r->mask, dst->pixman_image,
					composite->unbounded.x + src_x,
					composite->unbounded.y + src_y,
					composite->unbounded.x + mask_x,
					composite->unbounded.y + mask_y,
					composite->unbounded.x,
					composite->unbounded.y,
					composite->unbounded.width,
					composite->unbounded.height);
    if (unlikely (r->compositor == NULL))
	return CAIRO_INT_STATUS_NOTHING_TO_DO;

    return CAIRO_STATUS_SUCCESS;
}

static void
span_renderer_fini (cairo_abstract_span_renderer_t *_r,
		    cairo_int_status_t status)
{
    cairo_image_span_renderer_t *r = (cairo_image_span_renderer_t *) _r;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (status == CAIRO_INT_STATUS_SUCCESS && r->base.finish)
	r->base.finish (r);

    if (r->compositor)
	pixman_image_compositor_destroy (r->compositor);

    if (r->src)
	pixman_image_unref (r->src);
    if (r->mask)
	pixman_image_unref (r->mask);
}
#else
typedef struct _cairo_image_span_renderer {
    cairo_span_renderer_t base;

    const cairo_composite_rectangles_t *composite;

    float opacity;
    uint8_t op;
    int bpp;

    pixman_image_t *src, *mask;
    union {
	struct fill {
	    ptrdiff_t stride;
	    uint8_t *data;
	    uint32_t pixel;
	} fill;
	struct blit {
	    int stride;
	    uint8_t *data;
	    int src_stride;
	    uint8_t *src_data;
	} blit;
	struct composite {
	    pixman_image_t *dst;
	    int src_x, src_y;
	    int mask_x, mask_y;
	    int run_length;
	} composite;
	struct finish {
	    cairo_rectangle_int_t extents;
	    int src_x, src_y;
	    ptrdiff_t stride;
	    uint8_t *data;
	} mask;
    } u;
    uint8_t _buf[0];
#define SZ_BUF (int)(sizeof (cairo_abstract_span_renderer_t) - sizeof (cairo_image_span_renderer_t))
} cairo_image_span_renderer_t;
COMPILE_TIME_ASSERT (sizeof (cairo_image_span_renderer_t) <= sizeof (cairo_abstract_span_renderer_t));

static cairo_status_t
_cairo_image_spans (void *abstract_renderer,
		    int y, int height,
		    const cairo_half_open_span_t *spans,
		    unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;
    uint8_t *mask, *row;
    int len;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    mask = r->u.mask.data + (y - r->u.mask.extents.y) * r->u.mask.stride;
    mask += spans[0].x - r->u.mask.extents.x;
    row = mask;

    do {
	len = spans[1].x - spans[0].x;
	if (spans[0].coverage) {
	    *row++ = r->opacity * spans[0].coverage;
	    if (--len)
		memset (row, row[-1], len);
	}
	row += len;
	spans++;
    } while (--num_spans > 1);

    len = row - mask;
    row = mask;
    while (--height) {
	mask += r->u.mask.stride;
	memcpy (mask, row, len);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_image_spans_and_zero (void *abstract_renderer,
			     int y, int height,
			     const cairo_half_open_span_t *spans,
			     unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;
    uint8_t *mask;
    int len;

    mask = r->u.mask.data;
    if (y > r->u.mask.extents.y) {
	len = (y - r->u.mask.extents.y) * r->u.mask.stride;
	memset (mask, 0, len);
	mask += len;
    }

    r->u.mask.extents.y = y + height;
    r->u.mask.data = mask + height * r->u.mask.stride;
    if (num_spans == 0) {
	memset (mask, 0, height * r->u.mask.stride);
    } else {
	uint8_t *row = mask;

	if (spans[0].x != r->u.mask.extents.x) {
	    len = spans[0].x - r->u.mask.extents.x;
	    memset (row, 0, len);
	    row += len;
	}

	do {
	    len = spans[1].x - spans[0].x;
	    *row++ = r->opacity * spans[0].coverage;
	    if (len > 1) {
		memset (row, row[-1], --len);
		row += len;
	    }
	    spans++;
	} while (--num_spans > 1);

	if (spans[0].x != r->u.mask.extents.x + r->u.mask.extents.width) {
	    len = r->u.mask.extents.x + r->u.mask.extents.width - spans[0].x;
	    memset (row, 0, len);
	}

	row = mask;
	while (--height) {
	    mask += r->u.mask.stride;
	    memcpy (mask, row, r->u.mask.extents.width);
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_image_finish_spans_and_zero (void *abstract_renderer)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (r->u.mask.extents.y < r->u.mask.extents.height)
	memset (r->u.mask.data, 0, (r->u.mask.extents.height - r->u.mask.extents.y) * r->u.mask.stride);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_fill8_spans (void *abstract_renderer, int y, int h,
	       const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    if (likely(h == 1)) {
	do {
	    if (spans[0].coverage) {
		int len = spans[1].x - spans[0].x;
		uint8_t *d = r->u.fill.data + r->u.fill.stride*y + spans[0].x;
		if (len == 1)
		    *d = r->u.fill.pixel;
		else
		    memset(d, r->u.fill.pixel, len);
	    }
	    spans++;
	} while (--num_spans > 1);
    } else {
	do {
	    if (spans[0].coverage) {
		int yy = y, hh = h;
		do {
		    int len = spans[1].x - spans[0].x;
		    uint8_t *d = r->u.fill.data + r->u.fill.stride*yy + spans[0].x;
		    if (len == 1)
			*d = r->u.fill.pixel;
		    else
			memset(d, r->u.fill.pixel, len);
		    yy++;
		} while (--hh);
	    }
	    spans++;
	} while (--num_spans > 1);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_fill16_spans (void *abstract_renderer, int y, int h,
	       const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    if (likely(h == 1)) {
	do {
	    if (spans[0].coverage) {
		int len = spans[1].x - spans[0].x;
		uint16_t *d = (uint16_t*)(r->u.fill.data + r->u.fill.stride*y + spans[0].x*2);
		while (len-- > 0)
		    *d++ = r->u.fill.pixel;
	    }
	    spans++;
	} while (--num_spans > 1);
    } else {
	do {
	    if (spans[0].coverage) {
		int yy = y, hh = h;
		do {
		    int len = spans[1].x - spans[0].x;
		    uint16_t *d = (uint16_t*)(r->u.fill.data + r->u.fill.stride*yy + spans[0].x*2);
		    while (len-- > 0)
			*d++ = r->u.fill.pixel;
		    yy++;
		} while (--hh);
	    }
	    spans++;
	} while (--num_spans > 1);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_fill32_spans (void *abstract_renderer, int y, int h,
	       const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    if (likely(h == 1)) {
	do {
	    if (spans[0].coverage) {
		int len = spans[1].x - spans[0].x;
		if (len > 32) {
		    pixman_fill ((uint32_t *)r->u.fill.data, r->u.fill.stride / sizeof(uint32_t), r->bpp,
				 spans[0].x, y, len, 1, r->u.fill.pixel);
		} else {
		    uint32_t *d = (uint32_t*)(r->u.fill.data + r->u.fill.stride*y + spans[0].x*4);
		    while (len-- > 0)
			*d++ = r->u.fill.pixel;
		}
	    }
	    spans++;
	} while (--num_spans > 1);
    } else {
	do {
	    if (spans[0].coverage) {
		if (spans[1].x - spans[0].x > 16) {
		    pixman_fill ((uint32_t *)r->u.fill.data, r->u.fill.stride / sizeof(uint32_t), r->bpp,
				 spans[0].x, y, spans[1].x - spans[0].x, h,
				 r->u.fill.pixel);
		} else {
		    int yy = y, hh = h;
		    do {
			int len = spans[1].x - spans[0].x;
			uint32_t *d = (uint32_t*)(r->u.fill.data + r->u.fill.stride*yy + spans[0].x*4);
			while (len-- > 0)
			    *d++ = r->u.fill.pixel;
			yy++;
		    } while (--hh);
		}
	    }
	    spans++;
	} while (--num_spans > 1);
    }

    return CAIRO_STATUS_SUCCESS;
}

#if 0
static cairo_status_t
_fill_spans (void *abstract_renderer, int y, int h,
	     const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    do {
	if (spans[0].coverage) {
		pixman_fill ((uint32_t *) r->data, r->stride, r->bpp,
			     spans[0].x, y,
			     spans[1].x - spans[0].x, h,
			     r->pixel);
	}
	spans++;
    } while (--num_spans > 1);

    return CAIRO_STATUS_SUCCESS;
}
#endif

static cairo_status_t
_blit_spans (void *abstract_renderer, int y, int h,
	     const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;
    int cpp;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    cpp = r->bpp/8;
    if (likely (h == 1)) {
	uint8_t *src = r->u.blit.src_data + y*r->u.blit.src_stride;
	uint8_t *dst = r->u.blit.data + y*r->u.blit.stride;
	do {
	    if (spans[0].coverage) {
		void *s = src + spans[0].x*cpp;
		void *d = dst + spans[0].x*cpp;
		int len = (spans[1].x - spans[0].x) * cpp;
		switch (len) {
		case 1:
		    *(uint8_t *)d = *(uint8_t *)s;
		    break;
		case 2:
		    *(uint16_t *)d = *(uint16_t *)s;
		    break;
		case 4:
		    *(uint32_t *)d = *(uint32_t *)s;
		    break;
#if HAVE_UINT64_T
		case 8:
		    *(uint64_t *)d = *(uint64_t *)s;
		    break;
#endif
		default:
		    memcpy(d, s, len);
		    break;
		}
	    }
	    spans++;
	} while (--num_spans > 1);
    } else {
	do {
	    if (spans[0].coverage) {
		int yy = y, hh = h;
		do {
		    void *src = r->u.blit.src_data + yy*r->u.blit.src_stride + spans[0].x*cpp;
		    void *dst = r->u.blit.data + yy*r->u.blit.stride + spans[0].x*cpp;
		    int len = (spans[1].x - spans[0].x) * cpp;
		    switch (len) {
		    case 1:
			*(uint8_t *)dst = *(uint8_t *)src;
			break;
		    case 2:
			*(uint16_t *)dst = *(uint16_t *)src;
			break;
		    case 4:
			*(uint32_t *)dst = *(uint32_t *)src;
			break;
#if HAVE_UINT64_T
		    case 8:
			*(uint64_t *)dst = *(uint64_t *)src;
			break;
#endif
		    default:
			memcpy(dst, src, len);
			break;
		    }
		    yy++;
		} while (--hh);
	    }
	    spans++;
	} while (--num_spans > 1);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_mono_spans (void *abstract_renderer, int y, int h,
	     const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    do {
	if (spans[0].coverage) {
	    pixman_image_composite32 (r->op,
				      r->src, NULL, r->u.composite.dst,
				      spans[0].x + r->u.composite.src_x,  y + r->u.composite.src_y,
				      0, 0,
				      spans[0].x, y,
				      spans[1].x - spans[0].x, h);
	}
	spans++;
    } while (--num_spans > 1);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_mono_unbounded_spans (void *abstract_renderer, int y, int h,
		       const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0) {
	pixman_image_composite32 (PIXMAN_OP_CLEAR,
				  r->src, NULL, r->u.composite.dst,
				  spans[0].x + r->u.composite.src_x,  y + r->u.composite.src_y,
				  0, 0,
				  r->composite->unbounded.x, y,
				  r->composite->unbounded.width, h);
	r->u.composite.mask_y = y + h;
	return CAIRO_STATUS_SUCCESS;
    }

    if (y != r->u.composite.mask_y) {
	pixman_image_composite32 (PIXMAN_OP_CLEAR,
				  r->src, NULL, r->u.composite.dst,
				  spans[0].x + r->u.composite.src_x,  y + r->u.composite.src_y,
				  0, 0,
				  r->composite->unbounded.x, r->u.composite.mask_y,
				  r->composite->unbounded.width, y - r->u.composite.mask_y);
    }

    if (spans[0].x != r->composite->unbounded.x) {
	    pixman_image_composite32 (PIXMAN_OP_CLEAR,
				      r->src, NULL, r->u.composite.dst,
				      spans[0].x + r->u.composite.src_x,  y + r->u.composite.src_y,
				      0, 0,
				      r->composite->unbounded.x, y,
				      spans[0].x - r->composite->unbounded.x, h);
    }

    do {
	int op = spans[0].coverage ? r->op : PIXMAN_OP_CLEAR;
	pixman_image_composite32 (op,
				  r->src, NULL, r->u.composite.dst,
				  spans[0].x + r->u.composite.src_x,  y + r->u.composite.src_y,
				  0, 0,
				  spans[0].x, y,
				  spans[1].x - spans[0].x, h);
	spans++;
    } while (--num_spans > 1);

    if (spans[0].x != r->composite->unbounded.x + r->composite->unbounded.width) {
	    pixman_image_composite32 (PIXMAN_OP_CLEAR,
				      r->src, NULL, r->u.composite.dst,
				      spans[0].x + r->u.composite.src_x,  y + r->u.composite.src_y,
				      0, 0,
				      spans[0].x, y,
				      r->composite->unbounded.x + r->composite->unbounded.width - spans[0].x, h);
    }

    r->u.composite.mask_y = y + h;
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_mono_finish_unbounded_spans (void *abstract_renderer)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (r->u.composite.mask_y < r->composite->unbounded.y + r->composite->unbounded.height) {
	pixman_image_composite32 (PIXMAN_OP_CLEAR,
				  r->src, NULL, r->u.composite.dst,
				  r->composite->unbounded.x + r->u.composite.src_x,  r->u.composite.mask_y + r->u.composite.src_y,
				  0, 0,
				  r->composite->unbounded.x, r->u.composite.mask_y,
				  r->composite->unbounded.width,
				  r->composite->unbounded.y + r->composite->unbounded.height - r->u.composite.mask_y);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
mono_renderer_init (cairo_image_span_renderer_t	*r,
		    const cairo_composite_rectangles_t *composite,
		    cairo_antialias_t			 antialias,
		    cairo_bool_t			 needs_clip)
{
    cairo_image_surface_t *dst = (cairo_image_surface_t *)composite->surface;

    if (antialias != CAIRO_ANTIALIAS_NONE)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    if (!_cairo_pattern_is_opaque_solid (&composite->mask_pattern.base))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    r->base.render_rows = NULL;
    if (composite->source_pattern.base.type == CAIRO_PATTERN_TYPE_SOLID) {
	const cairo_color_t *color;

	color = &composite->source_pattern.solid.color;
	if (composite->op == CAIRO_OPERATOR_CLEAR)
	    color = CAIRO_COLOR_TRANSPARENT;

	if (fill_reduces_to_source (composite->op, color, dst, &r->u.fill.pixel)) {
	    /* Use plain C for the fill operations as the span length is
	     * typically small, too small to payback the startup overheads of
	     * using SSE2 etc.
	     */
	    switch (PIXMAN_FORMAT_BPP(dst->pixman_format)) {
	    case 8: r->base.render_rows = _fill8_spans; break;
	    case 16: r->base.render_rows = _fill16_spans; break;
	    case 32: r->base.render_rows = _fill32_spans; break;
	    default: break;
	    }
	    r->u.fill.data = dst->data;
	    r->u.fill.stride = dst->stride;
	}
    } else if ((composite->op == CAIRO_OPERATOR_SOURCE ||
		(composite->op == CAIRO_OPERATOR_OVER &&
		 (dst->base.is_clear || (dst->base.content & CAIRO_CONTENT_ALPHA) == 0))) &&
	       composite->source_pattern.base.type == CAIRO_PATTERN_TYPE_SURFACE &&
	       composite->source_pattern.surface.surface->backend->type == CAIRO_SURFACE_TYPE_IMAGE &&
	       to_image_surface(composite->source_pattern.surface.surface)->format == dst->format)
    {
       cairo_image_surface_t *src =
	   to_image_surface(composite->source_pattern.surface.surface);
       int tx, ty;

	if (_cairo_matrix_is_integer_translation(&composite->source_pattern.base.matrix,
						 &tx, &ty) &&
	    composite->bounded.x + tx >= 0 &&
	    composite->bounded.y + ty >= 0 &&
	    composite->bounded.x + composite->bounded.width +  tx <= src->width &&
	    composite->bounded.y + composite->bounded.height + ty <= src->height) {

	    r->u.blit.stride = dst->stride;
	    r->u.blit.data = dst->data;
	    r->u.blit.src_stride = src->stride;
	    r->u.blit.src_data = src->data + src->stride * ty + tx * 4;
	    r->base.render_rows = _blit_spans;
	}
    }

    if (r->base.render_rows == NULL) {
	r->src = _pixman_image_for_pattern (dst, &composite->source_pattern.base, FALSE,
					    &composite->unbounded,
					    &composite->source_sample_area,
					    &r->u.composite.src_x, &r->u.composite.src_y);
	if (unlikely (r->src == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	r->u.composite.dst = to_pixman_image (composite->surface);
	r->op = _pixman_operator (composite->op);
	if (composite->is_bounded == 0) {
	    r->base.render_rows = _mono_unbounded_spans;
	    r->base.finish = _mono_finish_unbounded_spans;
	    r->u.composite.mask_y = composite->unbounded.y;
	} else
	    r->base.render_rows = _mono_spans;
    }
    r->bpp = PIXMAN_FORMAT_BPP(dst->pixman_format);

    return CAIRO_INT_STATUS_SUCCESS;
}

#define ONE_HALF 0x7f
#define RB_MASK 0x00ff00ff
#define RB_ONE_HALF 0x007f007f
#define RB_MASK_PLUS_ONE 0x01000100
#define G_SHIFT 8
static inline uint32_t
mul8x2_8 (uint32_t a, uint8_t b)
{
    uint32_t t = (a & RB_MASK) * b + RB_ONE_HALF;
    return ((t + ((t >> G_SHIFT) & RB_MASK)) >> G_SHIFT) & RB_MASK;
}

static inline uint32_t
add8x2_8x2 (uint32_t a, uint32_t b)
{
    uint32_t t = a + b;
    t |= RB_MASK_PLUS_ONE - ((t >> G_SHIFT) & RB_MASK);
    return t & RB_MASK;
}

static inline uint8_t
mul8_8 (uint8_t a, uint8_t b)
{
    uint16_t t = a * (uint16_t)b + ONE_HALF;
    return ((t >> G_SHIFT) + t) >> G_SHIFT;
}

static inline uint32_t
lerp8x4 (uint32_t src, uint8_t a, uint32_t dst)
{
    return (add8x2_8x2 (mul8x2_8 (src, a),
			mul8x2_8 (dst, ~a)) |
	    add8x2_8x2 (mul8x2_8 (src >> G_SHIFT, a),
			mul8x2_8 (dst >> G_SHIFT, ~a)) << G_SHIFT);
}

static cairo_status_t
_fill_a8_lerp_opaque_spans (void *abstract_renderer, int y, int h,
			    const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    if (likely(h == 1)) {
	uint8_t *d = r->u.fill.data + r->u.fill.stride*y;
	do {
	    uint8_t a = spans[0].coverage;
	    if (a) {
		int len = spans[1].x - spans[0].x;
		if (a == 0xff) {
		    memset(d + spans[0].x, r->u.fill.pixel, len);
		} else {
		    uint8_t s = mul8_8(a, r->u.fill.pixel);
		    uint8_t *dst = d + spans[0].x;
		    a = ~a;
		    while (len-- > 0) {
			uint8_t t = mul8_8(*dst, a);
			*dst++ = t + s;
		    }
		}
	    }
	    spans++;
	} while (--num_spans > 1);
    } else {
	do {
	    uint8_t a = spans[0].coverage;
	    if (a) {
		int yy = y, hh = h;
		if (a == 0xff) {
		    do {
			int len = spans[1].x - spans[0].x;
			uint8_t *d = r->u.fill.data + r->u.fill.stride*yy + spans[0].x;
			memset(d, r->u.fill.pixel, len);
			yy++;
		    } while (--hh);
		} else {
		    uint8_t s = mul8_8(a, r->u.fill.pixel);
		    a = ~a;
		    do {
			int len = spans[1].x - spans[0].x;
			uint8_t *d = r->u.fill.data + r->u.fill.stride*yy + spans[0].x;
			while (len-- > 0) {
			    uint8_t t = mul8_8(*d, a);
			    *d++ = t + s;
			}
			yy++;
		    } while (--hh);
		}
	    }
	    spans++;
	} while (--num_spans > 1);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_fill_xrgb32_lerp_opaque_spans (void *abstract_renderer, int y, int h,
				const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    if (likely(h == 1)) {
	do {
	    uint8_t a = spans[0].coverage;
	    if (a) {
		int len = spans[1].x - spans[0].x;
		uint32_t *d = (uint32_t*)(r->u.fill.data + r->u.fill.stride*y + spans[0].x*4);
		if (a == 0xff) {
		    if (len > 31) {
			pixman_fill ((uint32_t *)r->u.fill.data, r->u.fill.stride / sizeof(uint32_t), 32,
				     spans[0].x, y, len, 1, r->u.fill.pixel);
		    } else {
			uint32_t *d = (uint32_t*)(r->u.fill.data + r->u.fill.stride*y + spans[0].x*4);
			while (len-- > 0)
			    *d++ = r->u.fill.pixel;
		    }
		} else while (len-- > 0) {
		    *d = lerp8x4 (r->u.fill.pixel, a, *d);
		    d++;
		}
	    }
	    spans++;
	} while (--num_spans > 1);
    } else {
	do {
	    uint8_t a = spans[0].coverage;
	    if (a) {
		if (a == 0xff) {
		    if (spans[1].x - spans[0].x > 16) {
			pixman_fill ((uint32_t *)r->u.fill.data, r->u.fill.stride / sizeof(uint32_t), 32,
				     spans[0].x, y, spans[1].x - spans[0].x, h,
				     r->u.fill.pixel);
		    } else {
			int yy = y, hh = h;
			do {
			    int len = spans[1].x - spans[0].x;
			    uint32_t *d = (uint32_t*)(r->u.fill.data + r->u.fill.stride*yy + spans[0].x*4);
			    while (len-- > 0)
				*d++ = r->u.fill.pixel;
			    yy++;
			} while (--hh);
		    }
		} else {
		    int yy = y, hh = h;
		    do {
			int len = spans[1].x - spans[0].x;
			uint32_t *d = (uint32_t *)(r->u.fill.data + r->u.fill.stride*yy + spans[0].x*4);
			while (len-- > 0) {
			    *d = lerp8x4 (r->u.fill.pixel, a, *d);
			    d++;
			}
			yy++;
		    } while (--hh);
		}
	    }
	    spans++;
	} while (--num_spans > 1);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_fill_a8_lerp_spans (void *abstract_renderer, int y, int h,
		     const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    if (likely(h == 1)) {
	do {
	    uint8_t a = mul8_8 (spans[0].coverage, r->bpp);
	    if (a) {
		int len = spans[1].x - spans[0].x;
		uint8_t *d = r->u.fill.data + r->u.fill.stride*y + spans[0].x;
		uint16_t p = (uint16_t)a * r->u.fill.pixel + 0x7f;
		uint16_t ia = ~a;
		while (len-- > 0) {
		    uint16_t t = *d*ia + p;
		    *d++ = (t + (t>>8)) >> 8;
		}
	    }
	    spans++;
	} while (--num_spans > 1);
    } else {
	do {
	    uint8_t a = mul8_8 (spans[0].coverage, r->bpp);
	    if (a) {
		int yy = y, hh = h;
		uint16_t p = (uint16_t)a * r->u.fill.pixel + 0x7f;
		uint16_t ia = ~a;
		do {
		    int len = spans[1].x - spans[0].x;
		    uint8_t *d = r->u.fill.data + r->u.fill.stride*yy + spans[0].x;
		    while (len-- > 0) {
			uint16_t t = *d*ia + p;
			*d++ = (t + (t>>8)) >> 8;
		    }
		    yy++;
		} while (--hh);
	    }
	    spans++;
	} while (--num_spans > 1);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_fill_xrgb32_lerp_spans (void *abstract_renderer, int y, int h,
			 const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    if (likely(h == 1)) {
	do {
	    uint8_t a = mul8_8 (spans[0].coverage, r->bpp);
	    if (a) {
		int len = spans[1].x - spans[0].x;
		uint32_t *d = (uint32_t*)(r->u.fill.data + r->u.fill.stride*y + spans[0].x*4);
		while (len-- > 0) {
		    *d = lerp8x4 (r->u.fill.pixel, a, *d);
		    d++;
		}
	    }
	    spans++;
	} while (--num_spans > 1);
    } else {
	do {
	    uint8_t a = mul8_8 (spans[0].coverage, r->bpp);
	    if (a) {
		int yy = y, hh = h;
		do {
		    int len = spans[1].x - spans[0].x;
		    uint32_t *d = (uint32_t *)(r->u.fill.data + r->u.fill.stride*yy + spans[0].x*4);
		    while (len-- > 0) {
			*d = lerp8x4 (r->u.fill.pixel, a, *d);
			d++;
		    }
		    yy++;
		} while (--hh);
	    }
	    spans++;
	} while (--num_spans > 1);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_blit_xrgb32_lerp_spans (void *abstract_renderer, int y, int h,
			 const cairo_half_open_span_t *spans, unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    if (likely(h == 1)) {
	uint8_t *src = r->u.blit.src_data + y*r->u.blit.src_stride;
	uint8_t *dst = r->u.blit.data + y*r->u.blit.stride;
	do {
	    uint8_t a = mul8_8 (spans[0].coverage, r->bpp);
	    if (a) {
		uint32_t *s = (uint32_t*)src + spans[0].x;
		uint32_t *d = (uint32_t*)dst + spans[0].x;
		int len = spans[1].x - spans[0].x;
		if (a == 0xff) {
		    if (len == 1)
			*d = *s;
		    else
			memcpy(d, s, len*4);
		} else {
		    while (len-- > 0) {
			*d = lerp8x4 (*s, a, *d);
			s++, d++;
		    }
		}
	    }
	    spans++;
	} while (--num_spans > 1);
    } else {
	do {
	    uint8_t a = mul8_8 (spans[0].coverage, r->bpp);
	    if (a) {
		int yy = y, hh = h;
		do {
		    uint32_t *s = (uint32_t *)(r->u.blit.src_data + yy*r->u.blit.src_stride + spans[0].x * 4);
		    uint32_t *d = (uint32_t *)(r->u.blit.data + yy*r->u.blit.stride + spans[0].x * 4);
		    int len = spans[1].x - spans[0].x;
		    if (a == 0xff) {
			if (len == 1)
			    *d = *s;
			else
			    memcpy(d, s, len * 4);
		    } else {
			while (len-- > 0) {
			    *d = lerp8x4 (*s, a, *d);
			    s++, d++;
			}
		    }
		    yy++;
		} while (--hh);
	    }
	    spans++;
	} while (--num_spans > 1);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_inplace_spans (void *abstract_renderer,
		int y, int h,
		const cairo_half_open_span_t *spans,
		unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;
    uint8_t *mask;
    int x0, x1;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    if (num_spans == 2 && spans[0].coverage == 0xff) {
	pixman_image_composite32 (r->op, r->src, NULL, r->u.composite.dst,
				  spans[0].x + r->u.composite.src_x,
				  y + r->u.composite.src_y,
				  0, 0,
				  spans[0].x, y,
				  spans[1].x - spans[0].x, h);
	return CAIRO_STATUS_SUCCESS;
    }

    mask = (uint8_t *)pixman_image_get_data (r->mask);
    x1 = x0 = spans[0].x;
    do {
	int len = spans[1].x - spans[0].x;
	*mask++ = spans[0].coverage;
	if (len > 1) {
	    if (len >= r->u.composite.run_length && spans[0].coverage == 0xff) {
		if (x1 != x0) {
		    pixman_image_composite32 (r->op, r->src, r->mask, r->u.composite.dst,
					      x0 + r->u.composite.src_x,
					      y + r->u.composite.src_y,
					      0, 0,
					      x0, y,
					      x1 - x0, h);
		}
		pixman_image_composite32 (r->op, r->src, NULL, r->u.composite.dst,
					  spans[0].x + r->u.composite.src_x,
					  y + r->u.composite.src_y,
					  0, 0,
					  spans[0].x, y,
					  len, h);
		mask = (uint8_t *)pixman_image_get_data (r->mask);
		x0 = spans[1].x;
	    } else if (spans[0].coverage == 0x0 &&
		       x1 - x0 > r->u.composite.run_length) {
		pixman_image_composite32 (r->op, r->src, r->mask, r->u.composite.dst,
					  x0 + r->u.composite.src_x,
					  y + r->u.composite.src_y,
					  0, 0,
					  x0, y,
					  x1 - x0, h);
		mask = (uint8_t *)pixman_image_get_data (r->mask);
		x0 = spans[1].x;
	    }else {
		memset (mask, spans[0].coverage, --len);
		mask += len;
	    }
	}
	x1 = spans[1].x;
	spans++;
    } while (--num_spans > 1);

    if (x1 != x0) {
	pixman_image_composite32 (r->op, r->src, r->mask, r->u.composite.dst,
				  x0 + r->u.composite.src_x,
				  y + r->u.composite.src_y,
				  0, 0,
				  x0, y,
				  x1 - x0, h);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_inplace_opacity_spans (void *abstract_renderer, int y, int h,
			const cairo_half_open_span_t *spans,
			unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;
    uint8_t *mask;
    int x0, x1;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    mask = (uint8_t *)pixman_image_get_data (r->mask);
    x1 = x0 = spans[0].x;
    do {
	int len = spans[1].x - spans[0].x;
	uint8_t m = mul8_8(spans[0].coverage, r->bpp);
	*mask++ = m;
	if (len > 1) {
	    if (m == 0 &&
		x1 - x0 > r->u.composite.run_length) {
		pixman_image_composite32 (r->op, r->src, r->mask, r->u.composite.dst,
					  x0 + r->u.composite.src_x,
					  y + r->u.composite.src_y,
					  0, 0,
					  x0, y,
					  x1 - x0, h);
		mask = (uint8_t *)pixman_image_get_data (r->mask);
		x0 = spans[1].x;
	    }else {
		memset (mask, m, --len);
		mask += len;
	    }
	}
	x1 = spans[1].x;
	spans++;
    } while (--num_spans > 1);

    if (x1 != x0) {
	pixman_image_composite32 (r->op, r->src, r->mask, r->u.composite.dst,
				  x0 + r->u.composite.src_x,
				  y + r->u.composite.src_y,
				  0, 0,
				  x0, y,
				  x1 - x0, h);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_inplace_src_spans (void *abstract_renderer, int y, int h,
		    const cairo_half_open_span_t *spans,
		    unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;
    uint8_t *m, *base = (uint8_t*)pixman_image_get_data(r->mask);
    int x0;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    x0 = spans[0].x;
    m = base;
    do {
	int len = spans[1].x - spans[0].x;
	if (len >= r->u.composite.run_length && spans[0].coverage == 0xff) {
	    if (spans[0].x != x0) {
#if PIXMAN_HAS_OP_LERP
		pixman_image_composite32 (PIXMAN_OP_LERP_SRC,
					  r->src, r->mask, r->u.composite.dst,
					  x0 + r->u.composite.src_x,
					  y + r->u.composite.src_y,
					  0, 0,
					  x0, y,
					  spans[0].x - x0, h);
#else
		pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
					  r->mask, NULL, r->u.composite.dst,
					  0, 0,
					  0, 0,
					  x0, y,
					  spans[0].x - x0, h);
		pixman_image_composite32 (PIXMAN_OP_ADD,
					  r->src, r->mask, r->u.composite.dst,
					  x0 + r->u.composite.src_x,
					  y + r->u.composite.src_y,
					  0, 0,
					  x0, y,
					  spans[0].x - x0, h);
#endif
	    }

	    pixman_image_composite32 (PIXMAN_OP_SRC,
				      r->src, NULL, r->u.composite.dst,
				      spans[0].x + r->u.composite.src_x,
				      y + r->u.composite.src_y,
				      0, 0,
				      spans[0].x, y,
				      spans[1].x - spans[0].x, h);

	    m = base;
	    x0 = spans[1].x;
	} else if (spans[0].coverage == 0x0) {
	    if (spans[0].x != x0) {
#if PIXMAN_HAS_OP_LERP
		pixman_image_composite32 (PIXMAN_OP_LERP_SRC,
					  r->src, r->mask, r->u.composite.dst,
					  x0 + r->u.composite.src_x,
					  y + r->u.composite.src_y,
					  0, 0,
					  x0, y,
					  spans[0].x - x0, h);
#else
		pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
					  r->mask, NULL, r->u.composite.dst,
					  0, 0,
					  0, 0,
					  x0, y,
					  spans[0].x - x0, h);
		pixman_image_composite32 (PIXMAN_OP_ADD,
					  r->src, r->mask, r->u.composite.dst,
					  x0 + r->u.composite.src_x,
					  y + r->u.composite.src_y,
					  0, 0,
					  x0, y,
					  spans[0].x - x0, h);
#endif
	    }

	    m = base;
	    x0 = spans[1].x;
	} else {
	    *m++ = spans[0].coverage;
	    if (len > 1) {
		memset (m, spans[0].coverage, --len);
		m += len;
	    }
	}
	spans++;
    } while (--num_spans > 1);

    if (spans[0].x != x0) {
#if PIXMAN_HAS_OP_LERP
	pixman_image_composite32 (PIXMAN_OP_LERP_SRC,
				  r->src, r->mask, r->u.composite.dst,
				  x0 + r->u.composite.src_x,
				  y + r->u.composite.src_y,
				  0, 0,
				  x0, y,
				  spans[0].x - x0, h);
#else
	pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
				  r->mask, NULL, r->u.composite.dst,
				  0, 0,
				  0, 0,
				  x0, y,
				  spans[0].x - x0, h);
	pixman_image_composite32 (PIXMAN_OP_ADD,
				  r->src, r->mask, r->u.composite.dst,
				  x0 + r->u.composite.src_x,
				  y + r->u.composite.src_y,
				  0, 0,
				  x0, y,
				  spans[0].x - x0, h);
#endif
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_inplace_src_opacity_spans (void *abstract_renderer, int y, int h,
			    const cairo_half_open_span_t *spans,
			    unsigned num_spans)
{
    cairo_image_span_renderer_t *r = abstract_renderer;
    uint8_t *mask;
    int x0;

    if (num_spans == 0)
	return CAIRO_STATUS_SUCCESS;

    x0 = spans[0].x;
    mask = (uint8_t *)pixman_image_get_data (r->mask);
    do {
	int len = spans[1].x - spans[0].x;
	uint8_t m = mul8_8(spans[0].coverage, r->bpp);
	if (m == 0) {
	    if (spans[0].x != x0) {
#if PIXMAN_HAS_OP_LERP
		pixman_image_composite32 (PIXMAN_OP_LERP_SRC,
					  r->src, r->mask, r->u.composite.dst,
					  x0 + r->u.composite.src_x,
					  y + r->u.composite.src_y,
					  0, 0,
					  x0, y,
					  spans[0].x - x0, h);
#else
		pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
					  r->mask, NULL, r->u.composite.dst,
					  0, 0,
					  0, 0,
					  x0, y,
					  spans[0].x - x0, h);
		pixman_image_composite32 (PIXMAN_OP_ADD,
					  r->src, r->mask, r->u.composite.dst,
					  x0 + r->u.composite.src_x,
					  y + r->u.composite.src_y,
					  0, 0,
					  x0, y,
					  spans[0].x - x0, h);
#endif
	    }

	    mask = (uint8_t *)pixman_image_get_data (r->mask);
	    x0 = spans[1].x;
	} else {
	    *mask++ = m;
	    if (len > 1) {
		memset (mask, m, --len);
		mask += len;
	    }
	}
	spans++;
    } while (--num_spans > 1);

    if (spans[0].x != x0) {
#if PIXMAN_HAS_OP_LERP
	pixman_image_composite32 (PIXMAN_OP_LERP_SRC,
				  r->src, r->mask, r->u.composite.dst,
				  x0 + r->u.composite.src_x,
				  y + r->u.composite.src_y,
				  0, 0,
				  x0, y,
				  spans[0].x - x0, h);
#else
	pixman_image_composite32 (PIXMAN_OP_OUT_REVERSE,
				  r->mask, NULL, r->u.composite.dst,
				  0, 0,
				  0, 0,
				  x0, y,
				  spans[0].x - x0, h);
	pixman_image_composite32 (PIXMAN_OP_ADD,
				  r->src, r->mask, r->u.composite.dst,
				  x0 + r->u.composite.src_x,
				  y + r->u.composite.src_y,
				  0, 0,
				  x0, y,
				  spans[0].x - x0, h);
#endif
    }

    return CAIRO_STATUS_SUCCESS;
}

static void free_pixels (pixman_image_t *image, void *data)
{
	free (data);
}

static cairo_int_status_t
inplace_renderer_init (cairo_image_span_renderer_t	*r,
		       const cairo_composite_rectangles_t *composite,
		       cairo_antialias_t		 antialias,
		       cairo_bool_t			 needs_clip)
{
    cairo_image_surface_t *dst = (cairo_image_surface_t *)composite->surface;
    uint8_t *buf;

    if (composite->mask_pattern.base.type != CAIRO_PATTERN_TYPE_SOLID)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    r->base.render_rows = NULL;
    r->bpp = composite->mask_pattern.solid.color.alpha_short >> 8;

    if (composite->source_pattern.base.type == CAIRO_PATTERN_TYPE_SOLID) {
	const cairo_color_t *color;

	color = &composite->source_pattern.solid.color;
	if (composite->op == CAIRO_OPERATOR_CLEAR)
	    color = CAIRO_COLOR_TRANSPARENT;

	if (fill_reduces_to_source (composite->op, color, dst, &r->u.fill.pixel)) {
	    /* Use plain C for the fill operations as the span length is
	     * typically small, too small to payback the startup overheads of
	     * using SSE2 etc.
	     */
	    if (r->bpp == 0xff) {
		switch (dst->format) {
		case CAIRO_FORMAT_A8:
		    r->base.render_rows = _fill_a8_lerp_opaque_spans;
		    break;
		case CAIRO_FORMAT_RGB24:
		case CAIRO_FORMAT_ARGB32:
		    r->base.render_rows = _fill_xrgb32_lerp_opaque_spans;
		    break;
		case CAIRO_FORMAT_A1:
		case CAIRO_FORMAT_RGB16_565:
		case CAIRO_FORMAT_RGB30:
		case CAIRO_FORMAT_RGB96F:
		case CAIRO_FORMAT_RGBA128F:
		case CAIRO_FORMAT_INVALID:
		default: break;
		}
	    } else {
		switch (dst->format) {
		case CAIRO_FORMAT_A8:
		    r->base.render_rows = _fill_a8_lerp_spans;
		    break;
		case CAIRO_FORMAT_RGB24:
		case CAIRO_FORMAT_ARGB32:
		    r->base.render_rows = _fill_xrgb32_lerp_spans;
		    break;
		case CAIRO_FORMAT_A1:
		case CAIRO_FORMAT_RGB16_565:
		case CAIRO_FORMAT_RGB30:
		case CAIRO_FORMAT_RGB96F:
		case CAIRO_FORMAT_RGBA128F:
		case CAIRO_FORMAT_INVALID:
		default: break;
		}
	    }
	    r->u.fill.data = dst->data;
	    r->u.fill.stride = dst->stride;
	}
    } else if ((dst->format == CAIRO_FORMAT_ARGB32 || dst->format == CAIRO_FORMAT_RGB24) &&
	       (composite->op == CAIRO_OPERATOR_SOURCE ||
		(composite->op == CAIRO_OPERATOR_OVER &&
		 (dst->base.is_clear || (dst->base.content & CAIRO_CONTENT_ALPHA) == 0))) &&
	       composite->source_pattern.base.type == CAIRO_PATTERN_TYPE_SURFACE &&
	       composite->source_pattern.surface.surface->backend->type == CAIRO_SURFACE_TYPE_IMAGE &&
	       to_image_surface(composite->source_pattern.surface.surface)->format == dst->format)
    {
       cairo_image_surface_t *src =
	   to_image_surface(composite->source_pattern.surface.surface);
       int tx, ty;

	if (_cairo_matrix_is_integer_translation(&composite->source_pattern.base.matrix,
						 &tx, &ty) &&
	    composite->bounded.x + tx >= 0 &&
	    composite->bounded.y + ty >= 0 &&
	    composite->bounded.x + composite->bounded.width + tx <= src->width &&
	    composite->bounded.y + composite->bounded.height + ty <= src->height) {

	    assert(PIXMAN_FORMAT_BPP(dst->pixman_format) == 32);
	    r->u.blit.stride = dst->stride;
	    r->u.blit.data = dst->data;
	    r->u.blit.src_stride = src->stride;
	    r->u.blit.src_data = src->data + src->stride * ty + tx * 4;
	    r->base.render_rows = _blit_xrgb32_lerp_spans;
	}
    }
    if (r->base.render_rows == NULL) {
	const cairo_pattern_t *src = &composite->source_pattern.base;
	unsigned int width;

	if (composite->is_bounded == 0)
	    return CAIRO_INT_STATUS_UNSUPPORTED;

	r->base.render_rows = r->bpp == 0xff ? _inplace_spans : _inplace_opacity_spans;
	width = (composite->bounded.width + 3) & ~3;

	r->u.composite.run_length = 8;
	if (src->type == CAIRO_PATTERN_TYPE_LINEAR ||
	    src->type == CAIRO_PATTERN_TYPE_RADIAL)
		r->u.composite.run_length = 256;
	if (dst->base.is_clear &&
	    (composite->op == CAIRO_OPERATOR_SOURCE ||
	     composite->op == CAIRO_OPERATOR_OVER ||
	     composite->op == CAIRO_OPERATOR_ADD)) {
	    r->op = PIXMAN_OP_SRC;
	} else if (composite->op == CAIRO_OPERATOR_SOURCE) {
	    r->base.render_rows = r->bpp == 0xff ? _inplace_src_spans : _inplace_src_opacity_spans;
	    r->u.composite.mask_y = r->composite->unbounded.y;
	    width = (composite->unbounded.width + 3) & ~3;
	} else if (composite->op == CAIRO_OPERATOR_CLEAR) {
	    r->op = PIXMAN_OP_OUT_REVERSE;
	    src = NULL;
	} else {
	    r->op = _pixman_operator (composite->op);
	}

	r->src = _pixman_image_for_pattern (dst, src, FALSE,
					    &composite->bounded,
					    &composite->source_sample_area,
					    &r->u.composite.src_x, &r->u.composite.src_y);
	if (unlikely (r->src == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	/* Create an effectively unbounded mask by repeating the single line */
	buf = r->_buf;
	if (width > SZ_BUF) {
	    buf = _cairo_malloc (width);
	    if (unlikely (buf == NULL)) {
		pixman_image_unref (r->src);
		return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    }
	}
	r->mask = pixman_image_create_bits (PIXMAN_a8,
					    width, composite->unbounded.height,
					    (uint32_t *)buf, 0);
	if (unlikely (r->mask == NULL)) {
	    pixman_image_unref (r->src);
	    if (buf != r->_buf)
		free (buf);
	    return _cairo_error(CAIRO_STATUS_NO_MEMORY);
	}

	if (buf != r->_buf)
	    pixman_image_set_destroy_function (r->mask, free_pixels, buf);

	r->u.composite.dst = dst->pixman_image;
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
span_renderer_init (cairo_abstract_span_renderer_t	*_r,
		    const cairo_composite_rectangles_t *composite,
		    cairo_antialias_t			 antialias,
		    cairo_bool_t			 needs_clip)
{
    cairo_image_span_renderer_t *r = (cairo_image_span_renderer_t *)_r;
    cairo_image_surface_t *dst = (cairo_image_surface_t *)composite->surface;
    const cairo_pattern_t *source = &composite->source_pattern.base;
    cairo_operator_t op = composite->op;
    cairo_int_status_t status;

    TRACE ((stderr, "%s: antialias=%d, needs_clip=%d\n", __FUNCTION__,
	    antialias, needs_clip));

    if (needs_clip)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    r->composite = composite;
    r->mask = NULL;
    r->src = NULL;
    r->base.finish = NULL;

    status = mono_renderer_init (r, composite, antialias, needs_clip);
    if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	return status;

    status = inplace_renderer_init (r, composite, antialias, needs_clip);
    if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	return status;

    r->bpp = 0;

    if (op == CAIRO_OPERATOR_CLEAR) {
#if PIXMAN_HAS_OP_LERP
	op = PIXMAN_OP_LERP_CLEAR;
#else
	source = &_cairo_pattern_white.base;
	op = PIXMAN_OP_OUT_REVERSE;
#endif
    } else if (dst->base.is_clear &&
	       (op == CAIRO_OPERATOR_SOURCE ||
		op == CAIRO_OPERATOR_OVER ||
		op == CAIRO_OPERATOR_ADD)) {
	op = PIXMAN_OP_SRC;
    } else if (op == CAIRO_OPERATOR_SOURCE) {
	if (_cairo_pattern_is_opaque (&composite->source_pattern.base,
				      &composite->source_sample_area))
	{
	    op = PIXMAN_OP_OVER;
	}
	else
	{
#if PIXMAN_HAS_OP_LERP
	    op = PIXMAN_OP_LERP_SRC;
#else
	    return CAIRO_INT_STATUS_UNSUPPORTED;
#endif
	}
    } else {
	op = _pixman_operator (op);
    }
    r->op = op;

    r->src = _pixman_image_for_pattern (dst, source, FALSE,
					&composite->unbounded,
					&composite->source_sample_area,
					&r->u.mask.src_x, &r->u.mask.src_y);
    if (unlikely (r->src == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    r->opacity = 1.0;
    if (composite->mask_pattern.base.type == CAIRO_PATTERN_TYPE_SOLID) {
	r->opacity = composite->mask_pattern.solid.color.alpha;
    } else {
	pixman_image_t *mask;
	int mask_x, mask_y;

	mask = _pixman_image_for_pattern (dst,
					  &composite->mask_pattern.base,
					  TRUE,
					  &composite->unbounded,
					  &composite->mask_sample_area,
					  &mask_x, &mask_y);
	if (unlikely (mask == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	/* XXX Component-alpha? */
	if ((dst->base.content & CAIRO_CONTENT_COLOR) == 0 &&
	    _cairo_pattern_is_opaque (source, &composite->source_sample_area))
	{
	    pixman_image_unref (r->src);
	    r->src = mask;
	    r->u.mask.src_x = mask_x;
	    r->u.mask.src_y = mask_y;
	    mask = NULL;
	}

	if (mask) {
	    pixman_image_unref (mask);
	    return CAIRO_INT_STATUS_UNSUPPORTED;
	}
    }

    r->u.mask.extents = composite->unbounded;
    r->u.mask.stride = (r->u.mask.extents.width + 3) & ~3;
    if (r->u.mask.extents.height * r->u.mask.stride > SZ_BUF) {
	r->mask = pixman_image_create_bits (PIXMAN_a8,
					    r->u.mask.extents.width,
					    r->u.mask.extents.height,
					    NULL, 0);

	r->base.render_rows = _cairo_image_spans;
	r->base.finish = NULL;
    } else {
	r->mask = pixman_image_create_bits (PIXMAN_a8,
					    r->u.mask.extents.width,
					    r->u.mask.extents.height,
					    (uint32_t *)r->_buf, r->u.mask.stride);

	r->base.render_rows = _cairo_image_spans_and_zero;
	r->base.finish = _cairo_image_finish_spans_and_zero;
    }
    if (unlikely (r->mask == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    r->u.mask.data = (uint8_t *) pixman_image_get_data (r->mask);
    r->u.mask.stride = pixman_image_get_stride (r->mask);

    r->u.mask.extents.height += r->u.mask.extents.y;
    return CAIRO_STATUS_SUCCESS;
}

static void
span_renderer_fini (cairo_abstract_span_renderer_t *_r,
		    cairo_int_status_t status)
{
    cairo_image_span_renderer_t *r = (cairo_image_span_renderer_t *) _r;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (likely (status == CAIRO_INT_STATUS_SUCCESS)) {
	if (r->base.finish)
	    r->base.finish (r);
    }
    if (likely (status == CAIRO_INT_STATUS_SUCCESS && r->bpp == 0)) {
	const cairo_composite_rectangles_t *composite = r->composite;

	pixman_image_composite32 (r->op, r->src, r->mask,
				  to_pixman_image (composite->surface),
				  composite->unbounded.x + r->u.mask.src_x,
				  composite->unbounded.y + r->u.mask.src_y,
				  0, 0,
				  composite->unbounded.x,
				  composite->unbounded.y,
				  composite->unbounded.width,
				  composite->unbounded.height);
    }

    if (r->src)
	pixman_image_unref (r->src);
    if (r->mask)
	pixman_image_unref (r->mask);
}
#endif

const cairo_compositor_t *
_cairo_image_spans_compositor_get (void)
{
    static cairo_atomic_once_t once = CAIRO_ATOMIC_ONCE_INIT;
    static cairo_spans_compositor_t spans;
    static cairo_compositor_t shape;

    if (_cairo_atomic_init_once_enter(&once)) {
	_cairo_shape_mask_compositor_init (&shape,
					   _cairo_image_traps_compositor_get());
	shape.glyphs = NULL;

	_cairo_spans_compositor_init (&spans, &shape);

	spans.flags = 0;
#if PIXMAN_HAS_OP_LERP
	spans.flags |= CAIRO_SPANS_COMPOSITOR_HAS_LERP;
#endif

	//spans.acquire = acquire;
	//spans.release = release;
	spans.fill_boxes = fill_boxes;
	spans.draw_image_boxes = draw_image_boxes;
	//spans.copy_boxes = copy_boxes;
	spans.pattern_to_surface = _cairo_image_source_create_for_pattern;
	//spans.check_composite_boxes = check_composite_boxes;
	spans.composite_boxes = composite_boxes;
	//spans.check_span_renderer = check_span_renderer;
	spans.renderer_init = span_renderer_init;
	spans.renderer_fini = span_renderer_fini;

	_cairo_atomic_init_once_leave(&once);
    }

    return &spans.base;
}
