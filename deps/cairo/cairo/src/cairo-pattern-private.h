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

#ifndef CAIRO_PATTERN_PRIVATE_H
#define CAIRO_PATTERN_PRIVATE_H

#include "cairo-error-private.h"
#include "cairo-types-private.h"
#include "cairo-list-private.h"
#include "cairo-surface-private.h"

#include <stdio.h> /* FILE* */

CAIRO_BEGIN_DECLS

typedef struct _cairo_pattern_observer cairo_pattern_observer_t;

enum {
    CAIRO_PATTERN_NOTIFY_MATRIX = 0x1,
    CAIRO_PATTERN_NOTIFY_FILTER = 0x2,
    CAIRO_PATTERN_NOTIFY_EXTEND = 0x4,
    CAIRO_PATTERN_NOTIFY_OPACITY = 0x9,
};

struct _cairo_pattern_observer {
    void (*notify) (cairo_pattern_observer_t *,
		    cairo_pattern_t *pattern,
		    unsigned int flags);
    cairo_list_t link;
};

struct _cairo_pattern {
    cairo_reference_count_t	ref_count;
    cairo_status_t		status;
    cairo_user_data_array_t	user_data;
    cairo_list_t		observers;

    cairo_pattern_type_t	type;

    cairo_filter_t		filter;
    cairo_extend_t		extend;
    cairo_bool_t		has_component_alpha;
    cairo_bool_t		is_foreground_marker;

    cairo_matrix_t		matrix;
    double			opacity;
};

struct _cairo_solid_pattern {
    cairo_pattern_t base;
    cairo_color_t color;
};

typedef struct _cairo_surface_pattern {
    cairo_pattern_t base;

    cairo_surface_t *surface;

    /* This field is only used by the wrapper surface for retreiving
     * the region id from the target during create regions and passing
     * the region id to the target surface during playback.
     */
    unsigned int region_array_id;
} cairo_surface_pattern_t;

typedef struct _cairo_gradient_stop {
    double offset;
    cairo_color_stop_t color;
} cairo_gradient_stop_t;

typedef struct _cairo_gradient_pattern {
    cairo_pattern_t base;

    unsigned int	    n_stops;
    unsigned int	    stops_size;
    cairo_gradient_stop_t  *stops;
    cairo_gradient_stop_t   stops_embedded[2];
} cairo_gradient_pattern_t;

typedef struct _cairo_linear_pattern {
    cairo_gradient_pattern_t base;

    cairo_point_double_t pd1;
    cairo_point_double_t pd2;
} cairo_linear_pattern_t;

typedef struct _cairo_radial_pattern {
    cairo_gradient_pattern_t base;

    cairo_circle_double_t cd1;
    cairo_circle_double_t cd2;
} cairo_radial_pattern_t;

typedef union {
    cairo_gradient_pattern_t base;

    cairo_linear_pattern_t linear;
    cairo_radial_pattern_t radial;
} cairo_gradient_pattern_union_t;

/*
 * A mesh patch is a tensor-product patch (bicubic Bezier surface
 * patch). It has 16 control points. Each set of 4 points along the
 * sides of the 4x4 grid of control points is a Bezier curve that
 * defines one side of the patch. A color is assigned to each
 * corner. The inner 4 points provide additional control over the
 * shape and the color mapping.
 *
 * Cairo uses the same convention as the PDF Reference for numbering
 * the points and side of the patch.
 *
 *
 *                      Side 1
 *
 *          p[0][3] p[1][3] p[2][3] p[3][3]
 * Side 0   p[0][2] p[1][2] p[2][2] p[3][2]  Side 2
 *          p[0][1] p[1][1] p[2][1] p[3][1]
 *          p[0][0] p[1][0] p[2][0] p[3][0]
 *
 *                      Side 3
 *
 *
 *   Point            Color
 *  -------------------------
 *  points[0][0]    colors[0]
 *  points[0][3]    colors[1]
 *  points[3][3]    colors[2]
 *  points[3][0]    colors[3]
 */

typedef struct _cairo_mesh_patch {
    cairo_point_double_t points[4][4];
    cairo_color_t colors[4];
} cairo_mesh_patch_t;

typedef struct _cairo_mesh_pattern {
    cairo_pattern_t base;

    cairo_array_t patches;
    cairo_mesh_patch_t *current_patch;
    int current_side;
    cairo_bool_t has_control_point[4];
    cairo_bool_t has_color[4];
} cairo_mesh_pattern_t;

typedef struct _cairo_raster_source_pattern {
    cairo_pattern_t base;

    cairo_content_t content;
    cairo_rectangle_int_t extents;

    cairo_raster_source_acquire_func_t acquire;
    cairo_raster_source_release_func_t release;
    cairo_raster_source_snapshot_func_t snapshot;
    cairo_raster_source_copy_func_t copy;
    cairo_raster_source_finish_func_t finish;

    /* an explicit pre-allocated member in preference to the general user-data */
    void *user_data;
} cairo_raster_source_pattern_t;

typedef union {
    cairo_pattern_t		    base;

    cairo_solid_pattern_t	    solid;
    cairo_surface_pattern_t	    surface;
    cairo_gradient_pattern_union_t  gradient;
    cairo_mesh_pattern_t	    mesh;
    cairo_raster_source_pattern_t   raster_source;
} cairo_pattern_union_t;

/* cairo-pattern.c */

cairo_private cairo_pattern_t *
_cairo_pattern_create_in_error (cairo_status_t status);

cairo_private cairo_status_t
_cairo_pattern_create_copy (cairo_pattern_t	  **pattern,
			    const cairo_pattern_t  *other);

cairo_private void
_cairo_pattern_init (cairo_pattern_t *pattern,
		     cairo_pattern_type_t type);

cairo_private cairo_status_t
_cairo_pattern_init_copy (cairo_pattern_t	*pattern,
			  const cairo_pattern_t *other);

cairo_private void
_cairo_pattern_init_static_copy (cairo_pattern_t	*pattern,
				 const cairo_pattern_t *other);

cairo_private cairo_status_t
_cairo_pattern_init_snapshot (cairo_pattern_t       *pattern,
			      const cairo_pattern_t *other);

cairo_private void
_cairo_pattern_init_solid (cairo_solid_pattern_t	*pattern,
			   const cairo_color_t		*color);

cairo_private void
_cairo_pattern_init_for_surface (cairo_surface_pattern_t *pattern,
				 cairo_surface_t *surface);

cairo_private void
_cairo_pattern_fini (cairo_pattern_t *pattern);

cairo_private cairo_pattern_t *
_cairo_pattern_create_solid (const cairo_color_t	*color);

cairo_private cairo_pattern_t *
_cairo_pattern_create_foreground_marker (void);

cairo_private void
_cairo_pattern_transform (cairo_pattern_t      *pattern,
			  const cairo_matrix_t *ctm_inverse);

cairo_private void
_cairo_pattern_pretransform (cairo_pattern_t      *pattern,
                             const cairo_matrix_t *ctm);

cairo_private cairo_bool_t
_cairo_pattern_is_opaque_solid (const cairo_pattern_t *pattern);

cairo_private cairo_bool_t
_cairo_pattern_is_opaque (const cairo_pattern_t *pattern,
			  const cairo_rectangle_int_t *extents);

cairo_private cairo_bool_t
_cairo_pattern_is_clear (const cairo_pattern_t *pattern);

cairo_private cairo_bool_t
_cairo_gradient_pattern_is_solid (const cairo_gradient_pattern_t *gradient,
				  const cairo_rectangle_int_t *extents,
				  cairo_color_t *color);

cairo_private cairo_bool_t
_cairo_pattern_is_constant_alpha (const cairo_pattern_t          *abstract_pattern,
				  const cairo_rectangle_int_t    *extents,
				  double                         *alpha);

cairo_private void
_cairo_gradient_pattern_fit_to_range (const cairo_gradient_pattern_t *gradient,
				      double			      max_value,
				      cairo_matrix_t                 *out_matrix,
				      cairo_circle_double_t	      out_circle[2]);

cairo_private cairo_bool_t
_cairo_radial_pattern_focus_is_inside (const cairo_radial_pattern_t *radial);

cairo_private void
_cairo_gradient_pattern_box_to_parameter (const cairo_gradient_pattern_t *gradient,
					  double x0, double y0,
					  double x1, double y1,
					  double tolerance,
					  double out_range[2]);

cairo_private void
_cairo_gradient_pattern_interpolate (const cairo_gradient_pattern_t *gradient,
				     double			     t,
				     cairo_circle_double_t	    *out_circle);

cairo_private void
_cairo_pattern_alpha_range (const cairo_pattern_t *pattern,
			    double                *out_min,
			    double                *out_max);

cairo_private cairo_bool_t
_cairo_mesh_pattern_coord_box (const cairo_mesh_pattern_t *mesh,
			       double                     *out_xmin,
			       double                     *out_ymin,
			       double                     *out_xmax,
			       double                     *out_ymax);

cairo_private void
_cairo_pattern_sampled_area (const cairo_pattern_t *pattern,
			     const cairo_rectangle_int_t *extents,
			     cairo_rectangle_int_t *sample);

cairo_private void
_cairo_pattern_get_extents (const cairo_pattern_t	    *pattern,
			    cairo_rectangle_int_t           *extents,
			    cairo_bool_t                   is_vector);

cairo_private cairo_int_status_t
_cairo_pattern_get_ink_extents (const cairo_pattern_t	    *pattern,
				cairo_rectangle_int_t       *extents);

cairo_private uintptr_t
_cairo_pattern_hash (const cairo_pattern_t *pattern);

cairo_private uintptr_t
_cairo_linear_pattern_hash (uintptr_t hash,
			    const cairo_linear_pattern_t *linear);

cairo_private uintptr_t
_cairo_radial_pattern_hash (uintptr_t hash,
			    const cairo_radial_pattern_t *radial);

cairo_private cairo_bool_t
_cairo_linear_pattern_equal (const cairo_linear_pattern_t *a,
			     const cairo_linear_pattern_t *b);

cairo_private unsigned long
_cairo_pattern_size (const cairo_pattern_t *pattern);

cairo_private cairo_bool_t
_cairo_radial_pattern_equal (const cairo_radial_pattern_t *a,
			     const cairo_radial_pattern_t *b);

cairo_private cairo_bool_t
_cairo_pattern_equal (const cairo_pattern_t *a,
		      const cairo_pattern_t *b);

cairo_private cairo_filter_t
_cairo_pattern_analyze_filter (const cairo_pattern_t *pattern);

/* cairo-mesh-pattern-rasterizer.c */

cairo_private void
_cairo_mesh_pattern_rasterize (const cairo_mesh_pattern_t *mesh,
			       void                       *data,
			       int                         width,
			       int                         height,
			       int                         stride,
			       double                      x_offset,
			       double                      y_offset);

cairo_private cairo_surface_t *
_cairo_raster_source_pattern_acquire (const cairo_pattern_t *abstract_pattern,
				      cairo_surface_t *target,
				      const cairo_rectangle_int_t *extents);

cairo_private void
_cairo_raster_source_pattern_release (const cairo_pattern_t *abstract_pattern,
				      cairo_surface_t *surface);

cairo_private cairo_status_t
_cairo_raster_source_pattern_snapshot (cairo_pattern_t *abstract_pattern);

cairo_private cairo_status_t
_cairo_raster_source_pattern_init_copy (cairo_pattern_t *pattern,
					const cairo_pattern_t *other);

cairo_private void
_cairo_raster_source_pattern_finish (cairo_pattern_t *abstract_pattern);

cairo_private void
_cairo_debug_print_pattern (FILE *file, const cairo_pattern_t *pattern);

CAIRO_END_DECLS

#endif /* CAIRO_PATTERN_PRIVATE */
