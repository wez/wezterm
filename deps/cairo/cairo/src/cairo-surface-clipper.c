/* cairo - a vector graphics library with display and print output
 *
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
 * The Initial Developer of the Original Code is Red Hat, Inc.
 *
 * Contributor(s):
 *      Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-clip-inline.h"
#include "cairo-surface-clipper-private.h"

/* A collection of routines to facilitate vector surface clipping */

/* XXX Eliminate repeated paths and nested clips */

static cairo_status_t
_cairo_path_fixed_add_box (cairo_path_fixed_t *path,
			   const cairo_box_t *box)
{
    cairo_status_t status;

    status = _cairo_path_fixed_move_to (path, box->p1.x, box->p1.y);
    if (unlikely (status))
	return status;

    status = _cairo_path_fixed_line_to (path, box->p2.x, box->p1.y);
    if (unlikely (status))
	return status;

    status = _cairo_path_fixed_line_to (path, box->p2.x, box->p2.y);
    if (unlikely (status))
	return status;

    status = _cairo_path_fixed_line_to (path, box->p1.x, box->p2.y);
    if (unlikely (status))
	return status;

    return _cairo_path_fixed_close_path (path);
}

static cairo_status_t
_cairo_surface_clipper_intersect_clip_boxes (cairo_surface_clipper_t *clipper,
					     const cairo_clip_t *clip)
{
    cairo_path_fixed_t path;
    cairo_status_t status;
    int i;

    if (clip->num_boxes == 0)
	return CAIRO_STATUS_SUCCESS;

    /* Reconstruct the path for the clip boxes.
     * XXX maybe a new clipper callback?
     */

    _cairo_path_fixed_init (&path);
    for (i = 0; i < clip->num_boxes; i++) {
	status = _cairo_path_fixed_add_box (&path, &clip->boxes[i]);
	if (unlikely (status)) {
	    _cairo_path_fixed_fini (&path);
	    return status;
	}
    }

    status = clipper->intersect_clip_path (clipper, &path,
					   CAIRO_FILL_RULE_WINDING,
					   0.,
					   CAIRO_ANTIALIAS_DEFAULT);
    _cairo_path_fixed_fini (&path);

    return status;
}

static cairo_status_t
_cairo_surface_clipper_intersect_clip_path_recursive (cairo_surface_clipper_t *clipper,
						      cairo_clip_path_t *clip_path,
						      cairo_clip_path_t *end)
{
    cairo_status_t status;

    if (clip_path->prev != end) {
	status =
	    _cairo_surface_clipper_intersect_clip_path_recursive (clipper,
								  clip_path->prev,
								  end);
	if (unlikely (status))
	    return status;
    }

    return clipper->intersect_clip_path (clipper,
					 &clip_path->path,
					 clip_path->fill_rule,
					 clip_path->tolerance,
					 clip_path->antialias);
}

cairo_status_t
_cairo_surface_clipper_set_clip (cairo_surface_clipper_t *clipper,
				 const cairo_clip_t *clip)
{
    cairo_status_t status;
    cairo_bool_t incremental = FALSE;

    if (_cairo_clip_equal (clip, clipper->clip))
	return CAIRO_STATUS_SUCCESS;

    /* all clipped out state should never propagate this far */
    assert (!_cairo_clip_is_all_clipped (clip));

    /* XXX Is this an incremental clip? */
    if (clipper->clip && clip &&
	clip->num_boxes == clipper->clip->num_boxes &&
	memcmp (clip->boxes, clipper->clip->boxes,
		sizeof (cairo_box_t) * clip->num_boxes) == 0)
    {
	cairo_clip_path_t *clip_path = clip->path;
	while (clip_path != NULL && clip_path != clipper->clip->path)
	    clip_path = clip_path->prev;

	if (clip_path) {
	    incremental = TRUE;
	    status = _cairo_surface_clipper_intersect_clip_path_recursive (clipper,
									   clip->path,
									   clipper->clip->path);
	}
    }

    _cairo_clip_destroy (clipper->clip);
    clipper->clip = _cairo_clip_copy (clip);

    if (incremental)
	return status;

    status = clipper->intersect_clip_path (clipper, NULL, 0, 0, 0);
    if (unlikely (status))
	return status;

    if (clip == NULL)
	return CAIRO_STATUS_SUCCESS;

    status = _cairo_surface_clipper_intersect_clip_boxes (clipper, clip);
    if (unlikely (status))
	return status;

    if (clip->path != NULL) {
	    status = _cairo_surface_clipper_intersect_clip_path_recursive (clipper,
									   clip->path,
									   NULL);
    }

    return status;
}

void
_cairo_surface_clipper_init (cairo_surface_clipper_t *clipper,
			     cairo_surface_clipper_intersect_clip_path_func_t func)
{
    clipper->clip = NULL;
    clipper->intersect_clip_path = func;
}

void
_cairo_surface_clipper_reset (cairo_surface_clipper_t *clipper)
{
    _cairo_clip_destroy (clipper->clip);
    clipper->clip = NULL;
}
