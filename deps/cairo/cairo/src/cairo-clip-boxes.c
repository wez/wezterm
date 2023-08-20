/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2005 Red Hat, Inc.
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
 * The Initial Developer of the Original Code is University of Southern
 * California.
 *
 * Contributor(s):
 *	Carl D. Worth <cworth@cworth.org>
 *	Kristian Høgsberg <krh@redhat.com>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-box-inline.h"
#include "cairo-clip-inline.h"
#include "cairo-clip-private.h"
#include "cairo-error-private.h"
#include "cairo-freed-pool-private.h"
#include "cairo-gstate-private.h"
#include "cairo-path-fixed-private.h"
#include "cairo-pattern-private.h"
#include "cairo-composite-rectangles-private.h"
#include "cairo-region-private.h"

static inline int
pot (int v)
{
    v--;
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v++;
    return v;
}

static cairo_bool_t
_cairo_clip_contains_rectangle_box (const cairo_clip_t *clip,
				    const cairo_rectangle_int_t *rect,
				    const cairo_box_t *box)
{
    int i;

    /* clip == NULL means no clip, so the clip contains everything */
    if (clip == NULL)
	return TRUE;

    if (_cairo_clip_is_all_clipped (clip))
	return FALSE;

    /* If we have a non-trivial path, just say no */
    if (clip->path)
	return FALSE;

    if (! _cairo_rectangle_contains_rectangle (&clip->extents, rect))
	return FALSE;

    if (clip->num_boxes == 0)
	return TRUE;

    /* Check for a clip-box that wholly contains the rectangle */
    for (i = 0; i < clip->num_boxes; i++) {
	if (box->p1.x >= clip->boxes[i].p1.x &&
	    box->p1.y >= clip->boxes[i].p1.y &&
	    box->p2.x <= clip->boxes[i].p2.x &&
	    box->p2.y <= clip->boxes[i].p2.y)
	{
	    return TRUE;
	}
    }

    return FALSE;
}

cairo_bool_t
_cairo_clip_contains_box (const cairo_clip_t *clip,
			  const cairo_box_t *box)
{
    cairo_rectangle_int_t rect;

    _cairo_box_round_to_rectangle (box, &rect);
    return _cairo_clip_contains_rectangle_box(clip, &rect, box);
}

cairo_bool_t
_cairo_clip_contains_rectangle (const cairo_clip_t *clip,
				const cairo_rectangle_int_t *rect)
{
    cairo_box_t box;

    _cairo_box_from_rectangle_int (&box, rect);
    return _cairo_clip_contains_rectangle_box (clip, rect, &box);
}

cairo_clip_t *
_cairo_clip_intersect_rectilinear_path (cairo_clip_t *clip,
					const cairo_path_fixed_t *path,
					cairo_fill_rule_t fill_rule,
					cairo_antialias_t antialias)
{
    cairo_status_t status;
    cairo_boxes_t boxes;

    _cairo_boxes_init (&boxes);
    status = _cairo_path_fixed_fill_rectilinear_to_boxes (path,
							  fill_rule,
							  antialias,
							  &boxes);
    if (likely (status == CAIRO_STATUS_SUCCESS && boxes.num_boxes))
	clip = _cairo_clip_intersect_boxes (clip, &boxes);
    else
	clip = _cairo_clip_set_all_clipped (clip);
    _cairo_boxes_fini (&boxes);

    return clip;
}

static cairo_clip_t *
_cairo_clip_intersect_rectangle_box (cairo_clip_t *clip,
				     const cairo_rectangle_int_t *r,
				     const cairo_box_t *box)
{
    cairo_box_t extents_box;
    cairo_bool_t changed = FALSE;
    int i, j;

    if (clip == NULL) {
	clip = _cairo_clip_create ();
	if (clip == NULL)
	    return _cairo_clip_set_all_clipped (clip);
    }

    if (clip->num_boxes == 0) {
	clip->boxes = &clip->embedded_box;
	clip->boxes[0] = *box;
	clip->num_boxes = 1;
	if (clip->path == NULL) {
	    clip->extents = *r;
	} else {
	    if (! _cairo_rectangle_intersect (&clip->extents, r))
		return _cairo_clip_set_all_clipped (clip);
	}
	if (clip->path == NULL)
	    clip->is_region = _cairo_box_is_pixel_aligned (box);
	return clip;
    }

    /* Does the new box wholly subsume the clip? Perform a cheap check
     * for the common condition of a single clip rectangle.
     */
    if (clip->num_boxes == 1 &&
	clip->boxes[0].p1.x >= box->p1.x &&
	clip->boxes[0].p1.y >= box->p1.y &&
	clip->boxes[0].p2.x <= box->p2.x &&
	clip->boxes[0].p2.y <= box->p2.y)
    {
	return clip;
    }

    for (i = j = 0; i < clip->num_boxes; i++) {
	cairo_box_t *b = &clip->boxes[j];

	if (j != i)
	    *b = clip->boxes[i];

	if (box->p1.x > b->p1.x)
	    b->p1.x = box->p1.x, changed = TRUE;
	if (box->p2.x < b->p2.x)
	    b->p2.x = box->p2.x, changed = TRUE;

	if (box->p1.y > b->p1.y)
	    b->p1.y = box->p1.y, changed = TRUE;
	if (box->p2.y < b->p2.y)
	    b->p2.y = box->p2.y, changed = TRUE;

	j += b->p2.x > b->p1.x && b->p2.y > b->p1.y;
    }
    clip->num_boxes = j;

    if (clip->num_boxes == 0)
	return _cairo_clip_set_all_clipped (clip);

    if (! changed)
	return clip;

    extents_box = clip->boxes[0];
    for (i = 1; i < clip->num_boxes; i++) {
	    if (clip->boxes[i].p1.x < extents_box.p1.x)
		extents_box.p1.x = clip->boxes[i].p1.x;

	    if (clip->boxes[i].p1.y < extents_box.p1.y)
		extents_box.p1.y = clip->boxes[i].p1.y;

	    if (clip->boxes[i].p2.x > extents_box.p2.x)
		extents_box.p2.x = clip->boxes[i].p2.x;

	    if (clip->boxes[i].p2.y > extents_box.p2.y)
		extents_box.p2.y = clip->boxes[i].p2.y;
    }

    if (clip->path == NULL) {
	_cairo_box_round_to_rectangle (&extents_box, &clip->extents);
    } else {
	cairo_rectangle_int_t extents_rect;

	_cairo_box_round_to_rectangle (&extents_box, &extents_rect);
	if (! _cairo_rectangle_intersect (&clip->extents, &extents_rect))
	    return _cairo_clip_set_all_clipped (clip);
    }

    if (clip->region) {
	cairo_region_destroy (clip->region);
	clip->region = NULL;
    }

    clip->is_region = FALSE;
    return clip;
}

cairo_clip_t *
_cairo_clip_intersect_box (cairo_clip_t *clip,
			   const cairo_box_t *box)
{
    cairo_rectangle_int_t r;

    if (_cairo_clip_is_all_clipped (clip))
	return clip;

    _cairo_box_round_to_rectangle (box, &r);
    if (r.width == 0 || r.height == 0)
	return _cairo_clip_set_all_clipped (clip);

    return _cairo_clip_intersect_rectangle_box (clip, &r, box);
}

/* Copy a box set to a clip
 *
 * @param boxes  The box set to copy from.
 * @param clip   The clip to copy to (return buffer).
 * @returns      Zero if the allocation failed (the clip will be set to
 *               all-clipped), otherwise non-zero.
 */
static cairo_bool_t
_cairo_boxes_copy_to_clip (const cairo_boxes_t *boxes, cairo_clip_t *clip)
{
    /* XXX cow-boxes? */
    if (boxes->num_boxes == 1) {
	clip->boxes = &clip->embedded_box;
	clip->boxes[0] = boxes->chunks.base[0];
	clip->num_boxes = 1;
	return TRUE;
    }

    clip->boxes = _cairo_boxes_to_array (boxes, &clip->num_boxes);
    if (unlikely (clip->boxes == NULL))
    {
	_cairo_clip_set_all_clipped (clip);
	return FALSE;
    }

    return TRUE;
}

cairo_clip_t *
_cairo_clip_intersect_boxes (cairo_clip_t *clip,
			     const cairo_boxes_t *boxes)
{
    cairo_boxes_t clip_boxes;
    cairo_box_t limits;
    cairo_rectangle_int_t extents;

    if (_cairo_clip_is_all_clipped (clip))
	return clip;

    if (boxes->num_boxes == 0)
	return _cairo_clip_set_all_clipped (clip);

    if (boxes->num_boxes == 1)
	return _cairo_clip_intersect_box (clip, boxes->chunks.base);

    if (clip == NULL)
	clip = _cairo_clip_create ();

    if (clip->num_boxes) {
	_cairo_boxes_init_for_array (&clip_boxes, clip->boxes, clip->num_boxes);
	if (unlikely (_cairo_boxes_intersect (&clip_boxes, boxes, &clip_boxes))) {
	    clip = _cairo_clip_set_all_clipped (clip);
	    goto out;
	}

	if (clip->boxes != &clip->embedded_box)
	    free (clip->boxes);

	clip->boxes = NULL;
	boxes = &clip_boxes;
    }

    if (boxes->num_boxes == 0) {
	clip = _cairo_clip_set_all_clipped (clip);
	goto out;
    }

    _cairo_boxes_copy_to_clip (boxes, clip);

    _cairo_boxes_extents (boxes, &limits);

    _cairo_box_round_to_rectangle (&limits, &extents);
    if (clip->path == NULL) {
	clip->extents = extents;
    } else if (! _cairo_rectangle_intersect (&clip->extents, &extents)) {
	clip = _cairo_clip_set_all_clipped (clip);
	goto out;
    }

    if (clip->region) {
	cairo_region_destroy (clip->region);
	clip->region = NULL;
    }
    clip->is_region = FALSE;

out:
    if (boxes == &clip_boxes)
	_cairo_boxes_fini (&clip_boxes);

    return clip;
}

cairo_clip_t *
_cairo_clip_intersect_rectangle (cairo_clip_t       *clip,
				 const cairo_rectangle_int_t *r)
{
    cairo_box_t box;

    if (_cairo_clip_is_all_clipped (clip))
	return clip;

    if (r->width == 0 || r->height == 0)
	return _cairo_clip_set_all_clipped (clip);

    _cairo_box_from_rectangle_int (&box, r);

    return _cairo_clip_intersect_rectangle_box (clip, r, &box);
}

struct reduce {
    cairo_clip_t *clip;
    cairo_box_t limit;
    cairo_box_t extents;
    cairo_bool_t inside;

    cairo_point_t current_point;
    cairo_point_t last_move_to;
};

static void
_add_clipped_edge (struct reduce *r,
		   const cairo_point_t *p1,
		   const cairo_point_t *p2,
		   int y1, int y2)
{
    cairo_fixed_t x;

    x = _cairo_edge_compute_intersection_x_for_y (p1, p2, y1);
    if (x < r->extents.p1.x)
	r->extents.p1.x = x;

    x = _cairo_edge_compute_intersection_x_for_y (p1, p2, y2);
    if (x > r->extents.p2.x)
	r->extents.p2.x = x;

    if (y1 < r->extents.p1.y)
	r->extents.p1.y = y1;

    if (y2 > r->extents.p2.y)
	r->extents.p2.y = y2;

    r->inside = TRUE;
}

static void
_add_edge (struct reduce *r,
	   const cairo_point_t *p1,
	   const cairo_point_t *p2)
{
    int top, bottom;
    int top_y, bot_y;
    int n;

    if (p1->y < p2->y) {
	top = p1->y;
	bottom = p2->y;
    } else {
	top = p2->y;
	bottom = p1->y;
    }

    if (bottom < r->limit.p1.y || top > r->limit.p2.y)
	return;

    if (p1->x > p2->x) {
	const cairo_point_t *t = p1;
	p1 = p2;
	p2 = t;
    }

    if (p2->x <= r->limit.p1.x || p1->x >= r->limit.p2.x)
	return;

    for (n = 0; n < r->clip->num_boxes; n++) {
	const cairo_box_t *limits = &r->clip->boxes[n];

	if (bottom < limits->p1.y || top > limits->p2.y)
	    continue;

	if (p2->x <= limits->p1.x || p1->x >= limits->p2.x)
	    continue;

	if (p1->x >= limits->p1.x && p2->x <= limits->p1.x) {
	    top_y = top;
	    bot_y = bottom;
	} else {
	    int p1_y, p2_y;

	    p1_y = _cairo_edge_compute_intersection_y_for_x (p1, p2,
							     limits->p1.x);
	    p2_y = _cairo_edge_compute_intersection_y_for_x (p1, p2,
							     limits->p2.x);
	    if (p1_y < p2_y) {
		top_y = p1_y;
		bot_y = p2_y;
	    } else {
		top_y = p2_y;
		bot_y = p1_y;
	    }

	    if (top_y < top)
		top_y = top;
	    if (bot_y > bottom)
		bot_y = bottom;
	}

	if (top_y < limits->p1.y)
	    top_y = limits->p1.y;

	if (bot_y > limits->p2.y)
	    bot_y = limits->p2.y;
	if (bot_y > top_y)
	    _add_clipped_edge (r, p1, p2, top_y, bot_y);
    }
}

static cairo_status_t
_reduce_line_to (void *closure,
		       const cairo_point_t *point)
{
    struct reduce *r = closure;

    _add_edge (r, &r->current_point, point);
    r->current_point = *point;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_reduce_close (void *closure)
{
    struct reduce *r = closure;

    return _reduce_line_to (r, &r->last_move_to);
}

static cairo_status_t
_reduce_move_to (void *closure,
		 const cairo_point_t *point)
{
    struct reduce *r = closure;
    cairo_status_t status;

    /* close current subpath */
    status = _reduce_close (closure);

    /* make sure that the closure represents a degenerate path */
    r->current_point = *point;
    r->last_move_to = *point;

    return status;
}

static cairo_clip_t *
_cairo_clip_reduce_to_boxes (cairo_clip_t *clip)
{
    struct reduce r;
    cairo_clip_path_t *clip_path;
    cairo_status_t status;

	return clip;
    if (clip->path == NULL)
	return clip;

    r.clip = clip;
    r.extents.p1.x = r.extents.p1.y = INT_MAX;
    r.extents.p2.x = r.extents.p2.y = INT_MIN;
    r.inside = FALSE;

    r.limit.p1.x = _cairo_fixed_from_int (clip->extents.x);
    r.limit.p1.y = _cairo_fixed_from_int (clip->extents.y);
    r.limit.p2.x = _cairo_fixed_from_int (clip->extents.x + clip->extents.width);
    r.limit.p2.y = _cairo_fixed_from_int (clip->extents.y + clip->extents.height);

    clip_path = clip->path;
    do {
	r.current_point.x = 0;
	r.current_point.y = 0;
	r.last_move_to = r.current_point;

	status = _cairo_path_fixed_interpret_flat (&clip_path->path,
						   _reduce_move_to,
						   _reduce_line_to,
						   _reduce_close,
						   &r,
						   clip_path->tolerance);
	assert (status == CAIRO_STATUS_SUCCESS);
	_reduce_close (&r);
    } while ((clip_path = clip_path->prev));

    if (! r.inside) {
	_cairo_clip_path_destroy (clip->path);
	clip->path = NULL;
    }

    return _cairo_clip_intersect_box (clip, &r.extents);
}

cairo_clip_t *
_cairo_clip_reduce_to_rectangle (const cairo_clip_t *clip,
				 const cairo_rectangle_int_t *r)
{
    cairo_clip_t *copy;

    if (_cairo_clip_is_all_clipped (clip))
	return (cairo_clip_t *) clip;

    if (_cairo_clip_contains_rectangle (clip, r))
	return _cairo_clip_intersect_rectangle (NULL, r);

    copy = _cairo_clip_copy_intersect_rectangle (clip, r);
    if (_cairo_clip_is_all_clipped (copy))
	return copy;

    return _cairo_clip_reduce_to_boxes (copy);
}

cairo_clip_t *
_cairo_clip_reduce_for_composite (const cairo_clip_t *clip,
				  cairo_composite_rectangles_t *extents)
{
    const cairo_rectangle_int_t *r;

    r = extents->is_bounded ? &extents->bounded : &extents->unbounded;
    return _cairo_clip_reduce_to_rectangle (clip, r);
}

cairo_clip_t *
_cairo_clip_from_boxes (const cairo_boxes_t *boxes)
{
    cairo_box_t extents;
    cairo_clip_t *clip = _cairo_clip_create ();
    if (clip == NULL)
	return _cairo_clip_set_all_clipped (clip);

    if (unlikely (! _cairo_boxes_copy_to_clip (boxes, clip)))
	return clip;

    _cairo_boxes_extents (boxes, &extents);
    _cairo_box_round_to_rectangle (&extents, &clip->extents);

    return clip;
}
