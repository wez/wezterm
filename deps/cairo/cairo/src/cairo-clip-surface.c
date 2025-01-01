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
#include "cairo-clip-private.h"
#include "cairo-error-private.h"
#include "cairo-freed-pool-private.h"
#include "cairo-gstate-private.h"
#include "cairo-path-fixed-private.h"
#include "cairo-pattern-private.h"
#include "cairo-composite-rectangles-private.h"
#include "cairo-region-private.h"

cairo_status_t
_cairo_clip_combine_with_surface (const cairo_clip_t *clip,
				  cairo_surface_t *dst,
				  int dst_x, int dst_y)
{
    cairo_clip_path_t *copy_path;
    cairo_clip_path_t *clip_path;
    cairo_clip_t *copy;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    copy = _cairo_clip_copy_with_translation (clip, -dst_x, -dst_y);
    copy_path = copy->path;
    copy->path = NULL;

    if (copy->boxes) {
	status = _cairo_surface_paint (dst,
				       CAIRO_OPERATOR_IN,
				       &_cairo_pattern_white.base,
				       copy);
    }

    clip = NULL;
    if (_cairo_clip_is_region (copy))
	clip = copy;
    clip_path = copy_path;
    while (status == CAIRO_STATUS_SUCCESS && clip_path) {
	status = _cairo_surface_fill (dst,
				      CAIRO_OPERATOR_IN,
				      &_cairo_pattern_white.base,
				      &clip_path->path,
				      clip_path->fill_rule,
				      clip_path->tolerance,
				      clip_path->antialias,
				      clip);
	clip_path = clip_path->prev;
    }

    copy->path = copy_path;
    _cairo_clip_destroy (copy);
    return status;
}

static cairo_status_t
_cairo_path_fixed_add_box (cairo_path_fixed_t *path,
			   const cairo_box_t *box,
			   cairo_fixed_t fx,
			   cairo_fixed_t fy)
{
    cairo_status_t status;

    status = _cairo_path_fixed_move_to (path, box->p1.x + fx, box->p1.y + fy);
    if (unlikely (status))
	return status;

    status = _cairo_path_fixed_line_to (path, box->p2.x + fx, box->p1.y + fy);
    if (unlikely (status))
	return status;

    status = _cairo_path_fixed_line_to (path, box->p2.x + fx, box->p2.y + fy);
    if (unlikely (status))
	return status;

    status = _cairo_path_fixed_line_to (path, box->p1.x + fx, box->p2.y + fy);
    if (unlikely (status))
	return status;

    return _cairo_path_fixed_close_path (path);
}

cairo_surface_t *
_cairo_clip_get_surface (const cairo_clip_t *clip,
			 cairo_surface_t *target,
			 int *tx, int *ty)
{
    cairo_surface_t *surface;
    cairo_status_t status;
    cairo_clip_t *copy, *region;
    cairo_clip_path_t *copy_path, *clip_path;

    if (clip->num_boxes) {
	cairo_path_fixed_t path;
	int i;

	surface = _cairo_surface_create_scratch (target,
						 CAIRO_CONTENT_ALPHA,
						 clip->extents.width,
						 clip->extents.height,
						 CAIRO_COLOR_TRANSPARENT);
	if (unlikely (surface->status))
	    return surface;

	_cairo_path_fixed_init (&path);
	status = CAIRO_STATUS_SUCCESS;
	for (i = 0; status == CAIRO_STATUS_SUCCESS && i < clip->num_boxes; i++) {
	    status = _cairo_path_fixed_add_box (&path, &clip->boxes[i],
						-_cairo_fixed_from_int (clip->extents.x),
						-_cairo_fixed_from_int (clip->extents.y));
	}
	if (status == CAIRO_STATUS_SUCCESS)
	    status = _cairo_surface_fill (surface,
					  CAIRO_OPERATOR_ADD,
					  &_cairo_pattern_white.base,
					  &path,
					  CAIRO_FILL_RULE_WINDING,
					  1.,
					  CAIRO_ANTIALIAS_DEFAULT,
					  NULL);
	_cairo_path_fixed_fini (&path);
	if (unlikely (status)) {
	    cairo_surface_destroy (surface);
	    return _cairo_surface_create_in_error (status);
	}
    } else {
	surface = _cairo_surface_create_scratch (target,
						 CAIRO_CONTENT_ALPHA,
						 clip->extents.width,
						 clip->extents.height,
						 CAIRO_COLOR_WHITE);
	if (unlikely (surface->status))
	    return surface;
    }

    copy = _cairo_clip_copy_with_translation (clip,
					      -clip->extents.x,
					      -clip->extents.y);
    copy_path = copy->path;
    copy->path = NULL;

    region = copy;
    if (! _cairo_clip_is_region (copy))
	region = _cairo_clip_copy_region (copy);

    status = CAIRO_STATUS_SUCCESS;
    clip_path = copy_path;
    while (status == CAIRO_STATUS_SUCCESS && clip_path) {
	status = _cairo_surface_fill (surface,
				      CAIRO_OPERATOR_IN,
				      &_cairo_pattern_white.base,
				      &clip_path->path,
				      clip_path->fill_rule,
				      clip_path->tolerance,
				      clip_path->antialias,
				      region);
	clip_path = clip_path->prev;
    }

    copy->path = copy_path;
    _cairo_clip_destroy (copy);
    if (region != copy)
	_cairo_clip_destroy (region);

    if (unlikely (status)) {
	cairo_surface_destroy (surface);
	return _cairo_surface_create_in_error (status);
    }

    *tx = clip->extents.x;
    *ty = clip->extents.y;
    return surface;
}

cairo_surface_t *
_cairo_clip_get_image (const cairo_clip_t *clip,
		       cairo_surface_t *target,
		       const cairo_rectangle_int_t *extents)
{
    cairo_surface_t *surface;
    cairo_status_t status;

    surface = cairo_surface_create_similar_image (target,
						  CAIRO_FORMAT_A8,
						  extents->width,
						  extents->height);
    if (unlikely (surface->status))
	return surface;

    status = _cairo_surface_paint (surface, CAIRO_OPERATOR_SOURCE,
				   &_cairo_pattern_white.base, NULL);
    if (likely (status == CAIRO_STATUS_SUCCESS))
	status = _cairo_clip_combine_with_surface (clip, surface,
						   extents->x, extents->y);

    if (unlikely (status)) {
	cairo_surface_destroy (surface);
	surface = _cairo_surface_create_in_error (status);
    }

    return surface;
}
