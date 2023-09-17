/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
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

#define _DEFAULT_SOURCE /* for hypot() */
#include "cairoint.h"

#include "cairo-box-inline.h"
#include "cairo-boxes-private.h"
#include "cairo-error-private.h"
#include "cairo-path-fixed-private.h"
#include "cairo-slope-private.h"
#include "cairo-stroke-dash-private.h"
#include "cairo-traps-private.h"

typedef struct cairo_stroker {
    cairo_stroke_style_t style;

    const cairo_matrix_t *ctm;
    const cairo_matrix_t *ctm_inverse;
    double half_line_width;
    double tolerance;
    double spline_cusp_tolerance;
    double ctm_determinant;
    cairo_bool_t ctm_det_positive;

    void *closure;
    cairo_status_t (*add_external_edge) (void *closure,
					 const cairo_point_t *p1,
					 const cairo_point_t *p2);
    cairo_status_t (*add_triangle) (void *closure,
				    const cairo_point_t triangle[3]);
    cairo_status_t (*add_triangle_fan) (void *closure,
					const cairo_point_t *midpt,
					const cairo_point_t *points,
					int npoints);
    cairo_status_t (*add_convex_quad) (void *closure,
				       const cairo_point_t quad[4]);

    cairo_pen_t	  pen;

    cairo_point_t current_point;
    cairo_point_t first_point;

    cairo_bool_t has_initial_sub_path;

    cairo_bool_t has_current_face;
    cairo_stroke_face_t current_face;

    cairo_bool_t has_first_face;
    cairo_stroke_face_t first_face;

    cairo_stroker_dash_t dash;

    cairo_bool_t has_bounds;
    cairo_box_t bounds;
} cairo_stroker_t;

static void
_cairo_stroker_limit (cairo_stroker_t *stroker,
		      const cairo_path_fixed_t *path,
		      const cairo_box_t *boxes,
		      int num_boxes)
{
    double dx, dy;
    cairo_fixed_t fdx, fdy;

    stroker->has_bounds = TRUE;
    _cairo_boxes_get_extents (boxes, num_boxes, &stroker->bounds);

    /* Extend the bounds in each direction to account for the maximum area
     * we might generate trapezoids, to capture line segments that are outside
     * of the bounds but which might generate rendering that's within bounds.
     */

    _cairo_stroke_style_max_distance_from_path (&stroker->style, path,
						stroker->ctm, &dx, &dy);

    fdx = _cairo_fixed_from_double (dx);
    fdy = _cairo_fixed_from_double (dy);

    stroker->bounds.p1.x -= fdx;
    stroker->bounds.p2.x += fdx;

    stroker->bounds.p1.y -= fdy;
    stroker->bounds.p2.y += fdy;
}

static cairo_status_t
_cairo_stroker_init (cairo_stroker_t		*stroker,
		     const cairo_path_fixed_t	*path,
		     const cairo_stroke_style_t	*stroke_style,
		     const cairo_matrix_t	*ctm,
		     const cairo_matrix_t	*ctm_inverse,
		     double			 tolerance,
		     const cairo_box_t		*limits,
		     int			 num_limits)
{
    cairo_status_t status;

    stroker->style = *stroke_style;
    stroker->ctm = ctm;
    stroker->ctm_inverse = ctm_inverse;
    stroker->tolerance = tolerance;
    stroker->half_line_width = stroke_style->line_width / 2.0;

    /* To test whether we need to join two segments of a spline using
     * a round-join or a bevel-join, we can inspect the angle between the
     * two segments. If the difference between the chord distance
     * (half-line-width times the cosine of the bisection angle) and the
     * half-line-width itself is greater than tolerance then we need to
     * inject a point.
     */
    stroker->spline_cusp_tolerance = 1 - tolerance / stroker->half_line_width;
    stroker->spline_cusp_tolerance *= stroker->spline_cusp_tolerance;
    stroker->spline_cusp_tolerance *= 2;
    stroker->spline_cusp_tolerance -= 1;

    stroker->ctm_determinant = _cairo_matrix_compute_determinant (stroker->ctm);
    stroker->ctm_det_positive = stroker->ctm_determinant >= 0.0;

    status = _cairo_pen_init (&stroker->pen,
			      stroker->half_line_width, tolerance, ctm);
    if (unlikely (status))
	return status;

    stroker->has_current_face = FALSE;
    stroker->has_first_face = FALSE;
    stroker->has_initial_sub_path = FALSE;

    _cairo_stroker_dash_init (&stroker->dash, stroke_style);

    stroker->add_external_edge = NULL;

    stroker->has_bounds = FALSE;
    if (num_limits)
	_cairo_stroker_limit (stroker, path, limits, num_limits);

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_stroker_fini (cairo_stroker_t *stroker)
{
    _cairo_pen_fini (&stroker->pen);
}

static void
_translate_point (cairo_point_t *point, const cairo_point_t *offset)
{
    point->x += offset->x;
    point->y += offset->y;
}

static int
_cairo_stroker_join_is_clockwise (const cairo_stroke_face_t *in,
				  const cairo_stroke_face_t *out)
{
    cairo_slope_t in_slope, out_slope;

    _cairo_slope_init (&in_slope, &in->point, &in->cw);
    _cairo_slope_init (&out_slope, &out->point, &out->cw);

    return _cairo_slope_compare (&in_slope, &out_slope) < 0;
}

/**
 * _cairo_slope_compare_sgn:
 *
 * Return -1, 0 or 1 depending on the relative slopes of
 * two lines.
 **/
static int
_cairo_slope_compare_sgn (double dx1, double dy1, double dx2, double dy2)
{
    double  c = (dx1 * dy2 - dx2 * dy1);

    if (c > 0) return 1;
    if (c < 0) return -1;
    return 0;
}

static inline int
_range_step (int i, int step, int max)
{
    i += step;
    if (i < 0)
	i = max - 1;
    if (i >= max)
	i = 0;
    return i;
}

/*
 * Construct a fan around the midpoint using the vertices from pen between
 * inpt and outpt.
 */
static cairo_status_t
_tessellate_fan (cairo_stroker_t *stroker,
		 const cairo_slope_t *in_vector,
		 const cairo_slope_t *out_vector,
		 const cairo_point_t *midpt,
		 const cairo_point_t *inpt,
		 const cairo_point_t *outpt,
		 cairo_bool_t clockwise)
{
    cairo_point_t stack_points[64], *points = stack_points;
    cairo_pen_t *pen = &stroker->pen;
    int start, stop, num_points = 0;
    cairo_status_t status;

    if (stroker->has_bounds &&
	! _cairo_box_contains_point (&stroker->bounds, midpt))
	goto BEVEL;

    assert (stroker->pen.num_vertices);

    if (clockwise) {
	_cairo_pen_find_active_ccw_vertices (pen,
					     in_vector, out_vector,
					     &start, &stop);
	if (stroker->add_external_edge) {
	    cairo_point_t last;
	    last = *inpt;
	    while (start != stop) {
		cairo_point_t p = *midpt;
		_translate_point (&p, &pen->vertices[start].point);

		status = stroker->add_external_edge (stroker->closure,
						     &last, &p);
		if (unlikely (status))
		    return status;
		last = p;

		if (start-- == 0)
		    start += pen->num_vertices;
	    }
	    status = stroker->add_external_edge (stroker->closure,
						 &last, outpt);
	} else {
	    if (start == stop)
		goto BEVEL;

	    num_points = stop - start;
	    if (num_points < 0)
		num_points += pen->num_vertices;
	    num_points += 2;
	    if (num_points > ARRAY_LENGTH(stack_points)) {
		points = _cairo_malloc_ab (num_points, sizeof (cairo_point_t));
		if (unlikely (points == NULL))
		    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    }

	    points[0] = *inpt;
	    num_points = 1;
	    while (start != stop) {
		points[num_points] = *midpt;
		_translate_point (&points[num_points], &pen->vertices[start].point);
		num_points++;

		if (start-- == 0)
		    start += pen->num_vertices;
	    }
	    points[num_points++] = *outpt;
	}
    } else {
	_cairo_pen_find_active_cw_vertices (pen,
					    in_vector, out_vector,
					    &start, &stop);
	if (stroker->add_external_edge) {
	    cairo_point_t last;
	    last = *inpt;
	    while (start != stop) {
		cairo_point_t p = *midpt;
		_translate_point (&p, &pen->vertices[start].point);

		status = stroker->add_external_edge (stroker->closure,
						     &p, &last);
		if (unlikely (status))
		    return status;
		last = p;

		if (++start == pen->num_vertices)
		    start = 0;
	    }
	    status = stroker->add_external_edge (stroker->closure,
						 outpt, &last);
	} else {
	    if (start == stop)
		goto BEVEL;

	    num_points = stop - start;
	    if (num_points < 0)
		num_points += pen->num_vertices;
	    num_points += 2;
	    if (num_points > ARRAY_LENGTH(stack_points)) {
		points = _cairo_malloc_ab (num_points, sizeof (cairo_point_t));
		if (unlikely (points == NULL))
		    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    }

	    points[0] = *inpt;
	    num_points = 1;
	    while (start != stop) {
		points[num_points] = *midpt;
		_translate_point (&points[num_points], &pen->vertices[start].point);
		num_points++;

		if (++start == pen->num_vertices)
		    start = 0;
	    }
	    points[num_points++] = *outpt;
	}
    }

    if (num_points) {
	status = stroker->add_triangle_fan (stroker->closure,
					    midpt, points, num_points);
    }

    if (points != stack_points)
	free (points);

    return status;

BEVEL:
    /* Ensure a leak free connection... */
    if (stroker->add_external_edge != NULL) {
	if (clockwise)
	    return stroker->add_external_edge (stroker->closure, inpt, outpt);
	else
	    return stroker->add_external_edge (stroker->closure, outpt, inpt);
    } else {
	stack_points[0] = *midpt;
	stack_points[1] = *inpt;
	stack_points[2] = *outpt;
	return stroker->add_triangle (stroker->closure, stack_points);
    }
}

static cairo_status_t
_cairo_stroker_join (cairo_stroker_t *stroker,
		     const cairo_stroke_face_t *in,
		     const cairo_stroke_face_t *out)
{
    int	 clockwise = _cairo_stroker_join_is_clockwise (out, in);
    const cairo_point_t	*inpt, *outpt;
    cairo_point_t points[4];
    cairo_status_t status;

    if (in->cw.x  == out->cw.x  && in->cw.y  == out->cw.y &&
	in->ccw.x == out->ccw.x && in->ccw.y == out->ccw.y)
    {
	return CAIRO_STATUS_SUCCESS;
    }

    if (clockwise) {
	if (stroker->add_external_edge != NULL) {
	    status = stroker->add_external_edge (stroker->closure,
						 &out->cw, &in->point);
	    if (unlikely (status))
		return status;

	    status = stroker->add_external_edge (stroker->closure,
						 &in->point, &in->cw);
	    if (unlikely (status))
		return status;
	}

	inpt = &in->ccw;
	outpt = &out->ccw;
    } else {
	if (stroker->add_external_edge != NULL) {
	    status = stroker->add_external_edge (stroker->closure,
						 &in->ccw, &in->point);
	    if (unlikely (status))
		return status;

	    status = stroker->add_external_edge (stroker->closure,
						 &in->point, &out->ccw);
	    if (unlikely (status))
		return status;
	}

	inpt = &in->cw;
	outpt = &out->cw;
    }

    switch (stroker->style.line_join) {
    case CAIRO_LINE_JOIN_ROUND:
	/* construct a fan around the common midpoint */
	return _tessellate_fan (stroker,
				&in->dev_vector,
				&out->dev_vector,
				&in->point, inpt, outpt,
				clockwise);

    case CAIRO_LINE_JOIN_MITER:
    default: {
	/* dot product of incoming slope vector with outgoing slope vector */
	double	in_dot_out = -in->usr_vector.x * out->usr_vector.x +
			     -in->usr_vector.y * out->usr_vector.y;
	double	ml = stroker->style.miter_limit;

	/* Check the miter limit -- lines meeting at an acute angle
	 * can generate long miters, the limit converts them to bevel
	 *
	 * Consider the miter join formed when two line segments
	 * meet at an angle psi:
	 *
	 *	   /.\
	 *	  /. .\
	 *	 /./ \.\
	 *	/./psi\.\
	 *
	 * We can zoom in on the right half of that to see:
	 *
	 *	    |\
	 *	    | \ psi/2
	 *	    |  \
	 *	    |   \
	 *	    |    \
	 *	    |     \
	 *	  miter    \
	 *	 length     \
	 *	    |        \
	 *	    |        .\
	 *	    |    .     \
	 *	    |.   line   \
	 *	     \    width  \
	 *	      \           \
	 *
	 *
	 * The right triangle in that figure, (the line-width side is
	 * shown faintly with three '.' characters), gives us the
	 * following expression relating miter length, angle and line
	 * width:
	 *
	 *	1 /sin (psi/2) = miter_length / line_width
	 *
	 * The right-hand side of this relationship is the same ratio
	 * in which the miter limit (ml) is expressed. We want to know
	 * when the miter length is within the miter limit. That is
	 * when the following condition holds:
	 *
	 *	1/sin(psi/2) <= ml
	 *	1 <= ml sin(psi/2)
	 *	1 <= ml² sin²(psi/2)
	 *	2 <= ml² 2 sin²(psi/2)
	 *				2·sin²(psi/2) = 1-cos(psi)
	 *	2 <= ml² (1-cos(psi))
	 *
	 *				in · out = |in| |out| cos (psi)
	 *
	 * in and out are both unit vectors, so:
	 *
	 *				in · out = cos (psi)
	 *
	 *	2 <= ml² (1 - in · out)
	 *
	 */
	if (2 <= ml * ml * (1 - in_dot_out)) {
	    double		x1, y1, x2, y2;
	    double		mx, my;
	    double		dx1, dx2, dy1, dy2;
	    double		ix, iy;
	    double		fdx1, fdy1, fdx2, fdy2;
	    double		mdx, mdy;

	    /*
	     * we've got the points already transformed to device
	     * space, but need to do some computation with them and
	     * also need to transform the slope from user space to
	     * device space
	     */
	    /* outer point of incoming line face */
	    x1 = _cairo_fixed_to_double (inpt->x);
	    y1 = _cairo_fixed_to_double (inpt->y);
	    dx1 = in->usr_vector.x;
	    dy1 = in->usr_vector.y;
	    cairo_matrix_transform_distance (stroker->ctm, &dx1, &dy1);

	    /* outer point of outgoing line face */
	    x2 = _cairo_fixed_to_double (outpt->x);
	    y2 = _cairo_fixed_to_double (outpt->y);
	    dx2 = out->usr_vector.x;
	    dy2 = out->usr_vector.y;
	    cairo_matrix_transform_distance (stroker->ctm, &dx2, &dy2);

	    /*
	     * Compute the location of the outer corner of the miter.
	     * That's pretty easy -- just the intersection of the two
	     * outer edges.  We've got slopes and points on each
	     * of those edges.  Compute my directly, then compute
	     * mx by using the edge with the larger dy; that avoids
	     * dividing by values close to zero.
	     */
	    my = (((x2 - x1) * dy1 * dy2 - y2 * dx2 * dy1 + y1 * dx1 * dy2) /
		  (dx1 * dy2 - dx2 * dy1));
	    if (fabs (dy1) >= fabs (dy2))
		mx = (my - y1) * dx1 / dy1 + x1;
	    else
		mx = (my - y2) * dx2 / dy2 + x2;

	    /*
	     * When the two outer edges are nearly parallel, slight
	     * perturbations in the position of the outer points of the lines
	     * caused by representing them in fixed point form can cause the
	     * intersection point of the miter to move a large amount. If
	     * that moves the miter intersection from between the two faces,
	     * then draw a bevel instead.
	     */

	    ix = _cairo_fixed_to_double (in->point.x);
	    iy = _cairo_fixed_to_double (in->point.y);

	    /* slope of one face */
	    fdx1 = x1 - ix; fdy1 = y1 - iy;

	    /* slope of the other face */
	    fdx2 = x2 - ix; fdy2 = y2 - iy;

	    /* slope from the intersection to the miter point */
	    mdx = mx - ix; mdy = my - iy;

	    /*
	     * Make sure the miter point line lies between the two
	     * faces by comparing the slopes
	     */
	    if (_cairo_slope_compare_sgn (fdx1, fdy1, mdx, mdy) !=
		_cairo_slope_compare_sgn (fdx2, fdy2, mdx, mdy))
	    {
		if (stroker->add_external_edge != NULL) {
		    points[0].x = _cairo_fixed_from_double (mx);
		    points[0].y = _cairo_fixed_from_double (my);

		    if (clockwise) {
			status = stroker->add_external_edge (stroker->closure,
							     inpt, &points[0]);
			if (unlikely (status))
			    return status;

			status = stroker->add_external_edge (stroker->closure,
							     &points[0], outpt);
			if (unlikely (status))
			    return status;
		    } else {
			status = stroker->add_external_edge (stroker->closure,
							     outpt, &points[0]);
			if (unlikely (status))
			    return status;

			status = stroker->add_external_edge (stroker->closure,
							     &points[0], inpt);
			if (unlikely (status))
			    return status;
		    }

		    return CAIRO_STATUS_SUCCESS;
		} else {
		    points[0] = in->point;
		    points[1] = *inpt;
		    points[2].x = _cairo_fixed_from_double (mx);
		    points[2].y = _cairo_fixed_from_double (my);
		    points[3] = *outpt;

		    return stroker->add_convex_quad (stroker->closure, points);
		}
	    }
	}
    }

    /* fall through ... */

    case CAIRO_LINE_JOIN_BEVEL:
	if (stroker->add_external_edge != NULL) {
	    if (clockwise) {
		return stroker->add_external_edge (stroker->closure,
						   inpt, outpt);
	    } else {
		return stroker->add_external_edge (stroker->closure,
						   outpt, inpt);
	    }
	} else {
	    points[0] = in->point;
	    points[1] = *inpt;
	    points[2] = *outpt;

	    return stroker->add_triangle (stroker->closure, points);
	}
    }
}

static cairo_status_t
_cairo_stroker_add_cap (cairo_stroker_t *stroker,
			const cairo_stroke_face_t *f)
{
    switch (stroker->style.line_cap) {
    case CAIRO_LINE_CAP_ROUND: {
	cairo_slope_t slope;

	slope.dx = -f->dev_vector.dx;
	slope.dy = -f->dev_vector.dy;

	return _tessellate_fan (stroker,
				&f->dev_vector,
				&slope,
				&f->point, &f->cw, &f->ccw,
				FALSE);

    }

    case CAIRO_LINE_CAP_SQUARE: {
	double dx, dy;
	cairo_slope_t	fvector;
	cairo_point_t	quad[4];

	dx = f->usr_vector.x;
	dy = f->usr_vector.y;
	dx *= stroker->half_line_width;
	dy *= stroker->half_line_width;
	cairo_matrix_transform_distance (stroker->ctm, &dx, &dy);
	fvector.dx = _cairo_fixed_from_double (dx);
	fvector.dy = _cairo_fixed_from_double (dy);

	quad[0] = f->ccw;
	quad[1].x = f->ccw.x + fvector.dx;
	quad[1].y = f->ccw.y + fvector.dy;
	quad[2].x = f->cw.x + fvector.dx;
	quad[2].y = f->cw.y + fvector.dy;
	quad[3] = f->cw;

	if (stroker->add_external_edge != NULL) {
	    cairo_status_t status;

	    status = stroker->add_external_edge (stroker->closure,
						 &quad[0], &quad[1]);
	    if (unlikely (status))
		return status;

	    status = stroker->add_external_edge (stroker->closure,
						 &quad[1], &quad[2]);
	    if (unlikely (status))
		return status;

	    status = stroker->add_external_edge (stroker->closure,
						 &quad[2], &quad[3]);
	    if (unlikely (status))
		return status;

	    return CAIRO_STATUS_SUCCESS;
	} else {
	    return stroker->add_convex_quad (stroker->closure, quad);
	}
    }

    case CAIRO_LINE_CAP_BUTT:
    default:
	if (stroker->add_external_edge != NULL) {
	    return stroker->add_external_edge (stroker->closure,
					       &f->ccw, &f->cw);
	} else {
	    return CAIRO_STATUS_SUCCESS;
	}
    }
}

static cairo_status_t
_cairo_stroker_add_leading_cap (cairo_stroker_t     *stroker,
				const cairo_stroke_face_t *face)
{
    cairo_stroke_face_t reversed;
    cairo_point_t t;

    reversed = *face;

    /* The initial cap needs an outward facing vector. Reverse everything */
    reversed.usr_vector.x = -reversed.usr_vector.x;
    reversed.usr_vector.y = -reversed.usr_vector.y;
    reversed.dev_vector.dx = -reversed.dev_vector.dx;
    reversed.dev_vector.dy = -reversed.dev_vector.dy;
    t = reversed.cw;
    reversed.cw = reversed.ccw;
    reversed.ccw = t;

    return _cairo_stroker_add_cap (stroker, &reversed);
}

static cairo_status_t
_cairo_stroker_add_trailing_cap (cairo_stroker_t     *stroker,
				 const cairo_stroke_face_t *face)
{
    return _cairo_stroker_add_cap (stroker, face);
}

static inline cairo_bool_t
_compute_normalized_device_slope (double *dx, double *dy,
				  const cairo_matrix_t *ctm_inverse,
				  double *mag_out)
{
    double dx0 = *dx, dy0 = *dy;
    double mag;

    cairo_matrix_transform_distance (ctm_inverse, &dx0, &dy0);

    if (dx0 == 0.0 && dy0 == 0.0) {
	if (mag_out)
	    *mag_out = 0.0;
	return FALSE;
    }

    if (dx0 == 0.0) {
	*dx = 0.0;
	if (dy0 > 0.0) {
	    mag = dy0;
	    *dy = 1.0;
	} else {
	    mag = -dy0;
	    *dy = -1.0;
	}
    } else if (dy0 == 0.0) {
	*dy = 0.0;
	if (dx0 > 0.0) {
	    mag = dx0;
	    *dx = 1.0;
	} else {
	    mag = -dx0;
	    *dx = -1.0;
	}
    } else {
	mag = hypot (dx0, dy0);
	*dx = dx0 / mag;
	*dy = dy0 / mag;
    }

    if (mag_out)
	*mag_out = mag;

    return TRUE;
}

static void
_compute_face (const cairo_point_t *point,
	       const cairo_slope_t *dev_slope,
	       double slope_dx,
	       double slope_dy,
	       cairo_stroker_t *stroker,
	       cairo_stroke_face_t *face)
{
    double face_dx, face_dy;
    cairo_point_t offset_ccw, offset_cw;

    /*
     * rotate to get a line_width/2 vector along the face, note that
     * the vector must be rotated the right direction in device space,
     * but by 90° in user space. So, the rotation depends on
     * whether the ctm reflects or not, and that can be determined
     * by looking at the determinant of the matrix.
     */
    if (stroker->ctm_det_positive)
    {
	face_dx = - slope_dy * stroker->half_line_width;
	face_dy = slope_dx * stroker->half_line_width;
    }
    else
    {
	face_dx = slope_dy * stroker->half_line_width;
	face_dy = - slope_dx * stroker->half_line_width;
    }

    /* back to device space */
    cairo_matrix_transform_distance (stroker->ctm, &face_dx, &face_dy);

    offset_ccw.x = _cairo_fixed_from_double (face_dx);
    offset_ccw.y = _cairo_fixed_from_double (face_dy);
    offset_cw.x = -offset_ccw.x;
    offset_cw.y = -offset_ccw.y;

    face->ccw = *point;
    _translate_point (&face->ccw, &offset_ccw);

    face->point = *point;

    face->cw = *point;
    _translate_point (&face->cw, &offset_cw);

    face->usr_vector.x = slope_dx;
    face->usr_vector.y = slope_dy;

    face->dev_vector = *dev_slope;
}

static cairo_status_t
_cairo_stroker_add_caps (cairo_stroker_t *stroker)
{
    cairo_status_t status;

    /* check for a degenerative sub_path */
    if (stroker->has_initial_sub_path
	&& ! stroker->has_first_face
	&& ! stroker->has_current_face
	&& stroker->style.line_cap == CAIRO_LINE_CAP_ROUND)
    {
	/* pick an arbitrary slope to use */
	double dx = 1.0, dy = 0.0;
	cairo_slope_t slope = { CAIRO_FIXED_ONE, 0 };
	cairo_stroke_face_t face;

	_compute_normalized_device_slope (&dx, &dy,
					  stroker->ctm_inverse, NULL);

	/* arbitrarily choose first_point
	 * first_point and current_point should be the same */
	_compute_face (&stroker->first_point, &slope, dx, dy, stroker, &face);

	status = _cairo_stroker_add_leading_cap (stroker, &face);
	if (unlikely (status))
	    return status;

	status = _cairo_stroker_add_trailing_cap (stroker, &face);
	if (unlikely (status))
	    return status;
    }

    if (stroker->has_first_face) {
	status = _cairo_stroker_add_leading_cap (stroker,
						 &stroker->first_face);
	if (unlikely (status))
	    return status;
    }

    if (stroker->has_current_face) {
	status = _cairo_stroker_add_trailing_cap (stroker,
						  &stroker->current_face);
	if (unlikely (status))
	    return status;
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_stroker_add_sub_edge (cairo_stroker_t *stroker,
			     const cairo_point_t *p1,
			     const cairo_point_t *p2,
			     cairo_slope_t *dev_slope,
			     double slope_dx, double slope_dy,
			     cairo_stroke_face_t *start,
			     cairo_stroke_face_t *end)
{
    _compute_face (p1, dev_slope, slope_dx, slope_dy, stroker, start);
    *end = *start;

    if (p1->x == p2->x && p1->y == p2->y)
	return CAIRO_STATUS_SUCCESS;

    end->point = *p2;
    end->ccw.x += p2->x - p1->x;
    end->ccw.y += p2->y - p1->y;
    end->cw.x += p2->x - p1->x;
    end->cw.y += p2->y - p1->y;

    if (stroker->add_external_edge != NULL) {
	cairo_status_t status;

	status = stroker->add_external_edge (stroker->closure,
					     &end->cw, &start->cw);
	if (unlikely (status))
	    return status;

	status = stroker->add_external_edge (stroker->closure,
					     &start->ccw, &end->ccw);
	if (unlikely (status))
	    return status;

	return CAIRO_STATUS_SUCCESS;
    } else {
	cairo_point_t quad[4];

	quad[0] = start->cw;
	quad[1] = end->cw;
	quad[2] = end->ccw;
	quad[3] = start->ccw;

	return stroker->add_convex_quad (stroker->closure, quad);
    }
}

static cairo_status_t
_cairo_stroker_move_to (void *closure,
			const cairo_point_t *point)
{
    cairo_stroker_t *stroker = closure;
    cairo_status_t status;

    /* reset the dash pattern for new sub paths */
    _cairo_stroker_dash_start (&stroker->dash);

    /* Cap the start and end of the previous sub path as needed */
    status = _cairo_stroker_add_caps (stroker);
    if (unlikely (status))
	return status;

    stroker->first_point = *point;
    stroker->current_point = *point;

    stroker->has_first_face = FALSE;
    stroker->has_current_face = FALSE;
    stroker->has_initial_sub_path = FALSE;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_stroker_line_to (void *closure,
			const cairo_point_t *point)
{
    cairo_stroker_t *stroker = closure;
    cairo_stroke_face_t start, end;
    cairo_point_t *p1 = &stroker->current_point;
    cairo_slope_t dev_slope;
    double slope_dx, slope_dy;
    cairo_status_t status;

    stroker->has_initial_sub_path = TRUE;

    if (p1->x == point->x && p1->y == point->y)
	return CAIRO_STATUS_SUCCESS;

    _cairo_slope_init (&dev_slope, p1, point);
    slope_dx = _cairo_fixed_to_double (point->x - p1->x);
    slope_dy = _cairo_fixed_to_double (point->y - p1->y);
    _compute_normalized_device_slope (&slope_dx, &slope_dy,
				      stroker->ctm_inverse, NULL);

    status = _cairo_stroker_add_sub_edge (stroker,
					  p1, point,
					  &dev_slope,
					  slope_dx, slope_dy,
					  &start, &end);
    if (unlikely (status))
	return status;

    if (stroker->has_current_face) {
	/* Join with final face from previous segment */
	status = _cairo_stroker_join (stroker,
				      &stroker->current_face,
				      &start);
	if (unlikely (status))
	    return status;
    } else if (! stroker->has_first_face) {
	/* Save sub path's first face in case needed for closing join */
	stroker->first_face = start;
	stroker->has_first_face = TRUE;
    }
    stroker->current_face = end;
    stroker->has_current_face = TRUE;

    stroker->current_point = *point;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_stroker_add_point_line_to (void *closure,
				  const cairo_point_t *point,
				  const cairo_slope_t *tangent)
{
    return _cairo_stroker_line_to (closure, point);
};

static cairo_status_t
_cairo_stroker_spline_to (void *closure,
			  const cairo_point_t *point,
			  const cairo_slope_t *tangent)
{
    cairo_stroker_t *stroker = closure;
    cairo_stroke_face_t new_face;
    double slope_dx, slope_dy;
    cairo_point_t points[3];
    cairo_point_t intersect_point;

    stroker->has_initial_sub_path = TRUE;

    if (stroker->current_point.x == point->x &&
	stroker->current_point.y == point->y)
	return CAIRO_STATUS_SUCCESS;

    slope_dx = _cairo_fixed_to_double (tangent->dx);
    slope_dy = _cairo_fixed_to_double (tangent->dy);

    if (! _compute_normalized_device_slope (&slope_dx, &slope_dy,
					    stroker->ctm_inverse, NULL))
	return CAIRO_STATUS_SUCCESS;

    _compute_face (point, tangent,
		   slope_dx, slope_dy,
		   stroker, &new_face);

    assert (stroker->has_current_face);

    if ((new_face.dev_slope.x * stroker->current_face.dev_slope.x +
         new_face.dev_slope.y * stroker->current_face.dev_slope.y) < stroker->spline_cusp_tolerance) {

	const cairo_point_t *inpt, *outpt;
	int clockwise = _cairo_stroker_join_is_clockwise (&new_face,
							  &stroker->current_face);

	if (clockwise) {
	    inpt = &stroker->current_face.cw;
	    outpt = &new_face.cw;
	} else {
	    inpt = &stroker->current_face.ccw;
	    outpt = &new_face.ccw;
	}

	_tessellate_fan (stroker,
			 &stroker->current_face.dev_vector,
			 &new_face.dev_vector,
			 &stroker->current_face.point,
			 inpt, outpt,
			 clockwise);
    }

    if (_slow_segment_intersection (&stroker->current_face.cw,
				    &stroker->current_face.ccw,
				    &new_face.cw,
				    &new_face.ccw,
				    &intersect_point)) {
	points[0] = stroker->current_face.ccw;
	points[1] = new_face.ccw;
	points[2] = intersect_point;
	stroker->add_triangle (stroker->closure, points);

	points[0] = stroker->current_face.cw;
	points[1] = new_face.cw;
	stroker->add_triangle (stroker->closure, points);
    } else {
	points[0] = stroker->current_face.ccw;
	points[1] = stroker->current_face.cw;
	points[2] = new_face.cw;
	stroker->add_triangle (stroker->closure, points);

	points[0] = stroker->current_face.ccw;
	points[1] = new_face.cw;
	points[2] = new_face.ccw;
	stroker->add_triangle (stroker->closure, points);
    }

    stroker->current_face = new_face;
    stroker->has_current_face = TRUE;
    stroker->current_point = *point;

    return CAIRO_STATUS_SUCCESS;
}

/*
 * Dashed lines.  Cap each dash end, join around turns when on
 */
static cairo_status_t
_cairo_stroker_line_to_dashed (void *closure,
			       const cairo_point_t *p2)
{
    cairo_stroker_t *stroker = closure;
    double mag, remain, step_length = 0;
    double slope_dx, slope_dy;
    double dx2, dy2;
    cairo_stroke_face_t sub_start, sub_end;
    cairo_point_t *p1 = &stroker->current_point;
    cairo_slope_t dev_slope;
    cairo_line_t segment;
    cairo_bool_t fully_in_bounds;
    cairo_status_t status;

    stroker->has_initial_sub_path = stroker->dash.dash_starts_on;

    if (p1->x == p2->x && p1->y == p2->y)
	return CAIRO_STATUS_SUCCESS;

    fully_in_bounds = TRUE;
    if (stroker->has_bounds &&
	(! _cairo_box_contains_point (&stroker->bounds, p1) ||
	 ! _cairo_box_contains_point (&stroker->bounds, p2)))
    {
	fully_in_bounds = FALSE;
    }

    _cairo_slope_init (&dev_slope, p1, p2);

    slope_dx = _cairo_fixed_to_double (p2->x - p1->x);
    slope_dy = _cairo_fixed_to_double (p2->y - p1->y);

    if (! _compute_normalized_device_slope (&slope_dx, &slope_dy,
					    stroker->ctm_inverse, &mag))
    {
	return CAIRO_STATUS_SUCCESS;
    }

    remain = mag;
    segment.p1 = *p1;
    while (remain) {
	step_length = MIN (stroker->dash.dash_remain, remain);
	remain -= step_length;
	dx2 = slope_dx * (mag - remain);
	dy2 = slope_dy * (mag - remain);
	cairo_matrix_transform_distance (stroker->ctm, &dx2, &dy2);
	segment.p2.x = _cairo_fixed_from_double (dx2) + p1->x;
	segment.p2.y = _cairo_fixed_from_double (dy2) + p1->y;

	if (stroker->dash.dash_on &&
	    (fully_in_bounds ||
	     (! stroker->has_first_face && stroker->dash.dash_starts_on) ||
	     _cairo_box_intersects_line_segment (&stroker->bounds, &segment)))
	{
	    status = _cairo_stroker_add_sub_edge (stroker,
						  &segment.p1, &segment.p2,
						  &dev_slope,
						  slope_dx, slope_dy,
						  &sub_start, &sub_end);
	    if (unlikely (status))
		return status;

	    if (stroker->has_current_face)
	    {
		/* Join with final face from previous segment */
		status = _cairo_stroker_join (stroker,
					      &stroker->current_face,
					      &sub_start);
		if (unlikely (status))
		    return status;

		stroker->has_current_face = FALSE;
	    }
	    else if (! stroker->has_first_face &&
		       stroker->dash.dash_starts_on)
	    {
		/* Save sub path's first face in case needed for closing join */
		stroker->first_face = sub_start;
		stroker->has_first_face = TRUE;
	    }
	    else
	    {
		/* Cap dash start if not connecting to a previous segment */
		status = _cairo_stroker_add_leading_cap (stroker, &sub_start);
		if (unlikely (status))
		    return status;
	    }

	    if (remain) {
		/* Cap dash end if not at end of segment */
		status = _cairo_stroker_add_trailing_cap (stroker, &sub_end);
		if (unlikely (status))
		    return status;
	    } else {
		stroker->current_face = sub_end;
		stroker->has_current_face = TRUE;
	    }
	} else {
	    if (stroker->has_current_face) {
		/* Cap final face from previous segment */
		status = _cairo_stroker_add_trailing_cap (stroker,
							  &stroker->current_face);
		if (unlikely (status))
		    return status;

		stroker->has_current_face = FALSE;
	    }
	}

	_cairo_stroker_dash_step (&stroker->dash, step_length);
	segment.p1 = segment.p2;
    }

    if (stroker->dash.dash_on && ! stroker->has_current_face) {
	/* This segment ends on a transition to dash_on, compute a new face
	 * and add cap for the beginning of the next dash_on step.
	 *
	 * Note: this will create a degenerate cap if this is not the last line
	 * in the path. Whether this behaviour is desirable or not is debatable.
	 * On one side these degenerate caps can not be reproduced with regular
	 * path stroking.
	 * On the other hand, Acroread 7 also produces the degenerate caps.
	 */
	_compute_face (p2, &dev_slope,
		       slope_dx, slope_dy,
		       stroker,
		       &stroker->current_face);

	status = _cairo_stroker_add_leading_cap (stroker,
						 &stroker->current_face);
	if (unlikely (status))
	    return status;

	stroker->has_current_face = TRUE;
    }

    stroker->current_point = *p2;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_stroker_add_point_line_to_dashed (void *closure,
					 const cairo_point_t *point,
					 const cairo_slope_t *tangent)
{
    return _cairo_stroker_line_to_dashed (closure, point);
};

static cairo_status_t
_cairo_stroker_curve_to (void *closure,
			 const cairo_point_t *b,
			 const cairo_point_t *c,
			 const cairo_point_t *d)
{
    cairo_stroker_t *stroker = closure;
    cairo_spline_t spline;
    cairo_line_join_t line_join_save;
    cairo_stroke_face_t face;
    double slope_dx, slope_dy;
    cairo_spline_add_point_func_t line_to;
    cairo_spline_add_point_func_t spline_to;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    line_to = stroker->dash.dashed ?
	_cairo_stroker_add_point_line_to_dashed :
	_cairo_stroker_add_point_line_to;

    /* spline_to is only capable of rendering non-degenerate splines. */
    spline_to = stroker->dash.dashed ?
	_cairo_stroker_add_point_line_to_dashed :
	_cairo_stroker_spline_to;

    if (! _cairo_spline_init (&spline,
			      spline_to,
			      stroker,
			      &stroker->current_point, b, c, d))
    {
	cairo_slope_t fallback_slope;
	_cairo_slope_init (&fallback_slope, &stroker->current_point, d);
	return line_to (closure, d, &fallback_slope);
    }

    /* If the line width is so small that the pen is reduced to a
       single point, then we have nothing to do. */
    if (stroker->pen.num_vertices <= 1)
	return CAIRO_STATUS_SUCCESS;

    /* Compute the initial face */
    if (! stroker->dash.dashed || stroker->dash.dash_on) {
	slope_dx = _cairo_fixed_to_double (spline.initial_slope.dx);
	slope_dy = _cairo_fixed_to_double (spline.initial_slope.dy);
	if (_compute_normalized_device_slope (&slope_dx, &slope_dy,
					      stroker->ctm_inverse, NULL))
	{
	    _compute_face (&stroker->current_point,
			   &spline.initial_slope,
			   slope_dx, slope_dy,
			   stroker, &face);
	}
	if (stroker->has_current_face) {
	    status = _cairo_stroker_join (stroker,
					  &stroker->current_face, &face);
	    if (unlikely (status))
		return status;
	} else if (! stroker->has_first_face) {
	    stroker->first_face = face;
	    stroker->has_first_face = TRUE;
	}

	stroker->current_face = face;
	stroker->has_current_face = TRUE;
    }

    /* Temporarily modify the stroker to use round joins to guarantee
     * smooth stroked curves. */
    line_join_save = stroker->style.line_join;
    stroker->style.line_join = CAIRO_LINE_JOIN_ROUND;

    status = _cairo_spline_decompose (&spline, stroker->tolerance);
    if (unlikely (status))
	return status;

    /* And join the final face */
    if (! stroker->dash.dashed || stroker->dash.dash_on) {
	slope_dx = _cairo_fixed_to_double (spline.final_slope.dx);
	slope_dy = _cairo_fixed_to_double (spline.final_slope.dy);
	if (_compute_normalized_device_slope (&slope_dx, &slope_dy,
					      stroker->ctm_inverse, NULL))
	{
	    _compute_face (&stroker->current_point,
			   &spline.final_slope,
			   slope_dx, slope_dy,
			   stroker, &face);
	}

	status = _cairo_stroker_join (stroker, &stroker->current_face, &face);
	if (unlikely (status))
	    return status;

	stroker->current_face = face;
    }

    stroker->style.line_join = line_join_save;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_stroker_close_path (void *closure)
{
    cairo_stroker_t *stroker = closure;
    cairo_status_t status;

    if (stroker->dash.dashed)
	status = _cairo_stroker_line_to_dashed (stroker, &stroker->first_point);
    else
	status = _cairo_stroker_line_to (stroker, &stroker->first_point);
    if (unlikely (status))
	return status;

    if (stroker->has_first_face && stroker->has_current_face) {
	/* Join first and final faces of sub path */
	status = _cairo_stroker_join (stroker,
				      &stroker->current_face,
				      &stroker->first_face);
	if (unlikely (status))
	    return status;
    } else {
	/* Cap the start and end of the sub path as needed */
	status = _cairo_stroker_add_caps (stroker);
	if (unlikely (status))
	    return status;
    }

    stroker->has_initial_sub_path = FALSE;
    stroker->has_first_face = FALSE;
    stroker->has_current_face = FALSE;

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_path_fixed_stroke_to_shaper (cairo_path_fixed_t	*path,
				    const cairo_stroke_style_t	*stroke_style,
				    const cairo_matrix_t	*ctm,
				    const cairo_matrix_t	*ctm_inverse,
				    double		 tolerance,
				    cairo_status_t (*add_triangle) (void *closure,
								    const cairo_point_t triangle[3]),
				    cairo_status_t (*add_triangle_fan) (void *closure,
									const cairo_point_t *midpt,
									const cairo_point_t *points,
									int npoints),
				    cairo_status_t (*add_convex_quad) (void *closure,
								       const cairo_point_t quad[4]),
				    void *closure)
{
    cairo_stroker_t stroker;
    cairo_status_t status;

    status = _cairo_stroker_init (&stroker, path, stroke_style,
			          ctm, ctm_inverse, tolerance,
				  NULL, 0);
    if (unlikely (status))
	return status;

    stroker.add_triangle = add_triangle;
    stroker.add_triangle_fan = add_triangle_fan;
    stroker.add_convex_quad = add_convex_quad;
    stroker.closure = closure;

    status = _cairo_path_fixed_interpret (path,
					  _cairo_stroker_move_to,
					  stroker.dash.dashed ?
					  _cairo_stroker_line_to_dashed :
					  _cairo_stroker_line_to,
					  _cairo_stroker_curve_to,
					  _cairo_stroker_close_path,
					  &stroker);

    if (unlikely (status))
	goto BAIL;

    /* Cap the start and end of the final sub path as needed */
    status = _cairo_stroker_add_caps (&stroker);

BAIL:
    _cairo_stroker_fini (&stroker);

    return status;
}

cairo_status_t
_cairo_path_fixed_stroke_dashed_to_polygon (const cairo_path_fixed_t	*path,
					    const cairo_stroke_style_t	*stroke_style,
					    const cairo_matrix_t	*ctm,
					    const cairo_matrix_t	*ctm_inverse,
					    double		 tolerance,
					    cairo_polygon_t *polygon)
{
    cairo_stroker_t stroker;
    cairo_status_t status;

    status = _cairo_stroker_init (&stroker, path, stroke_style,
			          ctm, ctm_inverse, tolerance,
				  polygon->limits, polygon->num_limits);
    if (unlikely (status))
	return status;

    stroker.add_external_edge = _cairo_polygon_add_external_edge,
    stroker.closure = polygon;

    status = _cairo_path_fixed_interpret (path,
					  _cairo_stroker_move_to,
					  stroker.dash.dashed ?
					  _cairo_stroker_line_to_dashed :
					  _cairo_stroker_line_to,
					  _cairo_stroker_curve_to,
					  _cairo_stroker_close_path,
					  &stroker);

    if (unlikely (status))
	goto BAIL;

    /* Cap the start and end of the final sub path as needed */
    status = _cairo_stroker_add_caps (&stroker);

BAIL:
    _cairo_stroker_fini (&stroker);

    return status;
}

cairo_int_status_t
_cairo_path_fixed_stroke_polygon_to_traps (const cairo_path_fixed_t	*path,
                                           const cairo_stroke_style_t	*stroke_style,
                                           const cairo_matrix_t	*ctm,
                                           const cairo_matrix_t	*ctm_inverse,
                                           double		 tolerance,
                                           cairo_traps_t	*traps)
{
    cairo_int_status_t status;
    cairo_polygon_t polygon;

    _cairo_polygon_init (&polygon, traps->limits, traps->num_limits);
    status = _cairo_path_fixed_stroke_to_polygon (path,
						  stroke_style,
						  ctm,
						  ctm_inverse,
						  tolerance,
						  &polygon);
    if (unlikely (status))
	goto BAIL;

    status = _cairo_polygon_status (&polygon);
    if (unlikely (status))
	goto BAIL;

    status = _cairo_bentley_ottmann_tessellate_polygon (traps, &polygon,
							CAIRO_FILL_RULE_WINDING);

BAIL:
    _cairo_polygon_fini (&polygon);

    return status;
}
