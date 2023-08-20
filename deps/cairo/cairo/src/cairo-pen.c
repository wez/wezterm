/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2008 Chris Wilson
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
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-error-private.h"
#include "cairo-slope-private.h"

static void
_cairo_pen_compute_slopes (cairo_pen_t *pen);

cairo_status_t
_cairo_pen_init (cairo_pen_t	*pen,
		 double		 radius,
		 double		 tolerance,
		 const cairo_matrix_t	*ctm)
{
    int i;
    int reflect;

    if (CAIRO_INJECT_FAULT ())
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    VG (VALGRIND_MAKE_MEM_UNDEFINED (pen, sizeof (cairo_pen_t)));

    pen->radius = radius;
    pen->tolerance = tolerance;

    reflect = _cairo_matrix_compute_determinant (ctm) < 0.;

    pen->num_vertices = _cairo_pen_vertices_needed (tolerance,
						    radius,
						    ctm);

    if (pen->num_vertices > ARRAY_LENGTH (pen->vertices_embedded)) {
	pen->vertices = _cairo_malloc_ab (pen->num_vertices,
					  sizeof (cairo_pen_vertex_t));
	if (unlikely (pen->vertices == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    } else {
	pen->vertices = pen->vertices_embedded;
    }

    /*
     * Compute pen coordinates.  To generate the right ellipse, compute points around
     * a circle in user space and transform them to device space.  To get a consistent
     * orientation in device space, flip the pen if the transformation matrix
     * is reflecting
     */
    for (i=0; i < pen->num_vertices; i++) {
	cairo_pen_vertex_t *v = &pen->vertices[i];
	double theta = 2 * M_PI * i / (double) pen->num_vertices, dx, dy;
	if (reflect)
	    theta = -theta;
	dx = radius * cos (theta);
	dy = radius * sin (theta);
	cairo_matrix_transform_distance (ctm, &dx, &dy);
	v->point.x = _cairo_fixed_from_double (dx);
	v->point.y = _cairo_fixed_from_double (dy);
    }

    _cairo_pen_compute_slopes (pen);

    return CAIRO_STATUS_SUCCESS;
}

void
_cairo_pen_fini (cairo_pen_t *pen)
{
    if (pen->vertices != pen->vertices_embedded)
	free (pen->vertices);


    VG (VALGRIND_MAKE_MEM_UNDEFINED (pen, sizeof (cairo_pen_t)));
}

cairo_status_t
_cairo_pen_init_copy (cairo_pen_t *pen, const cairo_pen_t *other)
{
    VG (VALGRIND_MAKE_MEM_UNDEFINED (pen, sizeof (cairo_pen_t)));

    *pen = *other;

    if (CAIRO_INJECT_FAULT ())
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    pen->vertices = pen->vertices_embedded;
    if (pen->num_vertices) {
	if (pen->num_vertices > ARRAY_LENGTH (pen->vertices_embedded)) {
	    pen->vertices = _cairo_malloc_ab (pen->num_vertices,
					      sizeof (cairo_pen_vertex_t));
	    if (unlikely (pen->vertices == NULL))
		return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	}

	memcpy (pen->vertices, other->vertices,
		pen->num_vertices * sizeof (cairo_pen_vertex_t));
    }

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_pen_add_points (cairo_pen_t *pen, cairo_point_t *point, int num_points)
{
    cairo_status_t status;
    int num_vertices;
    int i;

    if (CAIRO_INJECT_FAULT ())
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    num_vertices = pen->num_vertices + num_points;
    if (num_vertices > ARRAY_LENGTH (pen->vertices_embedded) ||
	pen->vertices != pen->vertices_embedded)
    {
	cairo_pen_vertex_t *vertices;

	if (pen->vertices == pen->vertices_embedded) {
	    vertices = _cairo_malloc_ab (num_vertices,
		                         sizeof (cairo_pen_vertex_t));
	    if (unlikely (vertices == NULL))
		return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	    memcpy (vertices, pen->vertices,
		    pen->num_vertices * sizeof (cairo_pen_vertex_t));
	} else {
	    vertices = _cairo_realloc_ab (pen->vertices,
					  num_vertices,
					  sizeof (cairo_pen_vertex_t));
	    if (unlikely (vertices == NULL))
		return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	}

	pen->vertices = vertices;
    }

    pen->num_vertices = num_vertices;

    /* initialize new vertices */
    for (i=0; i < num_points; i++)
	pen->vertices[pen->num_vertices-num_points+i].point = point[i];

    status = _cairo_hull_compute (pen->vertices, &pen->num_vertices);
    if (unlikely (status))
	return status;

    _cairo_pen_compute_slopes (pen);

    return CAIRO_STATUS_SUCCESS;
}

/*
The circular pen in user space is transformed into an ellipse in
device space.

We construct the pen by computing points along the circumference
using equally spaced angles.

We show that this approximation to the ellipse has maximum error at the
major axis of the ellipse.

Set

	    M = major axis length
	    m = minor axis length

Align 'M' along the X axis and 'm' along the Y axis and draw
an ellipse parameterized by angle 't':

	    x = M cos t			y = m sin t

Perturb t by ± d and compute two new points (x+,y+), (x-,y-).
The distance from the average of these two points to (x,y) represents
the maximum error in approximating the ellipse with a polygon formed
from vertices 2∆ radians apart.

	    x+ = M cos (t+∆)		y+ = m sin (t+∆)
	    x- = M cos (t-∆)		y- = m sin (t-∆)

Now compute the approximation error, E:

	Ex = (x - (x+ + x-) / 2)
	Ex = (M cos(t) - (Mcos(t+∆) + Mcos(t-∆))/2)
	   = M (cos(t) - (cos(t)cos(∆) + sin(t)sin(∆) +
			  cos(t)cos(∆) - sin(t)sin(∆))/2)
	   = M(cos(t) - cos(t)cos(∆))
	   = M cos(t) (1 - cos(∆))

	Ey = y - (y+ - y-) / 2
	   = m sin (t) - (m sin(t+∆) + m sin(t-∆)) / 2
	   = m (sin(t) - (sin(t)cos(∆) + cos(t)sin(∆) +
			  sin(t)cos(∆) - cos(t)sin(∆))/2)
	   = m (sin(t) - sin(t)cos(∆))
	   = m sin(t) (1 - cos(∆))

	E² = Ex² + Ey²
	   = (M cos(t) (1 - cos (∆)))² + (m sin(t) (1-cos(∆)))²
	   = (1 - cos(∆))² (M² cos²(t) + m² sin²(t))
	   = (1 - cos(∆))² ((m² + M² - m²) cos² (t) + m² sin²(t))
	   = (1 - cos(∆))² (M² - m²) cos² (t) + (1 - cos(∆))² m²

Find the extremum by differentiation wrt t and setting that to zero

∂(E²)/∂(t) = (1-cos(∆))² (M² - m²) (-2 cos(t) sin(t))

         0 = 2 cos (t) sin (t)
	 0 = sin (2t)
	 t = nπ

Which is to say that the maximum and minimum errors occur on the
axes of the ellipse at 0 and π radians:

	E²(0) = (1-cos(∆))² (M² - m²) + (1-cos(∆))² m²
	      = (1-cos(∆))² M²
	E²(π) = (1-cos(∆))² m²

maximum error = M (1-cos(∆))
minimum error = m (1-cos(∆))

We must make maximum error ≤ tolerance, so compute the ∆ needed:

	    tolerance = M (1-cos(∆))
	tolerance / M = 1 - cos (∆)
	       cos(∆) = 1 - tolerance/M
                    ∆ = acos (1 - tolerance / M);

Remembering that ∆ is half of our angle between vertices,
the number of vertices is then

             vertices = ceil(2π/2∆).
                      = ceil(π/∆).

Note that this also equation works for M == m (a circle) as it
doesn't matter where on the circle the error is computed.
*/

int
_cairo_pen_vertices_needed (double	    tolerance,
			    double	    radius,
			    const cairo_matrix_t  *matrix)
{
    /*
     * the pen is a circle that gets transformed to an ellipse by matrix.
     * compute major axis length for a pen with the specified radius.
     * we don't need the minor axis length.
     */
    double major_axis = _cairo_matrix_transformed_circle_major_axis (matrix,
								     radius);
    int num_vertices;

    if (tolerance >= 4*major_axis) { /* XXX relaxed from 2*major for inkscape */
	num_vertices = 1;
    } else if (tolerance >= major_axis) {
	num_vertices = 4;
    } else {
	num_vertices = ceil (2*M_PI / acos (1 - tolerance / major_axis));

	/* number of vertices must be even */
	if (num_vertices % 2)
	    num_vertices++;

	/* And we must always have at least 4 vertices. */
	if (num_vertices < 4)
	    num_vertices = 4;
    }

    return num_vertices;
}

static void
_cairo_pen_compute_slopes (cairo_pen_t *pen)
{
    int i, i_prev;
    cairo_pen_vertex_t *prev, *v, *next;

    for (i=0, i_prev = pen->num_vertices - 1;
	 i < pen->num_vertices;
	 i_prev = i++) {
	prev = &pen->vertices[i_prev];
	v = &pen->vertices[i];
	next = &pen->vertices[(i + 1) % pen->num_vertices];

	_cairo_slope_init (&v->slope_cw, &prev->point, &v->point);
	_cairo_slope_init (&v->slope_ccw, &v->point, &next->point);
    }
}
/*
 * Find active pen vertex for clockwise edge of stroke at the given slope.
 *
 * The strictness of the inequalities here is delicate. The issue is
 * that the slope_ccw member of one pen vertex will be equivalent to
 * the slope_cw member of the next pen vertex in a counterclockwise
 * order. However, for this function, we care strongly about which
 * vertex is returned.
 *
 * [I think the "care strongly" above has to do with ensuring that the
 * pen's "extra points" from the spline's initial and final slopes are
 * properly found when beginning the spline stroking.]
 */
int
_cairo_pen_find_active_cw_vertex_index (const cairo_pen_t *pen,
					const cairo_slope_t *slope)
{
    int i;

    for (i=0; i < pen->num_vertices; i++) {
	if ((_cairo_slope_compare (slope, &pen->vertices[i].slope_ccw) < 0) &&
	    (_cairo_slope_compare (slope, &pen->vertices[i].slope_cw) >= 0))
	    break;
    }

    /* If the desired slope cannot be found between any of the pen
     * vertices, then we must have a degenerate pen, (such as a pen
     * that's been transformed to a line). In that case, we consider
     * the first pen vertex as the appropriate clockwise vertex.
     */
    if (i == pen->num_vertices)
	i = 0;

    return i;
}

/* Find active pen vertex for counterclockwise edge of stroke at the given slope.
 *
 * Note: See the comments for _cairo_pen_find_active_cw_vertex_index
 * for some details about the strictness of the inequalities here.
 */
int
_cairo_pen_find_active_ccw_vertex_index (const cairo_pen_t *pen,
					 const cairo_slope_t *slope)
{
    cairo_slope_t slope_reverse;
    int i;

    slope_reverse = *slope;
    slope_reverse.dx = -slope_reverse.dx;
    slope_reverse.dy = -slope_reverse.dy;

    for (i=pen->num_vertices-1; i >= 0; i--) {
	if ((_cairo_slope_compare (&pen->vertices[i].slope_ccw, &slope_reverse) >= 0) &&
	    (_cairo_slope_compare (&pen->vertices[i].slope_cw, &slope_reverse) < 0))
	    break;
    }

    /* If the desired slope cannot be found between any of the pen
     * vertices, then we must have a degenerate pen, (such as a pen
     * that's been transformed to a line). In that case, we consider
     * the last pen vertex as the appropriate counterclockwise vertex.
     */
    if (i < 0)
	i = pen->num_vertices - 1;

    return i;
}

void
_cairo_pen_find_active_cw_vertices (const cairo_pen_t *pen,
				    const cairo_slope_t *in,
				    const cairo_slope_t *out,
				    int *start, int *stop)
{

    int lo = 0, hi = pen->num_vertices;
    int i;

    i = (lo + hi) >> 1;
    do {
	if (_cairo_slope_compare (&pen->vertices[i].slope_cw, in) < 0)
	    lo = i;
	else
	    hi = i;
	i = (lo + hi) >> 1;
    } while (hi - lo > 1);
    if (_cairo_slope_compare (&pen->vertices[i].slope_cw, in) < 0)
	if (++i == pen->num_vertices)
	    i = 0;
    *start = i;

    if (_cairo_slope_compare (out, &pen->vertices[i].slope_ccw) >= 0) {
	lo = i;
	hi = i + pen->num_vertices;
	i = (lo + hi) >> 1;
	do {
	    int j = i;
	    if (j >= pen->num_vertices)
		j -= pen->num_vertices;
	    if (_cairo_slope_compare (&pen->vertices[j].slope_cw, out) > 0)
		hi = i;
	    else
		lo = i;
	    i = (lo + hi) >> 1;
	} while (hi - lo > 1);
	if (i >= pen->num_vertices)
	    i -= pen->num_vertices;
    }
    *stop = i;
}

void
_cairo_pen_find_active_ccw_vertices (const cairo_pen_t *pen,
				     const cairo_slope_t *in,
				     const cairo_slope_t *out,
				     int *start, int *stop)
{
    int lo = 0, hi = pen->num_vertices;
    int i;

    i = (lo + hi) >> 1;
    do {
	if (_cairo_slope_compare (in, &pen->vertices[i].slope_ccw) < 0)
	    lo = i;
	else
	    hi = i;
	i = (lo + hi) >> 1;
    } while (hi - lo > 1);
    if (_cairo_slope_compare (in, &pen->vertices[i].slope_ccw) < 0)
	if (++i == pen->num_vertices)
	    i = 0;
    *start = i;

    if (_cairo_slope_compare (&pen->vertices[i].slope_cw, out) <= 0) {
	lo = i;
	hi = i + pen->num_vertices;
	i = (lo + hi) >> 1;
	do {
	    int j = i;
	    if (j >= pen->num_vertices)
		j -= pen->num_vertices;
	    if (_cairo_slope_compare (out, &pen->vertices[j].slope_ccw) > 0)
		hi = i;
	    else
		lo = i;
	    i = (lo + hi) >> 1;
	} while (hi - lo > 1);
	if (i >= pen->num_vertices)
	    i -= pen->num_vertices;
    }
    *stop = i;
}
