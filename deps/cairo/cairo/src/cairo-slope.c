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

#include "cairo-slope-private.h"

/* Compare two slopes. Slope angles begin at 0 in the direction of the
   positive X axis and increase in the direction of the positive Y
   axis.

   This function always compares the slope vectors based on the
   smaller angular difference between them, (that is based on an
   angular difference that is strictly less than pi). To break ties
   when comparing slope vectors with an angular difference of exactly
   pi, the vector with a positive dx (or positive dy if dx's are zero)
   is considered to be more positive than the other.

   Also, all slope vectors with both dx==0 and dy==0 are considered
   equal and more positive than any non-zero vector.

   <  0 => a less positive than b
   == 0 => a equal to b
   >  0 => a more positive than b
*/
int
_cairo_slope_compare (const cairo_slope_t *a, const cairo_slope_t *b)
{
    cairo_int64_t ady_bdx = _cairo_int32x32_64_mul (a->dy, b->dx);
    cairo_int64_t bdy_adx = _cairo_int32x32_64_mul (b->dy, a->dx);
    int cmp;

    cmp = _cairo_int64_cmp (ady_bdx, bdy_adx);
    if (cmp)
	return cmp;

    /* special-case zero vectors.  the intended logic here is:
     * zero vectors all compare equal, and more positive than any
     * non-zero vector.
     */
    if (a->dx == 0 && a->dy == 0 && b->dx == 0 && b->dy ==0)
	return 0;
    if (a->dx == 0 && a->dy == 0)
	return 1;
    if (b->dx == 0 && b->dy ==0)
	return -1;

    /* Finally, we're looking at two vectors that are either equal or
     * that differ by exactly pi. We can identify the "differ by pi"
     * case by looking for a change in sign in either dx or dy between
     * a and b.
     *
     * And in these cases, we eliminate the ambiguity by reducing the angle
     * of b by an infinitesimally small amount, (that is, 'a' will
     * always be considered less than 'b').
     */
    if ((a->dx ^ b->dx) < 0 || (a->dy ^ b->dy) < 0) {
	if (a->dx > 0 || (a->dx == 0 && a->dy > 0))
	    return -1;
	else
	    return +1;
    }

    /* Finally, for identical slopes, we obviously return 0. */
    return 0;
}
