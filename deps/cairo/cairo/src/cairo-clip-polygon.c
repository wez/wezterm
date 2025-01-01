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

#include "cairoint.h"
#include "cairo-clip-inline.h"
#include "cairo-clip-private.h"
#include "cairo-error-private.h"
#include "cairo-freed-pool-private.h"
#include "cairo-gstate-private.h"
#include "cairo-path-fixed-private.h"
#include "cairo-pattern-private.h"
#include "cairo-composite-rectangles-private.h"
#include "cairo-region-private.h"

static cairo_bool_t
can_convert_to_polygon (const cairo_clip_t *clip)
{
    cairo_clip_path_t *clip_path = clip->path;
    cairo_antialias_t antialias = clip_path->antialias;

    while ((clip_path = clip_path->prev) != NULL) {
	if (clip_path->antialias != antialias)
	    return FALSE;
    }

    return TRUE;
}

cairo_int_status_t
_cairo_clip_get_polygon (const cairo_clip_t *clip,
			 cairo_polygon_t *polygon,
			 cairo_fill_rule_t *fill_rule,
			 cairo_antialias_t *antialias)
{
    cairo_status_t status;
    cairo_clip_path_t *clip_path;

    if (_cairo_clip_is_all_clipped (clip)) {
	_cairo_polygon_init (polygon, NULL, 0);
	return CAIRO_INT_STATUS_SUCCESS;
    }

    /* If there is no clip, we need an infinite polygon */
    assert (clip && (clip->path || clip->num_boxes));

    if (clip->path == NULL) {
	*fill_rule = CAIRO_FILL_RULE_WINDING;
	*antialias = CAIRO_ANTIALIAS_DEFAULT;
	return _cairo_polygon_init_box_array (polygon,
					      clip->boxes,
					      clip->num_boxes);
    }

    /* check that residual is all of the same type/tolerance */
    if (! can_convert_to_polygon (clip))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    if (clip->num_boxes < 2)
	_cairo_polygon_init_with_clip (polygon, clip);
    else
	_cairo_polygon_init_with_clip (polygon, NULL);

    clip_path = clip->path;
    *fill_rule = clip_path->fill_rule;
    *antialias = clip_path->antialias;

    status = _cairo_path_fixed_fill_to_polygon (&clip_path->path,
						clip_path->tolerance,
						polygon);
    if (unlikely (status))
	goto err;

    if (clip->num_boxes > 1) {
	status = _cairo_polygon_intersect_with_boxes (polygon, fill_rule,
						      clip->boxes, clip->num_boxes);
	if (unlikely (status))
	    goto err;
    }

    polygon->limits = NULL;
    polygon->num_limits = 0;

    while ((clip_path = clip_path->prev) != NULL) {
	cairo_polygon_t next;

	_cairo_polygon_init (&next, NULL, 0);
	status = _cairo_path_fixed_fill_to_polygon (&clip_path->path,
						    clip_path->tolerance,
						    &next);
	if (likely (status == CAIRO_STATUS_SUCCESS))
		status = _cairo_polygon_intersect (polygon, *fill_rule,
						   &next, clip_path->fill_rule);
	_cairo_polygon_fini (&next);
	if (unlikely (status))
	    goto err;

	*fill_rule = CAIRO_FILL_RULE_WINDING;
    }

    return CAIRO_STATUS_SUCCESS;

err:
    _cairo_polygon_fini (polygon);
    return status;
}

cairo_bool_t
_cairo_clip_is_polygon (const cairo_clip_t *clip)
{
    if (_cairo_clip_is_all_clipped (clip))
	return TRUE;

    /* If there is no clip, we need an infinite polygon */
    if (clip == NULL)
	return FALSE;

    if (clip->path == NULL)
	return TRUE;

    /* check that residual is all of the same type/tolerance */
    return can_convert_to_polygon (clip);
}
