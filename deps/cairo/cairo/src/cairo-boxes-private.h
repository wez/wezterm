/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2009 Intel Corporation
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
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_BOXES_H
#define CAIRO_BOXES_H

#include "cairo-types-private.h"
#include "cairo-compiler-private.h"

#include <stdio.h>
#include <stdlib.h>

struct _cairo_boxes_t {
    cairo_status_t status;

    cairo_box_t limit;
    const cairo_box_t *limits;
    int num_limits;

    int num_boxes;

    unsigned int is_pixel_aligned;

    struct _cairo_boxes_chunk {
	struct _cairo_boxes_chunk *next;
	cairo_box_t *base;
	int count;
	int size;
    } chunks, *tail;
    cairo_box_t boxes_embedded[32];
};

cairo_private void
_cairo_boxes_init (cairo_boxes_t *boxes);

cairo_private void
_cairo_boxes_init_with_clip (cairo_boxes_t *boxes,
			     cairo_clip_t *clip);

cairo_private void
_cairo_boxes_init_for_array (cairo_boxes_t *boxes,
			     cairo_box_t *array,
			     int num_boxes);

cairo_private void
_cairo_boxes_init_from_rectangle (cairo_boxes_t *boxes,
				  int x, int y, int w, int h);

cairo_private void
_cairo_boxes_limit (cairo_boxes_t	*boxes,
		    const cairo_box_t	*limits,
		    int			 num_limits);

cairo_private cairo_status_t
_cairo_boxes_add (cairo_boxes_t *boxes,
		  cairo_antialias_t antialias,
		  const cairo_box_t *box);

cairo_private void
_cairo_boxes_extents (const cairo_boxes_t *boxes,
		      cairo_box_t *box);

cairo_private cairo_box_t *
_cairo_boxes_to_array (const cairo_boxes_t *boxes,
		       int *num_boxes);

cairo_private cairo_status_t
_cairo_boxes_intersect (const cairo_boxes_t *a,
			const cairo_boxes_t *b,
			cairo_boxes_t *out);

cairo_private void
_cairo_boxes_clear (cairo_boxes_t *boxes);

cairo_private_no_warn cairo_bool_t
_cairo_boxes_for_each_box (cairo_boxes_t *boxes,
			   cairo_bool_t (*func) (cairo_box_t *box, void *data),
			   void *data);

cairo_private cairo_status_t
_cairo_rasterise_polygon_to_boxes (cairo_polygon_t			*polygon,
				   cairo_fill_rule_t			 fill_rule,
				   cairo_boxes_t *boxes);

cairo_private void
_cairo_boxes_fini (cairo_boxes_t *boxes);

cairo_private void
_cairo_debug_print_boxes (FILE *stream,
			  const cairo_boxes_t *boxes);

#endif /* CAIRO_BOXES_H */
