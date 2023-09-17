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

#ifndef CAIRO_PATH_FIXED_PRIVATE_H
#define CAIRO_PATH_FIXED_PRIVATE_H

#include "cairo-types-private.h"
#include "cairo-compiler-private.h"
#include "cairo-list-private.h"

#define WATCH_PATH 0
#if WATCH_PATH
#include <stdio.h>
#endif

enum cairo_path_op {
    CAIRO_PATH_OP_MOVE_TO = 0,
    CAIRO_PATH_OP_LINE_TO = 1,
    CAIRO_PATH_OP_CURVE_TO = 2,
    CAIRO_PATH_OP_CLOSE_PATH = 3
};

/* we want to make sure a single byte is used for the enum */
typedef char cairo_path_op_t;

/* make _cairo_path_fixed fit into ~512 bytes -- about 50 items */
#define CAIRO_PATH_BUF_SIZE ((512 - sizeof (cairo_path_buf_t)) \
			   / (2 * sizeof (cairo_point_t) + sizeof (cairo_path_op_t)))

#define cairo_path_head(path__) (&(path__)->buf.base)
#define cairo_path_tail(path__) cairo_path_buf_prev (cairo_path_head (path__))

#define cairo_path_buf_next(pos__) \
    cairo_list_entry ((pos__)->link.next, cairo_path_buf_t, link)
#define cairo_path_buf_prev(pos__) \
    cairo_list_entry ((pos__)->link.prev, cairo_path_buf_t, link)

#define cairo_path_foreach_buf_start(pos__, path__) \
    pos__ = cairo_path_head (path__); do
#define cairo_path_foreach_buf_end(pos__, path__) \
    while ((pos__ = cairo_path_buf_next (pos__)) !=  cairo_path_head (path__))


typedef struct _cairo_path_buf {
    cairo_list_t link;
    unsigned int num_ops;
    unsigned int size_ops;
    unsigned int num_points;
    unsigned int size_points;

    cairo_path_op_t *op;
    cairo_point_t *points;
} cairo_path_buf_t;

typedef struct _cairo_path_buf_fixed {
    cairo_path_buf_t base;

    cairo_path_op_t op[CAIRO_PATH_BUF_SIZE];
    cairo_point_t points[2 * CAIRO_PATH_BUF_SIZE];
} cairo_path_buf_fixed_t;

/*
  NOTES:
  has_curve_to => !stroke_is_rectilinear
  fill_is_rectilinear => stroke_is_rectilinear
  fill_is_empty => fill_is_rectilinear
  fill_maybe_region => fill_is_rectilinear
*/
struct _cairo_path_fixed {
    cairo_point_t last_move_point;
    cairo_point_t current_point;
    unsigned int has_current_point	: 1;
    unsigned int needs_move_to		: 1;
    unsigned int has_extents		: 1;
    unsigned int has_curve_to		: 1;
    unsigned int stroke_is_rectilinear	: 1;
    unsigned int fill_is_rectilinear	: 1;
    unsigned int fill_maybe_region	: 1;
    unsigned int fill_is_empty		: 1;

    cairo_box_t extents;

    cairo_path_buf_fixed_t  buf;
};

cairo_private void
_cairo_path_fixed_translate (cairo_path_fixed_t *path,
			     cairo_fixed_t offx,
			     cairo_fixed_t offy);

cairo_private cairo_status_t
_cairo_path_fixed_append (cairo_path_fixed_t		    *path,
			  const cairo_path_fixed_t	    *other,
			  cairo_fixed_t			     tx,
			  cairo_fixed_t			     ty);

cairo_private uintptr_t
_cairo_path_fixed_hash (const cairo_path_fixed_t *path);

cairo_private unsigned long
_cairo_path_fixed_size (const cairo_path_fixed_t *path);

cairo_private cairo_bool_t
_cairo_path_fixed_equal (const cairo_path_fixed_t *a,
			 const cairo_path_fixed_t *b);

typedef struct _cairo_path_fixed_iter {
    const cairo_path_buf_t *first;
    const cairo_path_buf_t *buf;
    unsigned int n_op;
    unsigned int n_point;
} cairo_path_fixed_iter_t;

cairo_private void
_cairo_path_fixed_iter_init (cairo_path_fixed_iter_t *iter,
			     const cairo_path_fixed_t *path);

cairo_private cairo_bool_t
_cairo_path_fixed_iter_is_fill_box (cairo_path_fixed_iter_t *_iter,
				    cairo_box_t *box);

cairo_private cairo_bool_t
_cairo_path_fixed_iter_at_end (const cairo_path_fixed_iter_t *iter);

static inline cairo_bool_t
_cairo_path_fixed_fill_is_empty (const cairo_path_fixed_t *path)
{
    return path->fill_is_empty;
}

static inline cairo_bool_t
_cairo_path_fixed_fill_is_rectilinear (const cairo_path_fixed_t *path)
{
    if (! path->fill_is_rectilinear)
	return 0;

    if (! path->has_current_point || path->needs_move_to)
	return 1;

    /* check whether the implicit close preserves the rectilinear property */
    return path->current_point.x == path->last_move_point.x ||
	   path->current_point.y == path->last_move_point.y;
}

static inline cairo_bool_t
_cairo_path_fixed_stroke_is_rectilinear (const cairo_path_fixed_t *path)
{
    return path->stroke_is_rectilinear;
}

static inline cairo_bool_t
_cairo_path_fixed_fill_maybe_region (const cairo_path_fixed_t *path)
{
    if (! path->fill_maybe_region)
	return 0;

    if (! path->has_current_point || path->needs_move_to)
	return 1;

    /* check whether the implicit close preserves the rectilinear property
     * (the integer point property is automatically preserved)
     */
    return path->current_point.x == path->last_move_point.x ||
	   path->current_point.y == path->last_move_point.y;
}

cairo_private cairo_bool_t
_cairo_path_fixed_is_stroke_box (const cairo_path_fixed_t *path,
				 cairo_box_t *box);

cairo_private cairo_bool_t
_cairo_path_fixed_is_simple_quad (const cairo_path_fixed_t *path);

#endif /* CAIRO_PATH_FIXED_PRIVATE_H */
