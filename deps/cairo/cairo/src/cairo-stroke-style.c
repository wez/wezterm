/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2005 Red Hat, Inc
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
 *	Carl Worth <cworth@cworth.org>
 */

#include "cairoint.h"
#include "cairo-error-private.h"

void
_cairo_stroke_style_init (cairo_stroke_style_t *style)
{
    VG (VALGRIND_MAKE_MEM_UNDEFINED (style, sizeof (cairo_stroke_style_t)));

    style->line_width = CAIRO_GSTATE_LINE_WIDTH_DEFAULT;
    style->line_cap = CAIRO_GSTATE_LINE_CAP_DEFAULT;
    style->line_join = CAIRO_GSTATE_LINE_JOIN_DEFAULT;
    style->miter_limit = CAIRO_GSTATE_MITER_LIMIT_DEFAULT;

    style->dash = NULL;
    style->num_dashes = 0;
    style->dash_offset = 0.0;

    style->is_hairline = FALSE;
}

cairo_status_t
_cairo_stroke_style_init_copy (cairo_stroke_style_t *style,
			       const cairo_stroke_style_t *other)
{
    if (CAIRO_INJECT_FAULT ())
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    VG (VALGRIND_MAKE_MEM_UNDEFINED (style, sizeof (cairo_stroke_style_t)));

    style->line_width = other->line_width;
    style->line_cap = other->line_cap;
    style->line_join = other->line_join;
    style->miter_limit = other->miter_limit;

    style->num_dashes = other->num_dashes;

    if (other->dash == NULL) {
	style->dash = NULL;
    } else {
	style->dash = _cairo_malloc_ab (style->num_dashes, sizeof (double));
	if (unlikely (style->dash == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	memcpy (style->dash, other->dash,
		style->num_dashes * sizeof (double));
    }

    style->dash_offset = other->dash_offset;

    style->is_hairline = other->is_hairline;

    return CAIRO_STATUS_SUCCESS;
}

void
_cairo_stroke_style_fini (cairo_stroke_style_t *style)
{
    free (style->dash);
    style->dash = NULL;

    style->num_dashes = 0;

    VG (VALGRIND_MAKE_MEM_UNDEFINED (style, sizeof (cairo_stroke_style_t)));
}

/*
 * For a stroke in the given style, compute the maximum distance
 * from the path that vertices could be generated.  In the case
 * of rotation in the ctm, the distance will not be exact.
 */
void
_cairo_stroke_style_max_distance_from_path (const cairo_stroke_style_t *style,
					    const cairo_path_fixed_t *path,
                                            const cairo_matrix_t *ctm,
                                            double *dx, double *dy)
{
    double style_expansion = 0.5;

    if (style->line_cap == CAIRO_LINE_CAP_SQUARE)
	style_expansion = M_SQRT1_2;

    if (style->line_join == CAIRO_LINE_JOIN_MITER &&
	! path->stroke_is_rectilinear &&
	style_expansion < M_SQRT2 * style->miter_limit)
    {
	style_expansion = M_SQRT2 * style->miter_limit;
    }

    style_expansion *= style->line_width;

    if (_cairo_matrix_has_unity_scale (ctm)) {
	*dx = *dy = style_expansion;
    } else {
	*dx = style_expansion * hypot (ctm->xx, ctm->xy);
	*dy = style_expansion * hypot (ctm->yy, ctm->yx);
    }
}

void
_cairo_stroke_style_max_line_distance_from_path (const cairo_stroke_style_t *style,
						 const cairo_path_fixed_t *path,
						 const cairo_matrix_t *ctm,
						 double *dx, double *dy)
{
    double style_expansion = 0.5 * style->line_width;
    if (_cairo_matrix_has_unity_scale (ctm)) {
	*dx = *dy = style_expansion;
    } else {
	*dx = style_expansion * hypot (ctm->xx, ctm->xy);
	*dy = style_expansion * hypot (ctm->yy, ctm->yx);
    }
}

void
_cairo_stroke_style_max_join_distance_from_path (const cairo_stroke_style_t *style,
						 const cairo_path_fixed_t *path,
						 const cairo_matrix_t *ctm,
						 double *dx, double *dy)
{
    double style_expansion = 0.5;

    if (style->line_join == CAIRO_LINE_JOIN_MITER &&
	! path->stroke_is_rectilinear &&
	style_expansion < M_SQRT2 * style->miter_limit)
    {
	style_expansion = M_SQRT2 * style->miter_limit;
    }

    style_expansion *= style->line_width;

    if (_cairo_matrix_has_unity_scale (ctm)) {
	*dx = *dy = style_expansion;
    } else {
	*dx = style_expansion * hypot (ctm->xx, ctm->xy);
	*dy = style_expansion * hypot (ctm->yy, ctm->yx);
    }
}
/*
 * Computes the period of a dashed stroke style.
 * Returns 0 for non-dashed styles.
 */
double
_cairo_stroke_style_dash_period (const cairo_stroke_style_t *style)
{
    double period;
    unsigned int i;

    period = 0.0;
    for (i = 0; i < style->num_dashes; i++)
	period += style->dash[i];

    if (style->num_dashes & 1)
	period *= 2.0;

    return period;
}

/*
 * Coefficient of the linear approximation (minimizing square difference)
 * of the surface covered by round caps
 *
 * This can be computed in the following way:
 * the area inside the circle with radius w/2 and the region -d/2 <= x <= d/2 is:
 *   f(w,d) = 2 * integrate (sqrt (w*w/4 - x*x), x, -d/2, d/2)
 * The square difference to a generic linear approximation (c*d) in the range (0,w) would be:
 *   integrate ((f(w,d) - c*d)^2, d, 0, w)
 * To minimize this difference it is sufficient to find a solution of the differential with
 * respect to c:
 *   solve ( diff (integrate ((f(w,d) - c*d)^2, d, 0, w), c), c)
 * Which leads to c = 9/32*pi*w
 * Since we're not interested in the true area, but just in a coverage estimate,
 * we always divide the real area by the line width (w).
 * The same computation for square caps would be
 *   f(w,d) = 2 * integrate(w/2, x, -d/2, d/2)
 *   c = 1*w
 * but in this case it would not be an approximation, since f is already linear in d.
 */
#define ROUND_MINSQ_APPROXIMATION (9*M_PI/32)

/*
 * Computes the length of the "on" part of a dashed stroke style,
 * taking into account also line caps.
 * Returns 0 for non-dashed styles.
 */
double
_cairo_stroke_style_dash_stroked (const cairo_stroke_style_t *style)
{
    double stroked, cap_scale;
    unsigned int i;

    switch (style->line_cap) {
    default: ASSERT_NOT_REACHED;
    case CAIRO_LINE_CAP_BUTT:   cap_scale = 0.0; break;
    case CAIRO_LINE_CAP_ROUND:  cap_scale = ROUND_MINSQ_APPROXIMATION; break;
    case CAIRO_LINE_CAP_SQUARE: cap_scale = 1.0; break;
    }

    stroked = 0.0;
    if (style->num_dashes & 1) {
        /* Each dash element is used both as on and as off. The order in which they are summed is
	 * irrelevant, so sum the coverage of one dash element, taken both on and off at each iteration */
	for (i = 0; i < style->num_dashes; i++)
	    stroked += style->dash[i] + cap_scale * MIN (style->dash[i], style->line_width);
    } else {
        /* Even (0, 2, ...) dashes are on and simply counted for the coverage, odd dashes are off, thus
	 * their coverage is approximated based on the area covered by the caps of adjacent on dases. */
	for (i = 0; i + 1 < style->num_dashes; i += 2)
	    stroked += style->dash[i] + cap_scale * MIN (style->dash[i+1], style->line_width);
    }

    return stroked;
}

/*
 * Verifies if _cairo_stroke_style_dash_approximate should be used to generate
 * an approximation of the dash pattern in the specified style, when used for
 * stroking a path with the given CTM and tolerance.
 * Always %FALSE for non-dashed styles.
 */
cairo_bool_t
_cairo_stroke_style_dash_can_approximate (const cairo_stroke_style_t *style,
					  const cairo_matrix_t *ctm,
					  double tolerance)
{
    double period;

    if (! style->num_dashes)
        return FALSE;

    period = _cairo_stroke_style_dash_period (style);
    return _cairo_matrix_transformed_circle_major_axis (ctm, period) < tolerance;
}

/*
 * Create a 2-dashes approximation of a dashed style, by making the "on" and "off"
 * parts respect the original ratio.
 */
void
_cairo_stroke_style_dash_approximate (const cairo_stroke_style_t *style,
				      const cairo_matrix_t *ctm,
				      double tolerance,
				      double *dash_offset,
				      double *dashes,
				      unsigned int *num_dashes)
{
    double coverage, scale, offset;
    cairo_bool_t on = TRUE;
    unsigned int i = 0;

    coverage = _cairo_stroke_style_dash_stroked (style) / _cairo_stroke_style_dash_period (style);
    coverage = MIN (coverage, 1.0);
    scale = tolerance / _cairo_matrix_transformed_circle_major_axis (ctm, 1.0);

    /* We stop searching for a starting point as soon as the
     * offset reaches zero.  Otherwise when an initial dash
     * segment shrinks to zero it will be skipped over. */
    offset = style->dash_offset;
    while (offset > 0.0 && offset >= style->dash[i]) {
	offset -= style->dash[i];
	on = !on;
	if (++i == style->num_dashes)
	    i = 0;
    }

    *num_dashes = 2;

    /*
     * We want to create a new dash pattern with the same relative coverage,
     * but composed of just 2 elements with total length equal to scale.
     * Based on the formula in _cairo_stroke_style_dash_stroked:
     * scale * coverage = dashes[0] + cap_scale * MIN (dashes[1], line_width)
     *                  = MIN (dashes[0] + cap_scale * (scale - dashes[0]),
     *                         dashes[0] + cap_scale * line_width) = 
     *                  = MIN (dashes[0] * (1 - cap_scale) + cap_scale * scale,
     *	                       dashes[0] + cap_scale * line_width)
     *
     * Solving both cases we get:
     *   dashes[0] = scale * (coverage - cap_scale) / (1 - cap_scale)
     *	  when scale - dashes[0] <= line_width
     *	dashes[0] = scale * coverage - cap_scale * line_width
     *	  when scale - dashes[0] > line_width.
     *
     * Comparing the two cases we get:
     *   second > first
     *   second > scale * (coverage - cap_scale) / (1 - cap_scale)
     *   second - cap_scale * second - scale * coverage + scale * cap_scale > 0
     * 	 (scale * coverage - cap_scale * line_width) - cap_scale * second - scale * coverage + scale * cap_scale > 0
     *   - line_width - second + scale > 0
     *   scale - second > line_width
     * which is the condition for the second solution to be the valid one.
     * So when second > first, the second solution is the correct one (i.e.
     * the solution is always MAX (first, second).
     */
    switch (style->line_cap) {
    default:
        ASSERT_NOT_REACHED;
	dashes[0] = 0.0;
	break;

    case CAIRO_LINE_CAP_BUTT:
        /* Simplified formula (substituting 0 for cap_scale): */
        dashes[0] = scale * coverage;
	break;

    case CAIRO_LINE_CAP_ROUND:
        dashes[0] = MAX(scale * (coverage - ROUND_MINSQ_APPROXIMATION) / (1.0 - ROUND_MINSQ_APPROXIMATION),
			scale * coverage - ROUND_MINSQ_APPROXIMATION * style->line_width);
	break;

    case CAIRO_LINE_CAP_SQUARE:
        /*
	 * Special attention is needed to handle the case cap_scale == 1 (since the first solution
	 * is either indeterminate or -inf in this case). Since dash lengths are always >=0, using
	 * 0 as first solution always leads to the correct solution.
	 */
        dashes[0] = MAX(0.0, scale * coverage - style->line_width);
	break;
    }

    dashes[1] = scale - dashes[0];

    *dash_offset = on ? 0.0 : dashes[0];
}
