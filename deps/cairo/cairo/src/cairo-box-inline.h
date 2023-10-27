/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2010 Andrea Canciani
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
 *	Andrea Canciani <ranma42@gmail.com>
 */

#ifndef CAIRO_BOX_H
#define CAIRO_BOX_H

#include "cairo-types-private.h"
#include "cairo-compiler-private.h"
#include "cairo-fixed-private.h"

static inline void
_cairo_box_set (cairo_box_t *box,
		const cairo_point_t *p1,
		const cairo_point_t *p2)
{
    box->p1 = *p1;
    box->p2 = *p2;
}

static inline void
_cairo_box_from_integers (cairo_box_t *box, int x, int y, int w, int h)
{
    box->p1.x = _cairo_fixed_from_int (x);
    box->p1.y = _cairo_fixed_from_int (y);
    box->p2.x = _cairo_fixed_from_int (x + w);
    box->p2.y = _cairo_fixed_from_int (y + h);
}

static inline void
_cairo_box_from_rectangle_int (cairo_box_t *box,
			       const cairo_rectangle_int_t *rect)
{
    box->p1.x = _cairo_fixed_from_int (rect->x);
    box->p1.y = _cairo_fixed_from_int (rect->y);
    box->p2.x = _cairo_fixed_from_int (rect->x + rect->width);
    box->p2.y = _cairo_fixed_from_int (rect->y + rect->height);
}

/* assumes box->p1 is top-left, p2 bottom-right */
static inline void
_cairo_box_add_point (cairo_box_t *box,
		      const cairo_point_t *point)
{
    if (point->x < box->p1.x)
	box->p1.x = point->x;
    else if (point->x > box->p2.x)
	box->p2.x = point->x;

    if (point->y < box->p1.y)
	box->p1.y = point->y;
    else if (point->y > box->p2.y)
	box->p2.y = point->y;
}

static inline void
_cairo_box_add_box (cairo_box_t *box,
		    const cairo_box_t *add)
{
    if (add->p1.x < box->p1.x)
	box->p1.x = add->p1.x;
    if (add->p2.x > box->p2.x)
	box->p2.x = add->p2.x;

    if (add->p1.y < box->p1.y)
	box->p1.y = add->p1.y;
    if (add->p2.y > box->p2.y)
	box->p2.y = add->p2.y;
}

/* assumes box->p1 is top-left, p2 bottom-right */
static inline cairo_bool_t
_cairo_box_contains_point (const cairo_box_t *box,
			   const cairo_point_t *point)
{
    return box->p1.x <= point->x  && point->x <= box->p2.x &&
	box->p1.y <= point->y  && point->y <= box->p2.y;
}

static inline cairo_bool_t
_cairo_box_is_pixel_aligned (const cairo_box_t *box)
{
#if CAIRO_FIXED_FRAC_BITS <= 8 && 0
    return ((cairo_fixed_unsigned_t)(box->p1.x & CAIRO_FIXED_FRAC_MASK) << 24 |
	    (box->p1.y & CAIRO_FIXED_FRAC_MASK) << 16 |
	    (box->p2.x & CAIRO_FIXED_FRAC_MASK) << 8 |
	    (box->p2.y & CAIRO_FIXED_FRAC_MASK) << 0) == 0;
#else /* GCC on i7 prefers this variant (bizarrely according to the profiler) */
    cairo_fixed_t f;

    f = 0;
    f |= box->p1.x & CAIRO_FIXED_FRAC_MASK;
    f |= box->p1.y & CAIRO_FIXED_FRAC_MASK;
    f |= box->p2.x & CAIRO_FIXED_FRAC_MASK;
    f |= box->p2.y & CAIRO_FIXED_FRAC_MASK;

    return f == 0;
#endif
}

#endif /* CAIRO_BOX_H */
