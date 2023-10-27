/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/*
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
 * The Initial Developer of the Original Code is Chris Wilson
 *
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilsonc.co.uk>
 */

#include "cairoint.h"

#include "cairo-error-private.h"
#include "cairo-tristrip-private.h"

void
_cairo_tristrip_init (cairo_tristrip_t *strip)
{
    VG (VALGRIND_MAKE_MEM_UNDEFINED (strip, sizeof (cairo_tristrip_t)));

    strip->status = CAIRO_STATUS_SUCCESS;

    strip->num_limits = 0;
    strip->num_points = 0;

    strip->size_points = ARRAY_LENGTH (strip->points_embedded);
    strip->points = strip->points_embedded;
}

void
_cairo_tristrip_fini (cairo_tristrip_t *strip)
{
    if (strip->points != strip->points_embedded)
	free (strip->points);

    VG (VALGRIND_MAKE_MEM_UNDEFINED (strip, sizeof (cairo_tristrip_t)));
}


void
_cairo_tristrip_limit (cairo_tristrip_t	*strip,
		       const cairo_box_t	*limits,
		       int			 num_limits)
{
    strip->limits = limits;
    strip->num_limits = num_limits;
}

void
_cairo_tristrip_init_with_clip (cairo_tristrip_t *strip,
				const cairo_clip_t *clip)
{
    _cairo_tristrip_init (strip);
    if (clip)
	_cairo_tristrip_limit (strip, clip->boxes, clip->num_boxes);
}

/* make room for at least one more trap */
static cairo_bool_t
_cairo_tristrip_grow (cairo_tristrip_t *strip)
{
    cairo_point_t *points;
    int new_size = 4 * strip->size_points;

    if (CAIRO_INJECT_FAULT ()) {
	strip->status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	return FALSE;
    }

    if (strip->points == strip->points_embedded) {
	points = _cairo_malloc_ab (new_size, sizeof (cairo_point_t));
	if (points != NULL)
	    memcpy (points, strip->points, sizeof (strip->points_embedded));
    } else {
	points = _cairo_realloc_ab (strip->points,
	                               new_size, sizeof (cairo_trapezoid_t));
    }

    if (unlikely (points == NULL)) {
	strip->status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	return FALSE;
    }

    strip->points = points;
    strip->size_points = new_size;
    return TRUE;
}

void
_cairo_tristrip_add_point (cairo_tristrip_t *strip,
			   const cairo_point_t *p)
{
    if (unlikely (strip->num_points == strip->size_points)) {
	if (unlikely (! _cairo_tristrip_grow (strip)))
	    return;
    }

    strip->points[strip->num_points++] = *p;
}

/* Insert degenerate triangles to advance to the given point. The
 * next point inserted must also be @p. */
void
_cairo_tristrip_move_to (cairo_tristrip_t *strip,
			 const cairo_point_t *p)
{
    if (strip->num_points == 0)
	return;

    _cairo_tristrip_add_point (strip, &strip->points[strip->num_points-1]);
    _cairo_tristrip_add_point (strip, p);
#if 0
    /* and one more for luck! (to preserve cw/ccw ordering) */
    _cairo_tristrip_add_point (strip, p);
#endif
}

void
_cairo_tristrip_translate (cairo_tristrip_t *strip, int x, int y)
{
    cairo_fixed_t xoff, yoff;
    cairo_point_t *p;
    int i;

    xoff = _cairo_fixed_from_int (x);
    yoff = _cairo_fixed_from_int (y);

    for (i = 0, p = strip->points; i < strip->num_points; i++, p++) {
	p->x += xoff;
	p->y += yoff;
    }
}

void
_cairo_tristrip_extents (const cairo_tristrip_t *strip,
			 cairo_box_t *extents)
{
    int i;

    if (strip->num_points == 0) {
	extents->p1.x = extents->p1.y = 0;
	extents->p2.x = extents->p2.y = 0;
	return;
    }

    extents->p2 = extents->p1 = strip->points[0];
    for (i = 1; i < strip->num_points; i++) {
	const cairo_point_t *p =  &strip->points[i];

	if (p->x < extents->p1.x)
	    extents->p1.x = p->x;
	else if (p->x > extents->p2.x)
	    extents->p2.x = p->x;

	if (p->y < extents->p1.y)
	    extents->p1.y = p->y;
	else if (p->y > extents->p2.y)
	    extents->p2.y = p->y;
    }
}
