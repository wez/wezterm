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
 */

#include "cairoint.h"

#include "cairo-arc-private.h"

#define MAX_FULL_CIRCLES 65536

/* Spline deviation from the circle in radius would be given by:

	error = sqrt (x**2 + y**2) - 1

   A simpler error function to work with is:

	e = x**2 + y**2 - 1

   From "Good approximation of circles by curvature-continuous Bezier
   curves", Tor Dokken and Morten Daehlen, Computer Aided Geometric
   Design 8 (1990) 22-41, we learn:

	abs (max(e)) = 4/27 * sin**6(angle/4) / cos**2(angle/4)

   and
	abs (error) =~ 1/2 * e

   Of course, this error value applies only for the particular spline
   approximation that is used in _cairo_gstate_arc_segment.
*/
static double
_arc_error_normalized (double angle)
{
    return 2.0/27.0 * pow (sin (angle / 4), 6) / pow (cos (angle / 4), 2);
}

static double
_arc_max_angle_for_tolerance_normalized (double tolerance)
{
    double angle, error;
    int i;

    /* Use table lookup to reduce search time in most cases. */
    struct {
	double angle;
	double error;
    } table[] = {
	{ M_PI / 1.0,   0.0185185185185185036127 },
	{ M_PI / 2.0,   0.000272567143730179811158 },
	{ M_PI / 3.0,   2.38647043651461047433e-05 },
	{ M_PI / 4.0,   4.2455377443222443279e-06 },
	{ M_PI / 5.0,   1.11281001494389081528e-06 },
	{ M_PI / 6.0,   3.72662000942734705475e-07 },
	{ M_PI / 7.0,   1.47783685574284411325e-07 },
	{ M_PI / 8.0,   6.63240432022601149057e-08 },
	{ M_PI / 9.0,   3.2715520137536980553e-08 },
	{ M_PI / 10.0,  1.73863223499021216974e-08 },
	{ M_PI / 11.0,  9.81410988043554039085e-09 },
    };
    int table_size = ARRAY_LENGTH (table);
    const int max_segments = 1000; /* this value is chosen arbitrarily. this gives an error of about 1.74909e-20 */

    for (i = 0; i < table_size; i++)
	if (table[i].error < tolerance)
	    return table[i].angle;

    ++i;

    do {
	angle = M_PI / i++;
	error = _arc_error_normalized (angle);
    } while (error > tolerance && i < max_segments);

    return angle;
}

static int
_arc_segments_needed (double	      angle,
		      double	      radius,
		      cairo_matrix_t *ctm,
		      double	      tolerance)
{
    double major_axis, max_angle;

    /* the error is amplified by at most the length of the
     * major axis of the circle; see cairo-pen.c for a more detailed analysis
     * of this. */
    major_axis = _cairo_matrix_transformed_circle_major_axis (ctm, radius);
    max_angle = _arc_max_angle_for_tolerance_normalized (tolerance / major_axis);

    return ceil (fabs (angle) / max_angle);
}

/* We want to draw a single spline approximating a circular arc radius
   R from angle A to angle B. Since we want a symmetric spline that
   matches the endpoints of the arc in position and slope, we know
   that the spline control points must be:

	(R * cos(A), R * sin(A))
	(R * cos(A) - h * sin(A), R * sin(A) + h * cos (A))
	(R * cos(B) + h * sin(B), R * sin(B) - h * cos (B))
	(R * cos(B), R * sin(B))

   for some value of h.

   "Approximation of circular arcs by cubic polynomials", Michael
   Goldapp, Computer Aided Geometric Design 8 (1991) 227-238, provides
   various values of h along with error analysis for each.

   From that paper, a very practical value of h is:

	h = 4/3 * R * tan(angle/4)

   This value does not give the spline with minimal error, but it does
   provide a very good approximation, (6th-order convergence), and the
   error expression is quite simple, (see the comment for
   _arc_error_normalized).
*/
static void
_cairo_arc_segment (cairo_t *cr,
		    double   xc,
		    double   yc,
		    double   radius,
		    double   angle_A,
		    double   angle_B)
{
    double r_sin_A, r_cos_A;
    double r_sin_B, r_cos_B;
    double h;

    r_sin_A = radius * sin (angle_A);
    r_cos_A = radius * cos (angle_A);
    r_sin_B = radius * sin (angle_B);
    r_cos_B = radius * cos (angle_B);

    h = 4.0/3.0 * tan ((angle_B - angle_A) / 4.0);

    cairo_curve_to (cr,
		    xc + r_cos_A - h * r_sin_A,
		    yc + r_sin_A + h * r_cos_A,
		    xc + r_cos_B + h * r_sin_B,
		    yc + r_sin_B - h * r_cos_B,
		    xc + r_cos_B,
		    yc + r_sin_B);
}

static void
_cairo_arc_in_direction (cairo_t	  *cr,
			 double		   xc,
			 double		   yc,
			 double		   radius,
			 double		   angle_min,
			 double		   angle_max,
			 cairo_direction_t dir)
{
    if (cairo_status (cr))
        return;

    assert (angle_max >= angle_min);

    if (angle_max - angle_min > 2 * M_PI * MAX_FULL_CIRCLES) {
	angle_max = fmod (angle_max - angle_min, 2 * M_PI);
	angle_min = fmod (angle_min, 2 * M_PI);
	angle_max += angle_min + 2 * M_PI * MAX_FULL_CIRCLES;
    }

    /* Recurse if drawing arc larger than pi */
    if (angle_max - angle_min > M_PI) {
	double angle_mid = angle_min + (angle_max - angle_min) / 2.0;
	if (dir == CAIRO_DIRECTION_FORWARD) {
	    _cairo_arc_in_direction (cr, xc, yc, radius,
				     angle_min, angle_mid,
				     dir);

	    _cairo_arc_in_direction (cr, xc, yc, radius,
				     angle_mid, angle_max,
				     dir);
	} else {
	    _cairo_arc_in_direction (cr, xc, yc, radius,
				     angle_mid, angle_max,
				     dir);

	    _cairo_arc_in_direction (cr, xc, yc, radius,
				     angle_min, angle_mid,
				     dir);
	}
    } else if (angle_max != angle_min) {
	cairo_matrix_t ctm;
	int i, segments;
	double step;

	cairo_get_matrix (cr, &ctm);
	segments = _arc_segments_needed (angle_max - angle_min,
					 radius, &ctm,
					 cairo_get_tolerance (cr));
	step = (angle_max - angle_min) / segments;
	segments -= 1;

	if (dir == CAIRO_DIRECTION_REVERSE) {
	    double t;

	    t = angle_min;
	    angle_min = angle_max;
	    angle_max = t;

	    step = -step;
	}

	cairo_line_to (cr,
		       xc + radius * cos (angle_min),
		       yc + radius * sin (angle_min));

	for (i = 0; i < segments; i++, angle_min += step) {
	    _cairo_arc_segment (cr, xc, yc, radius,
				angle_min, angle_min + step);
	}

	_cairo_arc_segment (cr, xc, yc, radius,
			    angle_min, angle_max);
    } else {
	cairo_line_to (cr,
		       xc + radius * cos (angle_min),
		       yc + radius * sin (angle_min));
    }
}

/**
 * _cairo_arc_path:
 * @cr: a cairo context
 * @xc: X position of the center of the arc
 * @yc: Y position of the center of the arc
 * @radius: the radius of the arc
 * @angle1: the start angle, in radians
 * @angle2: the end angle, in radians
 *
 * Compute a path for the given arc and append it onto the current
 * path within @cr. The arc will be accurate within the current
 * tolerance and given the current transformation.
 **/
void
_cairo_arc_path (cairo_t *cr,
		 double	  xc,
		 double	  yc,
		 double	  radius,
		 double	  angle1,
		 double	  angle2)
{
    _cairo_arc_in_direction (cr, xc, yc,
			     radius,
			     angle1, angle2,
			     CAIRO_DIRECTION_FORWARD);
}

/**
 * _cairo_arc_path_negative:
 * @xc: X position of the center of the arc
 * @yc: Y position of the center of the arc
 * @radius: the radius of the arc
 * @angle1: the start angle, in radians
 * @angle2: the end angle, in radians
 * @ctm: the current transformation matrix
 * @tolerance: the current tolerance value
 * @path: the path onto which the arc will be appended
 *
 * Compute a path for the given arc (defined in the negative
 * direction) and append it onto the current path within @cr. The arc
 * will be accurate within the current tolerance and given the current
 * transformation.
 **/
void
_cairo_arc_path_negative (cairo_t *cr,
			  double   xc,
			  double   yc,
			  double   radius,
			  double   angle1,
			  double   angle2)
{
    _cairo_arc_in_direction (cr, xc, yc,
			     radius,
			     angle2, angle1,
			     CAIRO_DIRECTION_REVERSE);
}
