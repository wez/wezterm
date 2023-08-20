/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2013 Intel Corporation
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
 * Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA 02111-1307 USA
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

#include "cairo-box-inline.h"
#include "cairo-path-fixed-private.h"
#include "cairo-slope-private.h"
#include "cairo-stroke-dash-private.h"
#include "cairo-traps-private.h"

#include <float.h>

struct stroker {
    const cairo_stroke_style_t	*style;

    const cairo_matrix_t *ctm;
    const cairo_matrix_t *ctm_inverse;
    double spline_cusp_tolerance;
    double half_line_width;
    double tolerance;
    double ctm_determinant;
    cairo_bool_t ctm_det_positive;
    cairo_line_join_t line_join;

    cairo_traps_t *traps;

    cairo_pen_t pen;

    cairo_point_t first_point;

    cairo_bool_t has_initial_sub_path;

    cairo_bool_t has_current_face;
    cairo_stroke_face_t current_face;

    cairo_bool_t has_first_face;
    cairo_stroke_face_t first_face;

    cairo_stroker_dash_t dash;

    cairo_bool_t has_bounds;
    cairo_box_t tight_bounds;
    cairo_box_t line_bounds;
    cairo_box_t join_bounds;
};

static cairo_status_t
stroker_init (struct stroker		*stroker,
	      const cairo_path_fixed_t	*path,
	      const cairo_stroke_style_t	*style,
	      const cairo_matrix_t	*ctm,
	      const cairo_matrix_t	*ctm_inverse,
	      double			 tolerance,
	      cairo_traps_t		*traps)
{
    cairo_status_t status;

    stroker->style = style;
    stroker->ctm = ctm;
    stroker->ctm_inverse = NULL;
    if (! _cairo_matrix_is_identity (ctm_inverse))
	stroker->ctm_inverse = ctm_inverse;
    stroker->line_join = style->line_join;
    stroker->half_line_width = style->line_width / 2.0;
    stroker->tolerance = tolerance;
    stroker->traps = traps;

    /* If `CAIRO_LINE_JOIN_ROUND` is selected and a joint's `arc height`
     * is greater than `tolerance` then two segments are joined with
     * round-join, otherwise bevel-join is used.
     *
     * `Arc height` is the difference of the "half of a line width" and
     * the "half of a line width" times `cos(half the angle between segment vectors)`.
     *
     * See detailed description in the `_cairo_path_fixed_stroke_to_polygon()`
     * function in the `cairo-path-stroke-polygon.c` file or follow the
     * https://gitlab.freedesktop.org/cairo/cairo/-/merge_requests/372#note_1698225
     * link to see the detailed description with an illustration.
     */
    double scaled_hlw = hypot(stroker->half_line_width * ctm->xx,
			      stroker->half_line_width * ctm->yx);

    if (scaled_hlw <= tolerance) {
	stroker->spline_cusp_tolerance = -1.0;
    } else {
	stroker->spline_cusp_tolerance = 1 - tolerance / scaled_hlw;
	stroker->spline_cusp_tolerance *= stroker->spline_cusp_tolerance;
	stroker->spline_cusp_tolerance *= 2;
	stroker->spline_cusp_tolerance -= 1;
    }

    stroker->ctm_determinant = _cairo_matrix_compute_determinant (stroker->ctm);
    stroker->ctm_det_positive = stroker->ctm_determinant >= 0.0;

    status = _cairo_pen_init (&stroker->pen,
		              stroker->half_line_width,
			      tolerance, ctm);
    if (unlikely (status))
	return status;

    stroker->has_current_face = FALSE;
    stroker->has_first_face = FALSE;
    stroker->has_initial_sub_path = FALSE;

    _cairo_stroker_dash_init (&stroker->dash, style);

    stroker->has_bounds = traps->num_limits;
    if (stroker->has_bounds) {
	/* Extend the bounds in each direction to account for the maximum area
	 * we might generate trapezoids, to capture line segments that are outside
	 * of the bounds but which might generate rendering that's within bounds.
	 */
	double dx, dy;
	cairo_fixed_t fdx, fdy;

	stroker->tight_bounds = traps->bounds;

	_cairo_stroke_style_max_distance_from_path (stroker->style, path,
						    stroker->ctm, &dx, &dy);

	_cairo_stroke_style_max_line_distance_from_path (stroker->style, path,
							 stroker->ctm, &dx, &dy);

	fdx = _cairo_fixed_from_double (dx);
	fdy = _cairo_fixed_from_double (dy);

	stroker->line_bounds = stroker->tight_bounds;
	stroker->line_bounds.p1.x -= fdx;
	stroker->line_bounds.p2.x += fdx;
	stroker->line_bounds.p1.y -= fdy;
	stroker->line_bounds.p2.y += fdy;

	_cairo_stroke_style_max_join_distance_from_path (stroker->style, path,
							 stroker->ctm, &dx, &dy);

	fdx = _cairo_fixed_from_double (dx);
	fdy = _cairo_fixed_from_double (dy);

	stroker->join_bounds = stroker->tight_bounds;
	stroker->join_bounds.p1.x -= fdx;
	stroker->join_bounds.p2.x += fdx;
	stroker->join_bounds.p1.y -= fdy;
	stroker->join_bounds.p2.y += fdy;
    }

    return CAIRO_STATUS_SUCCESS;
}

static void
stroker_fini (struct stroker *stroker)
{
    _cairo_pen_fini (&stroker->pen);
}

static void
translate_point (cairo_point_t *point, cairo_point_t *offset)
{
    point->x += offset->x;
    point->y += offset->y;
}

static int
join_is_clockwise (const cairo_stroke_face_t *in,
		   const cairo_stroke_face_t *out)
{
    return _cairo_slope_compare (&in->dev_vector, &out->dev_vector) < 0;
}

static int
slope_compare_sgn (double dx1, double dy1, double dx2, double dy2)
{
    double c = dx1 * dy2 - dx2 * dy1;
    if (c > 0) return 1;
    if (c < 0) return -1;
    return 0;
}

static cairo_bool_t
stroker_intersects_join (const struct stroker *stroker,
			 const cairo_point_t *in,
			 const cairo_point_t *out)
{
    cairo_line_t segment;

    if (! stroker->has_bounds)
	return TRUE;

    segment.p1 = *in;
    segment.p2 = *out;
    return _cairo_box_intersects_line_segment (&stroker->join_bounds, &segment);
}

static void
join (struct stroker *stroker,
      cairo_stroke_face_t *in,
      cairo_stroke_face_t *out)
{
    int clockwise = join_is_clockwise (out, in);
    cairo_point_t *inpt, *outpt;

    if (in->cw.x == out->cw.x &&
	in->cw.y == out->cw.y &&
	in->ccw.x == out->ccw.x &&
	in->ccw.y == out->ccw.y)
    {
	return;
    }

    if (clockwise) {
	inpt = &in->ccw;
	outpt = &out->ccw;
    } else {
	inpt = &in->cw;
	outpt = &out->cw;
    }

    if (! stroker_intersects_join (stroker, inpt, outpt))
	    return;

    switch (stroker->line_join) {
    case CAIRO_LINE_JOIN_ROUND:
	/* construct a fan around the common midpoint */
	if ((in->dev_slope.x * out->dev_slope.x +
	     in->dev_slope.y * out->dev_slope.y) < stroker->spline_cusp_tolerance)
	{
	    int start, stop;
	    cairo_point_t tri[3], edges[4];
	    cairo_pen_t *pen = &stroker->pen;

	    edges[0] = in->cw;
	    edges[1] = in->ccw;
	    tri[0] = in->point;
	    tri[1] = *inpt;
	    if (clockwise) {
		_cairo_pen_find_active_ccw_vertices (pen,
						     &in->dev_vector, &out->dev_vector,
						     &start, &stop);
		while (start != stop) {
		    tri[2] = in->point;
		    translate_point (&tri[2], &pen->vertices[start].point);
		    edges[2] = in->point;
		    edges[3] = tri[2];
		    _cairo_traps_tessellate_triangle_with_edges (stroker->traps,
								 tri, edges);
		    tri[1] = tri[2];
		    edges[0] = edges[2];
		    edges[1] = edges[3];

		    if (start-- == 0)
			start += pen->num_vertices;
		}
	    } else {
		_cairo_pen_find_active_cw_vertices (pen,
						    &in->dev_vector, &out->dev_vector,
						    &start, &stop);
		while (start != stop) {
		    tri[2] = in->point;
		    translate_point (&tri[2], &pen->vertices[start].point);
		    edges[2] = in->point;
		    edges[3] = tri[2];
		    _cairo_traps_tessellate_triangle_with_edges (stroker->traps,
								 tri, edges);
		    tri[1] = tri[2];
		    edges[0] = edges[2];
		    edges[1] = edges[3];

		    if (++start == pen->num_vertices)
			start = 0;
		}
	    }
	    tri[2] = *outpt;
	    edges[2] = out->cw;
	    edges[3] = out->ccw;
	    _cairo_traps_tessellate_triangle_with_edges (stroker->traps,
							 tri, edges);
	} else {
	    cairo_point_t t[] = { { in->point.x, in->point.y}, { inpt->x, inpt->y }, { outpt->x, outpt->y } };
	    cairo_point_t e[] = { { in->cw.x, in->cw.y}, { in->ccw.x, in->ccw.y },
				  { out->cw.x, out->cw.y}, { out->ccw.x, out->ccw.y } };
	    _cairo_traps_tessellate_triangle_with_edges (stroker->traps, t, e);
	}
	break;

    case CAIRO_LINE_JOIN_MITER:
    default: {
	/* dot product of incoming slope vector with outgoing slope vector */
	double in_dot_out = (-in->usr_vector.x * out->usr_vector.x +
			     -in->usr_vector.y * out->usr_vector.y);
	double ml = stroker->style->miter_limit;

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
	    cairo_point_t	outer;
	    cairo_point_t	quad[4];
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
	    if (slope_compare_sgn (fdx1, fdy1, mdx, mdy) !=
		slope_compare_sgn (fdx2, fdy2, mdx, mdy))
	    {
		/*
		 * Draw the quadrilateral
		 */
		outer.x = _cairo_fixed_from_double (mx);
		outer.y = _cairo_fixed_from_double (my);

		quad[0] = in->point;
		quad[1] = *inpt;
		quad[2] = outer;
		quad[3] = *outpt;

		_cairo_traps_tessellate_convex_quad (stroker->traps, quad);
		break;
	    }
	}
    }
    /* fall through ... */
    case CAIRO_LINE_JOIN_BEVEL: {
	cairo_point_t t[] = { { in->point.x, in->point.y }, { inpt->x, inpt->y }, { outpt->x, outpt->y } };
	cairo_point_t e[] = { { in->cw.x, in->cw.y }, { in->ccw.x, in->ccw.y },
			      { out->cw.x, out->cw.y }, { out->ccw.x, out->ccw.y } };
	_cairo_traps_tessellate_triangle_with_edges (stroker->traps, t, e);
	break;
    }
    }
}

static void
add_cap (struct stroker *stroker, cairo_stroke_face_t *f)
{
    switch (stroker->style->line_cap) {
    case CAIRO_LINE_CAP_ROUND: {
	int start, stop;
	cairo_slope_t in_slope, out_slope;
	cairo_point_t tri[3], edges[4];
	cairo_pen_t *pen = &stroker->pen;

	in_slope = f->dev_vector;
	out_slope.dx = -in_slope.dx;
	out_slope.dy = -in_slope.dy;
	_cairo_pen_find_active_cw_vertices (pen, &in_slope, &out_slope,
					    &start, &stop);
	edges[0] = f->cw;
	edges[1] = f->ccw;
	tri[0] = f->point;
	tri[1] = f->cw;
	while (start != stop) {
	    tri[2] = f->point;
	    translate_point (&tri[2], &pen->vertices[start].point);
	    edges[2] = f->point;
	    edges[3] = tri[2];
	    _cairo_traps_tessellate_triangle_with_edges (stroker->traps,
							 tri, edges);

	    tri[1] = tri[2];
	    edges[0] = edges[2];
	    edges[1] = edges[3];
	    if (++start == pen->num_vertices)
		start = 0;
	}
	tri[2] = f->ccw;
	edges[2] = f->cw;
	edges[3] = f->ccw;
	_cairo_traps_tessellate_triangle_with_edges (stroker->traps,
						     tri, edges);
	break;
    }

    case CAIRO_LINE_CAP_SQUARE: {
	double dx, dy;
	cairo_slope_t fvector;
	cairo_point_t quad[4];

	dx = f->usr_vector.x;
	dy = f->usr_vector.y;
	dx *= stroker->half_line_width;
	dy *= stroker->half_line_width;
	cairo_matrix_transform_distance (stroker->ctm, &dx, &dy);
	fvector.dx = _cairo_fixed_from_double (dx);
	fvector.dy = _cairo_fixed_from_double (dy);

	quad[0] = f->cw;
	quad[1].x = f->cw.x + fvector.dx;
	quad[1].y = f->cw.y + fvector.dy;
	quad[2].x = f->ccw.x + fvector.dx;
	quad[2].y = f->ccw.y + fvector.dy;
	quad[3] = f->ccw;

	_cairo_traps_tessellate_convex_quad (stroker->traps, quad);
	break;
    }

    case CAIRO_LINE_CAP_BUTT:
    default:
	break;
    }
}

static void
add_leading_cap (struct stroker     *stroker,
		 cairo_stroke_face_t *face)
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

    add_cap (stroker, &reversed);
}

static void
add_trailing_cap (struct stroker *stroker, cairo_stroke_face_t *face)
{
    add_cap (stroker, face);
}

static inline double
normalize_slope (double *dx, double *dy)
{
    double dx0 = *dx, dy0 = *dy;

    if (dx0 == 0.0 && dy0 == 0.0)
	return 0;

    if (dx0 == 0.0) {
	*dx = 0.0;
	if (dy0 > 0.0) {
	    *dy = 1.0;
	    return dy0;
	} else {
	    *dy = -1.0;
	    return -dy0;
	}
    } else if (dy0 == 0.0) {
	*dy = 0.0;
	if (dx0 > 0.0) {
	    *dx = 1.0;
	    return dx0;
	} else {
	    *dx = -1.0;
	    return -dx0;
	}
    } else {
	double mag = hypot (dx0, dy0);
	*dx = dx0 / mag;
	*dy = dy0 / mag;
	return mag;
    }
}

static void
compute_face (const cairo_point_t *point,
	      const cairo_slope_t *dev_slope,
	      struct stroker *stroker,
	      cairo_stroke_face_t *face)
{
    double face_dx, face_dy;
    cairo_point_t offset_ccw, offset_cw;
    double slope_dx, slope_dy;

    slope_dx = _cairo_fixed_to_double (dev_slope->dx);
    slope_dy = _cairo_fixed_to_double (dev_slope->dy);
    face->length = normalize_slope (&slope_dx, &slope_dy);
    face->dev_slope.x = slope_dx;
    face->dev_slope.y = slope_dy;

    /*
     * rotate to get a line_width/2 vector along the face, note that
     * the vector must be rotated the right direction in device space,
     * but by 90° in user space. So, the rotation depends on
     * whether the ctm reflects or not, and that can be determined
     * by looking at the determinant of the matrix.
     */
    if (stroker->ctm_inverse) {
	cairo_matrix_transform_distance (stroker->ctm_inverse, &slope_dx, &slope_dy);
	normalize_slope (&slope_dx, &slope_dy);

	if (stroker->ctm_det_positive) {
	    face_dx = - slope_dy * stroker->half_line_width;
	    face_dy = slope_dx * stroker->half_line_width;
	} else {
	    face_dx = slope_dy * stroker->half_line_width;
	    face_dy = - slope_dx * stroker->half_line_width;
	}

	/* back to device space */
	cairo_matrix_transform_distance (stroker->ctm, &face_dx, &face_dy);
    } else {
	face_dx = - slope_dy * stroker->half_line_width;
	face_dy = slope_dx * stroker->half_line_width;
    }

    offset_ccw.x = _cairo_fixed_from_double (face_dx);
    offset_ccw.y = _cairo_fixed_from_double (face_dy);
    offset_cw.x = -offset_ccw.x;
    offset_cw.y = -offset_ccw.y;

    face->ccw = *point;
    translate_point (&face->ccw, &offset_ccw);

    face->point = *point;

    face->cw = *point;
    translate_point (&face->cw, &offset_cw);

    face->usr_vector.x = slope_dx;
    face->usr_vector.y = slope_dy;

    face->dev_vector = *dev_slope;
}

static void
add_caps (struct stroker *stroker)
{
    /* check for a degenerative sub_path */
    if (stroker->has_initial_sub_path &&
	!stroker->has_first_face &&
	!stroker->has_current_face &&
	stroker->style->line_cap == CAIRO_LINE_CAP_ROUND)
    {
	/* pick an arbitrary slope to use */
	cairo_slope_t slope = { CAIRO_FIXED_ONE, 0 };
	cairo_stroke_face_t face;

	/* arbitrarily choose first_point
	 * first_point and current_point should be the same */
	compute_face (&stroker->first_point, &slope, stroker, &face);

	add_leading_cap (stroker, &face);
	add_trailing_cap (stroker, &face);
    }

    if (stroker->has_first_face)
	add_leading_cap (stroker, &stroker->first_face);

    if (stroker->has_current_face)
	add_trailing_cap (stroker, &stroker->current_face);
}

static cairo_bool_t
stroker_intersects_edge (const struct stroker *stroker,
			 const cairo_stroke_face_t *start,
			 const cairo_stroke_face_t *end)
{
    cairo_box_t box;

    if (! stroker->has_bounds)
	return TRUE;

    if (_cairo_box_contains_point (&stroker->tight_bounds, &start->cw))
	return TRUE;
    box.p2 = box.p1 = start->cw;

    if (_cairo_box_contains_point (&stroker->tight_bounds, &start->ccw))
	return TRUE;
    _cairo_box_add_point (&box, &start->ccw);

    if (_cairo_box_contains_point (&stroker->tight_bounds, &end->cw))
	return TRUE;
    _cairo_box_add_point (&box, &end->cw);

    if (_cairo_box_contains_point (&stroker->tight_bounds, &end->ccw))
	return TRUE;
    _cairo_box_add_point (&box, &end->ccw);

    return (box.p2.x > stroker->tight_bounds.p1.x &&
	    box.p1.x < stroker->tight_bounds.p2.x &&
	    box.p2.y > stroker->tight_bounds.p1.y &&
	    box.p1.y < stroker->tight_bounds.p2.y);
}

static void
add_sub_edge (struct stroker *stroker,
	      const cairo_point_t *p1, const cairo_point_t *p2,
	      const cairo_slope_t *dev_slope,
	      cairo_stroke_face_t *start, cairo_stroke_face_t *end)
{
    cairo_point_t rectangle[4];

    compute_face (p1, dev_slope, stroker, start);

    *end = *start;
    end->point = *p2;
    rectangle[0].x = p2->x - p1->x;
    rectangle[0].y = p2->y - p1->y;
    translate_point (&end->ccw, &rectangle[0]);
    translate_point (&end->cw, &rectangle[0]);

    if (p1->x == p2->x && p1->y == p2->y)
	return;

    if (! stroker_intersects_edge (stroker, start, end))
	return;

    rectangle[0] = start->cw;
    rectangle[1] = start->ccw;
    rectangle[2] = end->ccw;
    rectangle[3] = end->cw;

    _cairo_traps_tessellate_convex_quad (stroker->traps, rectangle);
}

static cairo_status_t
move_to (void *closure, const cairo_point_t *point)
{
    struct stroker *stroker = closure;

    /* Cap the start and end of the previous sub path as needed */
    add_caps (stroker);

    stroker->first_point = *point;
    stroker->current_face.point = *point;

    stroker->has_first_face = FALSE;
    stroker->has_current_face = FALSE;
    stroker->has_initial_sub_path = FALSE;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
move_to_dashed (void *closure, const cairo_point_t *point)
{
    /* reset the dash pattern for new sub paths */
    struct stroker *stroker = closure;

    _cairo_stroker_dash_start (&stroker->dash);
    return move_to (closure, point);
}

static cairo_status_t
line_to (void *closure, const cairo_point_t *point)
{
    struct stroker *stroker = closure;
    cairo_stroke_face_t start, end;
    const cairo_point_t *p1 = &stroker->current_face.point;
    const cairo_point_t *p2 = point;
    cairo_slope_t dev_slope;

    stroker->has_initial_sub_path = TRUE;

    if (p1->x == p2->x && p1->y == p2->y)
	return CAIRO_STATUS_SUCCESS;

    _cairo_slope_init (&dev_slope, p1, p2);
    add_sub_edge (stroker, p1, p2, &dev_slope, &start, &end);

    if (stroker->has_current_face) {
	/* Join with final face from previous segment */
	join (stroker, &stroker->current_face, &start);
    } else if (!stroker->has_first_face) {
	/* Save sub path's first face in case needed for closing join */
	stroker->first_face = start;
	stroker->has_first_face = TRUE;
    }
    stroker->current_face = end;
    stroker->has_current_face = TRUE;

    return CAIRO_STATUS_SUCCESS;
}

/*
 * Dashed lines.  Cap each dash end, join around turns when on
 */
static cairo_status_t
line_to_dashed (void *closure, const cairo_point_t *point)
{
    struct stroker *stroker = closure;
    double mag, remain, step_length = 0;
    double slope_dx, slope_dy;
    double dx2, dy2;
    cairo_stroke_face_t sub_start, sub_end;
    const cairo_point_t *p1 = &stroker->current_face.point;
    const cairo_point_t *p2 = point;
    cairo_slope_t dev_slope;
    cairo_line_t segment;
    cairo_bool_t fully_in_bounds;

    stroker->has_initial_sub_path = stroker->dash.dash_starts_on;

    if (p1->x == p2->x && p1->y == p2->y)
	return CAIRO_STATUS_SUCCESS;

    fully_in_bounds = TRUE;
    if (stroker->has_bounds &&
	(! _cairo_box_contains_point (&stroker->join_bounds, p1) ||
	 ! _cairo_box_contains_point (&stroker->join_bounds, p2)))
    {
	fully_in_bounds = FALSE;
    }

    _cairo_slope_init (&dev_slope, p1, p2);

    slope_dx = _cairo_fixed_to_double (p2->x - p1->x);
    slope_dy = _cairo_fixed_to_double (p2->y - p1->y);

    if (stroker->ctm_inverse)
	cairo_matrix_transform_distance (stroker->ctm_inverse, &slope_dx, &slope_dy);
    mag = normalize_slope (&slope_dx, &slope_dy);
    if (mag <= DBL_EPSILON)
	return CAIRO_STATUS_SUCCESS;

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
	     _cairo_box_intersects_line_segment (&stroker->join_bounds, &segment)))
	{
	    add_sub_edge (stroker,
			  &segment.p1, &segment.p2,
			  &dev_slope,
			  &sub_start, &sub_end);

	    if (stroker->has_current_face) {
		/* Join with final face from previous segment */
		join (stroker, &stroker->current_face, &sub_start);

		stroker->has_current_face = FALSE;
	    } else if (! stroker->has_first_face && stroker->dash.dash_starts_on) {
		/* Save sub path's first face in case needed for closing join */
		stroker->first_face = sub_start;
		stroker->has_first_face = TRUE;
	    } else {
		/* Cap dash start if not connecting to a previous segment */
		add_leading_cap (stroker, &sub_start);
	    }

	    if (remain) {
		/* Cap dash end if not at end of segment */
		add_trailing_cap (stroker, &sub_end);
	    } else {
		stroker->current_face = sub_end;
		stroker->has_current_face = TRUE;
	    }
	} else {
	    if (stroker->has_current_face) {
		/* Cap final face from previous segment */
		add_trailing_cap (stroker, &stroker->current_face);

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
	compute_face (point, &dev_slope, stroker, &stroker->current_face);

	add_leading_cap (stroker, &stroker->current_face);

	stroker->has_current_face = TRUE;
    } else
	stroker->current_face.point = *point;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
add_point (void *closure,
	   const cairo_point_t *point,
	   const cairo_slope_t *tangent)
{
    return line_to_dashed (closure, point);
};

static cairo_status_t
spline_to (void *closure,
	   const cairo_point_t *point,
	   const cairo_slope_t *tangent)
{
    struct stroker *stroker = closure;
    cairo_stroke_face_t face;

    if ((tangent->dx | tangent->dy) == 0) {
	cairo_point_t t;

	face = stroker->current_face;

	face.usr_vector.x = -face.usr_vector.x;
	face.usr_vector.y = -face.usr_vector.y;
	face.dev_slope.x = -face.dev_slope.x;
	face.dev_slope.y = -face.dev_slope.y;
	face.dev_vector.dx = -face.dev_vector.dx;
	face.dev_vector.dy = -face.dev_vector.dy;

	t = face.cw;
	face.cw = face.ccw;
	face.ccw = t;

	join (stroker, &stroker->current_face, &face);
    } else {
	cairo_point_t rectangle[4];

	compute_face (&stroker->current_face.point, tangent, stroker, &face);
	join (stroker, &stroker->current_face, &face);

	rectangle[0] = face.cw;
	rectangle[1] = face.ccw;

	rectangle[2].x = point->x - face.point.x;
	rectangle[2].y = point->y - face.point.y;
	face.point = *point;
	translate_point (&face.ccw, &rectangle[2]);
	translate_point (&face.cw, &rectangle[2]);

	rectangle[2] = face.ccw;
	rectangle[3] = face.cw;

	_cairo_traps_tessellate_convex_quad (stroker->traps, rectangle);
    }

    stroker->current_face = face;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
curve_to (void *closure,
	  const cairo_point_t *b,
	  const cairo_point_t *c,
	  const cairo_point_t *d)
{
    struct stroker *stroker = closure;
    cairo_line_join_t line_join_save;
    cairo_spline_t spline;
    cairo_stroke_face_t face;
    cairo_status_t status;

    if (stroker->has_bounds &&
	! _cairo_spline_intersects (&stroker->current_face.point, b, c, d,
				    &stroker->line_bounds))
	return line_to (closure, d);

    if (! _cairo_spline_init (&spline, spline_to, stroker,
			      &stroker->current_face.point, b, c, d))
	return line_to (closure, d);

    compute_face (&stroker->current_face.point, &spline.initial_slope,
		  stroker, &face);

    if (stroker->has_current_face) {
	/* Join with final face from previous segment */
	join (stroker, &stroker->current_face, &face);
    } else {
	if (! stroker->has_first_face) {
	    /* Save sub path's first face in case needed for closing join */
	    stroker->first_face = face;
	    stroker->has_first_face = TRUE;
	}
	stroker->has_current_face = TRUE;
    }
    stroker->current_face = face;

    /* Temporarily modify the stroker to use round joins to guarantee
     * smooth stroked curves. */
    line_join_save = stroker->line_join;
    stroker->line_join = CAIRO_LINE_JOIN_ROUND;

    status = _cairo_spline_decompose (&spline, stroker->tolerance);

    stroker->line_join = line_join_save;

    return status;
}

static cairo_status_t
curve_to_dashed (void *closure,
		 const cairo_point_t *b,
		 const cairo_point_t *c,
		 const cairo_point_t *d)
{
    struct stroker *stroker = closure;
    cairo_spline_t spline;
    cairo_line_join_t line_join_save;
    cairo_spline_add_point_func_t func;
    cairo_status_t status;

    func = add_point;

    if (stroker->has_bounds &&
	! _cairo_spline_intersects (&stroker->current_face.point, b, c, d,
				    &stroker->line_bounds))
	return func (closure, d, NULL);

    if (! _cairo_spline_init (&spline, func, stroker,
			      &stroker->current_face.point, b, c, d))
	return func (closure, d, NULL);

    /* Temporarily modify the stroker to use round joins to guarantee
     * smooth stroked curves. */
    line_join_save = stroker->line_join;
    stroker->line_join = CAIRO_LINE_JOIN_ROUND;

    status = _cairo_spline_decompose (&spline, stroker->tolerance);

    stroker->line_join = line_join_save;

    return status;
}

static cairo_status_t
_close_path (struct stroker *stroker)
{
    if (stroker->has_first_face && stroker->has_current_face) {
	/* Join first and final faces of sub path */
	join (stroker, &stroker->current_face, &stroker->first_face);
    } else {
	/* Cap the start and end of the sub path as needed */
	add_caps (stroker);
    }

    stroker->has_initial_sub_path = FALSE;
    stroker->has_first_face = FALSE;
    stroker->has_current_face = FALSE;
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
close_path (void *closure)
{
    struct stroker *stroker = closure;
    cairo_status_t status;

    status = line_to (stroker, &stroker->first_point);
    if (unlikely (status))
	return status;

    return _close_path (stroker);
}

static cairo_status_t
close_path_dashed (void *closure)
{
    struct stroker *stroker = closure;
    cairo_status_t status;

    status = line_to_dashed (stroker, &stroker->first_point);
    if (unlikely (status))
	return status;

    return _close_path (stroker);
}

cairo_int_status_t
_cairo_path_fixed_stroke_to_traps (const cairo_path_fixed_t	*path,
				   const cairo_stroke_style_t	*style,
				   const cairo_matrix_t		*ctm,
				   const cairo_matrix_t		*ctm_inverse,
				   double			 tolerance,
				   cairo_traps_t		*traps)
{
    struct stroker stroker;
    cairo_status_t status;

    status = stroker_init (&stroker, path, style,
			   ctm, ctm_inverse, tolerance,
			   traps);
    if (unlikely (status))
	return status;

    if (stroker.dash.dashed)
	status = _cairo_path_fixed_interpret (path,
					      move_to_dashed,
					      line_to_dashed,
					      curve_to_dashed,
					      close_path_dashed,
					      &stroker);
    else
	status = _cairo_path_fixed_interpret (path,
					      move_to,
					      line_to,
					      curve_to,
					      close_path,
					      &stroker);
    assert(status == CAIRO_STATUS_SUCCESS);
    add_caps (&stroker);

    stroker_fini (&stroker);

    return traps->status;
}
