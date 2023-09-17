/*
 * Copyright © 2004 Carl Worth
 * Copyright © 2006 Red Hat, Inc.
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
 * The Initial Developer of the Original Code is Carl Worth
 *
 * Contributor(s):
 *	Carl D. Worth <cworth@cworth.org>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

/* Provide definitions for standalone compilation */
#include "cairoint.h"

#include "cairo-error-private.h"
#include "cairo-freelist-private.h"
#include "cairo-combsort-inline.h"

#define DEBUG_POLYGON 0

typedef cairo_point_t cairo_bo_point32_t;

typedef struct _cairo_bo_intersect_ordinate {
    int32_t ordinate;
    enum { EXACT, INEXACT } exactness;
} cairo_bo_intersect_ordinate_t;

typedef struct _cairo_bo_intersect_point {
    cairo_bo_intersect_ordinate_t x;
    cairo_bo_intersect_ordinate_t y;
} cairo_bo_intersect_point_t;

typedef struct _cairo_bo_edge cairo_bo_edge_t;

typedef struct _cairo_bo_deferred {
    cairo_bo_edge_t *right;
    int32_t top;
} cairo_bo_deferred_t;

struct _cairo_bo_edge {
    cairo_edge_t edge;
    cairo_bo_edge_t *prev;
    cairo_bo_edge_t *next;
    cairo_bo_deferred_t deferred;
};

/* the parent is always given by index/2 */
#define PQ_PARENT_INDEX(i) ((i) >> 1)
#define PQ_FIRST_ENTRY 1

/* left and right children are index * 2 and (index * 2) +1 respectively */
#define PQ_LEFT_CHILD_INDEX(i) ((i) << 1)

typedef enum {
    CAIRO_BO_EVENT_TYPE_STOP,
    CAIRO_BO_EVENT_TYPE_INTERSECTION,
    CAIRO_BO_EVENT_TYPE_START
} cairo_bo_event_type_t;

typedef struct _cairo_bo_event {
    cairo_bo_event_type_t type;
    cairo_point_t point;
} cairo_bo_event_t;

typedef struct _cairo_bo_start_event {
    cairo_bo_event_type_t type;
    cairo_point_t point;
    cairo_bo_edge_t edge;
} cairo_bo_start_event_t;

typedef struct _cairo_bo_queue_event {
    cairo_bo_event_type_t type;
    cairo_point_t point;
    cairo_bo_edge_t *e1;
    cairo_bo_edge_t *e2;
} cairo_bo_queue_event_t;

typedef struct _pqueue {
    int size, max_size;

    cairo_bo_event_t **elements;
    cairo_bo_event_t *elements_embedded[1024];
} pqueue_t;

typedef struct _cairo_bo_event_queue {
    cairo_freepool_t pool;
    pqueue_t pqueue;
    cairo_bo_event_t **start_events;
} cairo_bo_event_queue_t;

typedef struct _cairo_bo_sweep_line {
    cairo_bo_edge_t *head;
    int32_t current_y;
    cairo_bo_edge_t *current_edge;
} cairo_bo_sweep_line_t;

static cairo_fixed_t
_line_compute_intersection_x_for_y (const cairo_line_t *line,
				    cairo_fixed_t y)
{
    cairo_fixed_t x, dy;

    if (y == line->p1.y)
	return line->p1.x;
    if (y == line->p2.y)
	return line->p2.x;

    x = line->p1.x;
    dy = line->p2.y - line->p1.y;
    if (dy != 0) {
	x += _cairo_fixed_mul_div_floor (y - line->p1.y,
					 line->p2.x - line->p1.x,
					 dy);
    }

    return x;
}

static inline int
_cairo_bo_point32_compare (cairo_bo_point32_t const *a,
			   cairo_bo_point32_t const *b)
{
    int cmp;

    cmp = a->y - b->y;
    if (cmp)
	return cmp;

    return a->x - b->x;
}

/* Compare the slope of a to the slope of b, returning 1, 0, -1 if the
 * slope a is respectively greater than, equal to, or less than the
 * slope of b.
 *
 * For each edge, consider the direction vector formed from:
 *
 *	top -> bottom
 *
 * which is:
 *
 *	(dx, dy) = (line.p2.x - line.p1.x, line.p2.y - line.p1.y)
 *
 * We then define the slope of each edge as dx/dy, (which is the
 * inverse of the slope typically used in math instruction). We never
 * compute a slope directly as the value approaches infinity, but we
 * can derive a slope comparison without division as follows, (where
 * the ? represents our compare operator).
 *
 * 1.	   slope(a) ? slope(b)
 * 2.	    adx/ady ? bdx/bdy
 * 3.	(adx * bdy) ? (bdx * ady)
 *
 * Note that from step 2 to step 3 there is no change needed in the
 * sign of the result since both ady and bdy are guaranteed to be
 * greater than or equal to 0.
 *
 * When using this slope comparison to sort edges, some care is needed
 * when interpreting the results. Since the slope compare operates on
 * distance vectors from top to bottom it gives a correct left to
 * right sort for edges that have a common top point, (such as two
 * edges with start events at the same location). On the other hand,
 * the sense of the result will be exactly reversed for two edges that
 * have a common stop point.
 */
static inline int
_slope_compare (const cairo_bo_edge_t *a,
		const cairo_bo_edge_t *b)
{
    /* XXX: We're assuming here that dx and dy will still fit in 32
     * bits. That's not true in general as there could be overflow. We
     * should prevent that before the tessellation algorithm
     * begins.
     */
    int32_t adx = a->edge.line.p2.x - a->edge.line.p1.x;
    int32_t bdx = b->edge.line.p2.x - b->edge.line.p1.x;

    /* Since the dy's are all positive by construction we can fast
     * path several common cases.
     */

    /* First check for vertical lines. */
    if (adx == 0)
	return -bdx;
    if (bdx == 0)
	return adx;

    /* Then where the two edges point in different directions wrt x. */
    if ((adx ^ bdx) < 0)
	return adx;

    /* Finally we actually need to do the general comparison. */
    {
	int32_t ady = a->edge.line.p2.y - a->edge.line.p1.y;
	int32_t bdy = b->edge.line.p2.y - b->edge.line.p1.y;
	cairo_int64_t adx_bdy = _cairo_int32x32_64_mul (adx, bdy);
	cairo_int64_t bdx_ady = _cairo_int32x32_64_mul (bdx, ady);

	return _cairo_int64_cmp (adx_bdy, bdx_ady);
    }
}

/*
 * We need to compare the x-coordinates of a pair of lines for a particular y,
 * without loss of precision.
 *
 * The x-coordinate along an edge for a given y is:
 *   X = A_x + (Y - A_y) * A_dx / A_dy
 *
 * So the inequality we wish to test is:
 *   A_x + (Y - A_y) * A_dx / A_dy ∘ B_x + (Y - B_y) * B_dx / B_dy,
 * where ∘ is our inequality operator.
 *
 * By construction, we know that A_dy and B_dy (and (Y - A_y), (Y - B_y)) are
 * all positive, so we can rearrange it thus without causing a sign change:
 *   A_dy * B_dy * (A_x - B_x) ∘ (Y - B_y) * B_dx * A_dy
 *                                 - (Y - A_y) * A_dx * B_dy
 *
 * Given the assumption that all the deltas fit within 32 bits, we can compute
 * this comparison directly using 128 bit arithmetic. For certain, but common,
 * input we can reduce this down to a single 32 bit compare by inspecting the
 * deltas.
 *
 * (And put the burden of the work on developing fast 128 bit ops, which are
 * required throughout the tessellator.)
 *
 * See the similar discussion for _slope_compare().
 */
static int
edges_compare_x_for_y_general (const cairo_bo_edge_t *a,
			       const cairo_bo_edge_t *b,
			       int32_t y)
{
    /* XXX: We're assuming here that dx and dy will still fit in 32
     * bits. That's not true in general as there could be overflow. We
     * should prevent that before the tessellation algorithm
     * begins.
     */
    int32_t dx;
    int32_t adx, ady;
    int32_t bdx, bdy;
    enum {
       HAVE_NONE    = 0x0,
       HAVE_DX      = 0x1,
       HAVE_ADX     = 0x2,
       HAVE_DX_ADX  = HAVE_DX | HAVE_ADX,
       HAVE_BDX     = 0x4,
       HAVE_DX_BDX  = HAVE_DX | HAVE_BDX,
       HAVE_ADX_BDX = HAVE_ADX | HAVE_BDX,
       HAVE_ALL     = HAVE_DX | HAVE_ADX | HAVE_BDX
    } have_dx_adx_bdx = HAVE_ALL;

    /* don't bother solving for abscissa if the edges' bounding boxes
     * can be used to order them. */
    {
           int32_t amin, amax;
           int32_t bmin, bmax;
           if (a->edge.line.p1.x < a->edge.line.p2.x) {
                   amin = a->edge.line.p1.x;
                   amax = a->edge.line.p2.x;
           } else {
                   amin = a->edge.line.p2.x;
                   amax = a->edge.line.p1.x;
           }
           if (b->edge.line.p1.x < b->edge.line.p2.x) {
                   bmin = b->edge.line.p1.x;
                   bmax = b->edge.line.p2.x;
           } else {
                   bmin = b->edge.line.p2.x;
                   bmax = b->edge.line.p1.x;
           }
           if (amax < bmin) return -1;
           if (amin > bmax) return +1;
    }

    ady = a->edge.line.p2.y - a->edge.line.p1.y;
    adx = a->edge.line.p2.x - a->edge.line.p1.x;
    if (adx == 0)
	have_dx_adx_bdx &= ~HAVE_ADX;

    bdy = b->edge.line.p2.y - b->edge.line.p1.y;
    bdx = b->edge.line.p2.x - b->edge.line.p1.x;
    if (bdx == 0)
	have_dx_adx_bdx &= ~HAVE_BDX;

    dx = a->edge.line.p1.x - b->edge.line.p1.x;
    if (dx == 0)
	have_dx_adx_bdx &= ~HAVE_DX;

#define L _cairo_int64x32_128_mul (_cairo_int32x32_64_mul (ady, bdy), dx)
#define A _cairo_int64x32_128_mul (_cairo_int32x32_64_mul (adx, bdy), y - a->edge.line.p1.y)
#define B _cairo_int64x32_128_mul (_cairo_int32x32_64_mul (bdx, ady), y - b->edge.line.p1.y)
    switch (have_dx_adx_bdx) {
    default:
    case HAVE_NONE:
	return 0;
    case HAVE_DX:
	/* A_dy * B_dy * (A_x - B_x) ∘ 0 */
	return dx; /* ady * bdy is positive definite */
    case HAVE_ADX:
	/* 0 ∘  - (Y - A_y) * A_dx * B_dy */
	return adx; /* bdy * (y - a->top.y) is positive definite */
    case HAVE_BDX:
	/* 0 ∘ (Y - B_y) * B_dx * A_dy */
	return -bdx; /* ady * (y - b->top.y) is positive definite */
    case HAVE_ADX_BDX:
	/*  0 ∘ (Y - B_y) * B_dx * A_dy - (Y - A_y) * A_dx * B_dy */
	if ((adx ^ bdx) < 0) {
	    return adx;
	} else if (a->edge.line.p1.y == b->edge.line.p1.y) { /* common origin */
	    cairo_int64_t adx_bdy, bdx_ady;

	    /* ∴ A_dx * B_dy ∘ B_dx * A_dy */

	    adx_bdy = _cairo_int32x32_64_mul (adx, bdy);
	    bdx_ady = _cairo_int32x32_64_mul (bdx, ady);

	    return _cairo_int64_cmp (adx_bdy, bdx_ady);
	} else
	    return _cairo_int128_cmp (A, B);
    case HAVE_DX_ADX:
	/* A_dy * (A_x - B_x) ∘ - (Y - A_y) * A_dx */
	if ((-adx ^ dx) < 0) {
	    return dx;
	} else {
	    cairo_int64_t ady_dx, dy_adx;

	    ady_dx = _cairo_int32x32_64_mul (ady, dx);
	    dy_adx = _cairo_int32x32_64_mul (a->edge.line.p1.y - y, adx);

	    return _cairo_int64_cmp (ady_dx, dy_adx);
	}
    case HAVE_DX_BDX:
	/* B_dy * (A_x - B_x) ∘ (Y - B_y) * B_dx */
	if ((bdx ^ dx) < 0) {
	    return dx;
	} else {
	    cairo_int64_t bdy_dx, dy_bdx;

	    bdy_dx = _cairo_int32x32_64_mul (bdy, dx);
	    dy_bdx = _cairo_int32x32_64_mul (y - b->edge.line.p1.y, bdx);

	    return _cairo_int64_cmp (bdy_dx, dy_bdx);
	}
    case HAVE_ALL:
	/* XXX try comparing (a->edge.line.p2.x - b->edge.line.p2.x) et al */
	return _cairo_int128_cmp (L, _cairo_int128_sub (B, A));
    }
#undef B
#undef A
#undef L
}

/*
 * We need to compare the x-coordinate of a line for a particular y wrt to a
 * given x, without loss of precision.
 *
 * The x-coordinate along an edge for a given y is:
 *   X = A_x + (Y - A_y) * A_dx / A_dy
 *
 * So the inequality we wish to test is:
 *   A_x + (Y - A_y) * A_dx / A_dy ∘ X
 * where ∘ is our inequality operator.
 *
 * By construction, we know that A_dy (and (Y - A_y)) are
 * all positive, so we can rearrange it thus without causing a sign change:
 *   (Y - A_y) * A_dx ∘ (X - A_x) * A_dy
 *
 * Given the assumption that all the deltas fit within 32 bits, we can compute
 * this comparison directly using 64 bit arithmetic.
 *
 * See the similar discussion for _slope_compare() and
 * edges_compare_x_for_y_general().
 */
static int
edge_compare_for_y_against_x (const cairo_bo_edge_t *a,
			      int32_t y,
			      int32_t x)
{
    int32_t adx, ady;
    int32_t dx, dy;
    cairo_int64_t L, R;

    if (x < a->edge.line.p1.x && x < a->edge.line.p2.x)
	return 1;
    if (x > a->edge.line.p1.x && x > a->edge.line.p2.x)
	return -1;

    adx = a->edge.line.p2.x - a->edge.line.p1.x;
    dx = x - a->edge.line.p1.x;

    if (adx == 0)
	return -dx;
    if (dx == 0 || (adx ^ dx) < 0)
	return adx;

    dy = y - a->edge.line.p1.y;
    ady = a->edge.line.p2.y - a->edge.line.p1.y;

    L = _cairo_int32x32_64_mul (dy, adx);
    R = _cairo_int32x32_64_mul (dx, ady);

    return _cairo_int64_cmp (L, R);
}

static int
edges_compare_x_for_y (const cairo_bo_edge_t *a,
		       const cairo_bo_edge_t *b,
		       int32_t y)
{
    /* If the sweep-line is currently on an end-point of a line,
     * then we know its precise x value (and considering that we often need to
     * compare events at end-points, this happens frequently enough to warrant
     * special casing).
     */
    enum {
       HAVE_NEITHER = 0x0,
       HAVE_AX      = 0x1,
       HAVE_BX      = 0x2,
       HAVE_BOTH    = HAVE_AX | HAVE_BX
    } have_ax_bx = HAVE_BOTH;
    int32_t ax = 0, bx = 0;

    if (y == a->edge.line.p1.y)
	ax = a->edge.line.p1.x;
    else if (y == a->edge.line.p2.y)
	ax = a->edge.line.p2.x;
    else
	have_ax_bx &= ~HAVE_AX;

    if (y == b->edge.line.p1.y)
	bx = b->edge.line.p1.x;
    else if (y == b->edge.line.p2.y)
	bx = b->edge.line.p2.x;
    else
	have_ax_bx &= ~HAVE_BX;

    switch (have_ax_bx) {
    default:
    case HAVE_NEITHER:
	return edges_compare_x_for_y_general (a, b, y);
    case HAVE_AX:
	return -edge_compare_for_y_against_x (b, y, ax);
    case HAVE_BX:
	return edge_compare_for_y_against_x (a, y, bx);
    case HAVE_BOTH:
	return ax - bx;
    }
}

static inline int
_line_equal (const cairo_line_t *a, const cairo_line_t *b)
{
    return (a->p1.x == b->p1.x && a->p1.y == b->p1.y &&
	    a->p2.x == b->p2.x && a->p2.y == b->p2.y);
}

static int
_cairo_bo_sweep_line_compare_edges (cairo_bo_sweep_line_t	*sweep_line,
				    const cairo_bo_edge_t	*a,
				    const cairo_bo_edge_t	*b)
{
    int cmp;

    /* compare the edges if not identical */
    if (! _line_equal (&a->edge.line, &b->edge.line)) {
	cmp = edges_compare_x_for_y (a, b, sweep_line->current_y);
	if (cmp)
	    return cmp;

	/* The two edges intersect exactly at y, so fall back on slope
	 * comparison. We know that this compare_edges function will be
	 * called only when starting a new edge, (not when stopping an
	 * edge), so we don't have to worry about conditionally inverting
	 * the sense of _slope_compare. */
	cmp = _slope_compare (a, b);
	if (cmp)
	    return cmp;
    }

    /* We've got two collinear edges now. */
    return b->edge.bottom - a->edge.bottom;
}

static inline cairo_int64_t
det32_64 (int32_t a, int32_t b,
	  int32_t c, int32_t d)
{
    /* det = a * d - b * c */
    return _cairo_int64_sub (_cairo_int32x32_64_mul (a, d),
			     _cairo_int32x32_64_mul (b, c));
}

static inline cairo_int128_t
det64x32_128 (cairo_int64_t a, int32_t       b,
	      cairo_int64_t c, int32_t       d)
{
    /* det = a * d - b * c */
    return _cairo_int128_sub (_cairo_int64x32_128_mul (a, d),
			      _cairo_int64x32_128_mul (c, b));
}

/* Compute the intersection of two lines as defined by two edges. The
 * result is provided as a coordinate pair of 128-bit integers.
 *
 * Returns %CAIRO_BO_STATUS_INTERSECTION if there is an intersection or
 * %CAIRO_BO_STATUS_PARALLEL if the two lines are exactly parallel.
 */
static cairo_bool_t
intersect_lines (cairo_bo_edge_t		*a,
		 cairo_bo_edge_t		*b,
		 cairo_bo_intersect_point_t	*intersection)
{
    cairo_int64_t a_det, b_det;

    /* XXX: We're assuming here that dx and dy will still fit in 32
     * bits. That's not true in general as there could be overflow. We
     * should prevent that before the tessellation algorithm begins.
     * What we're doing to mitigate this is to perform clamping in
     * cairo_bo_tessellate_polygon().
     */
    int32_t dx1 = a->edge.line.p1.x - a->edge.line.p2.x;
    int32_t dy1 = a->edge.line.p1.y - a->edge.line.p2.y;

    int32_t dx2 = b->edge.line.p1.x - b->edge.line.p2.x;
    int32_t dy2 = b->edge.line.p1.y - b->edge.line.p2.y;

    cairo_int64_t den_det;
    cairo_int64_t R;
    cairo_quorem64_t qr;

    den_det = det32_64 (dx1, dy1, dx2, dy2);

     /* Q: Can we determine that the lines do not intersect (within range)
      * much more cheaply than computing the intersection point i.e. by
      * avoiding the division?
      *
      *   X = ax + t * adx = bx + s * bdx;
      *   Y = ay + t * ady = by + s * bdy;
      *   ∴ t * (ady*bdx - bdy*adx) = bdx * (by - ay) + bdy * (ax - bx)
      *   => t * L = R
      *
      * Therefore we can reject any intersection (under the criteria for
      * valid intersection events) if:
      *   L^R < 0 => t < 0, or
      *   L<R => t > 1
      *
      * (where top/bottom must at least extend to the line endpoints).
      *
      * A similar substitution can be performed for s, yielding:
      *   s * (ady*bdx - bdy*adx) = ady * (ax - bx) - adx * (ay - by)
      */
    R = det32_64 (dx2, dy2,
		  b->edge.line.p1.x - a->edge.line.p1.x,
		  b->edge.line.p1.y - a->edge.line.p1.y);
    if (_cairo_int64_negative (den_det)) {
	if (_cairo_int64_ge (den_det, R))
	    return FALSE;
    } else {
	if (_cairo_int64_le (den_det, R))
	    return FALSE;
    }

    R = det32_64 (dy1, dx1,
		  a->edge.line.p1.y - b->edge.line.p1.y,
		  a->edge.line.p1.x - b->edge.line.p1.x);
    if (_cairo_int64_negative (den_det)) {
	if (_cairo_int64_ge (den_det, R))
	    return FALSE;
    } else {
	if (_cairo_int64_le (den_det, R))
	    return FALSE;
    }

    /* We now know that the two lines should intersect within range. */

    a_det = det32_64 (a->edge.line.p1.x, a->edge.line.p1.y,
		      a->edge.line.p2.x, a->edge.line.p2.y);
    b_det = det32_64 (b->edge.line.p1.x, b->edge.line.p1.y,
		      b->edge.line.p2.x, b->edge.line.p2.y);

    /* x = det (a_det, dx1, b_det, dx2) / den_det */
    qr = _cairo_int_96by64_32x64_divrem (det64x32_128 (a_det, dx1,
						       b_det, dx2),
					 den_det);
    if (_cairo_int64_eq (qr.rem, den_det))
	return FALSE;
#if 0
    intersection->x.exactness = _cairo_int64_is_zero (qr.rem) ? EXACT : INEXACT;
#else
    intersection->x.exactness = EXACT;
    if (! _cairo_int64_is_zero (qr.rem)) {
	if (_cairo_int64_negative (den_det) ^ _cairo_int64_negative (qr.rem))
	    qr.rem = _cairo_int64_negate (qr.rem);
	qr.rem = _cairo_int64_mul (qr.rem, _cairo_int32_to_int64 (2));
	if (_cairo_int64_ge (qr.rem, den_det)) {
	    qr.quo = _cairo_int64_add (qr.quo,
				       _cairo_int32_to_int64 (_cairo_int64_negative (qr.quo) ? -1 : 1));
	} else
	    intersection->x.exactness = INEXACT;
    }
#endif
    intersection->x.ordinate = _cairo_int64_to_int32 (qr.quo);

    /* y = det (a_det, dy1, b_det, dy2) / den_det */
    qr = _cairo_int_96by64_32x64_divrem (det64x32_128 (a_det, dy1,
						       b_det, dy2),
					 den_det);
    if (_cairo_int64_eq (qr.rem, den_det))
	return FALSE;
#if 0
    intersection->y.exactness = _cairo_int64_is_zero (qr.rem) ? EXACT : INEXACT;
#else
    intersection->y.exactness = EXACT;
    if (! _cairo_int64_is_zero (qr.rem)) {
	if (_cairo_int64_negative (den_det) ^ _cairo_int64_negative (qr.rem))
	    qr.rem = _cairo_int64_negate (qr.rem);
	qr.rem = _cairo_int64_mul (qr.rem, _cairo_int32_to_int64 (2));
	if (_cairo_int64_ge (qr.rem, den_det)) {
	    qr.quo = _cairo_int64_add (qr.quo,
				       _cairo_int32_to_int64 (_cairo_int64_negative (qr.quo) ? -1 : 1));
	} else
	    intersection->y.exactness = INEXACT;
    }
#endif
    intersection->y.ordinate = _cairo_int64_to_int32 (qr.quo);

    return TRUE;
}

static int
_cairo_bo_intersect_ordinate_32_compare (cairo_bo_intersect_ordinate_t	a,
					 int32_t			b)
{
    /* First compare the quotient */
    if (a.ordinate > b)
	return +1;
    if (a.ordinate < b)
	return -1;
    /* With quotient identical, if remainder is 0 then compare equal */
    /* Otherwise, the non-zero remainder makes a > b */
    return INEXACT == a.exactness;
}

/* Does the given edge contain the given point. The point must already
 * be known to be contained within the line determined by the edge,
 * (most likely the point results from an intersection of this edge
 * with another).
 *
 * If we had exact arithmetic, then this function would simply be a
 * matter of examining whether the y value of the point lies within
 * the range of y values of the edge. But since intersection points
 * are not exact due to being rounded to the nearest integer within
 * the available precision, we must also examine the x value of the
 * point.
 *
 * The definition of "contains" here is that the given intersection
 * point will be seen by the sweep line after the start event for the
 * given edge and before the stop event for the edge. See the comments
 * in the implementation for more details.
 */
static cairo_bool_t
_cairo_bo_edge_contains_intersect_point (cairo_bo_edge_t		*edge,
					 cairo_bo_intersect_point_t	*point)
{
    int cmp_top, cmp_bottom;

    /* XXX: When running the actual algorithm, we don't actually need to
     * compare against edge->top at all here, since any intersection above
     * top is eliminated early via a slope comparison. We're leaving these
     * here for now only for the sake of the quadratic-time intersection
     * finder which needs them.
     */

    cmp_top = _cairo_bo_intersect_ordinate_32_compare (point->y,
						       edge->edge.top);
    cmp_bottom = _cairo_bo_intersect_ordinate_32_compare (point->y,
							  edge->edge.bottom);

    if (cmp_top < 0 || cmp_bottom > 0)
    {
	return FALSE;
    }

    if (cmp_top > 0 && cmp_bottom < 0)
    {
	return TRUE;
    }

    /* At this stage, the point lies on the same y value as either
     * edge->top or edge->bottom, so we have to examine the x value in
     * order to properly determine containment. */

    /* If the y value of the point is the same as the y value of the
     * top of the edge, then the x value of the point must be greater
     * to be considered as inside the edge. Similarly, if the y value
     * of the point is the same as the y value of the bottom of the
     * edge, then the x value of the point must be less to be
     * considered as inside. */

    if (cmp_top == 0) {
	cairo_fixed_t top_x;

	top_x = _line_compute_intersection_x_for_y (&edge->edge.line,
						    edge->edge.top);
	return _cairo_bo_intersect_ordinate_32_compare (point->x, top_x) > 0;
    } else { /* cmp_bottom == 0 */
	cairo_fixed_t bot_x;

	bot_x = _line_compute_intersection_x_for_y (&edge->edge.line,
						    edge->edge.bottom);
	return _cairo_bo_intersect_ordinate_32_compare (point->x, bot_x) < 0;
    }
}

/* Compute the intersection of two edges. The result is provided as a
 * coordinate pair of 128-bit integers.
 *
 * Returns %CAIRO_BO_STATUS_INTERSECTION if there is an intersection
 * that is within both edges, %CAIRO_BO_STATUS_NO_INTERSECTION if the
 * intersection of the lines defined by the edges occurs outside of
 * one or both edges, and %CAIRO_BO_STATUS_PARALLEL if the two edges
 * are exactly parallel.
 *
 * Note that when determining if a candidate intersection is "inside"
 * an edge, we consider both the infinitesimal shortening and the
 * infinitesimal tilt rules described by John Hobby. Specifically, if
 * the intersection is exactly the same as an edge point, it is
 * effectively outside (no intersection is returned). Also, if the
 * intersection point has the same
 */
static cairo_bool_t
_cairo_bo_edge_intersect (cairo_bo_edge_t	*a,
			  cairo_bo_edge_t	*b,
			  cairo_bo_point32_t	*intersection)
{
    cairo_bo_intersect_point_t quorem;

    if (! intersect_lines (a, b, &quorem))
	return FALSE;

    if (! _cairo_bo_edge_contains_intersect_point (a, &quorem))
	return FALSE;

    if (! _cairo_bo_edge_contains_intersect_point (b, &quorem))
	return FALSE;

    /* Now that we've correctly compared the intersection point and
     * determined that it lies within the edge, then we know that we
     * no longer need any more bits of storage for the intersection
     * than we do for our edge coordinates. We also no longer need the
     * remainder from the division. */
    intersection->x = quorem.x.ordinate;
    intersection->y = quorem.y.ordinate;

    return TRUE;
}

static inline int
cairo_bo_event_compare (const cairo_bo_event_t *a,
			const cairo_bo_event_t *b)
{
    int cmp;

    cmp = _cairo_bo_point32_compare (&a->point, &b->point);
    if (cmp)
	return cmp;

    cmp = a->type - b->type;
    if (cmp)
	return cmp;

    return a - b;
}

static inline void
_pqueue_init (pqueue_t *pq)
{
    pq->max_size = ARRAY_LENGTH (pq->elements_embedded);
    pq->size = 0;

    pq->elements = pq->elements_embedded;
}

static inline void
_pqueue_fini (pqueue_t *pq)
{
    if (pq->elements != pq->elements_embedded)
	free (pq->elements);
}

static cairo_status_t
_pqueue_grow (pqueue_t *pq)
{
    cairo_bo_event_t **new_elements;
    pq->max_size *= 2;

    if (pq->elements == pq->elements_embedded) {
	new_elements = _cairo_malloc_ab (pq->max_size,
					 sizeof (cairo_bo_event_t *));
	if (unlikely (new_elements == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	memcpy (new_elements, pq->elements_embedded,
		sizeof (pq->elements_embedded));
    } else {
	new_elements = _cairo_realloc_ab (pq->elements,
					  pq->max_size,
					  sizeof (cairo_bo_event_t *));
	if (unlikely (new_elements == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    pq->elements = new_elements;
    return CAIRO_STATUS_SUCCESS;
}

static inline cairo_status_t
_pqueue_push (pqueue_t *pq, cairo_bo_event_t *event)
{
    cairo_bo_event_t **elements;
    int i, parent;

    if (unlikely (pq->size + 1 == pq->max_size)) {
	cairo_status_t status;

	status = _pqueue_grow (pq);
	if (unlikely (status))
	    return status;
    }

    elements = pq->elements;

    for (i = ++pq->size;
	 i != PQ_FIRST_ENTRY &&
	 cairo_bo_event_compare (event,
				 elements[parent = PQ_PARENT_INDEX (i)]) < 0;
	 i = parent)
    {
	elements[i] = elements[parent];
    }

    elements[i] = event;

    return CAIRO_STATUS_SUCCESS;
}

static inline void
_pqueue_pop (pqueue_t *pq)
{
    cairo_bo_event_t **elements = pq->elements;
    cairo_bo_event_t *tail;
    int child, i;

    tail = elements[pq->size--];
    if (pq->size == 0) {
	elements[PQ_FIRST_ENTRY] = NULL;
	return;
    }

    for (i = PQ_FIRST_ENTRY;
	 (child = PQ_LEFT_CHILD_INDEX (i)) <= pq->size;
	 i = child)
    {
	if (child != pq->size &&
	    cairo_bo_event_compare (elements[child+1],
				    elements[child]) < 0)
	{
	    child++;
	}

	if (cairo_bo_event_compare (elements[child], tail) >= 0)
	    break;

	elements[i] = elements[child];
    }
    elements[i] = tail;
}

static inline cairo_status_t
_cairo_bo_event_queue_insert (cairo_bo_event_queue_t	*queue,
			      cairo_bo_event_type_t	 type,
			      cairo_bo_edge_t		*e1,
			      cairo_bo_edge_t		*e2,
			      const cairo_point_t	 *point)
{
    cairo_bo_queue_event_t *event;

    event = _cairo_freepool_alloc (&queue->pool);
    if (unlikely (event == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    event->type = type;
    event->e1 = e1;
    event->e2 = e2;
    event->point = *point;

    return _pqueue_push (&queue->pqueue, (cairo_bo_event_t *) event);
}

static void
_cairo_bo_event_queue_delete (cairo_bo_event_queue_t *queue,
			      cairo_bo_event_t	     *event)
{
    _cairo_freepool_free (&queue->pool, event);
}

static cairo_bo_event_t *
_cairo_bo_event_dequeue (cairo_bo_event_queue_t *event_queue)
{
    cairo_bo_event_t *event, *cmp;

    event = event_queue->pqueue.elements[PQ_FIRST_ENTRY];
    cmp = *event_queue->start_events;
    if (event == NULL ||
	(cmp != NULL && cairo_bo_event_compare (cmp, event) < 0))
    {
	event = cmp;
	event_queue->start_events++;
    }
    else
    {
	_pqueue_pop (&event_queue->pqueue);
    }

    return event;
}

CAIRO_COMBSORT_DECLARE (_cairo_bo_event_queue_sort,
			cairo_bo_event_t *,
			cairo_bo_event_compare)

static void
_cairo_bo_event_queue_init (cairo_bo_event_queue_t	 *event_queue,
			    cairo_bo_event_t		**start_events,
			    int				  num_events)
{
    _cairo_bo_event_queue_sort (start_events, num_events);
    start_events[num_events] = NULL;

    event_queue->start_events = start_events;

    _cairo_freepool_init (&event_queue->pool,
			  sizeof (cairo_bo_queue_event_t));
    _pqueue_init (&event_queue->pqueue);
    event_queue->pqueue.elements[PQ_FIRST_ENTRY] = NULL;
}

static cairo_status_t
_cairo_bo_event_queue_insert_stop (cairo_bo_event_queue_t	*event_queue,
				   cairo_bo_edge_t		*edge)
{
    cairo_bo_point32_t point;

    point.y = edge->edge.bottom;
    point.x = _line_compute_intersection_x_for_y (&edge->edge.line,
						  point.y);
    return _cairo_bo_event_queue_insert (event_queue,
					 CAIRO_BO_EVENT_TYPE_STOP,
					 edge, NULL,
					 &point);
}

static void
_cairo_bo_event_queue_fini (cairo_bo_event_queue_t *event_queue)
{
    _pqueue_fini (&event_queue->pqueue);
    _cairo_freepool_fini (&event_queue->pool);
}

static inline cairo_status_t
_cairo_bo_event_queue_insert_if_intersect_below_current_y (cairo_bo_event_queue_t	*event_queue,
							   cairo_bo_edge_t	*left,
							   cairo_bo_edge_t *right)
{
    cairo_bo_point32_t intersection;

    if (_line_equal (&left->edge.line, &right->edge.line))
	return CAIRO_STATUS_SUCCESS;

    /* The names "left" and "right" here are correct descriptions of
     * the order of the two edges within the active edge list. So if a
     * slope comparison also puts left less than right, then we know
     * that the intersection of these two segments has already
     * occurred before the current sweep line position. */
    if (_slope_compare (left, right) <= 0)
	return CAIRO_STATUS_SUCCESS;

    if (! _cairo_bo_edge_intersect (left, right, &intersection))
	return CAIRO_STATUS_SUCCESS;

    return _cairo_bo_event_queue_insert (event_queue,
					 CAIRO_BO_EVENT_TYPE_INTERSECTION,
					 left, right,
					 &intersection);
}

static void
_cairo_bo_sweep_line_init (cairo_bo_sweep_line_t *sweep_line)
{
    sweep_line->head = NULL;
    sweep_line->current_y = INT32_MIN;
    sweep_line->current_edge = NULL;
}

static cairo_status_t
_cairo_bo_sweep_line_insert (cairo_bo_sweep_line_t	*sweep_line,
			     cairo_bo_edge_t		*edge)
{
    if (sweep_line->current_edge != NULL) {
	cairo_bo_edge_t *prev, *next;
	int cmp;

	cmp = _cairo_bo_sweep_line_compare_edges (sweep_line,
						  sweep_line->current_edge,
						  edge);
	if (cmp < 0) {
	    prev = sweep_line->current_edge;
	    next = prev->next;
	    while (next != NULL &&
		   _cairo_bo_sweep_line_compare_edges (sweep_line,
						       next, edge) < 0)
	    {
		prev = next, next = prev->next;
	    }

	    prev->next = edge;
	    edge->prev = prev;
	    edge->next = next;
	    if (next != NULL)
		next->prev = edge;
	} else if (cmp > 0) {
	    next = sweep_line->current_edge;
	    prev = next->prev;
	    while (prev != NULL &&
		   _cairo_bo_sweep_line_compare_edges (sweep_line,
						       prev, edge) > 0)
	    {
		next = prev, prev = next->prev;
	    }

	    next->prev = edge;
	    edge->next = next;
	    edge->prev = prev;
	    if (prev != NULL)
		prev->next = edge;
	    else
		sweep_line->head = edge;
	} else {
	    prev = sweep_line->current_edge;
	    edge->prev = prev;
	    edge->next = prev->next;
	    if (prev->next != NULL)
		prev->next->prev = edge;
	    prev->next = edge;
	}
    } else {
	sweep_line->head = edge;
    }

    sweep_line->current_edge = edge;

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_bo_sweep_line_delete (cairo_bo_sweep_line_t	*sweep_line,
			     cairo_bo_edge_t	*edge)
{
    if (edge->prev != NULL)
	edge->prev->next = edge->next;
    else
	sweep_line->head = edge->next;

    if (edge->next != NULL)
	edge->next->prev = edge->prev;

    if (sweep_line->current_edge == edge)
	sweep_line->current_edge = edge->prev ? edge->prev : edge->next;
}

static void
_cairo_bo_sweep_line_swap (cairo_bo_sweep_line_t	*sweep_line,
			   cairo_bo_edge_t		*left,
			   cairo_bo_edge_t		*right)
{
    if (left->prev != NULL)
	left->prev->next = right;
    else
	sweep_line->head = right;

    if (right->next != NULL)
	right->next->prev = left;

    left->next = right->next;
    right->next = left;

    right->prev = left->prev;
    left->prev = right;
}

static inline cairo_bool_t
edges_colinear (const cairo_bo_edge_t *a, const cairo_bo_edge_t *b)
{
    if (_line_equal (&a->edge.line, &b->edge.line))
	return TRUE;

    if (_slope_compare (a, b))
	return FALSE;

    /* The choice of y is not truly arbitrary since we must guarantee that it
     * is greater than the start of either line.
     */
    if (a->edge.line.p1.y == b->edge.line.p1.y) {
	return a->edge.line.p1.x == b->edge.line.p1.x;
    } else if (a->edge.line.p2.y == b->edge.line.p2.y) {
	return a->edge.line.p2.x == b->edge.line.p2.x;
    } else if (a->edge.line.p1.y < b->edge.line.p1.y) {
	return edge_compare_for_y_against_x (b,
					     a->edge.line.p1.y,
					     a->edge.line.p1.x) == 0;
    } else {
	return edge_compare_for_y_against_x (a,
					     b->edge.line.p1.y,
					     b->edge.line.p1.x) == 0;
    }
}

static void
_cairo_bo_edge_end (cairo_bo_edge_t	*left,
		    int32_t		 bot,
		    cairo_polygon_t	*polygon)
{
    cairo_bo_deferred_t *d = &left->deferred;

    if (likely (d->top < bot)) {
	_cairo_polygon_add_line (polygon,
				 &left->edge.line,
				 d->top, bot,
				 1);
	_cairo_polygon_add_line (polygon,
				 &d->right->edge.line,
				 d->top, bot,
				 -1);
    }

    d->right = NULL;
}


static inline void
_cairo_bo_edge_start_or_continue (cairo_bo_edge_t	*left,
				  cairo_bo_edge_t	*right,
				  int			 top,
				  cairo_polygon_t	*polygon)
{
    if (left->deferred.right == right)
	return;

    if (left->deferred.right != NULL) {
	if (right != NULL && edges_colinear (left->deferred.right, right))
	{
	    /* continuation on right, so just swap edges */
	    left->deferred.right = right;
	    return;
	}

	_cairo_bo_edge_end (left, top, polygon);
    }

    if (right != NULL && ! edges_colinear (left, right)) {
	left->deferred.top = top;
	left->deferred.right = right;
    }
}

static inline void
_active_edges_to_polygon (cairo_bo_edge_t		*left,
			  int32_t			 top,
			  cairo_fill_rule_t		 fill_rule,
			  cairo_polygon_t	        *polygon)
{
    cairo_bo_edge_t *right;
    unsigned int mask;

    if (fill_rule == CAIRO_FILL_RULE_WINDING)
	mask = ~0;
    else
	mask = 1;

    while (left != NULL) {
	int in_out = left->edge.dir;

	right = left->next;
	if (left->deferred.right == NULL) {
	    while (right != NULL && right->deferred.right == NULL)
		right = right->next;

	    if (right != NULL && edges_colinear (left, right)) {
		/* continuation on left */
		left->deferred = right->deferred;
		right->deferred.right = NULL;
	    }
	}

	right = left->next;
	while (right != NULL) {
	    if (right->deferred.right != NULL)
		_cairo_bo_edge_end (right, top, polygon);

	    in_out += right->edge.dir;
	    if ((in_out & mask) == 0) {
		/* skip co-linear edges */
		if (right->next == NULL || !edges_colinear (right, right->next))
		    break;
	    }

	    right = right->next;
	}

	_cairo_bo_edge_start_or_continue (left, right, top, polygon);

	left = right;
	if (left != NULL)
	    left = left->next;
    }
}


static cairo_status_t
_cairo_bentley_ottmann_tessellate_bo_edges (cairo_bo_event_t   **start_events,
					    int			 num_events,
					    cairo_fill_rule_t	 fill_rule,
					    cairo_polygon_t	*polygon)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS; /* silence compiler */
    cairo_bo_event_queue_t event_queue;
    cairo_bo_sweep_line_t sweep_line;
    cairo_bo_event_t *event;
    cairo_bo_edge_t *left, *right;
    cairo_bo_edge_t *e1, *e2;

    _cairo_bo_event_queue_init (&event_queue, start_events, num_events);
    _cairo_bo_sweep_line_init (&sweep_line);

    while ((event = _cairo_bo_event_dequeue (&event_queue))) {
	if (event->point.y != sweep_line.current_y) {
	    _active_edges_to_polygon (sweep_line.head,
				      sweep_line.current_y,
				      fill_rule, polygon);

	    sweep_line.current_y = event->point.y;
	}

	switch (event->type) {
	case CAIRO_BO_EVENT_TYPE_START:
	    e1 = &((cairo_bo_start_event_t *) event)->edge;

	    status = _cairo_bo_sweep_line_insert (&sweep_line, e1);
	    if (unlikely (status))
		goto unwind;

	    status = _cairo_bo_event_queue_insert_stop (&event_queue, e1);
	    if (unlikely (status))
		goto unwind;

	    left = e1->prev;
	    right = e1->next;

	    if (left != NULL) {
		status = _cairo_bo_event_queue_insert_if_intersect_below_current_y (&event_queue, left, e1);
		if (unlikely (status))
		    goto unwind;
	    }

	    if (right != NULL) {
		status = _cairo_bo_event_queue_insert_if_intersect_below_current_y (&event_queue, e1, right);
		if (unlikely (status))
		    goto unwind;
	    }

	    break;

	case CAIRO_BO_EVENT_TYPE_STOP:
	    e1 = ((cairo_bo_queue_event_t *) event)->e1;
	    _cairo_bo_event_queue_delete (&event_queue, event);

	    left = e1->prev;
	    right = e1->next;

	    _cairo_bo_sweep_line_delete (&sweep_line, e1);

	    if (e1->deferred.right != NULL)
		_cairo_bo_edge_end (e1, e1->edge.bottom, polygon);

	    if (left != NULL && right != NULL) {
		status = _cairo_bo_event_queue_insert_if_intersect_below_current_y (&event_queue, left, right);
		if (unlikely (status))
		    goto unwind;
	    }

	    break;

	case CAIRO_BO_EVENT_TYPE_INTERSECTION:
	    e1 = ((cairo_bo_queue_event_t *) event)->e1;
	    e2 = ((cairo_bo_queue_event_t *) event)->e2;
	    _cairo_bo_event_queue_delete (&event_queue, event);

	    /* skip this intersection if its edges are not adjacent */
	    if (e2 != e1->next)
		break;

	    left = e1->prev;
	    right = e2->next;

	    _cairo_bo_sweep_line_swap (&sweep_line, e1, e2);

	    /* after the swap e2 is left of e1 */

	    if (left != NULL) {
		status = _cairo_bo_event_queue_insert_if_intersect_below_current_y (&event_queue, left, e2);
		if (unlikely (status))
		    goto unwind;
	    }

	    if (right != NULL) {
		status = _cairo_bo_event_queue_insert_if_intersect_below_current_y (&event_queue, e1, right);
		if (unlikely (status))
		    goto unwind;
	    }

	    break;
	}
    }

 unwind:
    _cairo_bo_event_queue_fini (&event_queue);

    return status;
}

cairo_status_t
_cairo_polygon_reduce (cairo_polygon_t *polygon,
		       cairo_fill_rule_t fill_rule)
{
    cairo_status_t status;
    cairo_bo_start_event_t stack_events[CAIRO_STACK_ARRAY_LENGTH (cairo_bo_start_event_t)];
    cairo_bo_start_event_t *events;
    cairo_bo_event_t *stack_event_ptrs[ARRAY_LENGTH (stack_events) + 1];
    cairo_bo_event_t **event_ptrs;
    int num_limits;
    int num_events;
    int i;

    num_events = polygon->num_edges;
    if (unlikely (0 == num_events))
	return CAIRO_STATUS_SUCCESS;

    if (DEBUG_POLYGON) {
	FILE *file = fopen ("reduce_in.txt", "w");
	_cairo_debug_print_polygon (file, polygon);
	fclose (file);
    }

    events = stack_events;
    event_ptrs = stack_event_ptrs;
    if (num_events > ARRAY_LENGTH (stack_events)) {
	events = _cairo_malloc_ab_plus_c (num_events,
					  sizeof (cairo_bo_start_event_t) +
					  sizeof (cairo_bo_event_t *),
					  sizeof (cairo_bo_event_t *));
	if (unlikely (events == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	event_ptrs = (cairo_bo_event_t **) (events + num_events);
    }

    for (i = 0; i < num_events; i++) {
	event_ptrs[i] = (cairo_bo_event_t *) &events[i];

	events[i].type = CAIRO_BO_EVENT_TYPE_START;
	events[i].point.y = polygon->edges[i].top;
	events[i].point.x =
	    _line_compute_intersection_x_for_y (&polygon->edges[i].line,
						events[i].point.y);

	events[i].edge.edge = polygon->edges[i];
	events[i].edge.deferred.right = NULL;
	events[i].edge.prev = NULL;
	events[i].edge.next = NULL;
    }

    num_limits = polygon->num_limits; polygon->num_limits = 0;
    polygon->num_edges = 0;

    status = _cairo_bentley_ottmann_tessellate_bo_edges (event_ptrs,
							 num_events,
							 fill_rule,
							 polygon);
    polygon->num_limits = num_limits;

    if (events != stack_events)
	free (events);

    if (DEBUG_POLYGON) {
	FILE *file = fopen ("reduce_out.txt", "w");
	_cairo_debug_print_polygon (file, polygon);
	fclose (file);
    }

    return status;
}
