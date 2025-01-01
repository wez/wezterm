/* cairo - a vector graphics library with display and print output
 *
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
 * The Initial Developer of the Original Code is Red Hat, Inc.
 *
 * Contributor(s):
 *	Carl D. Worth <cworth@redhat.com>
 */

#ifndef CAIRO_GSTATE_PRIVATE_H
#define CAIRO_GSTATE_PRIVATE_H

#include "cairo-clip-private.h"

struct _cairo_gstate {
    cairo_operator_t op;

    double opacity;
    double tolerance;
    cairo_antialias_t antialias;

    cairo_stroke_style_t stroke_style;

    cairo_fill_rule_t fill_rule;

    cairo_font_face_t *font_face;
    cairo_scaled_font_t *scaled_font;	/* Specific to the current CTM */
    cairo_scaled_font_t *previous_scaled_font;	/* holdover */
    cairo_matrix_t font_matrix;
    cairo_font_options_t font_options;

    cairo_clip_t *clip;

    cairo_surface_t *target;		/* The target to which all rendering is directed */
    cairo_surface_t *parent_target;	/* The previous target which was receiving rendering */
    cairo_surface_t *original_target;	/* The original target the initial gstate was created with */

    /* the user is allowed to update the device after we have cached the matrices... */
    cairo_observer_t device_transform_observer;

    cairo_matrix_t ctm;
    cairo_matrix_t ctm_inverse;
    cairo_matrix_t source_ctm_inverse; /* At the time ->source was set */
    cairo_bool_t is_identity;

    cairo_pattern_t *source;

    struct _cairo_gstate *next;
};

/* cairo-gstate.c */
cairo_private cairo_status_t
_cairo_gstate_init (cairo_gstate_t  *gstate,
		    cairo_surface_t *target);

cairo_private void
_cairo_gstate_fini (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_save (cairo_gstate_t **gstate, cairo_gstate_t **freelist);

cairo_private cairo_status_t
_cairo_gstate_restore (cairo_gstate_t **gstate, cairo_gstate_t **freelist);

cairo_private cairo_bool_t
_cairo_gstate_is_group (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_redirect_target (cairo_gstate_t *gstate, cairo_surface_t *child);

cairo_private cairo_surface_t *
_cairo_gstate_get_target (cairo_gstate_t *gstate);

cairo_private cairo_surface_t *
_cairo_gstate_get_original_target (cairo_gstate_t *gstate);

cairo_private cairo_clip_t *
_cairo_gstate_get_clip (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_set_source (cairo_gstate_t *gstate, cairo_pattern_t *source);

cairo_private cairo_pattern_t *
_cairo_gstate_get_source (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_set_operator (cairo_gstate_t *gstate, cairo_operator_t op);

cairo_private cairo_operator_t
_cairo_gstate_get_operator (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_set_opacity (cairo_gstate_t *gstate, double opacity);

cairo_private double
_cairo_gstate_get_opacity (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_set_tolerance (cairo_gstate_t *gstate, double tolerance);

cairo_private double
_cairo_gstate_get_tolerance (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_set_fill_rule (cairo_gstate_t *gstate, cairo_fill_rule_t fill_rule);

cairo_private cairo_fill_rule_t
_cairo_gstate_get_fill_rule (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_set_line_width (cairo_gstate_t *gstate, double width);

cairo_private double
_cairo_gstate_get_line_width (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_set_hairline (cairo_gstate_t *gstate, cairo_bool_t set_hairline);

cairo_private cairo_bool_t
_cairo_gstate_get_hairline (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_set_line_cap (cairo_gstate_t *gstate, cairo_line_cap_t line_cap);

cairo_private cairo_line_cap_t
_cairo_gstate_get_line_cap (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_set_line_join (cairo_gstate_t *gstate, cairo_line_join_t line_join);

cairo_private cairo_line_join_t
_cairo_gstate_get_line_join (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_set_dash (cairo_gstate_t *gstate, const double *dash, int num_dashes, double offset);

cairo_private void
_cairo_gstate_get_dash (cairo_gstate_t *gstate, double *dash, int *num_dashes, double *offset);

cairo_private cairo_status_t
_cairo_gstate_set_miter_limit (cairo_gstate_t *gstate, double limit);

cairo_private double
_cairo_gstate_get_miter_limit (cairo_gstate_t *gstate);

cairo_private void
_cairo_gstate_get_matrix (cairo_gstate_t *gstate, cairo_matrix_t *matrix);

cairo_private cairo_status_t
_cairo_gstate_translate (cairo_gstate_t *gstate, double tx, double ty);

cairo_private cairo_status_t
_cairo_gstate_scale (cairo_gstate_t *gstate, double sx, double sy);

cairo_private cairo_status_t
_cairo_gstate_rotate (cairo_gstate_t *gstate, double angle);

cairo_private cairo_status_t
_cairo_gstate_transform (cairo_gstate_t	      *gstate,
			 const cairo_matrix_t *matrix);

cairo_private cairo_status_t
_cairo_gstate_set_matrix (cairo_gstate_t       *gstate,
			  const cairo_matrix_t *matrix);

cairo_private void
_cairo_gstate_identity_matrix (cairo_gstate_t *gstate);

cairo_private void
_cairo_gstate_user_to_device (cairo_gstate_t *gstate, double *x, double *y);

cairo_private void
_cairo_gstate_user_to_device_distance (cairo_gstate_t *gstate, double *dx, double *dy);

cairo_private void
_cairo_gstate_device_to_user (cairo_gstate_t *gstate, double *x, double *y);

cairo_private void
_cairo_gstate_device_to_user_distance (cairo_gstate_t *gstate, double *dx, double *dy);

cairo_private void
_do_cairo_gstate_user_to_backend (cairo_gstate_t *gstate, double *x, double *y);

static inline void
_cairo_gstate_user_to_backend (cairo_gstate_t *gstate, double *x, double *y)
{
    if (! gstate->is_identity)
	_do_cairo_gstate_user_to_backend (gstate, x, y);
}

cairo_private void
_do_cairo_gstate_user_to_backend_distance (cairo_gstate_t *gstate, double *x, double *y);

static inline void
_cairo_gstate_user_to_backend_distance (cairo_gstate_t *gstate, double *x, double *y)
{
    if (! gstate->is_identity)
	_do_cairo_gstate_user_to_backend_distance (gstate, x, y);
}

cairo_private void
_do_cairo_gstate_backend_to_user (cairo_gstate_t *gstate, double *x, double *y);

static inline void
_cairo_gstate_backend_to_user (cairo_gstate_t *gstate, double *x, double *y)
{
    if (! gstate->is_identity)
	_do_cairo_gstate_backend_to_user (gstate, x, y);
}

cairo_private void
_do_cairo_gstate_backend_to_user_distance (cairo_gstate_t *gstate, double *x, double *y);

static inline void
_cairo_gstate_backend_to_user_distance (cairo_gstate_t *gstate, double *x, double *y)
{
    if (! gstate->is_identity)
	_do_cairo_gstate_backend_to_user_distance (gstate, x, y);
}

cairo_private void
_cairo_gstate_backend_to_user_rectangle (cairo_gstate_t *gstate,
                                         double *x1, double *y1,
                                         double *x2, double *y2,
                                         cairo_bool_t *is_tight);

cairo_private void
_cairo_gstate_path_extents (cairo_gstate_t     *gstate,
			    cairo_path_fixed_t *path,
			    double *x1, double *y1,
			    double *x2, double *y2);

cairo_private cairo_status_t
_cairo_gstate_paint (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_mask (cairo_gstate_t  *gstate,
		    cairo_pattern_t *mask);

cairo_private cairo_status_t
_cairo_gstate_stroke (cairo_gstate_t *gstate, cairo_path_fixed_t *path);

cairo_private cairo_status_t
_cairo_gstate_fill (cairo_gstate_t *gstate, cairo_path_fixed_t *path);

cairo_private cairo_status_t
_cairo_gstate_copy_page (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_show_page (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_stroke_extents (cairo_gstate_t	 *gstate,
			      cairo_path_fixed_t *path,
                              double *x1, double *y1,
			      double *x2, double *y2);

cairo_private cairo_status_t
_cairo_gstate_fill_extents (cairo_gstate_t     *gstate,
			    cairo_path_fixed_t *path,
                            double *x1, double *y1,
			    double *x2, double *y2);

cairo_private cairo_status_t
_cairo_gstate_in_stroke (cairo_gstate_t	    *gstate,
			 cairo_path_fixed_t *path,
			 double		     x,
			 double		     y,
			 cairo_bool_t	    *inside_ret);

cairo_private cairo_bool_t
_cairo_gstate_in_fill (cairo_gstate_t	  *gstate,
		       cairo_path_fixed_t *path,
		       double		   x,
		       double		   y);

cairo_private cairo_bool_t
_cairo_gstate_in_clip (cairo_gstate_t	  *gstate,
		       double		   x,
		       double		   y);

cairo_private cairo_status_t
_cairo_gstate_clip (cairo_gstate_t *gstate, cairo_path_fixed_t *path);

cairo_private cairo_status_t
_cairo_gstate_reset_clip (cairo_gstate_t *gstate);

cairo_private cairo_bool_t
_cairo_gstate_clip_extents (cairo_gstate_t *gstate,
		            double         *x1,
		            double         *y1,
			    double         *x2,
			    double         *y2);

cairo_private cairo_rectangle_list_t*
_cairo_gstate_copy_clip_rectangle_list (cairo_gstate_t *gstate);

cairo_private cairo_status_t
_cairo_gstate_show_surface (cairo_gstate_t	*gstate,
			    cairo_surface_t	*surface,
			    double		 x,
			    double		 y,
			    double		width,
			    double		height);

cairo_private cairo_status_t
_cairo_gstate_tag_begin (cairo_gstate_t	*gstate,
			 const char     *tag_name,
			 const char     *attributes);

cairo_private cairo_status_t
_cairo_gstate_tag_end (cairo_gstate_t	*gstate,
		       const char       *tag_name);

cairo_private cairo_status_t
_cairo_gstate_set_font_size (cairo_gstate_t *gstate,
			     double          size);

cairo_private void
_cairo_gstate_get_font_matrix (cairo_gstate_t *gstate,
			       cairo_matrix_t *matrix);

cairo_private cairo_status_t
_cairo_gstate_set_font_matrix (cairo_gstate_t	    *gstate,
			       const cairo_matrix_t *matrix);

cairo_private void
_cairo_gstate_get_font_options (cairo_gstate_t       *gstate,
				cairo_font_options_t *options);

cairo_private void
_cairo_gstate_set_font_options (cairo_gstate_t	           *gstate,
				const cairo_font_options_t *options);

cairo_private cairo_status_t
_cairo_gstate_get_font_face (cairo_gstate_t     *gstate,
			     cairo_font_face_t **font_face);

cairo_private cairo_status_t
_cairo_gstate_get_scaled_font (cairo_gstate_t       *gstate,
			       cairo_scaled_font_t **scaled_font);

cairo_private cairo_status_t
_cairo_gstate_get_font_extents (cairo_gstate_t *gstate,
				cairo_font_extents_t *extents);

cairo_private cairo_status_t
_cairo_gstate_set_font_face (cairo_gstate_t    *gstate,
			     cairo_font_face_t *font_face);

cairo_private cairo_status_t
_cairo_gstate_glyph_extents (cairo_gstate_t *gstate,
			     const cairo_glyph_t *glyphs,
			     int num_glyphs,
			     cairo_text_extents_t *extents);

cairo_private cairo_status_t
_cairo_gstate_show_text_glyphs (cairo_gstate_t		   *gstate,
				const cairo_glyph_t	   *glyphs,
				int			    num_glyphs,
				cairo_glyph_text_info_t    *info);

cairo_private cairo_status_t
_cairo_gstate_glyph_path (cairo_gstate_t      *gstate,
			  const cairo_glyph_t *glyphs,
			  int		       num_glyphs,
			  cairo_path_fixed_t  *path);

cairo_private cairo_status_t
_cairo_gstate_set_antialias (cairo_gstate_t *gstate,
			     cairo_antialias_t antialias);

cairo_private cairo_antialias_t
_cairo_gstate_get_antialias (cairo_gstate_t *gstate);

#endif /* CAIRO_GSTATE_PRIVATE_H */
