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
 *	Kristian Høgsberg <krh@redhat.com>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_CLIP_PRIVATE_H
#define CAIRO_CLIP_PRIVATE_H

#include "cairo-types-private.h"

#include "cairo-boxes-private.h"
#include "cairo-error-private.h"
#include "cairo-compiler-private.h"
#include "cairo-error-private.h"
#include "cairo-path-fixed-private.h"
#include "cairo-reference-count-private.h"

extern const cairo_private cairo_rectangle_list_t _cairo_rectangles_nil;

struct _cairo_clip_path {
    cairo_reference_count_t	 ref_count;
    cairo_path_fixed_t		 path;
    cairo_fill_rule_t		 fill_rule;
    double			 tolerance;
    cairo_antialias_t		 antialias;
    cairo_clip_path_t		*prev;
};

struct _cairo_clip {
    cairo_rectangle_int_t extents;
    cairo_clip_path_t *path;

    cairo_box_t *boxes;
    int num_boxes;

    cairo_region_t *region;
    cairo_bool_t is_region;

    cairo_box_t embedded_box;
};

cairo_private cairo_clip_t *
_cairo_clip_create (void);

cairo_private cairo_clip_path_t *
_cairo_clip_path_reference (cairo_clip_path_t *clip_path);

cairo_private void
_cairo_clip_path_destroy (cairo_clip_path_t *clip_path);

cairo_private void
_cairo_clip_destroy (cairo_clip_t *clip);

cairo_private extern const cairo_clip_t __cairo_clip_all;

cairo_private cairo_clip_t *
_cairo_clip_copy (const cairo_clip_t *clip);

cairo_private cairo_clip_t *
_cairo_clip_copy_region (const cairo_clip_t *clip);

cairo_private cairo_clip_t *
_cairo_clip_copy_path (const cairo_clip_t *clip);

cairo_private cairo_clip_t *
_cairo_clip_translate (cairo_clip_t *clip, int tx, int ty);

cairo_private cairo_clip_t *
_cairo_clip_transform (cairo_clip_t *clip, const cairo_matrix_t *m);

cairo_private cairo_clip_t *
_cairo_clip_copy_with_translation (const cairo_clip_t *clip, int tx, int ty);

cairo_private cairo_bool_t
_cairo_clip_equal (const cairo_clip_t *clip_a,
		   const cairo_clip_t *clip_b);

cairo_private cairo_clip_t *
_cairo_clip_intersect_rectangle (cairo_clip_t       *clip,
				 const cairo_rectangle_int_t *rectangle);

cairo_private cairo_clip_t *
_cairo_clip_intersect_clip (cairo_clip_t *clip,
			    const cairo_clip_t *other);

cairo_private cairo_clip_t *
_cairo_clip_intersect_box (cairo_clip_t       *clip,
			   const cairo_box_t *box);

cairo_private cairo_clip_t *
_cairo_clip_intersect_boxes (cairo_clip_t *clip,
			     const cairo_boxes_t *boxes);

cairo_private cairo_clip_t *
_cairo_clip_intersect_rectilinear_path (cairo_clip_t       *clip,
					const cairo_path_fixed_t *path,
					cairo_fill_rule_t   fill_rule,
					cairo_antialias_t   antialias);

cairo_private cairo_clip_t *
_cairo_clip_intersect_path (cairo_clip_t       *clip,
			    const cairo_path_fixed_t *path,
			    cairo_fill_rule_t   fill_rule,
			    double              tolerance,
			    cairo_antialias_t   antialias);

cairo_private const cairo_rectangle_int_t *
_cairo_clip_get_extents (const cairo_clip_t *clip);

cairo_private cairo_surface_t *
_cairo_clip_get_surface (const cairo_clip_t *clip, cairo_surface_t *dst, int *tx, int *ty);

cairo_private cairo_surface_t *
_cairo_clip_get_image (const cairo_clip_t *clip,
		       cairo_surface_t *target,
		       const cairo_rectangle_int_t *extents);

cairo_private cairo_status_t
_cairo_clip_combine_with_surface (const cairo_clip_t *clip,
				  cairo_surface_t *dst,
				  int dst_x, int dst_y);

cairo_private cairo_clip_t *
_cairo_clip_from_boxes (const cairo_boxes_t *boxes);

cairo_private cairo_region_t *
_cairo_clip_get_region (const cairo_clip_t *clip);

cairo_private cairo_bool_t
_cairo_clip_is_region (const cairo_clip_t *clip);

cairo_private cairo_clip_t *
_cairo_clip_reduce_to_rectangle (const cairo_clip_t *clip,
				 const cairo_rectangle_int_t *r);

cairo_private cairo_clip_t *
_cairo_clip_reduce_for_composite (const cairo_clip_t *clip,
				  cairo_composite_rectangles_t *extents);

cairo_private cairo_bool_t
_cairo_clip_contains_rectangle (const cairo_clip_t *clip,
				const cairo_rectangle_int_t *rect);

cairo_private cairo_bool_t
_cairo_clip_contains_box (const cairo_clip_t *clip,
			  const cairo_box_t *box);

cairo_private cairo_bool_t
_cairo_clip_contains_extents (const cairo_clip_t *clip,
			      const cairo_composite_rectangles_t *extents);

cairo_private cairo_rectangle_list_t*
_cairo_clip_copy_rectangle_list (cairo_clip_t *clip, cairo_gstate_t *gstate);

cairo_private cairo_rectangle_list_t *
_cairo_rectangle_list_create_in_error (cairo_status_t status);

cairo_private cairo_bool_t
_cairo_clip_is_polygon (const cairo_clip_t *clip);

cairo_private cairo_int_status_t
_cairo_clip_get_polygon (const cairo_clip_t *clip,
			 cairo_polygon_t *polygon,
			 cairo_fill_rule_t *fill_rule,
			 cairo_antialias_t *antialias);

#endif /* CAIRO_CLIP_PRIVATE_H */
