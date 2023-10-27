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
 * The Initial Developer of the Original Code is Intel Corporation
 *
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_CONTOUR_PRIVATE_H
#define CAIRO_CONTOUR_PRIVATE_H

#include "cairo-types-private.h"
#include "cairo-compiler-private.h"
#include "cairo-error-private.h"
#include "cairo-list-private.h"

#include <stdio.h>

CAIRO_BEGIN_DECLS

/* A contour is simply a closed chain of points that divide the infinite plane
 * into inside and outside. Each contour is a simple polygon, that is it
 * contains no holes or self-intersections, but maybe either concave or convex.
 */

struct _cairo_contour_chain {
    cairo_point_t *points;
    int num_points, size_points;
    struct _cairo_contour_chain *next;
};

struct _cairo_contour_iter {
    cairo_point_t *point;
    cairo_contour_chain_t *chain;
};

struct _cairo_contour {
    cairo_list_t next;
    int direction;
    cairo_contour_chain_t chain, *tail;

    cairo_point_t embedded_points[64];
};

/* Initial definition of a shape is a set of contours (some representing holes) */
struct _cairo_shape {
    cairo_list_t contours;
};

typedef struct _cairo_shape cairo_shape_t;

#if 0
cairo_private cairo_status_t
_cairo_shape_init_from_polygon (cairo_shape_t *shape,
				const cairo_polygon_t *polygon);

cairo_private cairo_status_t
_cairo_shape_reduce (cairo_shape_t *shape, double tolerance);
#endif

cairo_private void
_cairo_contour_init (cairo_contour_t *contour,
		     int direction);

cairo_private cairo_int_status_t
__cairo_contour_add_point (cairo_contour_t *contour,
			   const cairo_point_t *point);

cairo_private void
_cairo_contour_simplify (cairo_contour_t *contour, double tolerance);

cairo_private void
_cairo_contour_reverse (cairo_contour_t *contour);

cairo_private cairo_int_status_t
_cairo_contour_add (cairo_contour_t *dst,
		    const cairo_contour_t *src);

cairo_private cairo_int_status_t
_cairo_contour_add_reversed (cairo_contour_t *dst,
			     const cairo_contour_t *src);

cairo_private void
__cairo_contour_remove_last_chain (cairo_contour_t *contour);

cairo_private void
_cairo_contour_reset (cairo_contour_t *contour);

cairo_private void
_cairo_contour_fini (cairo_contour_t *contour);

cairo_private void
_cairo_debug_print_contour (FILE *file, cairo_contour_t *contour);

CAIRO_END_DECLS

#endif /* CAIRO_CONTOUR_PRIVATE_H */
