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

#ifndef CAIRO_TRAPS_PRIVATE_H
#define CAIRO_TRAPS_PRIVATE_H

#include "cairo-compiler-private.h"
#include "cairo-error-private.h"
#include "cairo-types-private.h"

CAIRO_BEGIN_DECLS

struct _cairo_traps {
    cairo_status_t status;

    cairo_box_t bounds;
    const cairo_box_t *limits;
    int num_limits;

    unsigned int maybe_region : 1; /* hint: 0 implies that it cannot be */
    unsigned int has_intersections : 1;
    unsigned int is_rectilinear : 1;
    unsigned int is_rectangular : 1;

    int num_traps;
    int traps_size;
    cairo_trapezoid_t *traps;
    cairo_trapezoid_t  traps_embedded[16];
};

/* cairo-traps.c */
cairo_private void
_cairo_traps_init (cairo_traps_t *traps);

cairo_private void
_cairo_traps_init_with_clip (cairo_traps_t *traps,
			     const cairo_clip_t *clip);

cairo_private void
_cairo_traps_limit (cairo_traps_t	*traps,
		    const cairo_box_t	*boxes,
		    int			 num_boxes);

cairo_private cairo_status_t
_cairo_traps_init_boxes (cairo_traps_t	    *traps,
		         const cairo_boxes_t *boxes);

cairo_private void
_cairo_traps_clear (cairo_traps_t *traps);

cairo_private void
_cairo_traps_fini (cairo_traps_t *traps);

#define _cairo_traps_status(T) (T)->status

cairo_private void
_cairo_traps_translate (cairo_traps_t *traps, int x, int y);

cairo_private void
_cairo_traps_tessellate_triangle_with_edges (cairo_traps_t *traps,
					     const cairo_point_t t[3],
					     const cairo_point_t edges[4]);

cairo_private void
_cairo_traps_tessellate_convex_quad (cairo_traps_t *traps,
				     const cairo_point_t q[4]);

cairo_private cairo_status_t
_cairo_traps_tessellate_rectangle (cairo_traps_t *traps,
				   const cairo_point_t *top_left,
				   const cairo_point_t *bottom_right);

cairo_private void
_cairo_traps_add_trap (cairo_traps_t *traps,
		       cairo_fixed_t top, cairo_fixed_t bottom,
		       const cairo_line_t *left,
		       const cairo_line_t *right);

cairo_private int
_cairo_traps_contain (const cairo_traps_t *traps,
		      double x, double y);

cairo_private void
_cairo_traps_extents (const cairo_traps_t *traps,
		      cairo_box_t         *extents);

cairo_private cairo_int_status_t
_cairo_traps_extract_region (cairo_traps_t  *traps,
			     cairo_antialias_t antialias,
			     cairo_region_t **region);

cairo_private cairo_bool_t
_cairo_traps_to_boxes (cairo_traps_t *traps,
		       cairo_antialias_t antialias,
		       cairo_boxes_t *boxes);

cairo_private cairo_status_t
_cairo_traps_path (const cairo_traps_t *traps,
		   cairo_path_fixed_t  *path);

cairo_private cairo_int_status_t
_cairo_rasterise_polygon_to_traps (cairo_polygon_t			*polygon,
				   cairo_fill_rule_t			 fill_rule,
				   cairo_antialias_t			 antialias,
				   cairo_traps_t *traps);

CAIRO_END_DECLS

#endif /* CAIRO_TRAPS_PRIVATE_H */
