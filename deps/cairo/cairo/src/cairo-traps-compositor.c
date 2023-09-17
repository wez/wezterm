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

#include "cairo-box-inline.h"
#include "cairo-boxes-private.h"
#include "cairo-clip-inline.h"
#include "cairo-clip-private.h"
#include "cairo-composite-rectangles-private.h"
#include "cairo-compositor-private.h"
#include "cairo-error-private.h"
#include "cairo-image-surface-private.h"
#include "cairo-pattern-inline.h"
#include "cairo-paginated-private.h"
#include "cairo-recording-surface-inline.h"
#include "cairo-surface-subsurface-private.h"
#include "cairo-surface-snapshot-inline.h"
#include "cairo-surface-observer-private.h"
#include "cairo-region-private.h"
#include "cairo-spans-private.h"
#include "cairo-traps-private.h"
#include "cairo-tristrip-private.h"

typedef cairo_int_status_t
(*draw_func_t) (const cairo_traps_compositor_t *compositor,
		cairo_surface_t			*dst,
		void				*closure,
		cairo_operator_t		 op,
		cairo_surface_t		*src,
		int				 src_x,
		int				 src_y,
		int				 dst_x,
		int				 dst_y,
		const cairo_rectangle_int_t	*extents,
		cairo_clip_t			*clip);

static void do_unaligned_row(void (*blt)(void *closure,
					 int16_t x, int16_t y,
					 int16_t w, int16_t h,
					 uint16_t coverage),
			     void *closure,
			     const cairo_box_t *b,
			     int tx, int y, int h,
			     uint16_t coverage)
{
    int x1 = _cairo_fixed_integer_part (b->p1.x) - tx;
    int x2 = _cairo_fixed_integer_part (b->p2.x) - tx;
    if (x2 > x1) {
	if (! _cairo_fixed_is_integer (b->p1.x)) {
	    blt(closure, x1, y, 1, h,
		coverage * (256 - _cairo_fixed_fractional_part (b->p1.x)));
	    x1++;
	}

	if (x2 > x1)
	    blt(closure, x1, y, x2-x1, h, (coverage << 8) - (coverage >> 8));

	if (! _cairo_fixed_is_integer (b->p2.x))
	    blt(closure, x2, y, 1, h,
		coverage * _cairo_fixed_fractional_part (b->p2.x));
    } else
	blt(closure, x1, y, 1, h,
	    coverage * (b->p2.x - b->p1.x));
}

static void do_unaligned_box(void (*blt)(void *closure,
					 int16_t x, int16_t y,
					 int16_t w, int16_t h,
					 uint16_t coverage),
			     void *closure,
			     const cairo_box_t *b, int tx, int ty)
{
    int y1 = _cairo_fixed_integer_part (b->p1.y) - ty;
    int y2 = _cairo_fixed_integer_part (b->p2.y) - ty;
    if (y2 > y1) {
	if (! _cairo_fixed_is_integer (b->p1.y)) {
	    do_unaligned_row(blt, closure, b, tx, y1, 1,
			     256 - _cairo_fixed_fractional_part (b->p1.y));
	    y1++;
	}

	if (y2 > y1)
	    do_unaligned_row(blt, closure, b, tx, y1, y2-y1, 256);

	if (! _cairo_fixed_is_integer (b->p2.y))
	    do_unaligned_row(blt, closure, b, tx, y2, 1,
			     _cairo_fixed_fractional_part (b->p2.y));
    } else
	do_unaligned_row(blt, closure, b, tx, y1, 1,
			 b->p2.y - b->p1.y);
}

struct blt_in {
    const cairo_traps_compositor_t *compositor;
    cairo_surface_t *dst;
    cairo_boxes_t boxes;
};

static void blt_in(void *closure,
		   int16_t x, int16_t y,
		   int16_t w, int16_t h,
		   uint16_t coverage)
{
    struct blt_in *info = closure;
    cairo_color_t color;

    if (CAIRO_ALPHA_SHORT_IS_OPAQUE (coverage))
	return;

    _cairo_box_from_integers (&info->boxes.chunks.base[0], x, y, w, h);

    _cairo_color_init_rgba (&color, 0, 0, 0, coverage / (double) 0xffff);
    info->compositor->fill_boxes (info->dst,
				  CAIRO_OPERATOR_IN, &color,
				  &info->boxes);
}

static void
add_rect_with_offset (cairo_boxes_t *boxes, int x1, int y1, int x2, int y2, int dx, int dy)
{
    cairo_box_t box;
    cairo_int_status_t status;

    box.p1.x = _cairo_fixed_from_int (x1 - dx);
    box.p1.y = _cairo_fixed_from_int (y1 - dy);
    box.p2.x = _cairo_fixed_from_int (x2 - dx);
    box.p2.y = _cairo_fixed_from_int (y2 - dy);

    status = _cairo_boxes_add (boxes, CAIRO_ANTIALIAS_DEFAULT, &box);
    assert (status == CAIRO_INT_STATUS_SUCCESS);
}

static cairo_int_status_t
combine_clip_as_traps (const cairo_traps_compositor_t *compositor,
		       cairo_surface_t *mask,
		       const cairo_clip_t *clip,
		       const cairo_rectangle_int_t *extents)
{
    cairo_polygon_t polygon;
    cairo_fill_rule_t fill_rule;
    cairo_antialias_t antialias;
    cairo_traps_t traps;
    cairo_surface_t *src;
    cairo_box_t box;
    cairo_rectangle_int_t fixup;
    int src_x, src_y;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = _cairo_clip_get_polygon (clip, &polygon,
				      &fill_rule, &antialias);
    if (status)
	return status;

    _cairo_traps_init (&traps);
    status = _cairo_bentley_ottmann_tessellate_polygon (&traps,
							&polygon,
							fill_rule);
    _cairo_polygon_fini (&polygon);
    if (unlikely (status))
	return status;

    src = compositor->pattern_to_surface (mask, NULL, FALSE,
					  extents, NULL,
					  &src_x, &src_y);
    if (unlikely (src->status)) {
	_cairo_traps_fini (&traps);
	return src->status;
    }

    status = compositor->composite_traps (mask, CAIRO_OPERATOR_IN, src,
					  src_x, src_y,
					  extents->x, extents->y,
					  extents,
					  antialias, &traps);

    _cairo_traps_extents (&traps, &box);
    _cairo_box_round_to_rectangle (&box, &fixup);
    _cairo_traps_fini (&traps);
    cairo_surface_destroy (src);

    if (unlikely (status))
	return status;

    if (! _cairo_rectangle_intersect (&fixup, extents))
	return CAIRO_STATUS_SUCCESS;

    if (fixup.width < extents->width || fixup.height < extents->height) {
	cairo_boxes_t clear;

	_cairo_boxes_init (&clear);

	/* top */
	if (fixup.y != extents->y) {
	    add_rect_with_offset (&clear,
				  extents->x, extents->y,
				  extents->x + extents->width,
				  fixup.y,
				  extents->x, extents->y);
	}
	/* left */
	if (fixup.x != extents->x) {
	    add_rect_with_offset (&clear,
				  extents->x, fixup.y,
				  fixup.x,
				  fixup.y + fixup.height,
				  extents->x, extents->y);
	}
	/* right */
	if (fixup.x + fixup.width != extents->x + extents->width) {
	    add_rect_with_offset (&clear,
				  fixup.x + fixup.width,
				  fixup.y,
				  extents->x + extents->width,
				  fixup.y + fixup.height,
				  extents->x, extents->y);
	}
	/* bottom */
	if (fixup.y + fixup.height != extents->y + extents->height) {
	    add_rect_with_offset (&clear,
				  extents->x,
				  fixup.y + fixup.height,
				  extents->x + extents->width,
				  extents->y + extents->height,
				  extents->x, extents->y);
	}

	status = compositor->fill_boxes (mask,
					 CAIRO_OPERATOR_CLEAR,
					 CAIRO_COLOR_TRANSPARENT,
					 &clear);

	_cairo_boxes_fini (&clear);
    }

    return status;
}

static cairo_status_t
__clip_to_surface (const cairo_traps_compositor_t *compositor,
		   const cairo_composite_rectangles_t *composite,
		   const cairo_rectangle_int_t *extents,
		   cairo_surface_t **surface)
{
    cairo_surface_t *mask;
    cairo_polygon_t polygon;
    cairo_fill_rule_t fill_rule;
    cairo_antialias_t antialias;
    cairo_traps_t traps;
    cairo_boxes_t clear;
    cairo_surface_t *src;
    int src_x, src_y;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = _cairo_clip_get_polygon (composite->clip, &polygon,
				      &fill_rule, &antialias);
    if (status)
	return status;

    _cairo_traps_init (&traps);
    status = _cairo_bentley_ottmann_tessellate_polygon (&traps,
							&polygon,
							fill_rule);
    _cairo_polygon_fini (&polygon);
    if (unlikely (status))
	return status;

    mask = _cairo_surface_create_scratch (composite->surface,
					  CAIRO_CONTENT_ALPHA,
					  extents->width,
					  extents->height,
					  NULL);
    if (unlikely (mask->status)) {
	_cairo_traps_fini (&traps);
	return status;
    }

    src = compositor->pattern_to_surface (mask, NULL, FALSE,
					  extents, NULL,
					  &src_x, &src_y);
    if (unlikely (status = src->status))
	goto error;

    status = compositor->acquire (mask);
    if (unlikely (status))
	goto error;

    _cairo_boxes_init_from_rectangle (&clear,
				      0, 0,
				      extents->width,
				      extents->height);
    status = compositor->fill_boxes (mask,
				     CAIRO_OPERATOR_CLEAR,
				     CAIRO_COLOR_TRANSPARENT,
				     &clear);
    if (unlikely (status))
	goto error_release;

    status = compositor->composite_traps (mask, CAIRO_OPERATOR_ADD, src,
					  src_x, src_y,
					  extents->x, extents->y,
					  extents,
					  antialias, &traps);
    if (unlikely (status))
	goto error_release;

    compositor->release (mask);
    *surface = mask;
out:
    cairo_surface_destroy (src);
    _cairo_traps_fini (&traps);
    return status;

error_release:
    compositor->release (mask);
error:
    cairo_surface_destroy (mask);
    goto out;
}

static cairo_surface_t *
traps_get_clip_surface (const cairo_traps_compositor_t *compositor,
			const cairo_composite_rectangles_t *composite,
			const cairo_rectangle_int_t *extents)
{
    cairo_surface_t *surface = NULL;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = __clip_to_surface (compositor, composite, extents, &surface);
    if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	surface = _cairo_surface_create_scratch (composite->surface,
						 CAIRO_CONTENT_ALPHA,
						 extents->width,
						 extents->height,
						 CAIRO_COLOR_WHITE);
	if (unlikely (surface->status))
	    return surface;

	status = _cairo_clip_combine_with_surface (composite->clip, surface,
						   extents->x, extents->y);
    }
    if (unlikely (status)) {
	cairo_surface_destroy (surface);
	surface = _cairo_surface_create_in_error (status);
    }

    return surface;
}

static void blt_unaligned_boxes(const cairo_traps_compositor_t *compositor,
				cairo_surface_t *surface,
				int dx, int dy,
				cairo_box_t *boxes,
				int num_boxes)
{
    struct blt_in info;
    int i;

    info.compositor = compositor;
    info.dst = surface;
    _cairo_boxes_init (&info.boxes);
    info.boxes.num_boxes = 1;
    for (i = 0; i < num_boxes; i++) {
	cairo_box_t *b = &boxes[i];

	if (! _cairo_fixed_is_integer (b->p1.x) ||
	    ! _cairo_fixed_is_integer (b->p1.y) ||
	    ! _cairo_fixed_is_integer (b->p2.x) ||
	    ! _cairo_fixed_is_integer (b->p2.y))
	{
	    do_unaligned_box(blt_in, &info, b, dx, dy);
	}
    }
}

static cairo_surface_t *
create_composite_mask (const cairo_traps_compositor_t *compositor,
		       cairo_surface_t		*dst,
		       void			*draw_closure,
		       draw_func_t		 draw_func,
		       draw_func_t		 mask_func,
		       const cairo_composite_rectangles_t *extents)
{
    cairo_surface_t *surface, *src;
    cairo_int_status_t status;
    int src_x, src_y;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    surface = _cairo_surface_create_scratch (dst, CAIRO_CONTENT_ALPHA,
					     extents->bounded.width,
					     extents->bounded.height,
					     NULL);
    if (unlikely (surface->status))
	return surface;

    src = compositor->pattern_to_surface (surface,
					  &_cairo_pattern_white.base,
					  FALSE,
					  &extents->bounded,
					  &extents->bounded,
					  &src_x, &src_y);
    if (unlikely (src->status)) {
	cairo_surface_destroy (surface);
	return src;
    }

    status = compositor->acquire (surface);
    if (unlikely (status)) {
	cairo_surface_destroy (src);
	cairo_surface_destroy (surface);
	return _cairo_surface_create_in_error (status);
    }

    if (!surface->is_clear) {
	cairo_boxes_t clear;

	_cairo_boxes_init_from_rectangle (&clear,
					  0, 0,
					  extents->bounded.width,
					  extents->bounded.height);
	status = compositor->fill_boxes (surface,
					 CAIRO_OPERATOR_CLEAR,
					 CAIRO_COLOR_TRANSPARENT,
					 &clear);
	if (unlikely (status))
	    goto error;

	surface->is_clear = TRUE;
    }

    if (mask_func) {
	status = mask_func (compositor, surface, draw_closure,
			    CAIRO_OPERATOR_SOURCE, src, src_x, src_y,
			    extents->bounded.x, extents->bounded.y,
			    &extents->bounded, extents->clip);
	if (likely (status == CAIRO_INT_STATUS_SUCCESS)) {
	    surface->is_clear = FALSE;
	    goto out;
	}
	if (unlikely (status != CAIRO_INT_STATUS_UNSUPPORTED))
	    goto error;
    }

    /* Is it worth setting the clip region here? */
    status = draw_func (compositor, surface, draw_closure,
			CAIRO_OPERATOR_ADD, src, src_x, src_y,
			extents->bounded.x, extents->bounded.y,
			&extents->bounded, NULL);
    if (unlikely (status))
	goto error;

    surface->is_clear = FALSE;
    if (extents->clip->path != NULL) {
	status = combine_clip_as_traps (compositor, surface,
					extents->clip, &extents->bounded);
	if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	    status = _cairo_clip_combine_with_surface (extents->clip, surface,
						       extents->bounded.x,
						       extents->bounded.y);
	}
	if (unlikely (status))
	    goto error;
    } else if (extents->clip->boxes) {
	blt_unaligned_boxes(compositor, surface,
			    extents->bounded.x, extents->bounded.y,
			    extents->clip->boxes, extents->clip->num_boxes);

    }

out:
    compositor->release (surface);
    cairo_surface_destroy (src);
    return surface;

error:
    compositor->release (surface);
    if (status != CAIRO_INT_STATUS_NOTHING_TO_DO) {
	cairo_surface_destroy (surface);
	surface = _cairo_surface_create_in_error (status);
    }
    cairo_surface_destroy (src);
    return surface;
}

/* Handles compositing with a clip surface when the operator allows
 * us to combine the clip with the mask
 */
static cairo_status_t
clip_and_composite_with_mask (const cairo_traps_compositor_t *compositor,
			      const cairo_composite_rectangles_t*extents,
			      draw_func_t		 draw_func,
			      draw_func_t		 mask_func,
			      void			*draw_closure,
			      cairo_operator_t		 op,
			      cairo_surface_t	*src,
			      int src_x, int src_y)
{
    cairo_surface_t *dst = extents->surface;
    cairo_surface_t *mask;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    mask = create_composite_mask (compositor, dst, draw_closure,
				  draw_func, mask_func,
				  extents);
    if (unlikely (mask->status))
	return mask->status;

    if (mask->is_clear)
	goto skip;

    if (src != NULL || dst->content != CAIRO_CONTENT_ALPHA) {
	compositor->composite (dst, op, src, mask,
			       extents->bounded.x + src_x,
			       extents->bounded.y + src_y,
			       0, 0,
			       extents->bounded.x,      extents->bounded.y,
			       extents->bounded.width,  extents->bounded.height);
    } else {
	compositor->composite (dst, op, mask, NULL,
			       0, 0,
			       0, 0,
			       extents->bounded.x,      extents->bounded.y,
			       extents->bounded.width,  extents->bounded.height);
    }

skip:
    cairo_surface_destroy (mask);
    return CAIRO_STATUS_SUCCESS;
}

/* Handles compositing with a clip surface when we have to do the operation
 * in two pieces and combine them together.
 */
static cairo_status_t
clip_and_composite_combine (const cairo_traps_compositor_t *compositor,
			    const cairo_composite_rectangles_t*extents,
			    draw_func_t		 draw_func,
			    void			*draw_closure,
			    cairo_operator_t		 op,
			    cairo_surface_t	*src,
			    int src_x, int src_y)
{
    cairo_surface_t *dst = extents->surface;
    cairo_surface_t *tmp, *clip;
    cairo_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    tmp = _cairo_surface_create_scratch (dst, dst->content,
					 extents->bounded.width,
					 extents->bounded.height,
					 NULL);
    if (unlikely (tmp->status))
	return tmp->status;

    status = compositor->acquire (tmp);
    if (unlikely (status)) {
	cairo_surface_destroy (tmp);
	return status;
    }

    compositor->composite (tmp,
			   dst->is_clear ? CAIRO_OPERATOR_CLEAR : CAIRO_OPERATOR_SOURCE,
			   dst, NULL,
			   extents->bounded.x,      extents->bounded.y,
			   0, 0,
			   0, 0,
			   extents->bounded.width,  extents->bounded.height);

    status = draw_func (compositor, tmp, draw_closure, op,
			src, src_x, src_y,
			extents->bounded.x, extents->bounded.y,
			&extents->bounded, NULL);

    if (unlikely (status))
	goto cleanup;

    clip = traps_get_clip_surface (compositor, extents, &extents->bounded);
    if (unlikely ((status = clip->status)))
	goto cleanup;

    if (dst->is_clear) {
	compositor->composite (dst, CAIRO_OPERATOR_SOURCE, tmp, clip,
			       0, 0,
			       0, 0,
			       extents->bounded.x,      extents->bounded.y,
			       extents->bounded.width,  extents->bounded.height);
    } else {
	compositor->lerp (dst, tmp, clip,
			  0, 0,
			  0,0,
			  extents->bounded.x,     extents->bounded.y,
			  extents->bounded.width, extents->bounded.height);
    }
    cairo_surface_destroy (clip);

cleanup:
    compositor->release (tmp);
    cairo_surface_destroy (tmp);

    return status;
}

/* Handles compositing for %CAIRO_OPERATOR_SOURCE, which is special; it's
 * defined as (src IN mask IN clip) ADD (dst OUT (mask IN clip))
 */
static cairo_status_t
clip_and_composite_source (const cairo_traps_compositor_t	*compositor,
			   cairo_surface_t			*dst,
			   draw_func_t				 draw_func,
			   draw_func_t				 mask_func,
			   void					*draw_closure,
			   cairo_surface_t		*src,
			   int src_x,
			   int src_y,
			   const cairo_composite_rectangles_t	*extents)
{
    cairo_surface_t *mask;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    /* Create a surface that is mask IN clip */
    mask = create_composite_mask (compositor, dst, draw_closure,
				  draw_func, mask_func,
				  extents);
    if (unlikely (mask->status))
	return mask->status;

    if (mask->is_clear)
	goto skip;

    if (dst->is_clear) {
	compositor->composite (dst, CAIRO_OPERATOR_SOURCE, src, mask,
			       extents->bounded.x + src_x, extents->bounded.y + src_y,
			       0, 0,
			       extents->bounded.x,      extents->bounded.y,
			       extents->bounded.width,  extents->bounded.height);
    } else {
	compositor->lerp (dst, src, mask,
			  extents->bounded.x + src_x, extents->bounded.y + src_y,
			  0, 0,
			  extents->bounded.x,     extents->bounded.y,
			  extents->bounded.width, extents->bounded.height);
    }

skip:
    cairo_surface_destroy (mask);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
can_reduce_alpha_op (cairo_operator_t op)
{
    int iop = op;
    switch (iop) {
    case CAIRO_OPERATOR_OVER:
    case CAIRO_OPERATOR_SOURCE:
    case CAIRO_OPERATOR_ADD:
	return TRUE;
    default:
	return FALSE;
    }
}

static cairo_bool_t
reduce_alpha_op (cairo_composite_rectangles_t *extents)
{
    cairo_surface_t *dst = extents->surface;
    cairo_operator_t op = extents->op;
    const cairo_pattern_t *pattern = &extents->source_pattern.base;
    return dst->is_clear &&
	   dst->content == CAIRO_CONTENT_ALPHA &&
	   _cairo_pattern_is_opaque_solid (pattern) &&
	   can_reduce_alpha_op (op);
}

static cairo_status_t
fixup_unbounded_with_mask (const cairo_traps_compositor_t *compositor,
			   const cairo_composite_rectangles_t *extents)
{
    cairo_surface_t *dst = extents->surface;
    cairo_surface_t *mask;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    /* XXX can we avoid querying the clip surface again? */
    mask = traps_get_clip_surface (compositor, extents, &extents->unbounded);
    if (unlikely (mask->status))
	return mask->status;

    /* top */
    if (extents->bounded.y != extents->unbounded.y) {
	int x = extents->unbounded.x;
	int y = extents->unbounded.y;
	int width = extents->unbounded.width;
	int height = extents->bounded.y - y;

	compositor->composite (dst, CAIRO_OPERATOR_DEST_OUT, mask, NULL,
			       0, 0,
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

	compositor->composite (dst, CAIRO_OPERATOR_DEST_OUT, mask, NULL,
			       0, y - extents->unbounded.y,
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

	compositor->composite (dst, CAIRO_OPERATOR_DEST_OUT, mask, NULL,
			       x - extents->unbounded.x, y - extents->unbounded.y,
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

	compositor->composite (dst, CAIRO_OPERATOR_DEST_OUT, mask, NULL,
			       0, y - extents->unbounded.y,
			       0, 0,
			       x, y,
			       width, height);
    }

    cairo_surface_destroy (mask);

    return CAIRO_STATUS_SUCCESS;
}

static void
add_rect (cairo_boxes_t *boxes, int x1, int y1, int x2, int y2)
{
    cairo_box_t box;
    cairo_int_status_t status;

    box.p1.x = _cairo_fixed_from_int (x1);
    box.p1.y = _cairo_fixed_from_int (y1);
    box.p2.x = _cairo_fixed_from_int (x2);
    box.p2.y = _cairo_fixed_from_int (y2);

    status = _cairo_boxes_add (boxes, CAIRO_ANTIALIAS_DEFAULT, &box);
    assert (status == CAIRO_INT_STATUS_SUCCESS);
}

static cairo_status_t
fixup_unbounded (const cairo_traps_compositor_t *compositor,
		 cairo_composite_rectangles_t *extents,
		 cairo_boxes_t *boxes)
{
    cairo_surface_t *dst = extents->surface;
    cairo_boxes_t clear, tmp;
    cairo_box_t box;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (extents->bounded.width  == extents->unbounded.width &&
	extents->bounded.height == extents->unbounded.height)
    {
	return CAIRO_STATUS_SUCCESS;
    }

    assert (extents->clip->path == NULL);

    /* subtract the drawn boxes from the unbounded area */
    _cairo_boxes_init (&clear);

    box.p1.x = _cairo_fixed_from_int (extents->unbounded.x + extents->unbounded.width);
    box.p1.y = _cairo_fixed_from_int (extents->unbounded.y);
    box.p2.x = _cairo_fixed_from_int (extents->unbounded.x);
    box.p2.y = _cairo_fixed_from_int (extents->unbounded.y + extents->unbounded.height);

    if (boxes == NULL) {
	if (extents->bounded.width == 0 || extents->bounded.height == 0) {
	    goto empty;
	} else {
	    /* top */
	    if (extents->bounded.y != extents->unbounded.y) {
		add_rect (&clear,
			  extents->unbounded.x, extents->unbounded.y,
			  extents->unbounded.x + extents->unbounded.width,
			  extents->bounded.y);
	    }
	    /* left */
	    if (extents->bounded.x != extents->unbounded.x) {
		add_rect (&clear,
			  extents->unbounded.x, extents->bounded.y,
			  extents->bounded.x,
			  extents->bounded.y + extents->bounded.height);
	    }
	    /* right */
	    if (extents->bounded.x + extents->bounded.width != extents->unbounded.x + extents->unbounded.width) {
		add_rect (&clear,
			  extents->bounded.x + extents->bounded.width,
			  extents->bounded.y,
			  extents->unbounded.x + extents->unbounded.width,
			  extents->bounded.y + extents->bounded.height);
	    }
	    /* bottom */
	    if (extents->bounded.y + extents->bounded.height != extents->unbounded.y + extents->unbounded.height) {
		add_rect (&clear,
			  extents->unbounded.x,
			  extents->bounded.y + extents->bounded.height,
			  extents->unbounded.x + extents->unbounded.width,
			  extents->unbounded.y + extents->unbounded.height);
	    }
	}
    } else if (boxes->num_boxes) {
	_cairo_boxes_init (&tmp);

	assert (boxes->is_pixel_aligned);

	status = _cairo_boxes_add (&tmp, CAIRO_ANTIALIAS_DEFAULT, &box);
	assert (status == CAIRO_INT_STATUS_SUCCESS);

	tmp.chunks.next = &boxes->chunks;
	tmp.num_boxes += boxes->num_boxes;

	status = _cairo_bentley_ottmann_tessellate_boxes (&tmp,
							  CAIRO_FILL_RULE_WINDING,
							  &clear);
	tmp.chunks.next = NULL;
	if (unlikely (status))
	    goto error;
    } else {
empty:
	box.p1.x = _cairo_fixed_from_int (extents->unbounded.x);
	box.p2.x = _cairo_fixed_from_int (extents->unbounded.x + extents->unbounded.width);

	status = _cairo_boxes_add (&clear, CAIRO_ANTIALIAS_DEFAULT, &box);
	assert (status == CAIRO_INT_STATUS_SUCCESS);
    }

    /* Now intersect with the clip boxes */
    if (extents->clip->num_boxes) {
	_cairo_boxes_init_for_array (&tmp,
				     extents->clip->boxes,
				     extents->clip->num_boxes);
	status = _cairo_boxes_intersect (&clear, &tmp, &clear);
	if (unlikely (status))
	    goto error;
    }

    status = compositor->fill_boxes (dst,
				     CAIRO_OPERATOR_CLEAR,
				     CAIRO_COLOR_TRANSPARENT,
				     &clear);

error:
    _cairo_boxes_fini (&clear);
    return status;
}

enum {
    NEED_CLIP_REGION = 0x1,
    NEED_CLIP_SURFACE = 0x2,
    FORCE_CLIP_REGION = 0x4,
};

static cairo_bool_t
need_bounded_clip (cairo_composite_rectangles_t *extents)
{
    unsigned int flags = 0;

    if (extents->clip->num_boxes > 1 ||
	extents->mask.width > extents->unbounded.width ||
	extents->mask.height > extents->unbounded.height)
    {
	flags |= NEED_CLIP_REGION;
    }

    if (extents->clip->num_boxes > 1 ||
	extents->mask.width > extents->bounded.width ||
	extents->mask.height > extents->bounded.height)
    {
	flags |= FORCE_CLIP_REGION;
    }

    if (! _cairo_clip_is_region (extents->clip))
	flags |= NEED_CLIP_SURFACE;

    return flags;
}

static cairo_bool_t
need_unbounded_clip (cairo_composite_rectangles_t *extents)
{
    unsigned int flags = 0;
    if (! extents->is_bounded) {
	flags |= NEED_CLIP_REGION;
	if (! _cairo_clip_is_region (extents->clip))
	    flags |= NEED_CLIP_SURFACE;
    }
    if (extents->clip->path != NULL)
	flags |= NEED_CLIP_SURFACE;
    return flags;
}

static cairo_status_t
clip_and_composite (const cairo_traps_compositor_t *compositor,
		    cairo_composite_rectangles_t *extents,
		    draw_func_t		 draw_func,
		    draw_func_t		 mask_func,
		    void		*draw_closure,
		    unsigned int need_clip)
{
    cairo_surface_t *dst = extents->surface;
    cairo_operator_t op = extents->op;
    cairo_pattern_t *source = &extents->source_pattern.base;
    cairo_surface_t *src;
    int src_x, src_y;
    cairo_region_t *clip_region = NULL;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (reduce_alpha_op (extents)) {
	op = CAIRO_OPERATOR_ADD;
	source = NULL;
    }

    if (op == CAIRO_OPERATOR_CLEAR) {
	op = CAIRO_OPERATOR_DEST_OUT;
	source = NULL;
    }

    compositor->acquire (dst);

    if (need_clip & NEED_CLIP_REGION) {
	const cairo_rectangle_int_t *limit;

	if ((need_clip & FORCE_CLIP_REGION) == 0)
	    limit = &extents->unbounded;
	else
	    limit = &extents->destination;

	clip_region = _cairo_clip_get_region (extents->clip);
	if (clip_region != NULL &&
	    cairo_region_contains_rectangle (clip_region,
					     limit) == CAIRO_REGION_OVERLAP_IN)
	    clip_region = NULL;

	if (clip_region != NULL) {
	    status = compositor->set_clip_region (dst, clip_region);
	    if (unlikely (status)) {
		compositor->release (dst);
		return status;
	    }
	}
    }

    if (extents->bounded.width == 0 || extents->bounded.height == 0)
	goto skip;

    src = compositor->pattern_to_surface (dst, source, FALSE,
					  &extents->bounded,
					  &extents->source_sample_area,
					  &src_x, &src_y);
    if (unlikely (status = src->status))
	goto error;

    if (op == CAIRO_OPERATOR_SOURCE) {
	status = clip_and_composite_source (compositor, dst,
					    draw_func, mask_func, draw_closure,
					    src, src_x, src_y,
					    extents);
    } else {
	if (need_clip & NEED_CLIP_SURFACE) {
	    if (extents->is_bounded) {
		status = clip_and_composite_with_mask (compositor, extents,
						       draw_func, mask_func,
						       draw_closure,
						       op, src, src_x, src_y);
	    } else {
		status = clip_and_composite_combine (compositor, extents,
						     draw_func, draw_closure,
						     op, src, src_x, src_y);
	    }
	} else {
	    status = draw_func (compositor,
				dst, draw_closure,
				op, src, src_x, src_y,
				0, 0,
				&extents->bounded,
				extents->clip);
	}
    }
    cairo_surface_destroy (src);

skip:
    if (status == CAIRO_STATUS_SUCCESS && ! extents->is_bounded) {
	if (need_clip & NEED_CLIP_SURFACE)
	    status = fixup_unbounded_with_mask (compositor, extents);
	else
	    status = fixup_unbounded (compositor, extents, NULL);
    }

error:
    if (clip_region)
	compositor->set_clip_region (dst, NULL);

    compositor->release (dst);

    return status;
}

/* meta-ops */

typedef struct {
    cairo_traps_t traps;
    cairo_antialias_t antialias;
} composite_traps_info_t;

static cairo_int_status_t
composite_traps (const cairo_traps_compositor_t *compositor,
		 cairo_surface_t		*dst,
		 void				 *closure,
		 cairo_operator_t		 op,
		 cairo_surface_t		*src,
		 int src_x, int src_y,
		 int dst_x, int dst_y,
		 const cairo_rectangle_int_t *extents,
		 cairo_clip_t			*clip)
{
    composite_traps_info_t *info = closure;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    return compositor->composite_traps (dst, op, src,
					src_x - dst_x, src_y - dst_y,
					dst_x, dst_y,
					extents,
					info->antialias, &info->traps);
}

typedef struct {
    cairo_tristrip_t strip;
    cairo_antialias_t antialias;
} composite_tristrip_info_t;

static cairo_int_status_t
composite_tristrip (const cairo_traps_compositor_t *compositor,
		    cairo_surface_t		*dst,
		    void				 *closure,
		    cairo_operator_t		 op,
		    cairo_surface_t		*src,
		    int src_x, int src_y,
		    int dst_x, int dst_y,
		    const cairo_rectangle_int_t *extents,
		    cairo_clip_t			*clip)
{
    composite_tristrip_info_t *info = closure;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    return compositor->composite_tristrip (dst, op, src,
					   src_x - dst_x, src_y - dst_y,
					   dst_x, dst_y,
					   extents,
					   info->antialias, &info->strip);
}

static cairo_bool_t
is_recording_pattern (const cairo_pattern_t *pattern)
{
    cairo_surface_t *surface;

    if (pattern->type != CAIRO_PATTERN_TYPE_SURFACE)
	return FALSE;

    surface = ((const cairo_surface_pattern_t *) pattern)->surface;
    surface = _cairo_surface_get_source (surface, NULL);
    return _cairo_surface_is_recording (surface);
}

static cairo_surface_t *
recording_pattern_get_surface (const cairo_pattern_t *pattern)
{
    cairo_surface_t *surface;

    surface = ((const cairo_surface_pattern_t *) pattern)->surface;
    return _cairo_surface_get_source (surface, NULL);
}

static cairo_bool_t
recording_pattern_contains_sample (const cairo_pattern_t *pattern,
				   const cairo_rectangle_int_t *sample)
{
    cairo_recording_surface_t *surface;

    if (! is_recording_pattern (pattern))
	return FALSE;

    if (pattern->extend == CAIRO_EXTEND_NONE)
	return TRUE;

    surface = (cairo_recording_surface_t *) recording_pattern_get_surface (pattern);
    if (surface->unbounded)
	return TRUE;

    return _cairo_rectangle_contains_rectangle (&surface->extents, sample);
}

static cairo_bool_t
op_reduces_to_source (cairo_composite_rectangles_t *extents)
{
    if (extents->op == CAIRO_OPERATOR_SOURCE)
	return TRUE;

    if (extents->surface->is_clear)
	return extents->op == CAIRO_OPERATOR_OVER || extents->op == CAIRO_OPERATOR_ADD;

    return FALSE;
}

static cairo_status_t
composite_aligned_boxes (const cairo_traps_compositor_t *compositor,
			 cairo_composite_rectangles_t *extents,
			 cairo_boxes_t *boxes)
{
    cairo_surface_t *dst = extents->surface;
    cairo_operator_t op = extents->op;
    cairo_bool_t need_clip_mask = ! _cairo_clip_is_region (extents->clip);
    cairo_bool_t op_is_source;
    cairo_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (need_clip_mask &&
	(! extents->is_bounded || extents->op == CAIRO_OPERATOR_SOURCE))
    {
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    op_is_source = op_reduces_to_source (extents);

    /* Are we just copying a recording surface? */
    if (! need_clip_mask && op_is_source &&
	recording_pattern_contains_sample (&extents->source_pattern.base,
					   &extents->source_sample_area))
    {
	cairo_clip_t *recording_clip;
	const cairo_pattern_t *source = &extents->source_pattern.base;
	const cairo_matrix_t *m;
	cairo_matrix_t matrix;

	/* XXX could also do tiling repeat modes... */

	/* first clear the area about to be overwritten */
	if (! dst->is_clear) {
	    status = compositor->acquire (dst);
	    if (unlikely (status))
		return status;

	    status = compositor->fill_boxes (dst,
					     CAIRO_OPERATOR_CLEAR,
					     CAIRO_COLOR_TRANSPARENT,
					     boxes);
	    compositor->release (dst);
	    if (unlikely (status))
		return status;
	}

	m = &source->matrix;
	if (_cairo_surface_has_device_transform (dst)) {
	    cairo_matrix_multiply (&matrix,
				   &source->matrix,
				   &dst->device_transform);
	    m = &matrix;
	}

	recording_clip = _cairo_clip_from_boxes (boxes);
	status = _cairo_recording_surface_replay_with_clip (recording_pattern_get_surface (source),
							    m, dst, recording_clip, FALSE);
	_cairo_clip_destroy (recording_clip);

	return status;
    }

    status = compositor->acquire (dst);
    if (unlikely (status))
	return status;

    if (! need_clip_mask &&
	(op == CAIRO_OPERATOR_CLEAR ||
	 extents->source_pattern.base.type == CAIRO_PATTERN_TYPE_SOLID))
    {
	const cairo_color_t *color;

	if (op == CAIRO_OPERATOR_CLEAR) {
	    color = CAIRO_COLOR_TRANSPARENT;
	} else {
	    color = &((cairo_solid_pattern_t *) &extents->source_pattern)->color;
	    if (op_is_source)
		op = CAIRO_OPERATOR_SOURCE;
	}

	status = compositor->fill_boxes (dst, op, color, boxes);
    }
    else
    {
	cairo_surface_t *src, *mask = NULL;
	cairo_pattern_t *source = &extents->source_pattern.base;
	int src_x, src_y;
	int mask_x = 0, mask_y = 0;

	if (need_clip_mask) {
	    mask = traps_get_clip_surface (compositor,
					   extents, &extents->bounded);
	    if (unlikely (mask->status))
		return mask->status;

	    mask_x = -extents->bounded.x;
	    mask_y = -extents->bounded.y;

	    if (op == CAIRO_OPERATOR_CLEAR) {
		source = NULL;
		op = CAIRO_OPERATOR_DEST_OUT;
	    }
	} else if (op_is_source)
	    op = CAIRO_OPERATOR_SOURCE;

	src = compositor->pattern_to_surface (dst, source, FALSE,
					      &extents->bounded,
					      &extents->source_sample_area,
					      &src_x, &src_y);
	if (likely (src->status == CAIRO_STATUS_SUCCESS)) {
	    status = compositor->composite_boxes (dst, op, src, mask,
						  src_x, src_y,
						  mask_x, mask_y,
						  0, 0,
						  boxes, &extents->bounded);
	    cairo_surface_destroy (src);
	} else
	    status = src->status;

	cairo_surface_destroy (mask);
    }

    if (status == CAIRO_STATUS_SUCCESS && ! extents->is_bounded)
	status = fixup_unbounded (compositor, extents, boxes);

    compositor->release (dst);

    return status;
}

static cairo_status_t
upload_boxes (const cairo_traps_compositor_t *compositor,
	      cairo_composite_rectangles_t *extents,
	      cairo_boxes_t *boxes)
{
    cairo_surface_t *dst = extents->surface;
    const cairo_pattern_t *source = &extents->source_pattern.base;
    cairo_surface_t *src;
    cairo_rectangle_int_t limit;
    cairo_int_status_t status;
    int tx, ty;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    src = _cairo_pattern_get_source((cairo_surface_pattern_t *)source,
				    &limit);
    if (!(src->type == CAIRO_SURFACE_TYPE_IMAGE || src->type == dst->type))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    if (! _cairo_matrix_is_integer_translation (&source->matrix, &tx, &ty))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    /* Check that the data is entirely within the image */
    if (extents->bounded.x + tx < limit.x || extents->bounded.y + ty < limit.y)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    if (extents->bounded.x + extents->bounded.width  + tx > limit.x + limit.width ||
	extents->bounded.y + extents->bounded.height + ty > limit.y + limit.height)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    tx += limit.x;
    ty += limit.y;

    if (src->type == CAIRO_SURFACE_TYPE_IMAGE)
	status = compositor->draw_image_boxes (dst,
					       (cairo_image_surface_t *)src,
					       boxes, tx, ty);
    else
	status = compositor->copy_boxes (dst, src, boxes, &extents->bounded,
					 tx, ty);

    return status;
}

static cairo_int_status_t
trim_extents_to_traps (cairo_composite_rectangles_t *extents,
		       cairo_traps_t *traps)
{
    cairo_box_t box;

    _cairo_traps_extents (traps, &box);
    return _cairo_composite_rectangles_intersect_mask_extents (extents, &box);
}

static cairo_int_status_t
trim_extents_to_tristrip (cairo_composite_rectangles_t *extents,
			  cairo_tristrip_t *strip)
{
    cairo_box_t box;

    _cairo_tristrip_extents (strip, &box);
    return _cairo_composite_rectangles_intersect_mask_extents (extents, &box);
}

static cairo_int_status_t
trim_extents_to_boxes (cairo_composite_rectangles_t *extents,
		       cairo_boxes_t *boxes)
{
    cairo_box_t box;

    _cairo_boxes_extents (boxes, &box);
    return _cairo_composite_rectangles_intersect_mask_extents (extents, &box);
}

static cairo_int_status_t
boxes_for_traps (cairo_boxes_t *boxes,
		 cairo_traps_t *traps,
		 cairo_antialias_t antialias)
{
    int i, j;

    /* first check that the traps are rectilinear */
    if (antialias == CAIRO_ANTIALIAS_NONE) {
	for (i = 0; i < traps->num_traps; i++) {
	    const cairo_trapezoid_t *t = &traps->traps[i];
	    if (_cairo_fixed_integer_round_down (t->left.p1.x) !=
		_cairo_fixed_integer_round_down (t->left.p2.x) ||
		_cairo_fixed_integer_round_down (t->right.p1.x) !=
		_cairo_fixed_integer_round_down (t->right.p2.x))
	    {
		return CAIRO_INT_STATUS_UNSUPPORTED;
	    }
	}
    } else {
	for (i = 0; i < traps->num_traps; i++) {
	    const cairo_trapezoid_t *t = &traps->traps[i];
	    if (t->left.p1.x != t->left.p2.x || t->right.p1.x != t->right.p2.x)
		return CAIRO_INT_STATUS_UNSUPPORTED;
	}
    }

    _cairo_boxes_init (boxes);

    boxes->chunks.base  = (cairo_box_t *) traps->traps;
    boxes->chunks.size  = traps->num_traps;

    if (antialias != CAIRO_ANTIALIAS_NONE) {
	for (i = j = 0; i < traps->num_traps; i++) {
	    /* Note the traps and boxes alias so we need to take the local copies first. */
	    cairo_fixed_t x1 = traps->traps[i].left.p1.x;
	    cairo_fixed_t x2 = traps->traps[i].right.p1.x;
	    cairo_fixed_t y1 = traps->traps[i].top;
	    cairo_fixed_t y2 = traps->traps[i].bottom;

	    if (x1 == x2 || y1 == y2)
		    continue;

	    boxes->chunks.base[j].p1.x = x1;
	    boxes->chunks.base[j].p1.y = y1;
	    boxes->chunks.base[j].p2.x = x2;
	    boxes->chunks.base[j].p2.y = y2;
	    j++;

	    if (boxes->is_pixel_aligned) {
		boxes->is_pixel_aligned =
		    _cairo_fixed_is_integer (x1) && _cairo_fixed_is_integer (y1) &&
		    _cairo_fixed_is_integer (x2) && _cairo_fixed_is_integer (y2);
	    }
	}
    } else {
	boxes->is_pixel_aligned = TRUE;

	for (i = j = 0; i < traps->num_traps; i++) {
	    /* Note the traps and boxes alias so we need to take the local copies first. */
	    cairo_fixed_t x1 = traps->traps[i].left.p1.x;
	    cairo_fixed_t x2 = traps->traps[i].right.p1.x;
	    cairo_fixed_t y1 = traps->traps[i].top;
	    cairo_fixed_t y2 = traps->traps[i].bottom;

	    /* round down here to match Pixman's behavior when using traps. */
	    boxes->chunks.base[j].p1.x = _cairo_fixed_round_down (x1);
	    boxes->chunks.base[j].p1.y = _cairo_fixed_round_down (y1);
	    boxes->chunks.base[j].p2.x = _cairo_fixed_round_down (x2);
	    boxes->chunks.base[j].p2.y = _cairo_fixed_round_down (y2);
	    j += (boxes->chunks.base[j].p1.x != boxes->chunks.base[j].p2.x &&
		  boxes->chunks.base[j].p1.y != boxes->chunks.base[j].p2.y);
	}
    }
    boxes->chunks.count = j;
    boxes->num_boxes    = j;

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_status_t
clip_and_composite_boxes (const cairo_traps_compositor_t *compositor,
			  cairo_composite_rectangles_t *extents,
			  cairo_boxes_t *boxes);

static cairo_status_t
clip_and_composite_polygon (const cairo_traps_compositor_t *compositor,
			    cairo_composite_rectangles_t *extents,
			    cairo_polygon_t *polygon,
			    cairo_antialias_t antialias,
			    cairo_fill_rule_t fill_rule,
			    cairo_bool_t curvy)
{
    composite_traps_info_t traps;
    cairo_surface_t *dst = extents->surface;
    cairo_bool_t clip_surface = ! _cairo_clip_is_region (extents->clip);
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (polygon->num_edges == 0) {
	status = CAIRO_INT_STATUS_SUCCESS;

	if (! extents->is_bounded) {
	    cairo_region_t *clip_region = _cairo_clip_get_region (extents->clip);

	    if (clip_region &&
		cairo_region_contains_rectangle (clip_region,
						 &extents->unbounded) == CAIRO_REGION_OVERLAP_IN)
		clip_region = NULL;

	    if (clip_region != NULL) {
		status = compositor->set_clip_region (dst, clip_region);
		if (unlikely (status))
		    return status;
	    }

	    if (clip_surface)
		status = fixup_unbounded_with_mask (compositor, extents);
	    else
		status = fixup_unbounded (compositor, extents, NULL);

	    if (clip_region != NULL)
		compositor->set_clip_region (dst, NULL);
	}

	return status;
    }

    if (extents->clip->path != NULL && extents->is_bounded) {
	cairo_polygon_t clipper;
	cairo_fill_rule_t clipper_fill_rule;
	cairo_antialias_t clipper_antialias;

	status = _cairo_clip_get_polygon (extents->clip,
					  &clipper,
					  &clipper_fill_rule,
					  &clipper_antialias);
	if (likely (status == CAIRO_INT_STATUS_SUCCESS)) {
	    if (clipper_antialias == antialias) {
		status = _cairo_polygon_intersect (polygon, fill_rule,
						   &clipper, clipper_fill_rule);
		if (likely (status == CAIRO_INT_STATUS_SUCCESS)) {
		    cairo_clip_t * clip = _cairo_clip_copy_region (extents->clip);
		    _cairo_clip_destroy (extents->clip);
		    extents->clip = clip;

		    fill_rule = CAIRO_FILL_RULE_WINDING;
		}
		_cairo_polygon_fini (&clipper);
	    }
	}
    }

    if (antialias == CAIRO_ANTIALIAS_NONE && curvy) {
	cairo_boxes_t boxes;

	_cairo_boxes_init (&boxes);
	status = _cairo_rasterise_polygon_to_boxes (polygon, fill_rule, &boxes);
	if (likely (status == CAIRO_INT_STATUS_SUCCESS)) {
	    assert (boxes.is_pixel_aligned);
	    status = clip_and_composite_boxes (compositor, extents, &boxes);
	}
	_cairo_boxes_fini (&boxes);
	if ((status != CAIRO_INT_STATUS_UNSUPPORTED))
	    return status;
    }

    _cairo_traps_init (&traps.traps);

    if (antialias == CAIRO_ANTIALIAS_NONE && curvy) {
	status = _cairo_rasterise_polygon_to_traps (polygon, fill_rule, antialias, &traps.traps);
    } else {
	status = _cairo_bentley_ottmann_tessellate_polygon (&traps.traps, polygon, fill_rule);
    }
    if (unlikely (status))
	goto CLEANUP_TRAPS;

    status = trim_extents_to_traps (extents, &traps.traps);
    if (unlikely (status))
	goto CLEANUP_TRAPS;

    /* Use a fast path if the trapezoids consist of a set of boxes.  */
    status = CAIRO_INT_STATUS_UNSUPPORTED;
    if (1) {
	cairo_boxes_t boxes;

	status = boxes_for_traps (&boxes, &traps.traps, antialias);
	if (status == CAIRO_INT_STATUS_SUCCESS) {
	    status = clip_and_composite_boxes (compositor, extents, &boxes);
	    /* XXX need to reconstruct the traps! */
	    assert (status != CAIRO_INT_STATUS_UNSUPPORTED);
	}
    }
    if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	/* Otherwise render the trapezoids to a mask and composite in the usual
	 * fashion.
	 */
	unsigned int flags = 0;

	/* For unbounded operations, the X11 server will estimate the
	 * affected rectangle and apply the operation to that. However,
	 * there are cases where this is an overestimate (e.g. the
	 * clip-fill-{eo,nz}-unbounded test).
	 *
	 * The clip will trim that overestimate to our expectations.
	 */
	if (! extents->is_bounded)
	    flags |= FORCE_CLIP_REGION;

	traps.antialias = antialias;
	status = clip_and_composite (compositor, extents,
				     composite_traps, NULL, &traps,
				     need_unbounded_clip (extents) | flags);
    }

CLEANUP_TRAPS:
    _cairo_traps_fini (&traps.traps);

    return status;
}

struct composite_opacity_info {
    const cairo_traps_compositor_t *compositor;
    uint8_t op;
    cairo_surface_t *dst;
    cairo_surface_t *src;
    int src_x, src_y;
    double opacity;
};

static void composite_opacity(void *closure,
			      int16_t x, int16_t y,
			      int16_t w, int16_t h,
			      uint16_t coverage)
{
    struct composite_opacity_info *info = closure;
    const cairo_traps_compositor_t *compositor = info->compositor;
    cairo_surface_t *mask;
    int mask_x, mask_y;
    cairo_color_t color;
    cairo_solid_pattern_t solid;

    _cairo_color_init_rgba (&color, 0, 0, 0, info->opacity * coverage);
    _cairo_pattern_init_solid (&solid, &color);
    mask = compositor->pattern_to_surface (info->dst, &solid.base, TRUE,
					   &_cairo_unbounded_rectangle,
					   &_cairo_unbounded_rectangle,
					   &mask_x, &mask_y);
    if (likely (mask->status == CAIRO_STATUS_SUCCESS)) {
	if (info->src) {
	    compositor->composite (info->dst, info->op, info->src, mask,
				   x + info->src_x,  y + info->src_y,
				   mask_x,           mask_y,
				   x,                y,
				   w,                h);
	} else {
	    compositor->composite (info->dst, info->op, mask, NULL,
				   mask_x,            mask_y,
				   0,                 0,
				   x,                 y,
				   w,                 h);
	}
    }

    cairo_surface_destroy (mask);
}


static cairo_int_status_t
composite_opacity_boxes (const cairo_traps_compositor_t *compositor,
			 cairo_surface_t		*dst,
			 void				*closure,
			 cairo_operator_t		 op,
			 cairo_surface_t		*src,
			 int				 src_x,
			 int				 src_y,
			 int				 dst_x,
			 int				 dst_y,
			 const cairo_rectangle_int_t	*extents,
			 cairo_clip_t			*clip)
{
    const cairo_solid_pattern_t *mask = closure;
    struct composite_opacity_info info;
    int i;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    info.compositor = compositor;
    info.op = op;
    info.dst = dst;

    info.src = src;
    info.src_x = src_x;
    info.src_y = src_y;

    info.opacity = mask->color.alpha / (double) 0xffff;

    /* XXX for lots of boxes create a clip region for the fully opaque areas */
    for (i = 0; i < clip->num_boxes; i++)
	do_unaligned_box(composite_opacity, &info,
			 &clip->boxes[i], dst_x, dst_y);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
composite_boxes (const cairo_traps_compositor_t *compositor,
		 cairo_surface_t		*dst,
		 void				*closure,
		 cairo_operator_t		 op,
		 cairo_surface_t		*src,
		 int				 src_x,
		 int				 src_y,
		 int				 dst_x,
		 int				 dst_y,
		 const cairo_rectangle_int_t	*extents,
		 cairo_clip_t			*clip)
{
    cairo_traps_t traps;
    cairo_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = _cairo_traps_init_boxes (&traps, closure);
    if (unlikely (status))
	return status;

    status = compositor->composite_traps (dst, op, src,
					  src_x - dst_x, src_y - dst_y,
					  dst_x, dst_y,
					  extents,
					  CAIRO_ANTIALIAS_DEFAULT, &traps);
    _cairo_traps_fini (&traps);

    return status;
}

static cairo_status_t
clip_and_composite_boxes (const cairo_traps_compositor_t *compositor,
			  cairo_composite_rectangles_t *extents,
			  cairo_boxes_t *boxes)
{
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (boxes->num_boxes == 0 && extents->is_bounded)
	return CAIRO_STATUS_SUCCESS;

    status = trim_extents_to_boxes (extents, boxes);
    if (unlikely (status))
	return status;

    if (boxes->is_pixel_aligned && extents->clip->path == NULL &&
	extents->source_pattern.base.type == CAIRO_PATTERN_TYPE_SURFACE &&
	(op_reduces_to_source (extents) ||
	 (extents->op == CAIRO_OPERATOR_OVER &&
	  (extents->source_pattern.surface.surface->content & CAIRO_CONTENT_ALPHA) == 0)))
    {
	status = upload_boxes (compositor, extents, boxes);
	if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	    return status;
    }

    /* Can we reduce drawing through a clip-mask to simply drawing the clip? */
    if (extents->clip->path != NULL && extents->is_bounded) {
	cairo_polygon_t polygon;
	cairo_fill_rule_t fill_rule;
	cairo_antialias_t antialias;
	cairo_clip_t *clip;

	clip = _cairo_clip_copy (extents->clip);
	clip = _cairo_clip_intersect_boxes (clip, boxes);
	if (_cairo_clip_is_all_clipped (clip))
	    return CAIRO_INT_STATUS_NOTHING_TO_DO;

	status = _cairo_clip_get_polygon (clip, &polygon,
					  &fill_rule, &antialias);
	_cairo_clip_path_destroy (clip->path);
	clip->path = NULL;
	if (likely (status == CAIRO_INT_STATUS_SUCCESS)) {
	    cairo_clip_t *saved_clip = extents->clip;
	    extents->clip = clip;

	    status = clip_and_composite_polygon (compositor, extents, &polygon,
						 antialias, fill_rule, FALSE);

	    clip = extents->clip;
	    extents->clip = saved_clip;

	    _cairo_polygon_fini (&polygon);
	}
	_cairo_clip_destroy (clip);

	if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	    return status;
    }

    /* Use a fast path if the boxes are pixel aligned (or nearly aligned!) */
    if (boxes->is_pixel_aligned) {
	status = composite_aligned_boxes (compositor, extents, boxes);
	if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	    return status;
    }

    return clip_and_composite (compositor, extents,
			       composite_boxes, NULL, boxes,
			       need_unbounded_clip (extents));
}

static cairo_int_status_t
composite_traps_as_boxes (const cairo_traps_compositor_t *compositor,
			  cairo_composite_rectangles_t *extents,
			  composite_traps_info_t *info)
{
    cairo_boxes_t boxes;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (! _cairo_traps_to_boxes (&info->traps, info->antialias, &boxes))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    return clip_and_composite_boxes (compositor, extents, &boxes);
}

static cairo_int_status_t
clip_and_composite_traps (const cairo_traps_compositor_t *compositor,
			  cairo_composite_rectangles_t *extents,
			  composite_traps_info_t *info,
			  unsigned flags)
{
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = trim_extents_to_traps (extents, &info->traps);
    if (unlikely (status != CAIRO_INT_STATUS_SUCCESS))
	return status;

    status = CAIRO_INT_STATUS_UNSUPPORTED;
    if ((flags & FORCE_CLIP_REGION) == 0)
	status = composite_traps_as_boxes (compositor, extents, info);
    if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	/* For unbounded operations, the X11 server will estimate the
	 * affected rectangle and apply the operation to that. However,
	 * there are cases where this is an overestimate (e.g. the
	 * clip-fill-{eo,nz}-unbounded test).
	 *
	 * The clip will trim that overestimate to our expectations.
	 */
	if (! extents->is_bounded)
	    flags |= FORCE_CLIP_REGION;

	status = clip_and_composite (compositor, extents,
				     composite_traps, NULL, info,
				     need_unbounded_clip (extents) | flags);
    }

    return status;
}

static cairo_int_status_t
clip_and_composite_tristrip (const cairo_traps_compositor_t *compositor,
			     cairo_composite_rectangles_t *extents,
			     composite_tristrip_info_t *info)
{
    cairo_int_status_t status;
    unsigned int flags = 0;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = trim_extents_to_tristrip (extents, &info->strip);
    if (unlikely (status != CAIRO_INT_STATUS_SUCCESS))
	return status;

    if (! extents->is_bounded)
	flags |= FORCE_CLIP_REGION;

    status = clip_and_composite (compositor, extents,
				 composite_tristrip, NULL, info,
				 need_unbounded_clip (extents) | flags);

    return status;
}

struct composite_mask {
    cairo_surface_t *mask;
    int mask_x, mask_y;
};

static cairo_int_status_t
composite_mask (const cairo_traps_compositor_t *compositor,
		cairo_surface_t			*dst,
		void				*closure,
		cairo_operator_t		 op,
		cairo_surface_t			*src,
		int				 src_x,
		int				 src_y,
		int				 dst_x,
		int				 dst_y,
		const cairo_rectangle_int_t	*extents,
		cairo_clip_t			*clip)
{
    struct composite_mask *data = closure;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (src != NULL) {
	compositor->composite (dst, op, src, data->mask,
			       extents->x + src_x, extents->y + src_y,
			       extents->x + data->mask_x, extents->y + data->mask_y,
			       extents->x - dst_x,  extents->y - dst_y,
			       extents->width,      extents->height);
    } else {
	compositor->composite (dst, op, data->mask, NULL,
			       extents->x + data->mask_x, extents->y + data->mask_y,
			       0, 0,
			       extents->x - dst_x,  extents->y - dst_y,
			       extents->width,      extents->height);
    }

    return CAIRO_STATUS_SUCCESS;
}

struct composite_box_info {
    const cairo_traps_compositor_t *compositor;
    cairo_surface_t *dst;
    cairo_surface_t *src;
    int src_x, src_y;
    uint8_t op;
};

static void composite_box(void *closure,
			  int16_t x, int16_t y,
			  int16_t w, int16_t h,
			  uint16_t coverage)
{
    struct composite_box_info *info = closure;
    const cairo_traps_compositor_t *compositor = info->compositor;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (! CAIRO_ALPHA_SHORT_IS_OPAQUE (coverage)) {
	cairo_surface_t *mask;
	cairo_color_t color;
	cairo_solid_pattern_t solid;
	int mask_x, mask_y;

	_cairo_color_init_rgba (&color, 0, 0, 0, coverage / (double)0xffff);
	_cairo_pattern_init_solid (&solid, &color);

	mask = compositor->pattern_to_surface (info->dst, &solid.base, FALSE,
					       &_cairo_unbounded_rectangle,
					       &_cairo_unbounded_rectangle,
					       &mask_x, &mask_y);

	if (likely (mask->status == CAIRO_STATUS_SUCCESS)) {
	    compositor->composite (info->dst, info->op, info->src, mask,
				   x + info->src_x,  y + info->src_y,
				   mask_x,           mask_y,
				   x,                y,
				   w,                h);
	}

	cairo_surface_destroy (mask);
    } else {
	compositor->composite (info->dst, info->op, info->src, NULL,
			       x + info->src_x,  y + info->src_y,
			       0,                0,
			       x,                y,
			       w,                h);
    }
}

static cairo_int_status_t
composite_mask_clip_boxes (const cairo_traps_compositor_t *compositor,
			   cairo_surface_t		*dst,
			   void				*closure,
			   cairo_operator_t		 op,
			   cairo_surface_t		*src,
			   int				 src_x,
			   int				 src_y,
			   int				 dst_x,
			   int				 dst_y,
			   const cairo_rectangle_int_t	*extents,
			   cairo_clip_t			*clip)
{
    struct composite_mask *data = closure;
    struct composite_box_info info;
    int i;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    info.compositor = compositor;
    info.op = CAIRO_OPERATOR_SOURCE;
    info.dst = dst;
    info.src = data->mask;
    info.src_x = data->mask_x;
    info.src_y = data->mask_y;

    info.src_x += dst_x;
    info.src_y += dst_y;

    for (i = 0; i < clip->num_boxes; i++)
	do_unaligned_box(composite_box, &info, &clip->boxes[i], dst_x, dst_y);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
composite_mask_clip (const cairo_traps_compositor_t *compositor,
		     cairo_surface_t			*dst,
		     void				*closure,
		     cairo_operator_t			 op,
		     cairo_surface_t			*src,
		     int				 src_x,
		     int				 src_y,
		     int				 dst_x,
		     int				 dst_y,
		     const cairo_rectangle_int_t	*extents,
		     cairo_clip_t			*clip)
{
    struct composite_mask *data = closure;
    cairo_polygon_t polygon;
    cairo_fill_rule_t fill_rule;
    composite_traps_info_t info;
    cairo_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = _cairo_clip_get_polygon (clip, &polygon,
				      &fill_rule, &info.antialias);
    if (unlikely (status))
	return status;

    _cairo_traps_init (&info.traps);
    status = _cairo_bentley_ottmann_tessellate_polygon (&info.traps,
							&polygon,
							fill_rule);
    _cairo_polygon_fini (&polygon);
    if (unlikely (status))
	return status;

    status = composite_traps (compositor, dst, &info,
			      CAIRO_OPERATOR_SOURCE,
			      data->mask,
			      data->mask_x + dst_x, data->mask_y + dst_y,
			      dst_x, dst_y,
			      extents, NULL);
    _cairo_traps_fini (&info.traps);

    return status;
}

/* high-level compositor interface */

static cairo_int_status_t
_cairo_traps_compositor_paint (const cairo_compositor_t *_compositor,
			       cairo_composite_rectangles_t *extents)
{
    cairo_traps_compositor_t *compositor = (cairo_traps_compositor_t*)_compositor;
    cairo_boxes_t boxes;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = compositor->check_composite (extents);
    if (unlikely (status))
	return status;

     _cairo_clip_steal_boxes (extents->clip, &boxes);
     status = clip_and_composite_boxes (compositor, extents, &boxes);
     _cairo_clip_unsteal_boxes (extents->clip, &boxes);

    return status;
}

static cairo_int_status_t
_cairo_traps_compositor_mask (const cairo_compositor_t *_compositor,
			      cairo_composite_rectangles_t *extents)
{
    const cairo_traps_compositor_t *compositor = (cairo_traps_compositor_t*)_compositor;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = compositor->check_composite (extents);
    if (unlikely (status))
	return status;

    if (extents->mask_pattern.base.type == CAIRO_PATTERN_TYPE_SOLID &&
	extents->clip->path == NULL) {
	status = clip_and_composite (compositor, extents,
				     composite_opacity_boxes,
				     composite_opacity_boxes,
				     &extents->mask_pattern,
				     need_unbounded_clip (extents));
    } else {
	struct composite_mask data;

	data.mask = compositor->pattern_to_surface (extents->surface,
						    &extents->mask_pattern.base,
						    TRUE,
						    &extents->bounded,
						    &extents->mask_sample_area,
						    &data.mask_x,
						    &data.mask_y);
	if (unlikely (data.mask->status))
	    return data.mask->status;

	status = clip_and_composite (compositor, extents,
				     composite_mask,
				     extents->clip->path ? composite_mask_clip : composite_mask_clip_boxes,
				     &data, need_bounded_clip (extents));

	cairo_surface_destroy (data.mask);
    }

    return status;
}

static cairo_int_status_t
_cairo_traps_compositor_stroke (const cairo_compositor_t *_compositor,
				cairo_composite_rectangles_t *extents,
				const cairo_path_fixed_t *path,
				const cairo_stroke_style_t *style,
				const cairo_matrix_t	*ctm,
				const cairo_matrix_t	*ctm_inverse,
				double			 tolerance,
				cairo_antialias_t	 antialias)
{
    const cairo_traps_compositor_t *compositor = (cairo_traps_compositor_t *)_compositor;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = compositor->check_composite (extents);
    if (unlikely (status))
	return status;

    status = CAIRO_INT_STATUS_UNSUPPORTED;
    if (_cairo_path_fixed_stroke_is_rectilinear (path)) {
	cairo_boxes_t boxes;

	_cairo_boxes_init_with_clip (&boxes, extents->clip);
	status = _cairo_path_fixed_stroke_rectilinear_to_boxes (path,
								style,
								ctm,
								antialias,
								&boxes);
	if (likely (status == CAIRO_INT_STATUS_SUCCESS))
	    status = clip_and_composite_boxes (compositor, extents, &boxes);
	_cairo_boxes_fini (&boxes);
    }

    if (status == CAIRO_INT_STATUS_UNSUPPORTED && 0 &&
	_cairo_clip_is_region (extents->clip)) /* XXX */
    {
	composite_tristrip_info_t info;

	info.antialias = antialias;
	_cairo_tristrip_init_with_clip (&info.strip, extents->clip);
	status = _cairo_path_fixed_stroke_to_tristrip (path, style,
						       ctm, ctm_inverse,
						       tolerance,
						       &info.strip);
	if (likely (status == CAIRO_INT_STATUS_SUCCESS))
	    status = clip_and_composite_tristrip (compositor, extents, &info);
	_cairo_tristrip_fini (&info.strip);
    }

    if (status == CAIRO_INT_STATUS_UNSUPPORTED &&
	path->has_curve_to && antialias == CAIRO_ANTIALIAS_NONE) {
	cairo_polygon_t polygon;

	_cairo_polygon_init_with_clip (&polygon, extents->clip);
	status = _cairo_path_fixed_stroke_to_polygon (path, style,
						      ctm, ctm_inverse,
						      tolerance,
						      &polygon);
	if (likely (status == CAIRO_INT_STATUS_SUCCESS))
	    status = clip_and_composite_polygon (compositor,
						 extents, &polygon,
						 CAIRO_ANTIALIAS_NONE,
						 CAIRO_FILL_RULE_WINDING,
						 TRUE);
	_cairo_polygon_fini (&polygon);
    }

    if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	cairo_int_status_t (*func) (const cairo_path_fixed_t	*path,
				    const cairo_stroke_style_t	*stroke_style,
				    const cairo_matrix_t	*ctm,
				    const cairo_matrix_t	*ctm_inverse,
				    double			 tolerance,
				    cairo_traps_t		*traps);
	composite_traps_info_t info;
	unsigned flags;

	if (antialias == CAIRO_ANTIALIAS_BEST || antialias == CAIRO_ANTIALIAS_GOOD) {
	    func = _cairo_path_fixed_stroke_polygon_to_traps;
	    flags = 0;
	} else {
	    func = _cairo_path_fixed_stroke_to_traps;
	    flags = need_bounded_clip (extents) & ~NEED_CLIP_SURFACE;
	}

	info.antialias = antialias;
	_cairo_traps_init_with_clip (&info.traps, extents->clip);
	status = func (path, style, ctm, ctm_inverse, tolerance, &info.traps);
	if (likely (status == CAIRO_INT_STATUS_SUCCESS))
	    status = clip_and_composite_traps (compositor, extents, &info, flags);
	_cairo_traps_fini (&info.traps);
    }

    return status;
}

static cairo_int_status_t
_cairo_traps_compositor_fill (const cairo_compositor_t *_compositor,
			      cairo_composite_rectangles_t *extents,
			      const cairo_path_fixed_t	*path,
			      cairo_fill_rule_t		 fill_rule,
			      double			 tolerance,
			      cairo_antialias_t		 antialias)
{
    const cairo_traps_compositor_t *compositor = (cairo_traps_compositor_t *)_compositor;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = compositor->check_composite (extents);
    if (unlikely (status))
	return status;

    status = CAIRO_INT_STATUS_UNSUPPORTED;
    if (_cairo_path_fixed_fill_is_rectilinear (path)) {
	cairo_boxes_t boxes;

	_cairo_boxes_init_with_clip (&boxes, extents->clip);
	status = _cairo_path_fixed_fill_rectilinear_to_boxes (path,
							      fill_rule,
							      antialias,
							      &boxes);
	if (likely (status == CAIRO_INT_STATUS_SUCCESS))
	    status = clip_and_composite_boxes (compositor, extents, &boxes);
	_cairo_boxes_fini (&boxes);
    }

    if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	cairo_polygon_t polygon;

#if 0
	if (extents->mask.width  > extents->unbounded.width ||
	    extents->mask.height > extents->unbounded.height)
	{
	    cairo_box_t limits;
	    _cairo_box_from_rectangle (&limits, &extents->unbounded);
	    _cairo_polygon_init (&polygon, &limits, 1);
	}
	else
	{
	    _cairo_polygon_init (&polygon, NULL, 0);
	}

	status = _cairo_path_fixed_fill_to_polygon (path, tolerance, &polygon);
	if (likely (status == CAIRO_INT_STATUS_SUCCESS)) {
	    status = _cairo_polygon_intersect_with_boxes (&polygon, &fill_rule,
							  extents->clip->boxes,
							  extents->clip->num_boxes);
	}
#else
	_cairo_polygon_init_with_clip (&polygon, extents->clip);
	status = _cairo_path_fixed_fill_to_polygon (path, tolerance, &polygon);
#endif
	if (likely (status == CAIRO_INT_STATUS_SUCCESS)) {
	    status = clip_and_composite_polygon (compositor, extents, &polygon,
						 antialias, fill_rule, path->has_curve_to);
	}
	_cairo_polygon_fini (&polygon);
    }

    return status;
}

static cairo_int_status_t
composite_glyphs (const cairo_traps_compositor_t *compositor,
		  cairo_surface_t	*dst,
		  void *closure,
		  cairo_operator_t	 op,
		  cairo_surface_t	*src,
		  int src_x, int src_y,
		  int dst_x, int dst_y,
		  const cairo_rectangle_int_t *extents,
		  cairo_clip_t		*clip)
{
    cairo_composite_glyphs_info_t *info = closure;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (op == CAIRO_OPERATOR_ADD && (dst->content & CAIRO_CONTENT_COLOR) == 0)
	info->use_mask = 0;

    return compositor->composite_glyphs (dst, op, src,
					 src_x, src_y,
					 dst_x, dst_y,
					 info);
}

static cairo_int_status_t
_cairo_traps_compositor_glyphs (const cairo_compositor_t	*_compositor,
				cairo_composite_rectangles_t	*extents,
				cairo_scaled_font_t		*scaled_font,
				cairo_glyph_t			*glyphs,
				int				 num_glyphs,
				cairo_bool_t			 overlap)
{
    const cairo_traps_compositor_t *compositor = (cairo_traps_compositor_t *)_compositor;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    status = compositor->check_composite (extents);
    if (unlikely (status))
	return status;

    _cairo_scaled_font_freeze_cache (scaled_font);
    status = compositor->check_composite_glyphs (extents,
						 scaled_font, glyphs,
						 &num_glyphs);
    if (likely (status == CAIRO_INT_STATUS_SUCCESS)) {
	cairo_composite_glyphs_info_t info;

	info.font = scaled_font;
	info.glyphs = glyphs;
	info.num_glyphs = num_glyphs;
	info.use_mask = overlap || ! extents->is_bounded;
	info.extents = extents->bounded;

	status = clip_and_composite (compositor, extents,
				     composite_glyphs, NULL, &info,
				     need_bounded_clip (extents) | FORCE_CLIP_REGION);
    }
    _cairo_scaled_font_thaw_cache (scaled_font);

    return status;
}

void
_cairo_traps_compositor_init (cairo_traps_compositor_t *compositor,
			      const cairo_compositor_t  *delegate)
{
    compositor->base.delegate = delegate;

    compositor->base.paint = _cairo_traps_compositor_paint;
    compositor->base.mask = _cairo_traps_compositor_mask;
    compositor->base.fill = _cairo_traps_compositor_fill;
    compositor->base.stroke = _cairo_traps_compositor_stroke;
    compositor->base.glyphs = _cairo_traps_compositor_glyphs;
}
