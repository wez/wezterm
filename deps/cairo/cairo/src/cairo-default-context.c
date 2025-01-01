/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
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
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-private.h"
#include "cairo-arc-private.h"
#include "cairo-backend-private.h"
#include "cairo-clip-inline.h"
#include "cairo-default-context-private.h"
#include "cairo-error-private.h"
#include "cairo-freed-pool-private.h"
#include "cairo-path-private.h"
#include "cairo-pattern-private.h"

#define CAIRO_TOLERANCE_MINIMUM	_cairo_fixed_to_double(1)

#if !defined(INFINITY)
#define INFINITY HUGE_VAL
#endif

static freed_pool_t context_pool;

void
_cairo_default_context_reset_static_data (void)
{
    _freed_pool_reset (&context_pool);
}

void
_cairo_default_context_fini (cairo_default_context_t *cr)
{
    while (cr->gstate != &cr->gstate_tail[0]) {
	if (_cairo_gstate_restore (&cr->gstate, &cr->gstate_freelist))
	    break;
    }

    _cairo_gstate_fini (cr->gstate);
    cr->gstate_freelist = cr->gstate_freelist->next; /* skip over tail[1] */
    while (cr->gstate_freelist != NULL) {
	cairo_gstate_t *gstate = cr->gstate_freelist;
	cr->gstate_freelist = gstate->next;
	free (gstate);
    }

    _cairo_path_fixed_fini (cr->path);

    _cairo_fini (&cr->base);
}

static void
_cairo_default_context_destroy (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_default_context_fini (cr);

    /* mark the context as invalid to protect against misuse */
    cr->base.status = CAIRO_STATUS_NULL_POINTER;
    _freed_pool_put (&context_pool, cr);
}

static cairo_surface_t *
_cairo_default_context_get_original_target (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_original_target (cr->gstate);
}

static cairo_surface_t *
_cairo_default_context_get_current_target (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_target (cr->gstate);
}

static cairo_status_t
_cairo_default_context_save (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_save (&cr->gstate, &cr->gstate_freelist);
}

static cairo_status_t
_cairo_default_context_restore (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    if (unlikely (_cairo_gstate_is_group (cr->gstate)))
	return _cairo_error (CAIRO_STATUS_INVALID_RESTORE);

    return _cairo_gstate_restore (&cr->gstate, &cr->gstate_freelist);
}

static cairo_status_t
_cairo_default_context_push_group (void *abstract_cr, cairo_content_t content)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_surface_t *group_surface;
    cairo_clip_t *clip;
    cairo_status_t status;

    clip = _cairo_gstate_get_clip (cr->gstate);
    if (_cairo_clip_is_all_clipped (clip)) {
	group_surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 0, 0);
	status = group_surface->status;
	if (unlikely (status))
	    goto bail;
    } else {
	cairo_surface_t *parent_surface;
	cairo_rectangle_int_t extents;
	cairo_bool_t bounded, is_empty;

	parent_surface = _cairo_gstate_get_target (cr->gstate);

	if (unlikely (parent_surface->status))
	    return parent_surface->status;
	if (unlikely (parent_surface->finished))
	    return _cairo_error (CAIRO_STATUS_SURFACE_FINISHED);

	/* Get the extents that we'll use in creating our new group surface */
	bounded = _cairo_surface_get_extents (parent_surface, &extents);
	if (clip)
	    /* XXX: This assignment just fixes a compiler warning? */
	    is_empty = _cairo_rectangle_intersect (&extents,
						   _cairo_clip_get_extents (clip));

	if (!bounded) {
	    /* XXX: Generic solution? */
	    group_surface = cairo_recording_surface_create (content, NULL);
	    extents.x = extents.y = 0;
	} else {
	    group_surface = _cairo_surface_create_scratch (parent_surface,
							   content,
							   extents.width,
							   extents.height,
							   CAIRO_COLOR_TRANSPARENT);
	}
	status = group_surface->status;
	if (unlikely (status))
	    goto bail;

	/* Set device offsets on the new surface so that logically it appears at
	 * the same location on the parent surface -- when we pop_group this,
	 * the source pattern will get fixed up for the appropriate target surface
	 * device offsets, so we want to set our own surface offsets from /that/,
	 * and not from the device origin. */
	cairo_surface_set_device_offset (group_surface,
					 parent_surface->device_transform.x0 - extents.x,
					 parent_surface->device_transform.y0 - extents.y);

	cairo_surface_set_device_scale (group_surface,
					parent_surface->device_transform.xx,
					parent_surface->device_transform.yy);

	/* If we have a current path, we need to adjust it to compensate for
	 * the device offset just applied. */
	_cairo_path_fixed_translate (cr->path,
				     _cairo_fixed_from_int (-extents.x),
				     _cairo_fixed_from_int (-extents.y));
    }

    /* create a new gstate for the redirect */
    status = _cairo_gstate_save (&cr->gstate, &cr->gstate_freelist);
    if (unlikely (status))
	goto bail;

    status = _cairo_gstate_redirect_target (cr->gstate, group_surface);

bail:
    cairo_surface_destroy (group_surface);
    return status;
}

static cairo_pattern_t *
_cairo_default_context_pop_group (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_surface_t *group_surface;
    cairo_pattern_t *group_pattern;
    cairo_surface_t *parent_surface;
    cairo_matrix_t group_matrix;
    cairo_status_t status;

    /* Verify that we are at the right nesting level */
    if (unlikely (! _cairo_gstate_is_group (cr->gstate)))
	return _cairo_pattern_create_in_error (CAIRO_STATUS_INVALID_POP_GROUP);

    /* Get a reference to the active surface before restoring */
    group_surface = _cairo_gstate_get_target (cr->gstate);
    group_surface = cairo_surface_reference (group_surface);

    status = _cairo_gstate_restore (&cr->gstate, &cr->gstate_freelist);
    assert (status == CAIRO_STATUS_SUCCESS);

    parent_surface = _cairo_gstate_get_target (cr->gstate);

    group_pattern = cairo_pattern_create_for_surface (group_surface);
    status = group_pattern->status;
    if (unlikely (status))
        goto done;

    _cairo_gstate_get_matrix (cr->gstate, &group_matrix);
    cairo_pattern_set_matrix (group_pattern, &group_matrix);

    /* If we have a current path, we need to adjust it to compensate for
     * the device offset just removed. */
    _cairo_path_fixed_translate (cr->path,
				 _cairo_fixed_from_int (parent_surface->device_transform.x0 - group_surface->device_transform.x0),
				 _cairo_fixed_from_int (parent_surface->device_transform.y0 - group_surface->device_transform.y0));

done:
    cairo_surface_destroy (group_surface);

    return group_pattern;
}

static cairo_status_t
_cairo_default_context_set_source (void *abstract_cr,
				   cairo_pattern_t *source)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_source (cr->gstate, source);
}

static cairo_bool_t
_current_source_matches_solid (const cairo_pattern_t *pattern,
			       double red,
			       double green,
			       double blue,
			       double alpha)
{
    cairo_color_t color;

    if (pattern->type != CAIRO_PATTERN_TYPE_SOLID)
	return FALSE;

    red   = _cairo_restrict_value (red,   0.0, 1.0);
    green = _cairo_restrict_value (green, 0.0, 1.0);
    blue  = _cairo_restrict_value (blue,  0.0, 1.0);
    alpha = _cairo_restrict_value (alpha, 0.0, 1.0);

    _cairo_color_init_rgba (&color, red, green, blue, alpha);
    return _cairo_color_equal (&color,
			       &((cairo_solid_pattern_t *) pattern)->color);
}

static cairo_status_t
_cairo_default_context_set_source_rgba (void *abstract_cr, double red, double green, double blue, double alpha)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_pattern_t *pattern;
    cairo_status_t status;

    if (_current_source_matches_solid (cr->gstate->source,
				       red, green, blue, alpha))
	return CAIRO_STATUS_SUCCESS;

    /* push the current pattern to the freed lists */
    _cairo_default_context_set_source (cr, (cairo_pattern_t *) &_cairo_pattern_black);

    pattern = cairo_pattern_create_rgba (red, green, blue, alpha);
    if (unlikely (pattern->status))
	return pattern->status;

    status = _cairo_default_context_set_source (cr, pattern);
    cairo_pattern_destroy (pattern);

    return status;
}

static cairo_status_t
_cairo_default_context_set_source_surface (void *abstract_cr,
					   cairo_surface_t *surface,
					   double	   x,
					   double	   y)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_pattern_t *pattern;
    cairo_matrix_t matrix;
    cairo_status_t status;

    /* push the current pattern to the freed lists */
    _cairo_default_context_set_source (cr, (cairo_pattern_t *) &_cairo_pattern_black);

    pattern = cairo_pattern_create_for_surface (surface);
    if (unlikely (pattern->status)) {
        status = pattern->status;
        cairo_pattern_destroy (pattern);
        return status;
    }

    cairo_matrix_init_translate (&matrix, -x, -y);
    cairo_pattern_set_matrix (pattern, &matrix);

    status = _cairo_default_context_set_source (cr, pattern);
    cairo_pattern_destroy (pattern);

    return status;
}

static cairo_pattern_t *
_cairo_default_context_get_source (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_source (cr->gstate);
}

static cairo_status_t
_cairo_default_context_set_tolerance (void *abstract_cr,
				      double tolerance)
{
    cairo_default_context_t *cr = abstract_cr;

    if (tolerance < CAIRO_TOLERANCE_MINIMUM)
	tolerance = CAIRO_TOLERANCE_MINIMUM;

    return _cairo_gstate_set_tolerance (cr->gstate, tolerance);
}

static cairo_status_t
_cairo_default_context_set_operator (void *abstract_cr, cairo_operator_t op)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_operator (cr->gstate, op);
}

static cairo_status_t
_cairo_default_context_set_opacity (void *abstract_cr, double opacity)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_opacity (cr->gstate, opacity);
}

static cairo_status_t
_cairo_default_context_set_antialias (void *abstract_cr,
				      cairo_antialias_t antialias)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_antialias (cr->gstate, antialias);
}

static cairo_status_t
_cairo_default_context_set_fill_rule (void *abstract_cr,
				      cairo_fill_rule_t fill_rule)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_fill_rule (cr->gstate, fill_rule);
}

static cairo_status_t
_cairo_default_context_set_line_width (void *abstract_cr,
				       double line_width)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_line_width (cr->gstate, line_width);
}

static cairo_status_t
_cairo_default_context_set_hairline (void *abstract_cr, cairo_bool_t set_hairline)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_hairline (cr->gstate, set_hairline);
}

static cairo_status_t
_cairo_default_context_set_line_cap (void *abstract_cr,
				     cairo_line_cap_t line_cap)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_line_cap (cr->gstate, line_cap);
}

static cairo_status_t
_cairo_default_context_set_line_join (void *abstract_cr,
				      cairo_line_join_t line_join)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_line_join (cr->gstate, line_join);
}

static cairo_status_t
_cairo_default_context_set_dash (void *abstract_cr,
				 const double *dashes,
				 int	      num_dashes,
				 double	      offset)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_dash (cr->gstate,
				   dashes, num_dashes, offset);
}

static cairo_status_t
_cairo_default_context_set_miter_limit (void *abstract_cr,
					double limit)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_miter_limit (cr->gstate, limit);
}

static cairo_antialias_t
_cairo_default_context_get_antialias (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_antialias (cr->gstate);
}

static void
_cairo_default_context_get_dash (void *abstract_cr,
				 double *dashes,
				 int *num_dashes,
				 double *offset)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_get_dash (cr->gstate, dashes, num_dashes, offset);
}

static cairo_fill_rule_t
_cairo_default_context_get_fill_rule (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_fill_rule (cr->gstate);
}

static double
_cairo_default_context_get_line_width (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_line_width (cr->gstate);
}

static cairo_bool_t
_cairo_default_context_get_hairline (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_hairline (cr->gstate);
}

static cairo_line_cap_t
_cairo_default_context_get_line_cap (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_line_cap (cr->gstate);
}

static cairo_line_join_t
_cairo_default_context_get_line_join (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_line_join (cr->gstate);
}

static double
_cairo_default_context_get_miter_limit (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_miter_limit (cr->gstate);
}

static cairo_operator_t
_cairo_default_context_get_operator (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_operator (cr->gstate);
}

static double
_cairo_default_context_get_opacity (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_opacity (cr->gstate);
}

static double
_cairo_default_context_get_tolerance (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_tolerance (cr->gstate);
}


/* Current transformation matrix */

static cairo_status_t
_cairo_default_context_translate (void *abstract_cr,
				  double tx,
				  double ty)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_translate (cr->gstate, tx, ty);
}

static cairo_status_t
_cairo_default_context_scale (void *abstract_cr,
			      double sx,
			      double sy)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_scale (cr->gstate, sx, sy);
}

static cairo_status_t
_cairo_default_context_rotate (void *abstract_cr,
			       double theta)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_rotate (cr->gstate, theta);
}

static cairo_status_t
_cairo_default_context_transform (void *abstract_cr,
				  const cairo_matrix_t *matrix)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_transform (cr->gstate, matrix);
}

static cairo_status_t
_cairo_default_context_set_matrix (void *abstract_cr,
				   const cairo_matrix_t *matrix)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_matrix (cr->gstate, matrix);
}

static cairo_status_t
_cairo_default_context_set_identity_matrix (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_identity_matrix (cr->gstate);
    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_default_context_get_matrix (void *abstract_cr,
				   cairo_matrix_t *matrix)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_get_matrix (cr->gstate, matrix);
}

static void
_cairo_default_context_user_to_device (void *abstract_cr,
				       double *x,
				       double *y)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_user_to_device (cr->gstate, x, y);
}

static void
_cairo_default_context_user_to_device_distance (void *abstract_cr, double *dx, double *dy)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_user_to_device_distance (cr->gstate, dx, dy);
}

static void
_cairo_default_context_device_to_user (void *abstract_cr,
				       double *x,
				       double *y)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_device_to_user (cr->gstate, x, y);
}

static void
_cairo_default_context_device_to_user_distance (void *abstract_cr,
						double *dx,
						double *dy)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_device_to_user_distance (cr->gstate, dx, dy);
}

static void
_cairo_default_context_backend_to_user (void *abstract_cr,
					double *x,
					double *y)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_backend_to_user (cr->gstate, x, y);
}

static void
_cairo_default_context_backend_to_user_distance (void *abstract_cr, double *dx, double *dy)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_backend_to_user_distance (cr->gstate, dx, dy);
}

static void
_cairo_default_context_user_to_backend (void *abstract_cr,
					double *x,
					double *y)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_user_to_backend (cr->gstate, x, y);
}

static void
_cairo_default_context_user_to_backend_distance (void *abstract_cr,
						 double *dx,
						 double *dy)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_user_to_backend_distance (cr->gstate, dx, dy);
}

/* Path constructor */

static cairo_status_t
_cairo_default_context_new_path (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_path_fixed_fini (cr->path);
    _cairo_path_fixed_init (cr->path);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_default_context_new_sub_path (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_path_fixed_new_sub_path (cr->path);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_default_context_move_to (void *abstract_cr, double x, double y)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_fixed_t x_fixed, y_fixed;
    double width;

    _cairo_gstate_user_to_backend (cr->gstate, &x, &y);
    width = _cairo_gstate_get_line_width (cr->gstate);
    x_fixed = _cairo_fixed_from_double_clamped (x, width);
    y_fixed = _cairo_fixed_from_double_clamped (y, width);

    return _cairo_path_fixed_move_to (cr->path, x_fixed, y_fixed);
}

static cairo_status_t
_cairo_default_context_line_to (void *abstract_cr, double x, double y)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_fixed_t x_fixed, y_fixed;
    double width;

    _cairo_gstate_user_to_backend (cr->gstate, &x, &y);
    width = _cairo_gstate_get_line_width (cr->gstate);
    x_fixed = _cairo_fixed_from_double_clamped (x, width);
    y_fixed = _cairo_fixed_from_double_clamped (y, width);

    return _cairo_path_fixed_line_to (cr->path, x_fixed, y_fixed);
}

static cairo_status_t
_cairo_default_context_curve_to (void *abstract_cr,
				 double x1, double y1,
				 double x2, double y2,
				 double x3, double y3)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_fixed_t x1_fixed, y1_fixed;
    cairo_fixed_t x2_fixed, y2_fixed;
    cairo_fixed_t x3_fixed, y3_fixed;
    double width;

    _cairo_gstate_user_to_backend (cr->gstate, &x1, &y1);
    _cairo_gstate_user_to_backend (cr->gstate, &x2, &y2);
    _cairo_gstate_user_to_backend (cr->gstate, &x3, &y3);
    width = _cairo_gstate_get_line_width (cr->gstate);

    x1_fixed = _cairo_fixed_from_double_clamped (x1, width);
    y1_fixed = _cairo_fixed_from_double_clamped (y1, width);

    x2_fixed = _cairo_fixed_from_double_clamped (x2, width);
    y2_fixed = _cairo_fixed_from_double_clamped (y2, width);

    x3_fixed = _cairo_fixed_from_double_clamped (x3, width);
    y3_fixed = _cairo_fixed_from_double_clamped (y3, width);

    return _cairo_path_fixed_curve_to (cr->path,
				       x1_fixed, y1_fixed,
				       x2_fixed, y2_fixed,
				       x3_fixed, y3_fixed);
}

static cairo_status_t
_cairo_default_context_arc (void *abstract_cr,
			    double xc, double yc, double radius,
			    double angle1, double angle2,
			    cairo_bool_t forward)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_status_t status;

    /* Do nothing, successfully, if radius is <= 0 */
    if (radius <= 0.0) {
	cairo_fixed_t x_fixed, y_fixed;

	_cairo_gstate_user_to_backend (cr->gstate, &xc, &yc);
	x_fixed = _cairo_fixed_from_double (xc);
	y_fixed = _cairo_fixed_from_double (yc);
	status = _cairo_path_fixed_line_to (cr->path, x_fixed, y_fixed);
	if (unlikely (status))
	    return status;

	status = _cairo_path_fixed_line_to (cr->path, x_fixed, y_fixed);
	if (unlikely (status))
	    return status;

	return CAIRO_STATUS_SUCCESS;
    }

    status = _cairo_default_context_line_to (cr,
					     xc + radius * cos (angle1),
					     yc + radius * sin (angle1));

    if (unlikely (status))
	return status;

    if (forward)
	_cairo_arc_path (&cr->base, xc, yc, radius, angle1, angle2);
    else
	_cairo_arc_path_negative (&cr->base, xc, yc, radius, angle1, angle2);

    return CAIRO_STATUS_SUCCESS; /* any error will have already been set on cr */
}

static cairo_status_t
_cairo_default_context_rel_move_to (void *abstract_cr, double dx, double dy)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_fixed_t dx_fixed, dy_fixed;

    _cairo_gstate_user_to_backend_distance (cr->gstate, &dx, &dy);

    dx_fixed = _cairo_fixed_from_double (dx);
    dy_fixed = _cairo_fixed_from_double (dy);

    return _cairo_path_fixed_rel_move_to (cr->path, dx_fixed, dy_fixed);
}

static cairo_status_t
_cairo_default_context_rel_line_to (void *abstract_cr, double dx, double dy)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_fixed_t dx_fixed, dy_fixed;

    _cairo_gstate_user_to_backend_distance (cr->gstate, &dx, &dy);

    dx_fixed = _cairo_fixed_from_double (dx);
    dy_fixed = _cairo_fixed_from_double (dy);

    return _cairo_path_fixed_rel_line_to (cr->path, dx_fixed, dy_fixed);
}


static cairo_status_t
_cairo_default_context_rel_curve_to (void *abstract_cr,
				     double dx1, double dy1,
				     double dx2, double dy2,
				     double dx3, double dy3)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_fixed_t dx1_fixed, dy1_fixed;
    cairo_fixed_t dx2_fixed, dy2_fixed;
    cairo_fixed_t dx3_fixed, dy3_fixed;

    _cairo_gstate_user_to_backend_distance (cr->gstate, &dx1, &dy1);
    _cairo_gstate_user_to_backend_distance (cr->gstate, &dx2, &dy2);
    _cairo_gstate_user_to_backend_distance (cr->gstate, &dx3, &dy3);

    dx1_fixed = _cairo_fixed_from_double (dx1);
    dy1_fixed = _cairo_fixed_from_double (dy1);

    dx2_fixed = _cairo_fixed_from_double (dx2);
    dy2_fixed = _cairo_fixed_from_double (dy2);

    dx3_fixed = _cairo_fixed_from_double (dx3);
    dy3_fixed = _cairo_fixed_from_double (dy3);

    return _cairo_path_fixed_rel_curve_to (cr->path,
					   dx1_fixed, dy1_fixed,
					   dx2_fixed, dy2_fixed,
					   dx3_fixed, dy3_fixed);
}

static cairo_status_t
_cairo_default_context_close_path (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_path_fixed_close_path (cr->path);
}

static cairo_status_t
_cairo_default_context_rectangle (void *abstract_cr,
				  double x, double y,
				  double width, double height)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_status_t status;

    status = _cairo_default_context_move_to (cr, x, y);
    if (unlikely (status))
	return status;

    status = _cairo_default_context_rel_line_to (cr, width, 0);
    if (unlikely (status))
	return status;

    status = _cairo_default_context_rel_line_to (cr, 0, height);
    if (unlikely (status))
	return status;

    status = _cairo_default_context_rel_line_to (cr, -width, 0);
    if (unlikely (status))
	return status;

    return _cairo_default_context_close_path (cr);
}

static void
_cairo_default_context_path_extents (void *abstract_cr,
				     double *x1,
				     double *y1,
				     double *x2,
				     double *y2)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_path_extents (cr->gstate,
				cr->path,
				x1, y1, x2, y2);
}

static cairo_bool_t
_cairo_default_context_has_current_point (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return cr->path->has_current_point;
}

static cairo_bool_t
_cairo_default_context_get_current_point (void *abstract_cr,
					  double *x,
					  double *y)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_fixed_t x_fixed, y_fixed;

    if (_cairo_path_fixed_get_current_point (cr->path, &x_fixed, &y_fixed))
    {
	*x = _cairo_fixed_to_double (x_fixed);
	*y = _cairo_fixed_to_double (y_fixed);
	_cairo_gstate_backend_to_user (cr->gstate, x, y);

	return TRUE;
    }
    else
    {
	return FALSE;
    }
}

static cairo_path_t *
_cairo_default_context_copy_path (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_path_create (cr->path, &cr->base);
}

static cairo_path_t *
_cairo_default_context_copy_path_flat (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_path_create_flat (cr->path, &cr->base);
}

static cairo_status_t
_cairo_default_context_append_path (void *abstract_cr,
				    const cairo_path_t *path)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_path_append_to_context (path, &cr->base);
}

static cairo_status_t
_cairo_default_context_paint (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_paint (cr->gstate);
}

static cairo_status_t
_cairo_default_context_paint_with_alpha (void *abstract_cr,
					 double alpha)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_solid_pattern_t pattern;
    cairo_status_t status;
    cairo_color_t color;

    if (CAIRO_ALPHA_IS_OPAQUE (alpha))
	return _cairo_gstate_paint (cr->gstate);

    if (CAIRO_ALPHA_IS_ZERO (alpha) &&
        _cairo_operator_bounded_by_mask (cr->gstate->op)) {
	return CAIRO_STATUS_SUCCESS;
    }

    _cairo_color_init_rgba (&color, 0., 0., 0., alpha);
    _cairo_pattern_init_solid (&pattern, &color);

    status = _cairo_gstate_mask (cr->gstate, &pattern.base);
    _cairo_pattern_fini (&pattern.base);

    return status;
}

static cairo_status_t
_cairo_default_context_mask (void *abstract_cr,
			     cairo_pattern_t *mask)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_mask (cr->gstate, mask);
}

static cairo_status_t
_cairo_default_context_stroke_preserve (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_stroke (cr->gstate, cr->path);
}

static cairo_status_t
_cairo_default_context_stroke (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_status_t status;

    status = _cairo_gstate_stroke (cr->gstate, cr->path);
    if (unlikely (status))
	return status;

    return _cairo_default_context_new_path (cr);
}

static cairo_status_t
_cairo_default_context_in_stroke (void *abstract_cr,
				  double x, double y,
				  cairo_bool_t *inside)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_in_stroke (cr->gstate,
				    cr->path,
				    x, y,
				    inside);
}

static cairo_status_t
_cairo_default_context_stroke_extents (void *abstract_cr,
				       double *x1, double *y1, double *x2, double *y2)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_stroke_extents (cr->gstate,
					 cr->path,
					 x1, y1, x2, y2);
}

static cairo_status_t
_cairo_default_context_fill_preserve (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_fill (cr->gstate, cr->path);
}

static cairo_status_t
_cairo_default_context_fill (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_status_t status;

    status = _cairo_gstate_fill (cr->gstate, cr->path);
    if (unlikely (status))
	return status;

    return _cairo_default_context_new_path (cr);
}

static cairo_status_t
_cairo_default_context_in_fill (void *abstract_cr,
				double x, double y,
				cairo_bool_t *inside)
{
    cairo_default_context_t *cr = abstract_cr;

    *inside = _cairo_gstate_in_fill (cr->gstate,
				     cr->path,
				     x, y);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_default_context_fill_extents (void *abstract_cr,
				     double *x1, double *y1, double *x2, double *y2)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_fill_extents (cr->gstate,
				       cr->path,
				       x1, y1, x2, y2);
}

static cairo_status_t
_cairo_default_context_clip_preserve (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_clip (cr->gstate, cr->path);
}

static cairo_status_t
_cairo_default_context_clip (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_status_t status;

    status = _cairo_gstate_clip (cr->gstate, cr->path);
    if (unlikely (status))
	return status;

    return _cairo_default_context_new_path (cr);
}

static cairo_status_t
_cairo_default_context_in_clip (void *abstract_cr,
				double x, double y,
				cairo_bool_t *inside)
{
    cairo_default_context_t *cr = abstract_cr;

    *inside = _cairo_gstate_in_clip (cr->gstate, x, y);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_default_context_reset_clip (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_reset_clip (cr->gstate);
}

static cairo_status_t
_cairo_default_context_clip_extents (void *abstract_cr,
				     double *x1, double *y1, double *x2, double *y2)
{
    cairo_default_context_t *cr = abstract_cr;

    if (! _cairo_gstate_clip_extents (cr->gstate, x1, y1, x2, y2)) {
	*x1 = -INFINITY;
	*y1 = -INFINITY;
	*x2 = +INFINITY;
	*y2 = +INFINITY;
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_rectangle_list_t *
_cairo_default_context_copy_clip_rectangle_list (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_copy_clip_rectangle_list (cr->gstate);
}

static cairo_status_t
_cairo_default_context_copy_page (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_copy_page (cr->gstate);
}

static cairo_status_t
_cairo_default_context_tag_begin (void *abstract_cr,
				  const char *tag_name, const char *attributes)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_tag_begin (cr->gstate, tag_name, attributes);
}

static cairo_status_t
_cairo_default_context_tag_end (void *abstract_cr,
				const char *tag_name)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_tag_end (cr->gstate, tag_name);
}

static cairo_status_t
_cairo_default_context_show_page (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_show_page (cr->gstate);
}

static cairo_status_t
_cairo_default_context_set_font_face (void *abstract_cr,
				      cairo_font_face_t *font_face)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_font_face (cr->gstate, font_face);
}

static cairo_font_face_t *
_cairo_default_context_get_font_face (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_font_face_t *font_face;
    cairo_status_t status;

    status = _cairo_gstate_get_font_face (cr->gstate, &font_face);
    if (unlikely (status)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_font_face_t *) &_cairo_font_face_nil;
    }

    return font_face;
}

static cairo_status_t
_cairo_default_context_font_extents (void *abstract_cr,
				     cairo_font_extents_t *extents)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_get_font_extents (cr->gstate, extents);
}

static cairo_status_t
_cairo_default_context_set_font_size (void *abstract_cr,
				      double size)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_font_size (cr->gstate, size);
}

static cairo_status_t
_cairo_default_context_set_font_matrix (void *abstract_cr,
					const cairo_matrix_t *matrix)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_set_font_matrix (cr->gstate, matrix);
}

static void
_cairo_default_context_get_font_matrix (void *abstract_cr,
					cairo_matrix_t *matrix)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_get_font_matrix (cr->gstate, matrix);
}

static cairo_status_t
_cairo_default_context_set_font_options (void *abstract_cr,
					 const cairo_font_options_t *options)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_set_font_options (cr->gstate, options);
    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_default_context_get_font_options (void *abstract_cr,
					 cairo_font_options_t *options)
{
    cairo_default_context_t *cr = abstract_cr;

    _cairo_gstate_get_font_options (cr->gstate, options);
}

static cairo_status_t
_cairo_default_context_set_scaled_font (void *abstract_cr,
					cairo_scaled_font_t *scaled_font)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_bool_t was_previous;
    cairo_status_t status;

    if (scaled_font == cr->gstate->scaled_font)
	return CAIRO_STATUS_SUCCESS;

    was_previous = scaled_font == cr->gstate->previous_scaled_font;

    status = _cairo_gstate_set_font_face (cr->gstate, scaled_font->font_face);
    if (unlikely (status))
	return status;

    status = _cairo_gstate_set_font_matrix (cr->gstate, &scaled_font->font_matrix);
    if (unlikely (status))
	return status;

    _cairo_gstate_set_font_options (cr->gstate, &scaled_font->options);

    if (was_previous)
	cr->gstate->scaled_font = cairo_scaled_font_reference (scaled_font);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_scaled_font_t *
_cairo_default_context_get_scaled_font (void *abstract_cr)
{
    cairo_default_context_t *cr = abstract_cr;
    cairo_scaled_font_t *scaled_font;
    cairo_status_t status;

    status = _cairo_gstate_get_scaled_font (cr->gstate, &scaled_font);
    if (unlikely (status))
	return _cairo_scaled_font_create_in_error (status);

    return scaled_font;
}

static cairo_status_t
_cairo_default_context_glyphs (void *abstract_cr,
			       const cairo_glyph_t *glyphs,
			       int num_glyphs,
			       cairo_glyph_text_info_t *info)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_show_text_glyphs (cr->gstate, glyphs, num_glyphs, info);
}

static cairo_status_t
_cairo_default_context_glyph_path (void *abstract_cr,
				   const cairo_glyph_t *glyphs,
				   int num_glyphs)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_glyph_path (cr->gstate,
				     glyphs, num_glyphs,
				     cr->path);
}

static cairo_status_t
_cairo_default_context_glyph_extents (void                *abstract_cr,
				      const cairo_glyph_t    *glyphs,
				      int                    num_glyphs,
				      cairo_text_extents_t   *extents)
{
    cairo_default_context_t *cr = abstract_cr;

    return _cairo_gstate_glyph_extents (cr->gstate, glyphs, num_glyphs, extents);
}

static const cairo_backend_t _cairo_default_context_backend = {
    CAIRO_TYPE_DEFAULT,
    _cairo_default_context_destroy,

    _cairo_default_context_get_original_target,
    _cairo_default_context_get_current_target,

    _cairo_default_context_save,
    _cairo_default_context_restore,

    _cairo_default_context_push_group,
    _cairo_default_context_pop_group,

    _cairo_default_context_set_source_rgba,
    _cairo_default_context_set_source_surface,
    _cairo_default_context_set_source,
    _cairo_default_context_get_source,

    _cairo_default_context_set_antialias,
    _cairo_default_context_set_dash,
    _cairo_default_context_set_fill_rule,
    _cairo_default_context_set_line_cap,
    _cairo_default_context_set_line_join,
    _cairo_default_context_set_line_width,
    _cairo_default_context_set_hairline,
    _cairo_default_context_set_miter_limit,
    _cairo_default_context_set_opacity,
    _cairo_default_context_set_operator,
    _cairo_default_context_set_tolerance,
    _cairo_default_context_get_antialias,
    _cairo_default_context_get_dash,
    _cairo_default_context_get_fill_rule,
    _cairo_default_context_get_line_cap,
    _cairo_default_context_get_line_join,
    _cairo_default_context_get_line_width,
    _cairo_default_context_get_hairline,
    _cairo_default_context_get_miter_limit,
    _cairo_default_context_get_opacity,
    _cairo_default_context_get_operator,
    _cairo_default_context_get_tolerance,

    _cairo_default_context_translate,
    _cairo_default_context_scale,
    _cairo_default_context_rotate,
    _cairo_default_context_transform,
    _cairo_default_context_set_matrix,
    _cairo_default_context_set_identity_matrix,
    _cairo_default_context_get_matrix,

    _cairo_default_context_user_to_device,
    _cairo_default_context_user_to_device_distance,
    _cairo_default_context_device_to_user,
    _cairo_default_context_device_to_user_distance,

    _cairo_default_context_user_to_backend,
    _cairo_default_context_user_to_backend_distance,
    _cairo_default_context_backend_to_user,
    _cairo_default_context_backend_to_user_distance,

    _cairo_default_context_new_path,
    _cairo_default_context_new_sub_path,
    _cairo_default_context_move_to,
    _cairo_default_context_rel_move_to,
    _cairo_default_context_line_to,
    _cairo_default_context_rel_line_to,
    _cairo_default_context_curve_to,
    _cairo_default_context_rel_curve_to,
    NULL, /* arc-to */
    NULL, /* rel-arc-to */
    _cairo_default_context_close_path,
    _cairo_default_context_arc,
    _cairo_default_context_rectangle,
    _cairo_default_context_path_extents,
    _cairo_default_context_has_current_point,
    _cairo_default_context_get_current_point,
    _cairo_default_context_copy_path,
    _cairo_default_context_copy_path_flat,
    _cairo_default_context_append_path,

    NULL, /* stroke-to-path */

    _cairo_default_context_clip,
    _cairo_default_context_clip_preserve,
    _cairo_default_context_in_clip,
    _cairo_default_context_clip_extents,
    _cairo_default_context_reset_clip,
    _cairo_default_context_copy_clip_rectangle_list,

    _cairo_default_context_paint,
    _cairo_default_context_paint_with_alpha,
    _cairo_default_context_mask,

    _cairo_default_context_stroke,
    _cairo_default_context_stroke_preserve,
    _cairo_default_context_in_stroke,
    _cairo_default_context_stroke_extents,

    _cairo_default_context_fill,
    _cairo_default_context_fill_preserve,
    _cairo_default_context_in_fill,
    _cairo_default_context_fill_extents,

    _cairo_default_context_set_font_face,
    _cairo_default_context_get_font_face,
    _cairo_default_context_set_font_size,
    _cairo_default_context_set_font_matrix,
    _cairo_default_context_get_font_matrix,
    _cairo_default_context_set_font_options,
    _cairo_default_context_get_font_options,
    _cairo_default_context_set_scaled_font,
    _cairo_default_context_get_scaled_font,
    _cairo_default_context_font_extents,

    _cairo_default_context_glyphs,
    _cairo_default_context_glyph_path,
    _cairo_default_context_glyph_extents,

    _cairo_default_context_copy_page,
    _cairo_default_context_show_page,

    _cairo_default_context_tag_begin,
    _cairo_default_context_tag_end,
};

cairo_status_t
_cairo_default_context_init (cairo_default_context_t *cr, void *target)
{
    _cairo_init (&cr->base, &_cairo_default_context_backend);
    _cairo_path_fixed_init (cr->path);

    cr->gstate = &cr->gstate_tail[0];
    cr->gstate_freelist = &cr->gstate_tail[1];
    cr->gstate_tail[1].next = NULL;

    return _cairo_gstate_init (cr->gstate, target);
}

cairo_t *
_cairo_default_context_create (void *target)
{
    cairo_default_context_t *cr;
    cairo_status_t status;

    cr = _freed_pool_get (&context_pool);
    if (unlikely (cr == NULL)) {
	cr = _cairo_malloc (sizeof (cairo_default_context_t));
	if (unlikely (cr == NULL))
	    return _cairo_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));
    }

    status = _cairo_default_context_init (cr, target);
    if (unlikely (status)) {
	_freed_pool_put (&context_pool, cr);
	return _cairo_create_in_error (status);
    }

    return &cr->base;
}
