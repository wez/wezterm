/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
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
#include "cairo-contour-inline.h"
#include "cairo-contour-private.h"
#include "cairo-error-private.h"
#include "cairo-path-fixed-private.h"
#include "cairo-slope-private.h"

#define DEBUG 0

struct stroker {
    cairo_stroke_style_t style;

#if DEBUG
    cairo_contour_t path;
#endif

    struct stroke_contour {
	/* Note that these are not strictly contours as they may intersect */
	cairo_contour_t contour;
    } cw, ccw;
    cairo_uint64_t contour_tolerance;
    cairo_polygon_t *polygon;

    const cairo_matrix_t *ctm;
    const cairo_matrix_t *ctm_inverse;
    double tolerance;
    double spline_cusp_tolerance;
    double half_line_width;
    cairo_bool_t ctm_det_positive;

    cairo_pen_t pen;

    cairo_point_t first_point;

    cairo_bool_t has_initial_sub_path;

    cairo_bool_t has_current_face;
    cairo_stroke_face_t current_face;

    cairo_bool_t has_first_face;
    cairo_stroke_face_t first_face;

    cairo_bool_t has_bounds;
    cairo_box_t bounds;
};

static inline double
normalize_slope (double *dx, double *dy);

static void
compute_face (const cairo_point_t *point,
	      const cairo_slope_t *dev_slope,
	      struct stroker *stroker,
	      cairo_stroke_face_t *face);

static cairo_uint64_t
point_distance_sq (const cairo_point_t *p1,
			const cairo_point_t *p2)
{
    int32_t dx = p1->x - p2->x;
    int32_t dy = p1->y - p2->y;
    return _cairo_int32x32_64_mul (dx, dx) + _cairo_int32x32_64_mul (dy, dy);
}

static cairo_bool_t
within_tolerance (const cairo_point_t *p1,
	      const cairo_point_t *p2,
	      cairo_uint64_t tolerance)
{
    return FALSE;
    return _cairo_int64_lt (point_distance_sq (p1, p2), tolerance);
}

static void
contour_add_point (struct stroker *stroker,
		   struct stroke_contour *c,
		   const cairo_point_t *point)
{
    if (! within_tolerance (point, _cairo_contour_last_point (&c->contour),
			stroker->contour_tolerance))
	_cairo_contour_add_point (&c->contour, point);
    //*_cairo_contour_last_point (&c->contour) = *point;
}

static void
translate_point (cairo_point_t *point, const cairo_point_t *offset)
{
    point->x += offset->x;
    point->y += offset->y;
}

static int
slope_compare_sgn (double dx1, double dy1, double dx2, double dy2)
{
    double  c = (dx1 * dy2 - dx2 * dy1);

    if (c > 0) return 1;
    if (c < 0) return -1;
    return 0;
}

static inline int
range_step (int i, int step, int max)
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
static void
add_fan (struct stroker *stroker,
	 const cairo_slope_t *in_vector,
	 const cairo_slope_t *out_vector,
	 const cairo_point_t *midpt,
	 cairo_bool_t clockwise,
	 struct stroke_contour *c)
{
    cairo_pen_t *pen = &stroker->pen;
    int start, stop;

    if (stroker->has_bounds &&
	! _cairo_box_contains_point (&stroker->bounds, midpt))
	return;

    assert (stroker->pen.num_vertices);

    if (clockwise) {
	_cairo_pen_find_active_cw_vertices (pen,
					    in_vector, out_vector,
					    &start, &stop);
	while (start != stop) {
	    cairo_point_t p = *midpt;
	    translate_point (&p, &pen->vertices[start].point);
	    contour_add_point (stroker, c, &p);

	    if (++start == pen->num_vertices)
		start = 0;
	}
    } else {
	_cairo_pen_find_active_ccw_vertices (pen,
					     in_vector, out_vector,
					     &start, &stop);
	while (start != stop) {
	    cairo_point_t p = *midpt;
	    translate_point (&p, &pen->vertices[start].point);
	    contour_add_point (stroker, c, &p);

	    if (start-- == 0)
		start += pen->num_vertices;
	}
    }
}

static int
join_is_clockwise (const cairo_stroke_face_t *in,
		   const cairo_stroke_face_t *out)
{
    return _cairo_slope_compare (&in->dev_vector, &out->dev_vector) < 0;
}

static void
inner_join (struct stroker *stroker,
	    const cairo_stroke_face_t *in,
	    const cairo_stroke_face_t *out,
	    int clockwise)
{
#if 0
    cairo_point_t last;
    const cairo_point_t *p, *outpt;
    struct stroke_contour *inner;
    cairo_int64_t d_p, d_last;
    cairo_int64_t half_line_width;
    cairo_bool_t negate;

    /* XXX line segments shorter than line-width */

    if (clockwise) {
	inner = &stroker->ccw;
	outpt = &out->ccw;
	negate = 1;
    } else {
	inner = &stroker->cw;
	outpt = &out->cw;
	negate = 0;
    }

    half_line_width = CAIRO_FIXED_ONE*CAIRO_FIXED_ONE/2 * stroker->style.line_width * out->length + .5;

    /* On the inside, the previous end-point is always
     * closer to the new face by definition.
     */
    last = *_cairo_contour_last_point (&inner->contour);
    d_last = distance_from_face (out, &last, negate);
    _cairo_contour_remove_last_point (&inner->contour);

prev:
    if (inner->contour.chain.num_points == 0) {
	contour_add_point (stroker, inner, outpt);
	return;
    }
    p = _cairo_contour_last_point (&inner->contour);
    d_p = distance_from_face (out, p, negate);
    if (_cairo_int64_lt (d_p, half_line_width) &&
	!_cairo_int64_negative (distance_along_face (out, p)))
    {
	last = *p;
	d_last = d_p;
	_cairo_contour_remove_last_point (&inner->contour);
	goto prev;
    }

    compute_inner_joint (&last, d_last, p, d_p, half_line_width);
    contour_add_point (stroker, inner, &last);
#else
    const cairo_point_t *outpt;
    struct stroke_contour *inner;

    if (clockwise) {
	inner = &stroker->ccw;
	outpt = &out->ccw;
    } else {
	inner = &stroker->cw;
	outpt = &out->cw;
    }
    contour_add_point (stroker, inner, &in->point);
    contour_add_point (stroker, inner, outpt);
#endif
}

static void
inner_close (struct stroker *stroker,
	     const cairo_stroke_face_t *in,
	     cairo_stroke_face_t *out)
{
#if 0
    cairo_point_t last;
    const cairo_point_t *p, *outpt, *inpt;
    struct stroke_contour *inner;
    struct _cairo_contour_chain *chain;

    /* XXX line segments shorter than line-width */

    if (join_is_clockwise (in, out)) {
	inner = &stroker->ccw;
	outpt = &in->ccw;
	inpt = &out->ccw;
    } else {
	inner = &stroker->cw;
	outpt = &in->cw;
	inpt = &out->cw;
    }

    if (inner->contour.chain.num_points == 0) {
	contour_add_point (stroker, inner, &in->point);
	contour_add_point (stroker, inner, inpt);
	*_cairo_contour_first_point (&inner->contour) =
	    *_cairo_contour_last_point (&inner->contour);
	return;
    }

    line_width = stroker->style.line_width/2;
    line_width *= CAIRO_FIXED_ONE;

    d_last = sign * distance_from_face (out, outpt);
    last = *outpt;

    for (chain = &inner->contour.chain; chain; chain = chain->next) {
	for (i = 0; i < chain->num_points; i++) {
	    p = &chain->points[i];
	    if ((d_p = sign * distance_from_face (in, p)) >= line_width &&
		distance_from_edge (stroker, inpt, &last, p) < line_width)
	    {
		goto out;
	    }

	    if (p->x != last.x || p->y != last.y) {
		last = *p;
		d_last = d_p;
	    }
	}
    }
out:

    if (d_p != d_last) {
	double dot = (line_width - d_last) / (d_p - d_last);
	last.x += dot * (p->x - last.x);
	last.y += dot * (p->y - last.y);
    }
    *_cairo_contour_last_point (&inner->contour) = last;

    for (chain = &inner->contour.chain; chain; chain = chain->next) {
	for (i = 0; i < chain->num_points; i++) {
	    cairo_point_t *pp = &chain->points[i];
	    if (pp == p)
		return;
	    *pp = last;
	}
    }
#else
    const cairo_point_t *inpt;
    struct stroke_contour *inner;

    if (join_is_clockwise (in, out)) {
	inner = &stroker->ccw;
	inpt = &out->ccw;
    } else {
	inner = &stroker->cw;
	inpt = &out->cw;
    }

    contour_add_point (stroker, inner, &in->point);
    contour_add_point (stroker, inner, inpt);
    *_cairo_contour_first_point (&inner->contour) =
	*_cairo_contour_last_point (&inner->contour);
#endif
}

static void
outer_close (struct stroker *stroker,
	     const cairo_stroke_face_t *in,
	     const cairo_stroke_face_t *out)
{
    const cairo_point_t	*inpt, *outpt;
    struct stroke_contour *outer;
    int	clockwise;

    if (in->cw.x == out->cw.x && in->cw.y == out->cw.y &&
	in->ccw.x == out->ccw.x && in->ccw.y == out->ccw.y)
    {
	return;
    }

    clockwise = join_is_clockwise (in, out);
    if (clockwise) {
	inpt = &in->cw;
	outpt = &out->cw;
	outer = &stroker->cw;
    } else {
	inpt = &in->ccw;
	outpt = &out->ccw;
	outer = &stroker->ccw;
    }

    if (within_tolerance (inpt, outpt, stroker->contour_tolerance)) {
	*_cairo_contour_first_point (&outer->contour) =
	    *_cairo_contour_last_point (&outer->contour);
	return;
    }

    switch (stroker->style.line_join) {
    case CAIRO_LINE_JOIN_ROUND:
	if ((in->dev_slope.x * out->dev_slope.x +
	     in->dev_slope.y * out->dev_slope.y) < stroker->spline_cusp_tolerance)
	{
	    /* construct a fan around the common midpoint */
	    add_fan (stroker,
		     &in->dev_vector, &out->dev_vector, &in->point,
		     clockwise, outer);
	} /* else: bevel join */
	break;

    case CAIRO_LINE_JOIN_MITER:
    default: {
	/* dot product of incoming slope vector with outgoing slope vector */
	double	in_dot_out = in->dev_slope.x * out->dev_slope.x +
			     in->dev_slope.y * out->dev_slope.y;
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
	if (2 <= ml * ml * (1 + in_dot_out)) {
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
	    dx1 = in->dev_slope.x;
	    dy1 = in->dev_slope.y;

	    /* outer point of outgoing line face */
	    x2 = _cairo_fixed_to_double (outpt->x);
	    y2 = _cairo_fixed_to_double (outpt->y);
	    dx2 = out->dev_slope.x;
	    dy2 = out->dev_slope.y;

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
		cairo_point_t p;

		p.x = _cairo_fixed_from_double (mx);
		p.y = _cairo_fixed_from_double (my);

		*_cairo_contour_last_point (&outer->contour) = p;
		*_cairo_contour_first_point (&outer->contour) = p;
		return;
	    }
	}
	break;
    }

    case CAIRO_LINE_JOIN_BEVEL:
	break;
    }
    contour_add_point (stroker, outer, outpt);
}

static void
outer_join (struct stroker *stroker,
	    const cairo_stroke_face_t *in,
	    const cairo_stroke_face_t *out,
	    int clockwise)
{
    const cairo_point_t	*inpt, *outpt;
    struct stroke_contour *outer;

    if (in->cw.x == out->cw.x && in->cw.y == out->cw.y &&
	in->ccw.x == out->ccw.x && in->ccw.y == out->ccw.y)
    {
	return;
    }
    if (clockwise) {
	inpt = &in->cw;
	outpt = &out->cw;
	outer = &stroker->cw;
    } else {
	inpt = &in->ccw;
	outpt = &out->ccw;
	outer = &stroker->ccw;
    }

    switch (stroker->style.line_join) {
    case CAIRO_LINE_JOIN_ROUND:
	if ((in->dev_slope.x * out->dev_slope.x +
	     in->dev_slope.y * out->dev_slope.y) < stroker->spline_cusp_tolerance)
	{
	    /* construct a fan around the common midpoint */
	    add_fan (stroker,
		     &in->dev_vector, &out->dev_vector, &in->point,
		     clockwise, outer);
	} /* else: bevel join */
	break;

    case CAIRO_LINE_JOIN_MITER:
    default: {
	/* dot product of incoming slope vector with outgoing slope vector */
	double	in_dot_out = in->dev_slope.x * out->dev_slope.x +
			     in->dev_slope.y * out->dev_slope.y;
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
	if (2 <= ml * ml * (1 + in_dot_out)) {
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
	    dx1 = in->dev_slope.x;
	    dy1 = in->dev_slope.y;

	    /* outer point of outgoing line face */
	    x2 = _cairo_fixed_to_double (outpt->x);
	    y2 = _cairo_fixed_to_double (outpt->y);
	    dx2 = out->dev_slope.x;
	    dy2 = out->dev_slope.y;

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
		cairo_point_t p;

		p.x = _cairo_fixed_from_double (mx);
		p.y = _cairo_fixed_from_double (my);

		*_cairo_contour_last_point (&outer->contour) = p;
		return;
	    }
	}
	break;
    }

    case CAIRO_LINE_JOIN_BEVEL:
	break;
    }
    contour_add_point (stroker,outer, outpt);
}

static void
add_cap (struct stroker *stroker,
	 const cairo_stroke_face_t *f,
	 struct stroke_contour *c)
{
    switch (stroker->style.line_cap) {
    case CAIRO_LINE_CAP_ROUND: {
	cairo_slope_t slope;

	slope.dx = -f->dev_vector.dx;
	slope.dy = -f->dev_vector.dy;

	add_fan (stroker, &f->dev_vector, &slope, &f->point, FALSE, c);
	break;
    }

    case CAIRO_LINE_CAP_SQUARE: {
	cairo_slope_t fvector;
	cairo_point_t p;
	double dx, dy;

	dx = f->usr_vector.x;
	dy = f->usr_vector.y;
	dx *= stroker->half_line_width;
	dy *= stroker->half_line_width;
	cairo_matrix_transform_distance (stroker->ctm, &dx, &dy);
	fvector.dx = _cairo_fixed_from_double (dx);
	fvector.dy = _cairo_fixed_from_double (dy);

	p.x = f->ccw.x + fvector.dx;
	p.y = f->ccw.y + fvector.dy;
	contour_add_point (stroker, c, &p);

	p.x = f->cw.x + fvector.dx;
	p.y = f->cw.y + fvector.dy;
	contour_add_point (stroker, c, &p);
    }

    case CAIRO_LINE_CAP_BUTT:
    default:
	break;
    }
    contour_add_point (stroker, c, &f->cw);
}

static void
add_leading_cap (struct stroker *stroker,
		 const cairo_stroke_face_t *face,
		 struct stroke_contour *c)
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

    add_cap (stroker, &reversed, c);
}

static void
add_trailing_cap (struct stroker *stroker,
		  const cairo_stroke_face_t *face,
		  struct stroke_contour *c)
{
    add_cap (stroker, face, c);
}

static inline double
normalize_slope (double *dx, double *dy)
{
    double dx0 = *dx, dy0 = *dy;
    double mag;

    assert (dx0 != 0.0 || dy0 != 0.0);

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

    return mag;
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
    if (! _cairo_matrix_is_identity (stroker->ctm_inverse)) {
	/* Normalize the matrix! */
	cairo_matrix_transform_distance (stroker->ctm_inverse,
					 &slope_dx, &slope_dy);
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
	! stroker->has_first_face &&
	! stroker->has_current_face &&
	stroker->style.line_cap == CAIRO_LINE_CAP_ROUND)
    {
	/* pick an arbitrary slope to use */
	cairo_slope_t slope = { CAIRO_FIXED_ONE, 0 };
	cairo_stroke_face_t face;

	/* arbitrarily choose first_point */
	compute_face (&stroker->first_point, &slope, stroker, &face);

	add_leading_cap (stroker, &face, &stroker->ccw);
	add_trailing_cap (stroker, &face, &stroker->ccw);

	/* ensure the circle is complete */
	_cairo_contour_add_point (&stroker->ccw.contour,
				  _cairo_contour_first_point (&stroker->ccw.contour));

	_cairo_polygon_add_contour (stroker->polygon, &stroker->ccw.contour);
	_cairo_contour_reset (&stroker->ccw.contour);
    } else {
	if (stroker->has_current_face)
	    add_trailing_cap (stroker, &stroker->current_face, &stroker->ccw);

#if DEBUG
	{
	    FILE *file = fopen ("contours.txt", "a");
	    _cairo_debug_print_contour (file, &stroker->path);
	    _cairo_debug_print_contour (file, &stroker->cw.contour);
	    _cairo_debug_print_contour (file, &stroker->ccw.contour);
	    fclose (file);
	    _cairo_contour_reset (&stroker->path);
	}
#endif

	_cairo_polygon_add_contour (stroker->polygon, &stroker->ccw.contour);
	_cairo_contour_reset (&stroker->ccw.contour);

	if (stroker->has_first_face) {
	    _cairo_contour_add_point (&stroker->ccw.contour,
				      &stroker->first_face.cw);
	    add_leading_cap (stroker, &stroker->first_face, &stroker->ccw);
#if DEBUG
	    {
		FILE *file = fopen ("contours.txt", "a");
		_cairo_debug_print_contour (file, &stroker->ccw.contour);
		fclose (file);
	    }
#endif

	    _cairo_polygon_add_contour (stroker->polygon,
					&stroker->ccw.contour);
	    _cairo_contour_reset (&stroker->ccw.contour);
	}

	_cairo_polygon_add_contour (stroker->polygon, &stroker->cw.contour);
	_cairo_contour_reset (&stroker->cw.contour);
    }
}

static cairo_status_t
close_path (void *closure);

static cairo_status_t
move_to (void *closure,
	 const cairo_point_t *point)
{
    struct stroker *stroker = closure;

    /* Cap the start and end of the previous sub path as needed */
    add_caps (stroker);

    stroker->has_first_face = FALSE;
    stroker->has_current_face = FALSE;
    stroker->has_initial_sub_path = FALSE;

    stroker->first_point = *point;

#if DEBUG
    _cairo_contour_add_point (&stroker->path, point);
#endif

    stroker->current_face.point = *point;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
line_to (void *closure,
	 const cairo_point_t *point)
{
    struct stroker *stroker = closure;
    cairo_stroke_face_t start;
    cairo_point_t *p1 = &stroker->current_face.point;
    cairo_slope_t dev_slope;

    stroker->has_initial_sub_path = TRUE;

    if (p1->x == point->x && p1->y == point->y)
	return CAIRO_STATUS_SUCCESS;

#if DEBUG
    _cairo_contour_add_point (&stroker->path, point);
#endif

    _cairo_slope_init (&dev_slope, p1, point);
    compute_face (p1, &dev_slope, stroker, &start);

    if (stroker->has_current_face) {
	int clockwise = _cairo_slope_compare (&stroker->current_face.dev_vector,
					      &start.dev_vector);
	if (clockwise) {
	    clockwise = clockwise < 0;
	    /* Join with final face from previous segment */
	    if (! within_tolerance (&stroker->current_face.ccw, &start.ccw,
				    stroker->contour_tolerance) ||
		! within_tolerance (&stroker->current_face.cw, &start.cw,
				    stroker->contour_tolerance))
	    {
		outer_join (stroker, &stroker->current_face, &start, clockwise);
		inner_join (stroker, &stroker->current_face, &start, clockwise);
	    }
	}
    } else {
	if (! stroker->has_first_face) {
	    /* Save sub path's first face in case needed for closing join */
	    stroker->first_face = start;
	    stroker->has_first_face = TRUE;
	}
	stroker->has_current_face = TRUE;

	contour_add_point (stroker, &stroker->cw, &start.cw);
	contour_add_point (stroker, &stroker->ccw, &start.ccw);
    }

    stroker->current_face = start;
    stroker->current_face.point = *point;
    stroker->current_face.ccw.x += dev_slope.dx;
    stroker->current_face.ccw.y += dev_slope.dy;
    stroker->current_face.cw.x += dev_slope.dx;
    stroker->current_face.cw.y += dev_slope.dy;

    contour_add_point (stroker, &stroker->cw, &stroker->current_face.cw);
    contour_add_point (stroker, &stroker->ccw, &stroker->current_face.ccw);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
spline_to (void *closure,
	   const cairo_point_t *point,
	   const cairo_slope_t *tangent)
{
    struct stroker *stroker = closure;
    cairo_stroke_face_t face;

#if DEBUG
    _cairo_contour_add_point (&stroker->path, point);
#endif
    if ((tangent->dx | tangent->dy) == 0) {
	struct stroke_contour *outer;
	cairo_point_t t;
	int clockwise;

	face = stroker->current_face;

	face.usr_vector.x = -face.usr_vector.x;
	face.usr_vector.y = -face.usr_vector.y;
	face.dev_vector.dx = -face.dev_vector.dx;
	face.dev_vector.dy = -face.dev_vector.dy;

	t = face.cw;
	face.cw = face.ccw;
	face.ccw = t;

	clockwise = join_is_clockwise (&stroker->current_face, &face);
	if (clockwise) {
	    outer = &stroker->cw;
	} else {
	    outer = &stroker->ccw;
	}

	add_fan (stroker,
		 &stroker->current_face.dev_vector,
		 &face.dev_vector,
		 &stroker->current_face.point,
		 clockwise, outer);
    } else {
	compute_face (point, tangent, stroker, &face);

	if ((face.dev_slope.x * stroker->current_face.dev_slope.x +
	     face.dev_slope.y * stroker->current_face.dev_slope.y) < stroker->spline_cusp_tolerance)
	{
	    struct stroke_contour *outer;
	    int clockwise = join_is_clockwise (&stroker->current_face, &face);

	    stroker->current_face.cw.x += face.point.x - stroker->current_face.point.x;
	    stroker->current_face.cw.y += face.point.y - stroker->current_face.point.y;
	    contour_add_point (stroker, &stroker->cw, &stroker->current_face.cw);

	    stroker->current_face.ccw.x += face.point.x - stroker->current_face.point.x;
	    stroker->current_face.ccw.y += face.point.y - stroker->current_face.point.y;
	    contour_add_point (stroker, &stroker->ccw, &stroker->current_face.ccw);

	    if (clockwise) {
		outer = &stroker->cw;
	    } else {
		outer = &stroker->ccw;
	    }
	    add_fan (stroker,
		     &stroker->current_face.dev_vector,
		     &face.dev_vector,
		     &stroker->current_face.point,
		     clockwise, outer);
	}

	contour_add_point (stroker, &stroker->cw, &face.cw);
	contour_add_point (stroker, &stroker->ccw, &face.ccw);
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
    cairo_spline_t spline;
    cairo_stroke_face_t face;

    if (stroker->has_bounds &&
	! _cairo_spline_intersects (&stroker->current_face.point, b, c, d,
				    &stroker->bounds))
	return line_to (closure, d);

    if (! _cairo_spline_init (&spline, spline_to, stroker,
			      &stroker->current_face.point, b, c, d))
	return line_to (closure, d);

    compute_face (&stroker->current_face.point, &spline.initial_slope,
		  stroker, &face);

    if (stroker->has_current_face) {
	int clockwise = join_is_clockwise (&stroker->current_face, &face);
	/* Join with final face from previous segment */
	outer_join (stroker, &stroker->current_face, &face, clockwise);
	inner_join (stroker, &stroker->current_face, &face, clockwise);
    } else {
	if (! stroker->has_first_face) {
	    /* Save sub path's first face in case needed for closing join */
	    stroker->first_face = face;
	    stroker->has_first_face = TRUE;
	}
	stroker->has_current_face = TRUE;

	contour_add_point (stroker, &stroker->cw, &face.cw);
	contour_add_point (stroker, &stroker->ccw, &face.ccw);
    }
    stroker->current_face = face;

    return _cairo_spline_decompose (&spline, stroker->tolerance);
}

static cairo_status_t
close_path (void *closure)
{
    struct stroker *stroker = closure;
    cairo_status_t status;

    status = line_to (stroker, &stroker->first_point);
    if (unlikely (status))
	return status;

    if (stroker->has_first_face && stroker->has_current_face) {
	/* Join first and final faces of sub path */
	outer_close (stroker, &stroker->current_face, &stroker->first_face);
	inner_close (stroker, &stroker->current_face, &stroker->first_face);
#if 0
	*_cairo_contour_first_point (&stroker->ccw.contour) =
	    *_cairo_contour_last_point (&stroker->ccw.contour);

	*_cairo_contour_first_point (&stroker->cw.contour) =
	    *_cairo_contour_last_point (&stroker->cw.contour);
#endif

	_cairo_polygon_add_contour (stroker->polygon, &stroker->cw.contour);
	_cairo_polygon_add_contour (stroker->polygon, &stroker->ccw.contour);

#if DEBUG
	{
	    FILE *file = fopen ("contours.txt", "a");
	    _cairo_debug_print_contour (file, &stroker->path);
	    _cairo_debug_print_contour (file, &stroker->cw.contour);
	    _cairo_debug_print_contour (file, &stroker->ccw.contour);
	    fclose (file);

	    _cairo_contour_reset (&stroker->path);
	}
#endif
	_cairo_contour_reset (&stroker->cw.contour);
	_cairo_contour_reset (&stroker->ccw.contour);
    } else {
	/* Cap the start and end of the sub path as needed */
	add_caps (stroker);
    }

    stroker->has_initial_sub_path = FALSE;
    stroker->has_first_face = FALSE;
    stroker->has_current_face = FALSE;

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_path_fixed_stroke_to_polygon (const cairo_path_fixed_t	*path,
				     const cairo_stroke_style_t	*style,
				     const cairo_matrix_t	*ctm,
				     const cairo_matrix_t	*ctm_inverse,
				     double		 tolerance,
				     cairo_polygon_t *polygon)
{
    struct stroker stroker;
    cairo_status_t status;

    if (style->num_dashes) {
	return _cairo_path_fixed_stroke_dashed_to_polygon (path,
							   style,
							   ctm,
							   ctm_inverse,
							   tolerance,
							   polygon);
    }

    stroker.has_bounds = polygon->num_limits;
    if (stroker.has_bounds) {
	/* Extend the bounds in each direction to account for the maximum area
	 * we might generate trapezoids, to capture line segments that are
	 * outside of the bounds but which might generate rendering that's
	 * within bounds.
	 */
	double dx, dy;
	cairo_fixed_t fdx, fdy;
	int i;

	stroker.bounds = polygon->limits[0];
	for (i = 1; i < polygon->num_limits; i++)
	     _cairo_box_add_box (&stroker.bounds, &polygon->limits[i]);

	_cairo_stroke_style_max_distance_from_path (style, path, ctm, &dx, &dy);
	fdx = _cairo_fixed_from_double (dx);
	fdy = _cairo_fixed_from_double (dy);

	stroker.bounds.p1.x -= fdx;
	stroker.bounds.p2.x += fdx;
	stroker.bounds.p1.y -= fdy;
	stroker.bounds.p2.y += fdy;
    }

    stroker.style = *style;
    stroker.ctm = ctm;
    stroker.ctm_inverse = ctm_inverse;
    stroker.tolerance = tolerance;
    stroker.half_line_width = style->line_width / 2.;

    /* If `CAIRO_LINE_JOIN_ROUND` is selected and a joint's `arc height`
     * is greater than `tolerance` then two segments are joined with
     * round-join, otherwise bevel-join is used.
     *
     * (See https://gitlab.freedesktop.org/cairo/cairo/-/merge_requests/372#note_1698225
     *  for an illustration.)
     *
     * `Arc height` is the distance from the center of arc's chord to
     * the center of the arc. It is also the difference of arc's radius
     * and the "distance from a point where segments are joined to the
     * chord" (distance to the chord). Arc's radius is the half of a line
     * width and the "distance to the chord" is equal to "half of a line width"
     * times `cos(half the angle between segment vectors)`. So
     *
     *     arc_height = w/2 - w/2 * cos(phi/2),
     *
     * where `w/2` is the "half of a line width".
     *
     * Using the double angle cosine formula we can express the `cos(phi/2)`
     * with just `cos(phi)` which is also the dot product of segments'
     * unit vectors.
     *
     *     cos(phi/2) = sqrt ( (1 + cos(phi)) / 2 );
     *     cos(phi/2) is in [0; 1] range, cannot be negative;
     *
     *     cos(phi) = a . b  = (ax * bx + ay * by),
     *
     * where `a` and `b` are unit vectors of the segments to be joined.
     *
     * Since `arc height` should be greater than the `tolerance` to produce
     * a round-join we can write
     *
     *     w/2 * (1 - cos(phi/2))  >  tolerance;
     *     1 - tolerance / (w/2)  >  cos(phi/2);    [!]
     *
     * which can be rewritten with the above double angle formula to
     *
     *     cos(phi)  <  2 * ( 1 - tolerance / (w/2) )^2 - 1,
     *
     * [!] Note that `w/2` is in [tolerance; +inf] range, since `cos(phi/2)`
     * cannot be negative. The left part of the above inequality is the
     * dot product and the right part is the `spline_cusp_tolerance`:
     *
     *     (ax * bx + ay * by) < spline_cusp_tolerance.
     *
     * In the code below only the `spline_cusp_tolerance` is calculated.
     * The dot product is calculated later, in the condition expression
     * itself. "Half of a line width" must be scaled with CTM for tolerance
     * condition to be properly met. Also, since `arch height` cannot exceed
     * the "half of a line width" and since `cos(phi/2)` cannot be negative,
     * when `tolerance` is greater than the "half of a line width" the
     * bevel-join should be produced.
     */
    double scaled_hlw = hypot(stroker.half_line_width * ctm->xx,
			      stroker.half_line_width * ctm->yx);

    if (scaled_hlw <= tolerance) {
	stroker.spline_cusp_tolerance = -1.0;
    } else {
	stroker.spline_cusp_tolerance = 1 - tolerance / scaled_hlw;
	stroker.spline_cusp_tolerance *= stroker.spline_cusp_tolerance;
	stroker.spline_cusp_tolerance *= 2;
	stroker.spline_cusp_tolerance -= 1;
    }

    stroker.ctm_det_positive =
	_cairo_matrix_compute_determinant (ctm) >= 0.0;

    stroker.pen.num_vertices = 0;
    if (path->has_curve_to ||
	style->line_join == CAIRO_LINE_JOIN_ROUND ||
	style->line_cap == CAIRO_LINE_CAP_ROUND) {
	status = _cairo_pen_init (&stroker.pen,
				  stroker.half_line_width,
				  tolerance, ctm);
	if (unlikely (status))
	    return status;

	/* If the line width is so small that the pen is reduced to a
	   single point, then we have nothing to do. */
	if (stroker.pen.num_vertices <= 1)
	    return CAIRO_STATUS_SUCCESS;
    }

    stroker.has_current_face = FALSE;
    stroker.has_first_face = FALSE;
    stroker.has_initial_sub_path = FALSE;

#if DEBUG
    remove ("contours.txt");
    remove ("polygons.txt");
    _cairo_contour_init (&stroker.path, 0);
#endif
    _cairo_contour_init (&stroker.cw.contour, 1);
    _cairo_contour_init (&stroker.ccw.contour, -1);
    tolerance *= CAIRO_FIXED_ONE;
    tolerance *= tolerance;
    stroker.contour_tolerance = tolerance;
    stroker.polygon = polygon;

    status = _cairo_path_fixed_interpret (path,
					  move_to,
					  line_to,
					  curve_to,
					  close_path,
					  &stroker);
    /* Cap the start and end of the final sub path as needed */
    if (likely (status == CAIRO_STATUS_SUCCESS))
	add_caps (&stroker);

    _cairo_contour_fini (&stroker.cw.contour);
    _cairo_contour_fini (&stroker.ccw.contour);
    if (stroker.pen.num_vertices)
	_cairo_pen_fini (&stroker.pen);

#if DEBUG
    {
	FILE *file = fopen ("polygons.txt", "a");
	_cairo_debug_print_polygon (file, polygon);
	fclose (file);
    }
#endif

    return status;
}
