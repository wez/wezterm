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

static void
_cairo_clip_extract_region (cairo_clip_t *clip)
{
    cairo_rectangle_int_t stack_rects[CAIRO_STACK_ARRAY_LENGTH (cairo_rectangle_int_t)];
    cairo_rectangle_int_t *r = stack_rects;
    cairo_bool_t is_region;
    int i;

    if (clip->num_boxes == 0)
	return;

    if (clip->num_boxes > ARRAY_LENGTH (stack_rects)) {
	r = _cairo_malloc_ab (clip->num_boxes, sizeof (cairo_rectangle_int_t));
	if (r == NULL){
	    _cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	    return;
	}
    }

    is_region = clip->path == NULL;
    for (i = 0; i < clip->num_boxes; i++) {
	cairo_box_t *b = &clip->boxes[i];
	if (is_region)
	    is_region =
		_cairo_fixed_is_integer (b->p1.x | b->p1.y |  b->p2.x | b->p2.y);
	r[i].x = _cairo_fixed_integer_floor (b->p1.x);
	r[i].y = _cairo_fixed_integer_floor (b->p1.y);
	r[i].width  = _cairo_fixed_integer_ceil (b->p2.x) - r[i].x;
	r[i].height = _cairo_fixed_integer_ceil (b->p2.y) - r[i].y;
    }
    clip->is_region = is_region;

    clip->region = cairo_region_create_rectangles (r, i);

    if (r != stack_rects)
	free (r);
}

cairo_region_t *
_cairo_clip_get_region (const cairo_clip_t *clip)
{
    if (clip == NULL)
	return NULL;

    if (clip->region == NULL)
	_cairo_clip_extract_region ((cairo_clip_t *) clip);

    return clip->region;
}

cairo_bool_t
_cairo_clip_is_region (const cairo_clip_t *clip)
{
    if (clip == NULL)
	return TRUE;

    if (clip->is_region)
	return TRUE;

    /* XXX Geometric reduction? */

    if (clip->path)
	return FALSE;

    if (clip->num_boxes == 0)
	return TRUE;

    if (clip->region == NULL)
	_cairo_clip_extract_region ((cairo_clip_t *) clip);

    return clip->is_region;
}
