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
 *	Chris Wilson <chris@chris-wilson.co.u>
 */

#ifndef CAIRO_SURFACE_WRAPPER_PRIVATE_H
#define CAIRO_SURFACE_WRAPPER_PRIVATE_H

#include "cairoint.h"
#include "cairo-types-private.h"
#include "cairo-recording-surface-private.h"
#include "cairo-surface-backend-private.h"

CAIRO_BEGIN_DECLS

struct _cairo_surface_wrapper {
    cairo_surface_t *target;

    cairo_matrix_t transform;

    cairo_bool_t has_extents;
    cairo_rectangle_int_t extents;
    const cairo_clip_t *clip;

    unsigned int source_region_id;
    unsigned int mask_region_id;

    cairo_bool_t needs_transform;
};

cairo_private void
_cairo_surface_wrapper_init (cairo_surface_wrapper_t *wrapper,
			     cairo_surface_t *target);

cairo_private void
_cairo_surface_wrapper_intersect_extents (cairo_surface_wrapper_t *wrapper,
					  const cairo_rectangle_int_t *extents);

cairo_private void
_cairo_surface_wrapper_set_inverse_transform (cairo_surface_wrapper_t *wrapper,
					      const cairo_matrix_t *transform);

cairo_private void
_cairo_surface_wrapper_set_clip (cairo_surface_wrapper_t *wrapper,
				 const cairo_clip_t *clip);

cairo_private void
_cairo_surface_wrapper_fini (cairo_surface_wrapper_t *wrapper);

static inline cairo_bool_t
_cairo_surface_wrapper_has_fill_stroke (cairo_surface_wrapper_t *wrapper)
{
    return wrapper->target->backend->fill_stroke != NULL;
}

cairo_private cairo_status_t
_cairo_surface_wrapper_acquire_source_image (cairo_surface_wrapper_t *wrapper,
					     cairo_image_surface_t  **image_out,
					     void                   **image_extra);

cairo_private void
_cairo_surface_wrapper_release_source_image (cairo_surface_wrapper_t *wrapper,
					     cairo_image_surface_t  *image,
					     void                   *image_extra);


cairo_private cairo_status_t
_cairo_surface_wrapper_paint (cairo_surface_wrapper_t *wrapper,
			      cairo_operator_t	       op,
			      const cairo_pattern_t   *source,
			      unsigned int             source_region_id,
			      const cairo_clip_t      *clip);

cairo_private cairo_status_t
_cairo_surface_wrapper_mask (cairo_surface_wrapper_t *wrapper,
			     cairo_operator_t	      op,
			     const cairo_pattern_t   *source,
			     unsigned int             source_region_id,
			     const cairo_pattern_t   *mask,
                             unsigned int             mask_region_id,
			     const cairo_clip_t	     *clip);

cairo_private cairo_status_t
_cairo_surface_wrapper_stroke (cairo_surface_wrapper_t    *wrapper,
			       cairo_operator_t		   op,
			       const cairo_pattern_t	  *source,
			       unsigned int                source_region_id,
			       const cairo_path_fixed_t	  *path,
			       const cairo_stroke_style_t *stroke_style,
			       const cairo_matrix_t	  *ctm,
			       const cairo_matrix_t	  *ctm_inverse,
			       double			   tolerance,
			       cairo_antialias_t	   antialias,
			       const cairo_clip_t	  *clip);

cairo_private cairo_status_t
_cairo_surface_wrapper_fill_stroke (cairo_surface_wrapper_t    *wrapper,
				    cairo_operator_t	        fill_op,
				    const cairo_pattern_t      *fill_source,
				    unsigned int                fill_region_id,
				    cairo_fill_rule_t	        fill_rule,
				    double		        fill_tolerance,
				    cairo_antialias_t	        fill_antialias,
				    const cairo_path_fixed_t   *path,
				    cairo_operator_t	        stroke_op,
				    const cairo_pattern_t      *stroke_source,
				    unsigned int                stroke_region_id,
				    const cairo_stroke_style_t *stroke_style,
				    const cairo_matrix_t       *stroke_ctm,
				    const cairo_matrix_t       *stroke_ctm_inverse,
				    double		        stroke_tolerance,
				    cairo_antialias_t	        stroke_antialias,
				    const cairo_clip_t	       *clip);

cairo_private cairo_status_t
_cairo_surface_wrapper_fill (cairo_surface_wrapper_t  *wrapper,
			     cairo_operator_t	       op,
			     const cairo_pattern_t    *source,
			     unsigned int              source_region_id,
			     const cairo_path_fixed_t *path,
			     cairo_fill_rule_t	       fill_rule,
			     double		       tolerance,
			     cairo_antialias_t	       antialias,
			     const cairo_clip_t	      *clip);

cairo_private cairo_status_t
_cairo_surface_wrapper_show_text_glyphs (cairo_surface_wrapper_t     *wrapper,
					 cairo_operator_t	     op,
					 const cairo_pattern_t	    *source,
					 unsigned int                source_region_id,
					 const char		    *utf8,
					 int			     utf8_len,
					 const cairo_glyph_t	    *glyphs,
					 int			     num_glyphs,
					 const cairo_text_cluster_t *clusters,
					 int			     num_clusters,
					 cairo_text_cluster_flags_t  cluster_flags,
					 cairo_scaled_font_t	    *scaled_font,
					 const cairo_clip_t	    *clip);

cairo_private cairo_status_t
_cairo_surface_wrapper_tag (cairo_surface_wrapper_t     *wrapper,
			    cairo_bool_t                 begin,
			    const char                  *tag_name,
			    const char                  *attributes);

cairo_private cairo_surface_t *
_cairo_surface_wrapper_create_similar (cairo_surface_wrapper_t *wrapper,
				       cairo_content_t	content,
				       int		width,
				       int		height);
cairo_private cairo_bool_t
_cairo_surface_wrapper_get_extents (cairo_surface_wrapper_t *wrapper,
				    cairo_rectangle_int_t   *extents);

cairo_private void
_cairo_surface_wrapper_get_font_options (cairo_surface_wrapper_t    *wrapper,
					 cairo_font_options_t	    *options);

cairo_private cairo_surface_t *
_cairo_surface_wrapper_snapshot (cairo_surface_wrapper_t *wrapper);

cairo_private cairo_bool_t
_cairo_surface_wrapper_has_show_text_glyphs (cairo_surface_wrapper_t *wrapper);

static inline cairo_bool_t
_cairo_surface_wrapper_is_active (cairo_surface_wrapper_t *wrapper)
{
    return wrapper->target != (cairo_surface_t *) 0;
}

cairo_private cairo_bool_t
_cairo_surface_wrapper_get_target_extents (cairo_surface_wrapper_t *wrapper,
					   cairo_bool_t surface_is_unbounded,
					   cairo_rectangle_int_t *extents);

CAIRO_END_DECLS

#endif /* CAIRO_SURFACE_WRAPPER_PRIVATE_H */
