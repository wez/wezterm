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

#ifndef CAIRO_CONTOUR_INLINE_H
#define CAIRO_CONTOUR_INLINE_H

#include "cairo-contour-private.h"

CAIRO_BEGIN_DECLS

static inline cairo_int_status_t
_cairo_contour_add_point (cairo_contour_t *contour,
			  const cairo_point_t *point)
{
    struct _cairo_contour_chain *tail = contour->tail;

    if (unlikely (tail->num_points == tail->size_points))
	return __cairo_contour_add_point (contour, point);

    tail->points[tail->num_points++] = *point;
    return CAIRO_INT_STATUS_SUCCESS;
}

static inline cairo_point_t *
_cairo_contour_first_point (cairo_contour_t *c)
{
    return &c->chain.points[0];
}

static inline cairo_point_t *
_cairo_contour_last_point (cairo_contour_t *c)
{
    return &c->tail->points[c->tail->num_points-1];
}

static inline void
_cairo_contour_remove_last_point (cairo_contour_t *contour)
{
    if (contour->chain.num_points == 0)
	return;

    if (--contour->tail->num_points == 0)
	__cairo_contour_remove_last_chain (contour);
}

CAIRO_END_DECLS

#endif /* CAIRO_CONTOUR_INLINE_H */
