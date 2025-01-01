/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2005 Red Hat, Inc.
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
 */

#include "cairoint.h"

#include "cairo-clip-inline.h"
#include "cairo-clip-private.h"
#include "cairo-error-private.h"
#include "cairo-list-inline.h"
#include "cairo-gstate-private.h"
#include "cairo-pattern-private.h"
#include "cairo-traps-private.h"

static cairo_status_t
_cairo_gstate_init_copy (cairo_gstate_t *gstate, cairo_gstate_t *other);

static cairo_status_t
_cairo_gstate_ensure_font_face (cairo_gstate_t *gstate);

static cairo_status_t
_cairo_gstate_ensure_scaled_font (cairo_gstate_t *gstate);

static void
_cairo_gstate_unset_scaled_font (cairo_gstate_t *gstate);

static void
_cairo_gstate_transform_glyphs_to_backend (cairo_gstate_t      *gstate,
                                           const cairo_glyph_t *glyphs,
                                           int                  num_glyphs,
					   const cairo_text_cluster_t	*clusters,
					   int			 num_clusters,
					   cairo_text_cluster_flags_t cluster_flags,
                                           cairo_glyph_t       *transformed_glyphs,
					   int			*num_transformed_glyphs,
					   cairo_text_cluster_t *transformed_clusters);

static void
_cairo_gstate_update_device_transform (cairo_observer_t *observer,
				       void *arg)
{
    cairo_gstate_t *gstate = cairo_container_of (observer,
						 cairo_gstate_t,
						 device_transform_observer);

    gstate->is_identity = (_cairo_matrix_is_identity (&gstate->ctm) &&
			   _cairo_matrix_is_identity (&gstate->target->device_transform));
}

cairo_status_t
_cairo_gstate_init (cairo_gstate_t  *gstate,
		    cairo_surface_t *target)
{
    VG (VALGRIND_MAKE_MEM_UNDEFINED (gstate, sizeof (cairo_gstate_t)));

    gstate->next = NULL;

    gstate->op = CAIRO_GSTATE_OPERATOR_DEFAULT;
    gstate->opacity = 1.;

    gstate->tolerance = CAIRO_GSTATE_TOLERANCE_DEFAULT;
    gstate->antialias = CAIRO_ANTIALIAS_DEFAULT;

    _cairo_stroke_style_init (&gstate->stroke_style);

    gstate->fill_rule = CAIRO_GSTATE_FILL_RULE_DEFAULT;

    gstate->font_face = NULL;
    gstate->scaled_font = NULL;
    gstate->previous_scaled_font = NULL;

    cairo_matrix_init_scale (&gstate->font_matrix,
			     CAIRO_GSTATE_DEFAULT_FONT_SIZE,
			     CAIRO_GSTATE_DEFAULT_FONT_SIZE);

    _cairo_font_options_init_default (&gstate->font_options);

    gstate->clip = NULL;

    gstate->target = cairo_surface_reference (target);
    gstate->parent_target = NULL;
    gstate->original_target = cairo_surface_reference (target);

    gstate->device_transform_observer.callback = _cairo_gstate_update_device_transform;
    cairo_list_add (&gstate->device_transform_observer.link,
		    &gstate->target->device_transform_observers);

    gstate->is_identity = _cairo_matrix_is_identity (&gstate->target->device_transform);
    cairo_matrix_init_identity (&gstate->ctm);
    gstate->ctm_inverse = gstate->ctm;
    gstate->source_ctm_inverse = gstate->ctm;

    gstate->source = (cairo_pattern_t *) &_cairo_pattern_black.base;

    /* Now that the gstate is fully initialized and ready for the eventual
     * _cairo_gstate_fini(), we can check for errors (and not worry about
     * the resource deallocation). */
    return target->status;
}

/**
 * _cairo_gstate_init_copy:
 *
 * Initialize @gstate by performing a deep copy of state fields from
 * @other. Note that gstate->next is not copied but is set to %NULL by
 * this function.
 **/
static cairo_status_t
_cairo_gstate_init_copy (cairo_gstate_t *gstate, cairo_gstate_t *other)
{
    cairo_status_t status;

    VG (VALGRIND_MAKE_MEM_UNDEFINED (gstate, sizeof (cairo_gstate_t)));

    gstate->op = other->op;
    gstate->opacity = other->opacity;

    gstate->tolerance = other->tolerance;
    gstate->antialias = other->antialias;

    status = _cairo_stroke_style_init_copy (&gstate->stroke_style,
					    &other->stroke_style);
    if (unlikely (status))
	return status;

    gstate->fill_rule = other->fill_rule;

    gstate->font_face = cairo_font_face_reference (other->font_face);
    gstate->scaled_font = cairo_scaled_font_reference (other->scaled_font);
    gstate->previous_scaled_font = cairo_scaled_font_reference (other->previous_scaled_font);

    gstate->font_matrix = other->font_matrix;

    _cairo_font_options_init_copy (&gstate->font_options , &other->font_options);

    gstate->clip = _cairo_clip_copy (other->clip);

    gstate->target = cairo_surface_reference (other->target);
    /* parent_target is always set to NULL; it's only ever set by redirect_target */
    gstate->parent_target = NULL;
    gstate->original_target = cairo_surface_reference (other->original_target);

    gstate->device_transform_observer.callback = _cairo_gstate_update_device_transform;
    cairo_list_add (&gstate->device_transform_observer.link,
		    &gstate->target->device_transform_observers);

    gstate->is_identity = other->is_identity;
    gstate->ctm = other->ctm;
    gstate->ctm_inverse = other->ctm_inverse;
    gstate->source_ctm_inverse = other->source_ctm_inverse;

    gstate->source = cairo_pattern_reference (other->source);

    gstate->next = NULL;

    return CAIRO_STATUS_SUCCESS;
}

void
_cairo_gstate_fini (cairo_gstate_t *gstate)
{
    _cairo_stroke_style_fini (&gstate->stroke_style);

    cairo_font_face_destroy (gstate->font_face);
    gstate->font_face = NULL;

    cairo_scaled_font_destroy (gstate->previous_scaled_font);
    gstate->previous_scaled_font = NULL;

    cairo_scaled_font_destroy (gstate->scaled_font);
    gstate->scaled_font = NULL;

    _cairo_clip_destroy (gstate->clip);

    cairo_list_del (&gstate->device_transform_observer.link);

    cairo_surface_destroy (gstate->target);
    gstate->target = NULL;

    cairo_surface_destroy (gstate->parent_target);
    gstate->parent_target = NULL;

    cairo_surface_destroy (gstate->original_target);
    gstate->original_target = NULL;

    cairo_pattern_destroy (gstate->source);
    gstate->source = NULL;

    VG (VALGRIND_MAKE_MEM_UNDEFINED (gstate, sizeof (cairo_gstate_t)));
}

/**
 * _cairo_gstate_save:
 * @gstate: input/output gstate pointer
 *
 * Makes a copy of the current state of @gstate and saves it
 * to @gstate->next, then put the address of the newly allcated
 * copy into @gstate.  _cairo_gstate_restore() reverses this.
 **/
cairo_status_t
_cairo_gstate_save (cairo_gstate_t **gstate, cairo_gstate_t **freelist)
{
    cairo_gstate_t *top;
    cairo_status_t status;

    if (CAIRO_INJECT_FAULT ())
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    top = *freelist;
    if (top == NULL) {
	top = _cairo_malloc (sizeof (cairo_gstate_t));
	if (unlikely (top == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    } else
	*freelist = top->next;

    status = _cairo_gstate_init_copy (top, *gstate);
    if (unlikely (status)) {
	top->next = *freelist;
	*freelist = top;
	return status;
    }

    top->next = *gstate;
    *gstate = top;

    return CAIRO_STATUS_SUCCESS;
}

/**
 * _cairo_gstate_restore:
 * @gstate: input/output gstate pointer
 *
 * Reverses the effects of one _cairo_gstate_save() call.
 **/
cairo_status_t
_cairo_gstate_restore (cairo_gstate_t **gstate, cairo_gstate_t **freelist)
{
    cairo_gstate_t *top;

    top = *gstate;
    if (top->next == NULL)
	return _cairo_error (CAIRO_STATUS_INVALID_RESTORE);

    *gstate = top->next;

    _cairo_gstate_fini (top);
    VG (VALGRIND_MAKE_MEM_UNDEFINED (&top->next, sizeof (cairo_gstate_t *)));
    top->next = *freelist;
    *freelist = top;

    return CAIRO_STATUS_SUCCESS;
}

/**
 * _cairo_gstate_redirect_target:
 * @gstate: a #cairo_gstate_t
 * @child: the new child target
 *
 * Redirect @gstate rendering to a "child" target. The original
 * "parent" target with which the gstate was created will not be
 * affected. See _cairo_gstate_get_target().
 **/
cairo_status_t
_cairo_gstate_redirect_target (cairo_gstate_t *gstate, cairo_surface_t *child)
{
    /* If this gstate is already redirected, this is an error; we need a
     * new gstate to be able to redirect */
    assert (gstate->parent_target == NULL);

    /* Set up our new parent_target based on our current target;
     * gstate->parent_target will take the ref that is held by gstate->target
     */
    gstate->parent_target = gstate->target;

    /* Now set up our new target; we overwrite gstate->target directly,
     * since its ref is now owned by gstate->parent_target */
    gstate->target = cairo_surface_reference (child);
    gstate->is_identity &= _cairo_matrix_is_identity (&child->device_transform);
    cairo_list_move (&gstate->device_transform_observer.link,
		     &gstate->target->device_transform_observers);

    /* The clip is in surface backend coordinates for the previous target;
     * translate it into the child's backend coordinates. */
    _cairo_clip_destroy (gstate->clip);
    gstate->clip = _cairo_clip_copy_with_translation (gstate->next->clip,
						      child->device_transform.x0 - gstate->parent_target->device_transform.x0,
						      child->device_transform.y0 - gstate->parent_target->device_transform.y0);

    return CAIRO_STATUS_SUCCESS;
}

/**
 * _cairo_gstate_is_group:
 * @gstate: a #cairo_gstate_t
 *
 * Check if _cairo_gstate_redirect_target has been called on the head
 * of the stack.
 *
 * Return value: %TRUE if @gstate is redirected to a target different
 * than the previous state in the stack, %FALSE otherwise.
 **/
cairo_bool_t
_cairo_gstate_is_group (cairo_gstate_t *gstate)
{
    return gstate->parent_target != NULL;
}

/**
 * _cairo_gstate_get_target:
 * @gstate: a #cairo_gstate_t
 *
 * Return the current drawing target; if drawing is not redirected,
 * this will be the same as _cairo_gstate_get_original_target().
 *
 * Return value: the current target surface
 **/
cairo_surface_t *
_cairo_gstate_get_target (cairo_gstate_t *gstate)
{
    return gstate->target;
}

/**
 * _cairo_gstate_get_original_target:
 * @gstate: a #cairo_gstate_t
 *
 * Return the original target with which @gstate was created. This
 * function always returns the original target independent of any
 * child target that may have been set with
 * _cairo_gstate_redirect_target.
 *
 * Return value: the original target surface
 **/
cairo_surface_t *
_cairo_gstate_get_original_target (cairo_gstate_t *gstate)
{
    return gstate->original_target;
}

/**
 * _cairo_gstate_get_clip:
 * @gstate: a #cairo_gstate_t
 *
 * This space left intentionally blank.
 *
 * Return value: a pointer to the gstate's #cairo_clip_t structure.
 **/
cairo_clip_t *
_cairo_gstate_get_clip (cairo_gstate_t *gstate)
{
    return gstate->clip;
}

cairo_status_t
_cairo_gstate_set_source (cairo_gstate_t  *gstate,
			  cairo_pattern_t *source)
{
    if (source->status)
	return source->status;

    source = cairo_pattern_reference (source);
    cairo_pattern_destroy (gstate->source);
    gstate->source = source;
    gstate->source_ctm_inverse = gstate->ctm_inverse;

    return CAIRO_STATUS_SUCCESS;
}

cairo_pattern_t *
_cairo_gstate_get_source (cairo_gstate_t *gstate)
{
    if (gstate->source == &_cairo_pattern_black.base) {
	/* do not expose the static object to the user */
        gstate->source = _cairo_pattern_create_solid (CAIRO_COLOR_BLACK);
    }

    return gstate->source;
}

cairo_status_t
_cairo_gstate_set_operator (cairo_gstate_t *gstate, cairo_operator_t op)
{
    gstate->op = op;

    return CAIRO_STATUS_SUCCESS;
}

cairo_operator_t
_cairo_gstate_get_operator (cairo_gstate_t *gstate)
{
    return gstate->op;
}

cairo_status_t
_cairo_gstate_set_opacity (cairo_gstate_t *gstate, double op)
{
    gstate->opacity = op;

    return CAIRO_STATUS_SUCCESS;
}

double
_cairo_gstate_get_opacity (cairo_gstate_t *gstate)
{
    return gstate->opacity;
}

cairo_status_t
_cairo_gstate_set_tolerance (cairo_gstate_t *gstate, double tolerance)
{
    gstate->tolerance = tolerance;

    return CAIRO_STATUS_SUCCESS;
}

double
_cairo_gstate_get_tolerance (cairo_gstate_t *gstate)
{
    return gstate->tolerance;
}

cairo_status_t
_cairo_gstate_set_fill_rule (cairo_gstate_t *gstate, cairo_fill_rule_t fill_rule)
{
    gstate->fill_rule = fill_rule;

    return CAIRO_STATUS_SUCCESS;
}

cairo_fill_rule_t
_cairo_gstate_get_fill_rule (cairo_gstate_t *gstate)
{
    return gstate->fill_rule;
}

cairo_status_t
_cairo_gstate_set_line_width (cairo_gstate_t *gstate, double width)
{
    if (gstate->stroke_style.is_hairline)
	gstate->stroke_style.pre_hairline_line_width = width;
	else
	gstate->stroke_style.line_width = width;

    return CAIRO_STATUS_SUCCESS;
}

double
_cairo_gstate_get_line_width (cairo_gstate_t *gstate)
{
    return gstate->stroke_style.line_width;
}

cairo_status_t
_cairo_gstate_set_hairline (cairo_gstate_t *gstate, cairo_bool_t set_hairline)
{
    if (gstate->stroke_style.is_hairline != set_hairline) {
        gstate->stroke_style.is_hairline = set_hairline;

        if (set_hairline) {
            gstate->stroke_style.pre_hairline_line_width = gstate->stroke_style.line_width;
            gstate->stroke_style.line_width = 0.0;
        } else {
            gstate->stroke_style.line_width = gstate->stroke_style.pre_hairline_line_width;
        }
    }

    return CAIRO_STATUS_SUCCESS;
}

cairo_bool_t
_cairo_gstate_get_hairline (cairo_gstate_t *gstate)
{
    return gstate->stroke_style.is_hairline;
}

cairo_status_t
_cairo_gstate_set_line_cap (cairo_gstate_t *gstate, cairo_line_cap_t line_cap)
{
    gstate->stroke_style.line_cap = line_cap;

    return CAIRO_STATUS_SUCCESS;
}

cairo_line_cap_t
_cairo_gstate_get_line_cap (cairo_gstate_t *gstate)
{
    return gstate->stroke_style.line_cap;
}

cairo_status_t
_cairo_gstate_set_line_join (cairo_gstate_t *gstate, cairo_line_join_t line_join)
{
    gstate->stroke_style.line_join = line_join;

    return CAIRO_STATUS_SUCCESS;
}

cairo_line_join_t
_cairo_gstate_get_line_join (cairo_gstate_t *gstate)
{
    return gstate->stroke_style.line_join;
}

cairo_status_t
_cairo_gstate_set_dash (cairo_gstate_t *gstate, const double *dash, int num_dashes, double offset)
{
    double dash_total, on_total, off_total;
    int i, j;

    free (gstate->stroke_style.dash);

    gstate->stroke_style.num_dashes = num_dashes;

    if (gstate->stroke_style.num_dashes == 0) {
	gstate->stroke_style.dash = NULL;
	gstate->stroke_style.dash_offset = 0.0;
	return CAIRO_STATUS_SUCCESS;
    }

    gstate->stroke_style.dash = _cairo_malloc_ab (gstate->stroke_style.num_dashes, sizeof (double));
    if (unlikely (gstate->stroke_style.dash == NULL)) {
	gstate->stroke_style.num_dashes = 0;
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    on_total = off_total = dash_total = 0.0;
    for (i = j = 0; i < num_dashes; i++) {
	if (dash[i] < 0)
	    return _cairo_error (CAIRO_STATUS_INVALID_DASH);

	if (dash[i] == 0 && i > 0 && i < num_dashes - 1) {
	    if (dash[++i] < 0)
		return _cairo_error (CAIRO_STATUS_INVALID_DASH);

	    gstate->stroke_style.dash[j-1] += dash[i];
	    gstate->stroke_style.num_dashes -= 2;
	} else
	    gstate->stroke_style.dash[j++] = dash[i];

	if (dash[i]) {
	    dash_total += dash[i];
	    if ((i & 1) == 0)
		on_total += dash[i];
	    else
		off_total += dash[i];
	}
    }

    if (dash_total == 0.0)
	return _cairo_error (CAIRO_STATUS_INVALID_DASH);

    /* An odd dash value indicate symmetric repeating, so the total
     * is twice as long. */
    if (gstate->stroke_style.num_dashes & 1) {
	dash_total *= 2;
	on_total += off_total;
    }

    if (dash_total - on_total < CAIRO_FIXED_ERROR_DOUBLE) {
	/* Degenerate dash -> solid line */
	free (gstate->stroke_style.dash);
	gstate->stroke_style.dash = NULL;
	gstate->stroke_style.num_dashes = 0;
	gstate->stroke_style.dash_offset = 0.0;
	return CAIRO_STATUS_SUCCESS;
    }

    /* The dashing code doesn't like a negative offset or a big positive
     * offset, so we compute an equivalent offset which is guaranteed to be
     * positive and less than twice the pattern length. */
    offset = fmod (offset, dash_total);
    if (offset < 0.0)
	offset += dash_total;
    if (offset <= 0.0)		/* Take care of -0 */
	offset = 0.0;
    gstate->stroke_style.dash_offset = offset;

    return CAIRO_STATUS_SUCCESS;
}

void
_cairo_gstate_get_dash (cairo_gstate_t *gstate,
			double         *dashes,
			int            *num_dashes,
			double         *offset)
{
    if (dashes) {
	memcpy (dashes,
		gstate->stroke_style.dash,
		sizeof (double) * gstate->stroke_style.num_dashes);
    }

    if (num_dashes)
	*num_dashes = gstate->stroke_style.num_dashes;

    if (offset)
	*offset = gstate->stroke_style.dash_offset;
}

cairo_status_t
_cairo_gstate_set_miter_limit (cairo_gstate_t *gstate, double limit)
{
    gstate->stroke_style.miter_limit = limit;

    return CAIRO_STATUS_SUCCESS;
}

double
_cairo_gstate_get_miter_limit (cairo_gstate_t *gstate)
{
    return gstate->stroke_style.miter_limit;
}

void
_cairo_gstate_get_matrix (cairo_gstate_t *gstate, cairo_matrix_t *matrix)
{
    *matrix = gstate->ctm;
}

cairo_status_t
_cairo_gstate_translate (cairo_gstate_t *gstate, double tx, double ty)
{
    cairo_matrix_t tmp;

    if (! ISFINITE (tx) || ! ISFINITE (ty))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    _cairo_gstate_unset_scaled_font (gstate);

    cairo_matrix_init_translate (&tmp, tx, ty);
    cairo_matrix_multiply (&gstate->ctm, &tmp, &gstate->ctm);
    gstate->is_identity = FALSE;

    /* paranoid check against gradual numerical instability */
    if (! _cairo_matrix_is_invertible (&gstate->ctm))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    cairo_matrix_init_translate (&tmp, -tx, -ty);
    cairo_matrix_multiply (&gstate->ctm_inverse, &gstate->ctm_inverse, &tmp);

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_gstate_scale (cairo_gstate_t *gstate, double sx, double sy)
{
    cairo_matrix_t tmp;

    if (sx * sy == 0.) /* either sx or sy is 0, or det == 0 due to underflow */
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);
    if (! ISFINITE (sx) || ! ISFINITE (sy))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    _cairo_gstate_unset_scaled_font (gstate);

    cairo_matrix_init_scale (&tmp, sx, sy);
    cairo_matrix_multiply (&gstate->ctm, &tmp, &gstate->ctm);
    gstate->is_identity = FALSE;

    /* paranoid check against gradual numerical instability */
    if (! _cairo_matrix_is_invertible (&gstate->ctm))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    cairo_matrix_init_scale (&tmp, 1/sx, 1/sy);
    cairo_matrix_multiply (&gstate->ctm_inverse, &gstate->ctm_inverse, &tmp);

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_gstate_rotate (cairo_gstate_t *gstate, double angle)
{
    cairo_matrix_t tmp;

    if (angle == 0.)
	return CAIRO_STATUS_SUCCESS;

    if (! ISFINITE (angle))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    _cairo_gstate_unset_scaled_font (gstate);

    cairo_matrix_init_rotate (&tmp, angle);
    cairo_matrix_multiply (&gstate->ctm, &tmp, &gstate->ctm);
    gstate->is_identity = FALSE;

    /* paranoid check against gradual numerical instability */
    if (! _cairo_matrix_is_invertible (&gstate->ctm))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    cairo_matrix_init_rotate (&tmp, -angle);
    cairo_matrix_multiply (&gstate->ctm_inverse, &gstate->ctm_inverse, &tmp);

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_gstate_transform (cairo_gstate_t	      *gstate,
			 const cairo_matrix_t *matrix)
{
    cairo_matrix_t tmp;
    cairo_status_t status;

    if (! _cairo_matrix_is_invertible (matrix))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    if (_cairo_matrix_is_identity (matrix))
	return CAIRO_STATUS_SUCCESS;

    tmp = *matrix;
    status = cairo_matrix_invert (&tmp);
    if (unlikely (status))
	return status;

    _cairo_gstate_unset_scaled_font (gstate);

    cairo_matrix_multiply (&gstate->ctm, matrix, &gstate->ctm);
    cairo_matrix_multiply (&gstate->ctm_inverse, &gstate->ctm_inverse, &tmp);
    gstate->is_identity = FALSE;

    /* paranoid check against gradual numerical instability */
    if (! _cairo_matrix_is_invertible (&gstate->ctm))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_gstate_set_matrix (cairo_gstate_t       *gstate,
			  const cairo_matrix_t *matrix)
{
    cairo_status_t status;

    if (memcmp (matrix, &gstate->ctm, sizeof (cairo_matrix_t)) == 0)
	return CAIRO_STATUS_SUCCESS;

    if (! _cairo_matrix_is_invertible (matrix))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    if (_cairo_matrix_is_identity (matrix)) {
	_cairo_gstate_identity_matrix (gstate);
	return CAIRO_STATUS_SUCCESS;
    }

    _cairo_gstate_unset_scaled_font (gstate);

    gstate->ctm = *matrix;
    gstate->ctm_inverse = *matrix;
    status = cairo_matrix_invert (&gstate->ctm_inverse);
    assert (status == CAIRO_STATUS_SUCCESS);
    gstate->is_identity = FALSE;

    return CAIRO_STATUS_SUCCESS;
}

void
_cairo_gstate_identity_matrix (cairo_gstate_t *gstate)
{
    if (_cairo_matrix_is_identity (&gstate->ctm))
	return;

    _cairo_gstate_unset_scaled_font (gstate);

    cairo_matrix_init_identity (&gstate->ctm);
    cairo_matrix_init_identity (&gstate->ctm_inverse);
    gstate->is_identity = _cairo_matrix_is_identity (&gstate->target->device_transform);
}

void
_cairo_gstate_user_to_device (cairo_gstate_t *gstate, double *x, double *y)
{
    cairo_matrix_transform_point (&gstate->ctm, x, y);
}

void
_cairo_gstate_user_to_device_distance (cairo_gstate_t *gstate,
				       double *dx, double *dy)
{
    cairo_matrix_transform_distance (&gstate->ctm, dx, dy);
}

void
_cairo_gstate_device_to_user (cairo_gstate_t *gstate, double *x, double *y)
{
    cairo_matrix_transform_point (&gstate->ctm_inverse, x, y);
}

void
_cairo_gstate_device_to_user_distance (cairo_gstate_t *gstate,
				       double *dx, double *dy)
{
    cairo_matrix_transform_distance (&gstate->ctm_inverse, dx, dy);
}

void
_do_cairo_gstate_user_to_backend (cairo_gstate_t *gstate, double *x, double *y)
{
    cairo_matrix_transform_point (&gstate->ctm, x, y);
    cairo_matrix_transform_point (&gstate->target->device_transform, x, y);
}

void
_do_cairo_gstate_user_to_backend_distance (cairo_gstate_t *gstate, double *x, double *y)
{
    cairo_matrix_transform_distance (&gstate->ctm, x, y);
    cairo_matrix_transform_distance (&gstate->target->device_transform, x, y);
}

void
_do_cairo_gstate_backend_to_user (cairo_gstate_t *gstate, double *x, double *y)
{
    cairo_matrix_transform_point (&gstate->target->device_transform_inverse, x, y);
    cairo_matrix_transform_point (&gstate->ctm_inverse, x, y);
}

void
_do_cairo_gstate_backend_to_user_distance (cairo_gstate_t *gstate, double *x, double *y)
{
    cairo_matrix_transform_distance (&gstate->target->device_transform_inverse, x, y);
    cairo_matrix_transform_distance (&gstate->ctm_inverse, x, y);
}

void
_cairo_gstate_backend_to_user_rectangle (cairo_gstate_t *gstate,
                                         double *x1, double *y1,
                                         double *x2, double *y2,
                                         cairo_bool_t *is_tight)
{
    cairo_matrix_t matrix_inverse;

    if (! _cairo_matrix_is_identity (&gstate->target->device_transform_inverse) ||
    ! _cairo_matrix_is_identity (&gstate->ctm_inverse))
    {
	cairo_matrix_multiply (&matrix_inverse,
			       &gstate->target->device_transform_inverse,
			       &gstate->ctm_inverse);
	_cairo_matrix_transform_bounding_box (&matrix_inverse,
					      x1, y1, x2, y2, is_tight);
    }

    else
    {
	if (is_tight)
	    *is_tight = TRUE;
    }
}

/* XXX: NYI
cairo_status_t
_cairo_gstate_stroke_to_path (cairo_gstate_t *gstate)
{
    cairo_status_t status;

    _cairo_pen_init (&gstate);
    return CAIRO_STATUS_SUCCESS;
}
*/

void
_cairo_gstate_path_extents (cairo_gstate_t     *gstate,
			    cairo_path_fixed_t *path,
			    double *x1, double *y1,
			    double *x2, double *y2)
{
    cairo_box_t box;
    double px1, py1, px2, py2;

    if (_cairo_path_fixed_extents (path, &box)) {
	px1 = _cairo_fixed_to_double (box.p1.x);
	py1 = _cairo_fixed_to_double (box.p1.y);
	px2 = _cairo_fixed_to_double (box.p2.x);
	py2 = _cairo_fixed_to_double (box.p2.y);

	_cairo_gstate_backend_to_user_rectangle (gstate,
						 &px1, &py1, &px2, &py2,
						 NULL);
    } else {
	px1 = 0.0;
	py1 = 0.0;
	px2 = 0.0;
	py2 = 0.0;
    }

    if (x1)
	*x1 = px1;
    if (y1)
	*y1 = py1;
    if (x2)
	*x2 = px2;
    if (y2)
	*y2 = py2;
}

static void
_cairo_gstate_copy_pattern (cairo_pattern_t *pattern,
			    const cairo_pattern_t *original)
{
    /* First check if the we can replace the original with a much simpler
     * pattern. For example, gradients that are uniform or just have a single
     * stop can sometimes be replaced with a solid.
     */

    if (_cairo_pattern_is_clear (original)) {
        _cairo_pattern_init_solid ((cairo_solid_pattern_t *) pattern,
				   CAIRO_COLOR_TRANSPARENT);
	return;
    }

    if (original->type == CAIRO_PATTERN_TYPE_LINEAR ||
	original->type == CAIRO_PATTERN_TYPE_RADIAL)
    {
        cairo_color_t color;
	if (_cairo_gradient_pattern_is_solid ((cairo_gradient_pattern_t *) original,
					      NULL,
					      &color))
	{
	    _cairo_pattern_init_solid ((cairo_solid_pattern_t *) pattern,
				       &color);
	    return;
	}
    }

    _cairo_pattern_init_static_copy (pattern, original);
}

static void
_cairo_gstate_copy_transformed_pattern (cairo_gstate_t  *gstate,
					cairo_pattern_t *pattern,
					const cairo_pattern_t *original,
					const cairo_matrix_t  *ctm_inverse)
{
    /*
     * What calculations below do can be described in pseudo-code (so using nonexistent fields) as (using column vectors):
     * pattern->matrix = surface->device_transform *
     * 			 pattern->matrix *
     * 			 ctm_inverse *
     * 			 gstate->target->device_transform_inverse
     *
     * The inverse of which is:
     * pattern->matrix_inverse = gstate->target->device_transform *
     * 				 ctm *
     * 				 pattern->matrix_inverse *
     * 				 surface->device_transform_inverse
     */

    _cairo_gstate_copy_pattern (pattern, original);

    if (original->type == CAIRO_PATTERN_TYPE_SURFACE) {
	cairo_surface_pattern_t *surface_pattern;
	cairo_surface_t *surface;

        surface_pattern = (cairo_surface_pattern_t *) original;
        surface = surface_pattern->surface;

	if (_cairo_surface_has_device_transform (surface))
	    _cairo_pattern_pretransform (pattern, &surface->device_transform);
    }

    if (! _cairo_matrix_is_identity (ctm_inverse))
	_cairo_pattern_transform (pattern, ctm_inverse);

    if (_cairo_surface_has_device_transform (gstate->target)) {
        _cairo_pattern_transform (pattern,
                                  &gstate->target->device_transform_inverse);
    }
}

static void
_cairo_gstate_copy_transformed_source (cairo_gstate_t   *gstate,
				       cairo_pattern_t  *pattern)
{
    _cairo_gstate_copy_transformed_pattern (gstate, pattern,
					    gstate->source,
					    &gstate->source_ctm_inverse);
}

static void
_cairo_gstate_copy_transformed_mask (cairo_gstate_t   *gstate,
				     cairo_pattern_t  *pattern,
				     cairo_pattern_t  *mask)
{
    _cairo_gstate_copy_transformed_pattern (gstate, pattern,
					    mask,
					    &gstate->ctm_inverse);
}

static cairo_operator_t
_reduce_op (cairo_gstate_t *gstate)
{
    cairo_operator_t op;
    const cairo_pattern_t *pattern;

    op = gstate->op;
    if (op != CAIRO_OPERATOR_SOURCE)
	return op;

    pattern = gstate->source;
    if (pattern->type == CAIRO_PATTERN_TYPE_SOLID) {
	const cairo_solid_pattern_t *solid = (cairo_solid_pattern_t *) pattern;
	if (solid->color.alpha_short <= 0x00ff) {
	    op = CAIRO_OPERATOR_CLEAR;
	} else if ((gstate->target->content & CAIRO_CONTENT_ALPHA) == 0) {
	    if ((solid->color.red_short |
		 solid->color.green_short |
		 solid->color.blue_short) <= 0x00ff)
	    {
		op = CAIRO_OPERATOR_CLEAR;
	    }
	}
    } else if (pattern->type == CAIRO_PATTERN_TYPE_SURFACE) {
	const cairo_surface_pattern_t *surface = (cairo_surface_pattern_t *) pattern;
	if (surface->surface->is_clear &&
	    surface->surface->content & CAIRO_CONTENT_ALPHA)
	{
	    op = CAIRO_OPERATOR_CLEAR;
	}
    } else {
	const cairo_gradient_pattern_t *gradient = (cairo_gradient_pattern_t *) pattern;
	if (gradient->n_stops == 0)
	    op = CAIRO_OPERATOR_CLEAR;
    }

    return op;
}

static cairo_status_t
_cairo_gstate_get_pattern_status (const cairo_pattern_t *pattern)
{
    if (unlikely (pattern->type == CAIRO_PATTERN_TYPE_MESH &&
		  ((const cairo_mesh_pattern_t *) pattern)->current_patch))
    {
	/* If current patch != NULL, the pattern is under construction
	 * and cannot be used as a source */
	return CAIRO_STATUS_INVALID_MESH_CONSTRUCTION;
    }

    return pattern->status;
}

cairo_status_t
_cairo_gstate_paint (cairo_gstate_t *gstate)
{
    cairo_pattern_union_t source_pattern;
    const cairo_pattern_t *pattern;
    cairo_status_t status;
    cairo_operator_t op;

    status = _cairo_gstate_get_pattern_status (gstate->source);
    if (unlikely (status))
	return status;

    if (gstate->op == CAIRO_OPERATOR_DEST)
	return CAIRO_STATUS_SUCCESS;

    if (_cairo_clip_is_all_clipped (gstate->clip))
	return CAIRO_STATUS_SUCCESS;

    op = _reduce_op (gstate);
    if (op == CAIRO_OPERATOR_CLEAR) {
	pattern = &_cairo_pattern_clear.base;
    } else {
	_cairo_gstate_copy_transformed_source (gstate, &source_pattern.base);
	pattern = &source_pattern.base;
    }

    return _cairo_surface_paint (gstate->target,
				 op, pattern,
				 gstate->clip);
}

cairo_status_t
_cairo_gstate_mask (cairo_gstate_t  *gstate,
		    cairo_pattern_t *mask)
{
    cairo_pattern_union_t source_pattern, mask_pattern;
    const cairo_pattern_t *source;
    cairo_operator_t op;
    cairo_status_t status;

    status = _cairo_gstate_get_pattern_status (mask);
    if (unlikely (status))
	return status;

    status = _cairo_gstate_get_pattern_status (gstate->source);
    if (unlikely (status))
	return status;

    if (gstate->op == CAIRO_OPERATOR_DEST)
	return CAIRO_STATUS_SUCCESS;

    if (_cairo_clip_is_all_clipped (gstate->clip))
	return CAIRO_STATUS_SUCCESS;

    assert (gstate->opacity == 1.0);

    if (_cairo_pattern_is_opaque (mask, NULL))
	return _cairo_gstate_paint (gstate);

    if (_cairo_pattern_is_clear (mask) &&
	_cairo_operator_bounded_by_mask (gstate->op))
    {
	return CAIRO_STATUS_SUCCESS;
    }

    op = _reduce_op (gstate);
    if (op == CAIRO_OPERATOR_CLEAR) {
	source = &_cairo_pattern_clear.base;
    } else {
	_cairo_gstate_copy_transformed_source (gstate, &source_pattern.base);
	source = &source_pattern.base;
    }
    _cairo_gstate_copy_transformed_mask (gstate, &mask_pattern.base, mask);

    if (source->type == CAIRO_PATTERN_TYPE_SOLID && !source->is_foreground_marker &&
	mask_pattern.base.type == CAIRO_PATTERN_TYPE_SOLID &&
	_cairo_operator_bounded_by_source (op))
    {
	const cairo_solid_pattern_t *solid = (cairo_solid_pattern_t *) source;
	cairo_color_t combined;

	if (mask_pattern.base.has_component_alpha) {
#define M(R, A, B, c) R.c = A.c * B.c
	    M(combined, solid->color, mask_pattern.solid.color, red);
	    M(combined, solid->color, mask_pattern.solid.color, green);
	    M(combined, solid->color, mask_pattern.solid.color, blue);
	    M(combined, solid->color, mask_pattern.solid.color, alpha);
#undef M
	} else {
	    combined = solid->color;
	    _cairo_color_multiply_alpha (&combined, mask_pattern.solid.color.alpha);
	}

	_cairo_pattern_init_solid (&source_pattern.solid, &combined);

	status = _cairo_surface_paint (gstate->target, op,
				       &source_pattern.base,
				       gstate->clip);
    }
    else
    {
	status = _cairo_surface_mask (gstate->target, op,
				      source,
				      &mask_pattern.base,
				      gstate->clip);
    }

    return status;
}

cairo_status_t
_cairo_gstate_stroke (cairo_gstate_t *gstate, cairo_path_fixed_t *path)
{
    cairo_pattern_union_t source_pattern;
    cairo_stroke_style_t style;
    double dash[2];
    cairo_status_t status;
    cairo_matrix_t aggregate_transform;
    cairo_matrix_t aggregate_transform_inverse;

    status = _cairo_gstate_get_pattern_status (gstate->source);
    if (unlikely (status))
	return status;

    if (gstate->op == CAIRO_OPERATOR_DEST)
	return CAIRO_STATUS_SUCCESS;

    if (gstate->stroke_style.line_width <= 0.0 && !gstate->stroke_style.is_hairline)
	return CAIRO_STATUS_SUCCESS;

    if (_cairo_clip_is_all_clipped (gstate->clip))
	return CAIRO_STATUS_SUCCESS;

    assert (gstate->opacity == 1.0);

    cairo_matrix_multiply (&aggregate_transform,
                           &gstate->ctm,
                           &gstate->target->device_transform);
    cairo_matrix_multiply (&aggregate_transform_inverse,
                           &gstate->target->device_transform_inverse,
                           &gstate->ctm_inverse);

    memcpy (&style, &gstate->stroke_style, sizeof (gstate->stroke_style));
    if (_cairo_stroke_style_dash_can_approximate (&gstate->stroke_style, &aggregate_transform, gstate->tolerance)) {
        style.dash = dash;
        _cairo_stroke_style_dash_approximate (&gstate->stroke_style, &gstate->ctm, gstate->tolerance,
					      &style.dash_offset,
					      style.dash,
					      &style.num_dashes);
    }

    _cairo_gstate_copy_transformed_source (gstate, &source_pattern.base);

    return _cairo_surface_stroke (gstate->target,
				  gstate->op,
				  &source_pattern.base,
				  path,
				  &style,
				  &aggregate_transform,
				  &aggregate_transform_inverse,
				  gstate->tolerance,
				  gstate->antialias,
				  gstate->clip);
}

cairo_status_t
_cairo_gstate_in_stroke (cairo_gstate_t	    *gstate,
			 cairo_path_fixed_t *path,
			 double		     x,
			 double		     y,
			 cairo_bool_t	    *inside_ret)
{
    cairo_status_t status;
    cairo_rectangle_int_t extents;
    cairo_box_t limit;
    cairo_traps_t traps;

    if (gstate->stroke_style.line_width <= 0.0) {
	*inside_ret = FALSE;
	return CAIRO_STATUS_SUCCESS;
    }

    _cairo_gstate_user_to_backend (gstate, &x, &y);

    /* Before we perform the expensive stroke analysis,
     * check whether the point is within the extents of the path.
     */
    _cairo_path_fixed_approximate_stroke_extents (path,
						  &gstate->stroke_style,
						  &gstate->ctm,
						  gstate->target->is_vector,
						  &extents);
    if (x < extents.x || x > extents.x + extents.width ||
	y < extents.y || y > extents.y + extents.height)
    {
	*inside_ret = FALSE;
	return CAIRO_STATUS_SUCCESS;
    }

    limit.p1.x = _cairo_fixed_from_double (x) - 1;
    limit.p1.y = _cairo_fixed_from_double (y) - 1;
    limit.p2.x = limit.p1.x + 2;
    limit.p2.y = limit.p1.y + 2;

    _cairo_traps_init (&traps);
    _cairo_traps_limit (&traps, &limit, 1);

    status = _cairo_path_fixed_stroke_polygon_to_traps (path,
							&gstate->stroke_style,
							&gstate->ctm,
							&gstate->ctm_inverse,
							gstate->tolerance,
							&traps);
    if (unlikely (status))
	goto BAIL;

    *inside_ret = _cairo_traps_contain (&traps, x, y);

BAIL:
    _cairo_traps_fini (&traps);

    return status;
}

cairo_status_t
_cairo_gstate_fill (cairo_gstate_t *gstate, cairo_path_fixed_t *path)
{
    cairo_status_t status;

    status = _cairo_gstate_get_pattern_status (gstate->source);
    if (unlikely (status))
	return status;

    if (gstate->op == CAIRO_OPERATOR_DEST)
	return CAIRO_STATUS_SUCCESS;

    if (_cairo_clip_is_all_clipped (gstate->clip))
	return CAIRO_STATUS_SUCCESS;

    assert (gstate->opacity == 1.0);

    if (_cairo_path_fixed_fill_is_empty (path)) {
	if (_cairo_operator_bounded_by_mask (gstate->op))
	    return CAIRO_STATUS_SUCCESS;

	status = _cairo_surface_paint (gstate->target,
				       CAIRO_OPERATOR_CLEAR,
				       &_cairo_pattern_clear.base,
				       gstate->clip);
    } else {
	cairo_pattern_union_t source_pattern;
	const cairo_pattern_t *pattern;
	cairo_operator_t op;
	cairo_rectangle_int_t extents;
	cairo_box_t box;

	op = _reduce_op (gstate);
	if (op == CAIRO_OPERATOR_CLEAR) {
	    pattern = &_cairo_pattern_clear.base;
	} else {
	    _cairo_gstate_copy_transformed_source (gstate, &source_pattern.base);
	    pattern = &source_pattern.base;
	}

	/* Toolkits often paint the entire background with a fill */
	if (_cairo_surface_get_extents (gstate->target, &extents) &&
	    _cairo_path_fixed_is_box (path, &box) &&
	    box.p1.x <= _cairo_fixed_from_int (extents.x) &&
	    box.p1.y <= _cairo_fixed_from_int (extents.y) &&
	    box.p2.x >= _cairo_fixed_from_int (extents.x + extents.width) &&
	    box.p2.y >= _cairo_fixed_from_int (extents.y + extents.height))
	{
	    status = _cairo_surface_paint (gstate->target, op, pattern,
					   gstate->clip);
	}
	else
	{
	    status = _cairo_surface_fill (gstate->target, op, pattern,
					  path,
					  gstate->fill_rule,
					  gstate->tolerance,
					  gstate->antialias,
					  gstate->clip);
	}
    }

    return status;
}

cairo_bool_t
_cairo_gstate_in_fill (cairo_gstate_t	  *gstate,
		       cairo_path_fixed_t *path,
		       double		   x,
		       double		   y)
{
    _cairo_gstate_user_to_backend (gstate, &x, &y);

    return _cairo_path_fixed_in_fill (path,
				      gstate->fill_rule,
				      gstate->tolerance,
				      x, y);
}

cairo_bool_t
_cairo_gstate_in_clip (cairo_gstate_t	  *gstate,
		       double		   x,
		       double		   y)
{
    cairo_clip_t *clip = gstate->clip;
    int i;

    if (_cairo_clip_is_all_clipped (clip))
	return FALSE;

    if (clip == NULL)
	return TRUE;

    _cairo_gstate_user_to_backend (gstate, &x, &y);

    if (x <  clip->extents.x ||
	x >= clip->extents.x + clip->extents.width ||
	y <  clip->extents.y ||
	y >= clip->extents.y + clip->extents.height)
    {
	return FALSE;
    }

    if (clip->num_boxes) {
	int fx, fy;

	fx = _cairo_fixed_from_double (x);
	fy = _cairo_fixed_from_double (y);
	for (i = 0; i < clip->num_boxes; i++) {
	    if (fx >= clip->boxes[i].p1.x && fx <= clip->boxes[i].p2.x &&
		fy >= clip->boxes[i].p1.y && fy <= clip->boxes[i].p2.y)
		break;
	}
	if (i == clip->num_boxes)
	    return FALSE;
    }

    if (clip->path) {
	cairo_clip_path_t *clip_path = clip->path;
	do {
	    if (! _cairo_path_fixed_in_fill (&clip_path->path,
					     clip_path->fill_rule,
					     clip_path->tolerance,
					     x, y))
		return FALSE;
	} while ((clip_path = clip_path->prev) != NULL);
    }

    return TRUE;
}

cairo_status_t
_cairo_gstate_copy_page (cairo_gstate_t *gstate)
{
    cairo_surface_copy_page (gstate->target);
    return cairo_surface_status (gstate->target);
}

cairo_status_t
_cairo_gstate_show_page (cairo_gstate_t *gstate)
{
    cairo_surface_show_page (gstate->target);
    return cairo_surface_status (gstate->target);
}

static void
_cairo_gstate_extents_to_user_rectangle (cairo_gstate_t	  *gstate,
					 const cairo_box_t *extents,
					 double *x1, double *y1,
					 double *x2, double *y2)
{
    double px1, py1, px2, py2;

    px1 = _cairo_fixed_to_double (extents->p1.x);
    py1 = _cairo_fixed_to_double (extents->p1.y);
    px2 = _cairo_fixed_to_double (extents->p2.x);
    py2 = _cairo_fixed_to_double (extents->p2.y);

    _cairo_gstate_backend_to_user_rectangle (gstate,
					     &px1, &py1, &px2, &py2,
					     NULL);
    if (x1)
	*x1 = px1;
    if (y1)
	*y1 = py1;
    if (x2)
	*x2 = px2;
    if (y2)
	*y2 = py2;
}

cairo_status_t
_cairo_gstate_stroke_extents (cairo_gstate_t	 *gstate,
			      cairo_path_fixed_t *path,
                              double *x1, double *y1,
			      double *x2, double *y2)
{
    cairo_int_status_t status;
    cairo_box_t extents;
    cairo_bool_t empty;

    if (x1)
	*x1 = 0.0;
    if (y1)
	*y1 = 0.0;
    if (x2)
	*x2 = 0.0;
    if (y2)
	*y2 = 0.0;

    if (gstate->stroke_style.line_width <= 0.0)
	return CAIRO_STATUS_SUCCESS;

    status = CAIRO_INT_STATUS_UNSUPPORTED;
    if (_cairo_path_fixed_stroke_is_rectilinear (path)) {
	cairo_boxes_t boxes;

	_cairo_boxes_init (&boxes);
	status = _cairo_path_fixed_stroke_rectilinear_to_boxes (path,
								&gstate->stroke_style,
								&gstate->ctm,
								gstate->antialias,
								&boxes);
	empty = boxes.num_boxes == 0;
	if (! empty)
	    _cairo_boxes_extents (&boxes, &extents);
	_cairo_boxes_fini (&boxes);
    }

    if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	cairo_polygon_t polygon;

	_cairo_polygon_init (&polygon, NULL, 0);
	status = _cairo_path_fixed_stroke_to_polygon (path,
						      &gstate->stroke_style,
						      &gstate->ctm,
						      &gstate->ctm_inverse,
						      gstate->tolerance,
						      &polygon);
	empty = polygon.num_edges == 0;
	if (! empty)
	    extents = polygon.extents;
	_cairo_polygon_fini (&polygon);
    }
    if (! empty) {
	_cairo_gstate_extents_to_user_rectangle (gstate, &extents,
						 x1, y1, x2, y2);
    }

    return status;
}

cairo_status_t
_cairo_gstate_fill_extents (cairo_gstate_t     *gstate,
			    cairo_path_fixed_t *path,
                            double *x1, double *y1,
			    double *x2, double *y2)
{
    cairo_status_t status;
    cairo_box_t extents;
    cairo_bool_t empty;

    if (x1)
	*x1 = 0.0;
    if (y1)
	*y1 = 0.0;
    if (x2)
	*x2 = 0.0;
    if (y2)
	*y2 = 0.0;

    if (_cairo_path_fixed_fill_is_empty (path))
	return CAIRO_STATUS_SUCCESS;

    if (_cairo_path_fixed_fill_is_rectilinear (path)) {
	cairo_boxes_t boxes;

	_cairo_boxes_init (&boxes);
	status = _cairo_path_fixed_fill_rectilinear_to_boxes (path,
							      gstate->fill_rule,
							      gstate->antialias,
							      &boxes);
	empty = boxes.num_boxes == 0;
	if (! empty)
	    _cairo_boxes_extents (&boxes, &extents);

	_cairo_boxes_fini (&boxes);
    } else {
	cairo_traps_t traps;

	_cairo_traps_init (&traps);

	status = _cairo_path_fixed_fill_to_traps (path,
						  gstate->fill_rule,
						  gstate->tolerance,
						  &traps);
	empty = traps.num_traps == 0;
	if (! empty)
	    _cairo_traps_extents (&traps, &extents);

	_cairo_traps_fini (&traps);
    }

    if (! empty) {
	_cairo_gstate_extents_to_user_rectangle (gstate, &extents,
						 x1, y1, x2, y2);
    }

    return status;
}

cairo_status_t
_cairo_gstate_reset_clip (cairo_gstate_t *gstate)
{
    _cairo_clip_destroy (gstate->clip);
    gstate->clip = NULL;

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_gstate_clip (cairo_gstate_t *gstate, cairo_path_fixed_t *path)
{
    gstate->clip =
	_cairo_clip_intersect_path (gstate->clip,
				    path,
				    gstate->fill_rule,
				    gstate->tolerance,
				    gstate->antialias);
    /* XXX */
    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
_cairo_gstate_int_clip_extents (cairo_gstate_t        *gstate,
				cairo_rectangle_int_t *extents)
{
    cairo_bool_t is_bounded;

    is_bounded = _cairo_surface_get_extents (gstate->target, extents);

    if (gstate->clip) {
	_cairo_rectangle_intersect (extents,
				    _cairo_clip_get_extents (gstate->clip));
	is_bounded = TRUE;
    }

    return is_bounded;
}

cairo_bool_t
_cairo_gstate_clip_extents (cairo_gstate_t *gstate,
		            double         *x1,
		            double         *y1,
			    double         *x2,
			    double         *y2)
{
    cairo_rectangle_int_t extents;
    double px1, py1, px2, py2;

    if (! _cairo_gstate_int_clip_extents (gstate, &extents))
	return FALSE;

    px1 = extents.x;
    py1 = extents.y;
    px2 = extents.x + (int) extents.width;
    py2 = extents.y + (int) extents.height;

    _cairo_gstate_backend_to_user_rectangle (gstate,
					     &px1, &py1, &px2, &py2,
					     NULL);

    if (x1)
	*x1 = px1;
    if (y1)
	*y1 = py1;
    if (x2)
	*x2 = px2;
    if (y2)
	*y2 = py2;

    return TRUE;
}

cairo_rectangle_list_t*
_cairo_gstate_copy_clip_rectangle_list (cairo_gstate_t *gstate)
{
    cairo_rectangle_int_t extents;
    cairo_rectangle_list_t *list;
    cairo_clip_t *clip;

    if (_cairo_surface_get_extents (gstate->target, &extents))
	clip = _cairo_clip_copy_intersect_rectangle (gstate->clip, &extents);
    else
	clip = gstate->clip;

    list = _cairo_clip_copy_rectangle_list (clip, gstate);

    if (clip != gstate->clip)
	_cairo_clip_destroy (clip);

    return list;
}

cairo_status_t
_cairo_gstate_tag_begin (cairo_gstate_t *gstate,
			 const char *tag_name, const char *attributes)
{
    return _cairo_surface_tag (gstate->target,
			       TRUE, /* begin */
			       tag_name,
			       attributes ? attributes : "");
}

cairo_status_t
_cairo_gstate_tag_end (cairo_gstate_t *gstate,
		       const char *tag_name)
{
    return _cairo_surface_tag (gstate->target,
			       FALSE, /* begin */
			       tag_name,
			       NULL); /* attributes */
}

static void
_cairo_gstate_unset_scaled_font (cairo_gstate_t *gstate)
{
    if (gstate->scaled_font == NULL)
	return;

    if (gstate->previous_scaled_font != NULL)
	cairo_scaled_font_destroy (gstate->previous_scaled_font);

    gstate->previous_scaled_font = gstate->scaled_font;
    gstate->scaled_font = NULL;
}

cairo_status_t
_cairo_gstate_set_font_size (cairo_gstate_t *gstate,
			     double          size)
{
    _cairo_gstate_unset_scaled_font (gstate);

    cairo_matrix_init_scale (&gstate->font_matrix, size, size);

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_gstate_set_font_matrix (cairo_gstate_t	    *gstate,
			       const cairo_matrix_t *matrix)
{
    if (memcmp (matrix, &gstate->font_matrix, sizeof (cairo_matrix_t)) == 0)
	return CAIRO_STATUS_SUCCESS;

    _cairo_gstate_unset_scaled_font (gstate);

    gstate->font_matrix = *matrix;

    return CAIRO_STATUS_SUCCESS;
}

void
_cairo_gstate_get_font_matrix (cairo_gstate_t *gstate,
			       cairo_matrix_t *matrix)
{
    *matrix = gstate->font_matrix;
}

void
_cairo_gstate_set_font_options (cairo_gstate_t             *gstate,
				const cairo_font_options_t *options)
{
    if (memcmp (options, &gstate->font_options, sizeof (cairo_font_options_t)) == 0)
	return;

    _cairo_gstate_unset_scaled_font (gstate);

    _cairo_font_options_fini (&gstate->font_options);
    _cairo_font_options_init_copy (&gstate->font_options, options);
}

void
_cairo_gstate_get_font_options (cairo_gstate_t       *gstate,
				cairo_font_options_t *options)
{
    _cairo_font_options_fini (options);
    _cairo_font_options_init_copy (options, &gstate->font_options);
}

cairo_status_t
_cairo_gstate_get_font_face (cairo_gstate_t     *gstate,
			     cairo_font_face_t **font_face)
{
    cairo_status_t status;

    status = _cairo_gstate_ensure_font_face (gstate);
    if (unlikely (status))
	return status;

    *font_face = gstate->font_face;

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_gstate_get_scaled_font (cairo_gstate_t       *gstate,
			       cairo_scaled_font_t **scaled_font)
{
    cairo_status_t status;

    status = _cairo_gstate_ensure_scaled_font (gstate);
    if (unlikely (status))
	return status;

    *scaled_font = gstate->scaled_font;

    return CAIRO_STATUS_SUCCESS;
}

/*
 * Like everything else in this file, fonts involve Too Many Coordinate Spaces;
 * it is easy to get confused about what's going on.
 *
 * The user's view
 * ---------------
 *
 * Users ask for things in user space. When cairo starts, a user space unit
 * is about 1/96 inch, which is similar to (but importantly different from)
 * the normal "point" units most users think in terms of. When a user
 * selects a font, its scale is set to "one user unit". The user can then
 * independently scale the user coordinate system *or* the font matrix, in
 * order to adjust the rendered size of the font.
 *
 * Metrics are returned in user space, whether they are obtained from
 * the currently selected font in a  #cairo_t or from a #cairo_scaled_font_t
 * which is a font specialized to a particular scale matrix, CTM, and target
 * surface.
 *
 * The font's view
 * ---------------
 *
 * Fonts are designed and stored (in say .ttf files) in "font space", which
 * describes an "EM Square" (a design tile) and has some abstract number
 * such as 1000, 1024, or 2048 units per "EM". This is basically an
 * uninteresting space for us, but we need to remember that it exists.
 *
 * Font resources (from libraries or operating systems) render themselves
 * to a particular device. Since they do not want to make most programmers
 * worry about the font design space, the scaling API is simplified to
 * involve just telling the font the required pixel size of the EM square
 * (that is, in device space).
 *
 *
 * Cairo's gstate view
 * -------------------
 *
 * In addition to the CTM and CTM inverse, we keep a matrix in the gstate
 * called the "font matrix" which describes the user's most recent
 * font-scaling or font-transforming request. This is kept in terms of an
 * abstract scale factor, composed with the CTM and used to set the font's
 * pixel size. So if the user asks to "scale the font by 12", the matrix
 * is:
 *
 *   [ 12.0, 0.0, 0.0, 12.0, 0.0, 0.0 ]
 *
 * It is an affine matrix, like all cairo matrices, where its tx and ty
 * components are used to "nudging" fonts around and are handled in gstate
 * and then ignored by the "scaled-font" layer.
 *
 * In order to perform any action on a font, we must build an object
 * called a #cairo_font_scale_t; this contains the central 2x2 matrix
 * resulting from "font matrix * CTM" (sans the font matrix translation
 * components as stated in the previous paragraph).
 *
 * We pass this to the font when making requests of it, which causes it to
 * reply for a particular [user request, device] combination, under the CTM
 * (to accommodate the "zoom in" == "bigger fonts" issue above).
 *
 * The other terms in our communication with the font are therefore in
 * device space. When we ask it to perform text->glyph conversion, it will
 * produce a glyph string in device space. Glyph vectors we pass to it for
 * measuring or rendering should be in device space. The metrics which we
 * get back from the font will be in device space. The contents of the
 * global glyph image cache will be in device space.
 *
 *
 * Cairo's public view
 * -------------------
 *
 * Since the values entering and leaving via public API calls are in user
 * space, the gstate functions typically need to multiply arguments by the
 * CTM (for user-input glyph vectors), and return values by the CTM inverse
 * (for font responses such as metrics or glyph vectors).
 *
 */

static cairo_status_t
_cairo_gstate_ensure_font_face (cairo_gstate_t *gstate)
{
    cairo_font_face_t *font_face;

    if (gstate->font_face != NULL)
	return gstate->font_face->status;


    font_face = cairo_toy_font_face_create (CAIRO_FONT_FAMILY_DEFAULT,
					    CAIRO_FONT_SLANT_DEFAULT,
					    CAIRO_FONT_WEIGHT_DEFAULT);
    if (font_face->status)
	return font_face->status;

    gstate->font_face = font_face;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_gstate_ensure_scaled_font (cairo_gstate_t *gstate)
{
    cairo_status_t status;
    cairo_font_options_t options;
    cairo_scaled_font_t *scaled_font;
    cairo_matrix_t font_ctm;

    if (gstate->scaled_font != NULL)
	return gstate->scaled_font->status;

    status = _cairo_gstate_ensure_font_face (gstate);
    if (unlikely (status))
	return status;

    cairo_surface_get_font_options (gstate->target, &options);
    cairo_font_options_merge (&options, &gstate->font_options);

    cairo_matrix_multiply (&font_ctm,
			   &gstate->ctm,
			   &gstate->target->device_transform);

    scaled_font = cairo_scaled_font_create (gstate->font_face,
				            &gstate->font_matrix,
					    &font_ctm,
					    &options);

    status = cairo_scaled_font_status (scaled_font);
    if (unlikely (status))
	return status;

    gstate->scaled_font = scaled_font;

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_gstate_get_font_extents (cairo_gstate_t *gstate,
				cairo_font_extents_t *extents)
{
    cairo_status_t status = _cairo_gstate_ensure_scaled_font (gstate);
    if (unlikely (status))
	return status;

    cairo_scaled_font_extents (gstate->scaled_font, extents);

    return cairo_scaled_font_status (gstate->scaled_font);
}

cairo_status_t
_cairo_gstate_set_font_face (cairo_gstate_t    *gstate,
			     cairo_font_face_t *font_face)
{
    if (font_face && font_face->status)
	return _cairo_error (font_face->status);

    if (font_face == gstate->font_face)
	return CAIRO_STATUS_SUCCESS;

    cairo_font_face_destroy (gstate->font_face);
    gstate->font_face = cairo_font_face_reference (font_face);

    _cairo_gstate_unset_scaled_font (gstate);

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_gstate_glyph_extents (cairo_gstate_t *gstate,
			     const cairo_glyph_t *glyphs,
			     int num_glyphs,
			     cairo_text_extents_t *extents)
{
    cairo_status_t status;

    status = _cairo_gstate_ensure_scaled_font (gstate);
    if (unlikely (status))
	return status;

    cairo_scaled_font_glyph_extents (gstate->scaled_font,
				     glyphs, num_glyphs,
				     extents);

    return cairo_scaled_font_status (gstate->scaled_font);
}

cairo_status_t
_cairo_gstate_show_text_glyphs (cairo_gstate_t		   *gstate,
				const cairo_glyph_t	   *glyphs,
				int			    num_glyphs,
				cairo_glyph_text_info_t    *info)
{
    cairo_glyph_t stack_transformed_glyphs[CAIRO_STACK_ARRAY_LENGTH (cairo_glyph_t)];
    cairo_text_cluster_t stack_transformed_clusters[CAIRO_STACK_ARRAY_LENGTH (cairo_text_cluster_t)];
    cairo_pattern_union_t source_pattern;
    cairo_glyph_t *transformed_glyphs;
    const cairo_pattern_t *pattern;
    cairo_text_cluster_t *transformed_clusters;
    cairo_operator_t op;
    cairo_status_t status;

    status = _cairo_gstate_get_pattern_status (gstate->source);
    if (unlikely (status))
	return status;

    if (gstate->op == CAIRO_OPERATOR_DEST)
	return CAIRO_STATUS_SUCCESS;

    if (_cairo_clip_is_all_clipped (gstate->clip))
	return CAIRO_STATUS_SUCCESS;

    status = _cairo_gstate_ensure_scaled_font (gstate);
    if (unlikely (status))
	return status;

    transformed_glyphs = stack_transformed_glyphs;
    transformed_clusters = stack_transformed_clusters;

    if (num_glyphs > ARRAY_LENGTH (stack_transformed_glyphs)) {
	transformed_glyphs = cairo_glyph_allocate (num_glyphs);
	if (unlikely (transformed_glyphs == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    if (info != NULL) {
	if (info->num_clusters > ARRAY_LENGTH (stack_transformed_clusters)) {
	    transformed_clusters = cairo_text_cluster_allocate (info->num_clusters);
	    if (unlikely (transformed_clusters == NULL)) {
		status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
		goto CLEANUP_GLYPHS;
	    }
	}

	_cairo_gstate_transform_glyphs_to_backend (gstate,
						   glyphs, num_glyphs,
						   info->clusters,
						   info->num_clusters,
						   info->cluster_flags,
						   transformed_glyphs,
						   &num_glyphs,
						   transformed_clusters);
    } else {
	_cairo_gstate_transform_glyphs_to_backend (gstate,
						   glyphs, num_glyphs,
						   NULL, 0, 0,
						   transformed_glyphs,
						   &num_glyphs,
						   NULL);
    }

    if (num_glyphs == 0)
	goto CLEANUP_GLYPHS;

    op = _reduce_op (gstate);
    if (op == CAIRO_OPERATOR_CLEAR) {
	pattern = &_cairo_pattern_clear.base;
    } else {
	_cairo_gstate_copy_transformed_source (gstate, &source_pattern.base);
	pattern = &source_pattern.base;
    }

    /* For really huge font sizes, we can just do path;fill instead of
     * show_glyphs, as show_glyphs would put excess pressure on the cache,
     * and moreover, not all components below us correctly handle huge font
     * sizes.  I wanted to set the limit at 256.  But alas, seems like cairo's
     * rasterizer is something like ten times slower than freetype's for huge
     * sizes.  So, no win just yet.  For now, do it for insanely-huge sizes,
     * just to make sure we don't make anyone unhappy.  When we get a really
     * fast rasterizer in cairo, we may want to readjust this.
     *
     * Needless to say, do this only if show_text_glyphs is not available. */
    if (cairo_surface_has_show_text_glyphs (gstate->target) ||
	_cairo_scaled_font_get_max_scale (gstate->scaled_font) <= 10240)
    {

	if (info != NULL) {
	    status = _cairo_surface_show_text_glyphs (gstate->target, op, pattern,
						      info->utf8, info->utf8_len,
						      transformed_glyphs, num_glyphs,
						      transformed_clusters, info->num_clusters,
						      info->cluster_flags,
						      gstate->scaled_font,
						      gstate->clip);
	} else {
	    status = _cairo_surface_show_text_glyphs (gstate->target, op, pattern,
						      NULL, 0,
						      transformed_glyphs, num_glyphs,
						      NULL, 0, 0,
						      gstate->scaled_font,
						      gstate->clip);
	}
    }
    else
    {
	cairo_path_fixed_t path;

	_cairo_path_fixed_init (&path);

	status = _cairo_scaled_font_glyph_path (gstate->scaled_font,
						transformed_glyphs, num_glyphs,
						&path);

	if (status == CAIRO_STATUS_SUCCESS) {
	    status = _cairo_surface_fill (gstate->target, op, pattern,
					  &path,
					  CAIRO_FILL_RULE_WINDING,
					  gstate->tolerance,
					  gstate->scaled_font->options.antialias,
					  gstate->clip);
	}

	_cairo_path_fixed_fini (&path);
    }

CLEANUP_GLYPHS:
    if (transformed_glyphs != stack_transformed_glyphs)
      cairo_glyph_free (transformed_glyphs);
    if (transformed_clusters != stack_transformed_clusters)
      cairo_text_cluster_free (transformed_clusters);

    return status;
}

cairo_status_t
_cairo_gstate_glyph_path (cairo_gstate_t      *gstate,
			  const cairo_glyph_t *glyphs,
			  int		       num_glyphs,
			  cairo_path_fixed_t  *path)
{
    cairo_glyph_t stack_transformed_glyphs[CAIRO_STACK_ARRAY_LENGTH (cairo_glyph_t)];
    cairo_glyph_t *transformed_glyphs;
    cairo_status_t status;

    status = _cairo_gstate_ensure_scaled_font (gstate);
    if (unlikely (status))
	return status;

    if (num_glyphs < ARRAY_LENGTH (stack_transformed_glyphs)) {
	transformed_glyphs = stack_transformed_glyphs;
    } else {
	transformed_glyphs = cairo_glyph_allocate (num_glyphs);
	if (unlikely (transformed_glyphs == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    _cairo_gstate_transform_glyphs_to_backend (gstate,
					       glyphs, num_glyphs,
					       NULL, 0, 0,
					       transformed_glyphs,
					       &num_glyphs, NULL);

    status = _cairo_scaled_font_glyph_path (gstate->scaled_font,
					    transformed_glyphs, num_glyphs,
					    path);

    if (transformed_glyphs != stack_transformed_glyphs)
      cairo_glyph_free (transformed_glyphs);

    return status;
}

cairo_status_t
_cairo_gstate_set_antialias (cairo_gstate_t *gstate,
			     cairo_antialias_t antialias)
{
    gstate->antialias = antialias;

    return CAIRO_STATUS_SUCCESS;
}

cairo_antialias_t
_cairo_gstate_get_antialias (cairo_gstate_t *gstate)
{
    return gstate->antialias;
}

/**
 * _cairo_gstate_transform_glyphs_to_backend:
 * @gstate: a #cairo_gstate_t
 * @glyphs: the array of #cairo_glyph_t objects to be transformed
 * @num_glyphs: the number of elements in @glyphs
 * @transformed_glyphs: a pre-allocated array of at least @num_glyphs
 * #cairo_glyph_t objects
 * @num_transformed_glyphs: the number of elements in @transformed_glyphs
 * after dropping out of bounds glyphs, or %NULL if glyphs shouldn't be
 * dropped
 *
 * Transform an array of glyphs to backend space by first adding the offset
 * of the font matrix, then transforming from user space to backend space.
 * The result of the transformation is placed in @transformed_glyphs.
 *
 * This also uses information from the scaled font and the surface to
 * cull/drop glyphs that will not be visible.
 **/
static void
_cairo_gstate_transform_glyphs_to_backend (cairo_gstate_t	*gstate,
                                           const cairo_glyph_t	*glyphs,
                                           int			 num_glyphs,
					   const cairo_text_cluster_t	*clusters,
					   int			 num_clusters,
					   cairo_text_cluster_flags_t cluster_flags,
                                           cairo_glyph_t	*transformed_glyphs,
					   int			*num_transformed_glyphs,
					   cairo_text_cluster_t *transformed_clusters)
{
    cairo_rectangle_int_t surface_extents;
    cairo_matrix_t *ctm = &gstate->ctm;
    cairo_matrix_t *font_matrix = &gstate->font_matrix;
    cairo_matrix_t *device_transform = &gstate->target->device_transform;
    cairo_bool_t drop = FALSE;
    double x1 = 0, x2 = 0, y1 = 0, y2 = 0;
    int i, j, k;

    drop = TRUE;
    if (! _cairo_gstate_int_clip_extents (gstate, &surface_extents)) {
	drop = FALSE; /* unbounded surface */
    } else {
	double scale10 = 10 * _cairo_scaled_font_get_max_scale (gstate->scaled_font);
	if (surface_extents.width == 0 || surface_extents.height == 0) {
	  /* No visible area.  Don't draw anything */
	  *num_transformed_glyphs = 0;
	  return;
	}
	/* XXX We currently drop any glyphs that have their position outside
	 * of the surface boundaries by a safety margin depending on the
	 * font scale.  This however can fail in extreme cases where the
	 * font has really long swashes for example...  We can correctly
	 * handle that by looking the glyph up and using its device bbox
	 * to device if it's going to be visible, but I'm not inclined to
	 * do that now.
	 */
	x1 = surface_extents.x - scale10;
	y1 = surface_extents.y - scale10;
	x2 = surface_extents.x + (int) surface_extents.width  + scale10;
	y2 = surface_extents.y + (int) surface_extents.height + scale10;
    }

    if (!drop)
	*num_transformed_glyphs = num_glyphs;

#define KEEP_GLYPH(glyph) (x1 <= glyph.x && glyph.x <= x2 && y1 <= glyph.y && glyph.y <= y2)

    j = 0;
    if (_cairo_matrix_is_identity (ctm) &&
        _cairo_matrix_is_identity (device_transform) &&
	font_matrix->x0 == 0 && font_matrix->y0 == 0)
    {
	if (! drop) {
	    memcpy (transformed_glyphs, glyphs,
		    num_glyphs * sizeof (cairo_glyph_t));
	    memcpy (transformed_clusters, clusters,
		    num_clusters * sizeof (cairo_text_cluster_t));
	    j = num_glyphs;
	} else if (num_clusters == 0) {
	    for (i = 0; i < num_glyphs; i++) {
		transformed_glyphs[j].index = glyphs[i].index;
		transformed_glyphs[j].x = glyphs[i].x;
		transformed_glyphs[j].y = glyphs[i].y;
		if (KEEP_GLYPH (transformed_glyphs[j]))
		    j++;
	    }
	} else {
	    const cairo_glyph_t *cur_glyph;

	    if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
		cur_glyph = glyphs + num_glyphs - 1;
	    else
		cur_glyph = glyphs;

	    for (i = 0; i < num_clusters; i++) {
		cairo_bool_t cluster_visible = FALSE;

		for (k = 0; k < clusters[i].num_glyphs; k++) {
		    transformed_glyphs[j+k].index = cur_glyph->index;
		    transformed_glyphs[j+k].x = cur_glyph->x;
		    transformed_glyphs[j+k].y = cur_glyph->y;
		    if (KEEP_GLYPH (transformed_glyphs[j+k]))
			cluster_visible = TRUE;

		    if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
			cur_glyph--;
		    else
			cur_glyph++;
		}

		transformed_clusters[i] = clusters[i];
		if (cluster_visible)
		    j += k;
		else
		    transformed_clusters[i].num_glyphs = 0;
	    }
	}
    }
    else if (_cairo_matrix_is_translation (ctm) &&
             _cairo_matrix_is_translation (device_transform))
    {
        double tx = font_matrix->x0 + ctm->x0 + device_transform->x0;
        double ty = font_matrix->y0 + ctm->y0 + device_transform->y0;

	if (! drop || num_clusters == 0) {
	    for (i = 0; i < num_glyphs; i++) {
		transformed_glyphs[j].index = glyphs[i].index;
		transformed_glyphs[j].x = glyphs[i].x + tx;
		transformed_glyphs[j].y = glyphs[i].y + ty;
		if (!drop || KEEP_GLYPH (transformed_glyphs[j]))
		    j++;
	    }
	    memcpy (transformed_clusters, clusters,
		    num_clusters * sizeof (cairo_text_cluster_t));
	} else {
	    const cairo_glyph_t *cur_glyph;

	    if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
		cur_glyph = glyphs + num_glyphs - 1;
	    else
		cur_glyph = glyphs;

	    for (i = 0; i < num_clusters; i++) {
		cairo_bool_t cluster_visible = FALSE;

		for (k = 0; k < clusters[i].num_glyphs; k++) {
		    transformed_glyphs[j+k].index = cur_glyph->index;
		    transformed_glyphs[j+k].x = cur_glyph->x + tx;
		    transformed_glyphs[j+k].y = cur_glyph->y + ty;
		    if (KEEP_GLYPH (transformed_glyphs[j+k]))
			cluster_visible = TRUE;

		    if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
			cur_glyph--;
		    else
			cur_glyph++;
		}

		transformed_clusters[i] = clusters[i];
		if (cluster_visible)
		    j += k;
		else
		    transformed_clusters[i].num_glyphs = 0;
	    }
	}
    }
    else
    {
        cairo_matrix_t aggregate_transform;

        cairo_matrix_init_translate (&aggregate_transform,
                                     gstate->font_matrix.x0,
                                     gstate->font_matrix.y0);
        cairo_matrix_multiply (&aggregate_transform,
                               &aggregate_transform, ctm);
        cairo_matrix_multiply (&aggregate_transform,
                               &aggregate_transform, device_transform);

	if (! drop || num_clusters == 0) {
	    for (i = 0; i < num_glyphs; i++) {
		transformed_glyphs[j] = glyphs[i];
		cairo_matrix_transform_point (&aggregate_transform,
					      &transformed_glyphs[j].x,
					      &transformed_glyphs[j].y);
		if (! drop || KEEP_GLYPH (transformed_glyphs[j]))
		    j++;
	    }
	    memcpy (transformed_clusters, clusters,
		    num_clusters * sizeof (cairo_text_cluster_t));
	} else {
	    const cairo_glyph_t *cur_glyph;

	    if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
		cur_glyph = glyphs + num_glyphs - 1;
	    else
		cur_glyph = glyphs;

	    for (i = 0; i < num_clusters; i++) {
		cairo_bool_t cluster_visible = FALSE;
		for (k = 0; k < clusters[i].num_glyphs; k++) {
		    transformed_glyphs[j+k] = *cur_glyph;
		    cairo_matrix_transform_point (&aggregate_transform,
						  &transformed_glyphs[j+k].x,
						  &transformed_glyphs[j+k].y);
		    if (KEEP_GLYPH (transformed_glyphs[j+k]))
			cluster_visible = TRUE;

		    if (cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD)
			cur_glyph--;
		    else
			cur_glyph++;
		}

		transformed_clusters[i] = clusters[i];
		if (cluster_visible)
		    j += k;
		else
		    transformed_clusters[i].num_glyphs = 0;
	    }
	}
    }
    *num_transformed_glyphs = j;

    if (num_clusters != 0 && cluster_flags & CAIRO_TEXT_CLUSTER_FLAG_BACKWARD) {
	for (i = 0; i < --j; i++) {
	    cairo_glyph_t tmp;

	    tmp = transformed_glyphs[i];
	    transformed_glyphs[i] = transformed_glyphs[j];
	    transformed_glyphs[j] = tmp;
	}
    }
}
