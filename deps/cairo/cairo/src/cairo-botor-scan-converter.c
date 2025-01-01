/*
 * Copyright © 2004 Carl Worth
 * Copyright © 2006 Red Hat, Inc.
 * Copyright © 2007 David Turner
 * Copyright © 2008 M Joonas Pihlaja
 * Copyright © 2008 Chris Wilson
 * Copyright © 2009 Intel Corporation
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
 *      M Joonas Pihlaja <jpihlaja@cc.helsinki.fi>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

/* Provide definitions for standalone compilation */
#include "cairoint.h"

#include "cairo-error-private.h"
#include "cairo-list-inline.h"
#include "cairo-freelist-private.h"
#include "cairo-combsort-inline.h"

#include <setjmp.h>

#define STEP_X CAIRO_FIXED_ONE
#define STEP_Y CAIRO_FIXED_ONE
#define UNROLL3(x) x x x

#define STEP_XY (2*STEP_X*STEP_Y) /* Unit area in the step. */
#define AREA_TO_ALPHA(c)  (((c)*255 + STEP_XY/2) / STEP_XY)

typedef struct _cairo_bo_intersect_ordinate {
    int32_t ordinate;
    enum { EXACT, INEXACT } exactness;
} cairo_bo_intersect_ordinate_t;

typedef struct _cairo_bo_intersect_point {
    cairo_bo_intersect_ordinate_t x;
    cairo_bo_intersect_ordinate_t y;
} cairo_bo_intersect_point_t;

struct quorem {
    cairo_fixed_t quo;
    cairo_fixed_t rem;
};

struct run {
    struct run *next;
    int sign;
    cairo_fixed_t y;
};

typedef struct edge {
    cairo_list_t link;

    cairo_edge_t edge;

    /* Current x coordinate and advancement.
     * Initialised to the x coordinate of the top of the
     * edge. The quotient is in cairo_fixed_t units and the
     * remainder is mod dy in cairo_fixed_t units.
     */
    cairo_fixed_t dy;
    struct quorem x;
    struct quorem dxdy;
    struct quorem dxdy_full;

    cairo_bool_t vertical;
    unsigned int flags;

    int current_sign;
    struct run *runs;
} edge_t;

enum {
    START = 0x1,
    STOP = 0x2,
};

/* the parent is always given by index/2 */
#define PQ_PARENT_INDEX(i) ((i) >> 1)
#define PQ_FIRST_ENTRY 1

/* left and right children are index * 2 and (index * 2) +1 respectively */
#define PQ_LEFT_CHILD_INDEX(i) ((i) << 1)

typedef enum {
    EVENT_TYPE_STOP,
    EVENT_TYPE_INTERSECTION,
    EVENT_TYPE_START
} event_type_t;

typedef struct _event {
    cairo_fixed_t y;
    event_type_t type;
} event_t;

typedef struct _start_event {
    cairo_fixed_t y;
    event_type_t type;
    edge_t *edge;
} start_event_t;

typedef struct _queue_event {
    cairo_fixed_t y;
    event_type_t type;
    edge_t *e1;
    edge_t *e2;
} queue_event_t;

typedef struct _pqueue {
    int size, max_size;

    event_t **elements;
    event_t *elements_embedded[1024];
} pqueue_t;

struct cell {
    struct cell	*prev;
    struct cell	*next;
    int		 x;
    int		 uncovered_area;
    int		 covered_height;
};

typedef struct _sweep_line {
    cairo_list_t active;
    cairo_list_t stopped;
    cairo_list_t *insert_cursor;
    cairo_bool_t is_vertical;

    cairo_fixed_t current_row;
    cairo_fixed_t current_subrow;

    struct coverage {
	struct cell head;
	struct cell tail;

	struct cell *cursor;
	int count;

	cairo_freepool_t pool;
    } coverage;

    struct event_queue {
	pqueue_t pq;
	event_t **start_events;

	cairo_freepool_t pool;
    } queue;

    cairo_freepool_t runs;

    jmp_buf unwind;
} sweep_line_t;

cairo_always_inline static struct quorem
floored_divrem (int a, int b)
{
    struct quorem qr;
    qr.quo = a/b;
    qr.rem = a%b;
    if ((a^b)<0 && qr.rem) {
	qr.quo--;
	qr.rem += b;
    }
    return qr;
}

static struct quorem
floored_muldivrem(int x, int a, int b)
{
    struct quorem qr;
    long long xa = (long long)x*a;
    qr.quo = xa/b;
    qr.rem = xa%b;
    if ((xa>=0) != (b>=0) && qr.rem) {
	qr.quo--;
	qr.rem += b;
    }
    return qr;
}

static cairo_fixed_t
line_compute_intersection_x_for_y (const cairo_line_t *line,
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
edges_compare_x_for_y_general (const cairo_edge_t *a,
			       const cairo_edge_t *b,
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
           if (a->line.p1.x < a->line.p2.x) {
                   amin = a->line.p1.x;
                   amax = a->line.p2.x;
           } else {
                   amin = a->line.p2.x;
                   amax = a->line.p1.x;
           }
           if (b->line.p1.x < b->line.p2.x) {
                   bmin = b->line.p1.x;
                   bmax = b->line.p2.x;
           } else {
                   bmin = b->line.p2.x;
                   bmax = b->line.p1.x;
           }
           if (amax < bmin) return -1;
           if (amin > bmax) return +1;
    }

    ady = a->line.p2.y - a->line.p1.y;
    adx = a->line.p2.x - a->line.p1.x;
    if (adx == 0)
	have_dx_adx_bdx &= ~HAVE_ADX;

    bdy = b->line.p2.y - b->line.p1.y;
    bdx = b->line.p2.x - b->line.p1.x;
    if (bdx == 0)
	have_dx_adx_bdx &= ~HAVE_BDX;

    dx = a->line.p1.x - b->line.p1.x;
    if (dx == 0)
	have_dx_adx_bdx &= ~HAVE_DX;

#define L _cairo_int64x32_128_mul (_cairo_int32x32_64_mul (ady, bdy), dx)
#define A _cairo_int64x32_128_mul (_cairo_int32x32_64_mul (adx, bdy), y - a->line.p1.y)
#define B _cairo_int64x32_128_mul (_cairo_int32x32_64_mul (bdx, ady), y - b->line.p1.y)
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
	} else if (a->line.p1.y == b->line.p1.y) { /* common origin */
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
	    dy_adx = _cairo_int32x32_64_mul (a->line.p1.y - y, adx);

	    return _cairo_int64_cmp (ady_dx, dy_adx);
	}
    case HAVE_DX_BDX:
	/* B_dy * (A_x - B_x) ∘ (Y - B_y) * B_dx */
	if ((bdx ^ dx) < 0) {
	    return dx;
	} else {
	    cairo_int64_t bdy_dx, dy_bdx;

	    bdy_dx = _cairo_int32x32_64_mul (bdy, dx);
	    dy_bdx = _cairo_int32x32_64_mul (y - b->line.p1.y, bdx);

	    return _cairo_int64_cmp (bdy_dx, dy_bdx);
	}
    case HAVE_ALL:
	/* XXX try comparing (a->line.p2.x - b->line.p2.x) et al */
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
edge_compare_for_y_against_x (const cairo_edge_t *a,
			      int32_t y,
			      int32_t x)
{
    int32_t adx, ady;
    int32_t dx, dy;
    cairo_int64_t L, R;

    if (a->line.p1.x <= a->line.p2.x) {
	if (x < a->line.p1.x)
	    return 1;
	if (x > a->line.p2.x)
	    return -1;
    } else {
	if (x < a->line.p2.x)
	    return 1;
	if (x > a->line.p1.x)
	    return -1;
    }

    adx = a->line.p2.x - a->line.p1.x;
    dx = x - a->line.p1.x;

    if (adx == 0)
	return -dx;
    if (dx == 0 || (adx ^ dx) < 0)
	return adx;

    dy = y - a->line.p1.y;
    ady = a->line.p2.y - a->line.p1.y;

    L = _cairo_int32x32_64_mul (dy, adx);
    R = _cairo_int32x32_64_mul (dx, ady);

    return _cairo_int64_cmp (L, R);
}

static int
edges_compare_x_for_y (const cairo_edge_t *a,
		       const cairo_edge_t *b,
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

    /* XXX given we have x and dx? */

    if (y == a->line.p1.y)
	ax = a->line.p1.x;
    else if (y == a->line.p2.y)
	ax = a->line.p2.x;
    else
	have_ax_bx &= ~HAVE_AX;

    if (y == b->line.p1.y)
	bx = b->line.p1.x;
    else if (y == b->line.p2.y)
	bx = b->line.p2.x;
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
slope_compare (const edge_t *a,
	       const edge_t *b)
{
    cairo_int64_t L, R;
    int cmp;

    cmp = a->dxdy.quo - b->dxdy.quo;
    if (cmp)
	return cmp;

    if (a->dxdy.rem == 0)
	return -b->dxdy.rem;
    if (b->dxdy.rem == 0)
	return a->dxdy.rem;

    L = _cairo_int32x32_64_mul (b->dy, a->dxdy.rem);
    R = _cairo_int32x32_64_mul (a->dy, b->dxdy.rem);
    return _cairo_int64_cmp (L, R);
}

static inline int
line_equal (const cairo_line_t *a, const cairo_line_t *b)
{
    return a->p1.x == b->p1.x && a->p1.y == b->p1.y &&
           a->p2.x == b->p2.x && a->p2.y == b->p2.y;
}

static inline int
sweep_line_compare_edges (const edge_t	*a,
			  const edge_t	*b,
			  cairo_fixed_t y)
{
    int cmp;

    if (line_equal (&a->edge.line, &b->edge.line))
	return 0;

    cmp = edges_compare_x_for_y (&a->edge, &b->edge, y);
    if (cmp)
	return cmp;

    return slope_compare (a, b);
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
intersect_lines (const edge_t *a, const edge_t *b,
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
	/* compute ceiling away from zero */
	qr.quo = _cairo_int64_add (qr.quo,
				   _cairo_int32_to_int64 (_cairo_int64_negative (qr.quo) ? -1 : 1));
	intersection->y.exactness = INEXACT;
    }
#endif
    intersection->y.ordinate = _cairo_int64_to_int32 (qr.quo);

    return TRUE;
}

static int
bo_intersect_ordinate_32_compare (int32_t a, int32_t b, int exactness)
{
    int cmp;

    /* First compare the quotient */
    cmp = a - b;
    if (cmp)
	return cmp;

    /* With quotient identical, if remainder is 0 then compare equal */
    /* Otherwise, the non-zero remainder makes a > b */
    return -(INEXACT == exactness);
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
bo_edge_contains_intersect_point (const edge_t			*edge,
				  cairo_bo_intersect_point_t	*point)
{
    int cmp_top, cmp_bottom;

    /* XXX: When running the actual algorithm, we don't actually need to
     * compare against edge->top at all here, since any intersection above
     * top is eliminated early via a slope comparison. We're leaving these
     * here for now only for the sake of the quadratic-time intersection
     * finder which needs them.
     */

    cmp_top = bo_intersect_ordinate_32_compare (point->y.ordinate,
						edge->edge.top,
						point->y.exactness);
    if (cmp_top < 0)
	return FALSE;

    cmp_bottom = bo_intersect_ordinate_32_compare (point->y.ordinate,
						   edge->edge.bottom,
						   point->y.exactness);
    if (cmp_bottom > 0)
	return FALSE;

    if (cmp_top > 0 && cmp_bottom < 0)
	return TRUE;

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

	top_x = line_compute_intersection_x_for_y (&edge->edge.line,
						   edge->edge.top);
	return bo_intersect_ordinate_32_compare (top_x, point->x.ordinate, point->x.exactness) < 0;
    } else { /* cmp_bottom == 0 */
	cairo_fixed_t bot_x;

	bot_x = line_compute_intersection_x_for_y (&edge->edge.line,
						   edge->edge.bottom);
	return bo_intersect_ordinate_32_compare (point->x.ordinate, bot_x, point->x.exactness) < 0;
    }
}

static cairo_bool_t
edge_intersect (const edge_t		*a,
		const edge_t		*b,
		cairo_point_t	*intersection)
{
    cairo_bo_intersect_point_t quorem;

    if (! intersect_lines (a, b, &quorem))
	return FALSE;

    if (a->edge.top != a->edge.line.p1.y || a->edge.bottom != a->edge.line.p2.y) {
	if (! bo_edge_contains_intersect_point (a, &quorem))
	    return FALSE;
    }

    if (b->edge.top != b->edge.line.p1.y || b->edge.bottom != b->edge.line.p2.y) {
	if (! bo_edge_contains_intersect_point (b, &quorem))
	    return FALSE;
    }

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
event_compare (const event_t *a, const event_t *b)
{
    return a->y - b->y;
}

static void
pqueue_init (pqueue_t *pq)
{
    pq->max_size = ARRAY_LENGTH (pq->elements_embedded);
    pq->size = 0;

    pq->elements = pq->elements_embedded;
}

static void
pqueue_fini (pqueue_t *pq)
{
    if (pq->elements != pq->elements_embedded)
	free (pq->elements);
}

static cairo_bool_t
pqueue_grow (pqueue_t *pq)
{
    event_t **new_elements;
    pq->max_size *= 2;

    if (pq->elements == pq->elements_embedded) {
	new_elements = _cairo_malloc_ab (pq->max_size,
					 sizeof (event_t *));
	if (unlikely (new_elements == NULL))
	    return FALSE;

	memcpy (new_elements, pq->elements_embedded,
		sizeof (pq->elements_embedded));
    } else {
	new_elements = _cairo_realloc_ab (pq->elements,
					  pq->max_size,
					  sizeof (event_t *));
	if (unlikely (new_elements == NULL))
	    return FALSE;
    }

    pq->elements = new_elements;
    return TRUE;
}

static inline void
pqueue_push (sweep_line_t *sweep_line, event_t *event)
{
    event_t **elements;
    int i, parent;

    if (unlikely (sweep_line->queue.pq.size + 1 == sweep_line->queue.pq.max_size)) {
	if (unlikely (! pqueue_grow (&sweep_line->queue.pq))) {
	    longjmp (sweep_line->unwind,
		     _cairo_error (CAIRO_STATUS_NO_MEMORY));
	}
    }

    elements = sweep_line->queue.pq.elements;
    for (i = ++sweep_line->queue.pq.size;
	 i != PQ_FIRST_ENTRY &&
	 event_compare (event,
			elements[parent = PQ_PARENT_INDEX (i)]) < 0;
	 i = parent)
    {
	elements[i] = elements[parent];
    }

    elements[i] = event;
}

static inline void
pqueue_pop (pqueue_t *pq)
{
    event_t **elements = pq->elements;
    event_t *tail;
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
	    event_compare (elements[child+1],
			   elements[child]) < 0)
	{
	    child++;
	}

	if (event_compare (elements[child], tail) >= 0)
	    break;

	elements[i] = elements[child];
    }
    elements[i] = tail;
}

static inline void
event_insert (sweep_line_t	*sweep_line,
	      event_type_t	 type,
	      edge_t		*e1,
	      edge_t		*e2,
	      cairo_fixed_t	 y)
{
    queue_event_t *event;

    event = _cairo_freepool_alloc (&sweep_line->queue.pool);
    if (unlikely (event == NULL)) {
	longjmp (sweep_line->unwind,
		 _cairo_error (CAIRO_STATUS_NO_MEMORY));
    }

    event->y = y;
    event->type = type;
    event->e1 = e1;
    event->e2 = e2;

    pqueue_push (sweep_line, (event_t *) event);
}

static void
event_delete (sweep_line_t	*sweep_line,
	      event_t		*event)
{
    _cairo_freepool_free (&sweep_line->queue.pool, event);
}

static inline event_t *
event_next (sweep_line_t *sweep_line)
{
    event_t *event, *cmp;

    event = sweep_line->queue.pq.elements[PQ_FIRST_ENTRY];
    cmp = *sweep_line->queue.start_events;
    if (event == NULL ||
	(cmp != NULL && event_compare (cmp, event) < 0))
    {
	event = cmp;
	sweep_line->queue.start_events++;
    }
    else
    {
	pqueue_pop (&sweep_line->queue.pq);
    }

    return event;
}

CAIRO_COMBSORT_DECLARE (start_event_sort, event_t *, event_compare)

static inline void
event_insert_stop (sweep_line_t	*sweep_line,
		   edge_t	*edge)
{
    event_insert (sweep_line,
		  EVENT_TYPE_STOP,
		  edge, NULL,
		  edge->edge.bottom);
}

static inline void
event_insert_if_intersect_below_current_y (sweep_line_t	*sweep_line,
					   edge_t	*left,
					   edge_t	*right)
{
    cairo_point_t intersection;

    /* start points intersect */
    if (left->edge.line.p1.x == right->edge.line.p1.x &&
	left->edge.line.p1.y == right->edge.line.p1.y)
    {
	return;
    }

    /* end points intersect, process DELETE events first */
    if (left->edge.line.p2.x == right->edge.line.p2.x &&
	left->edge.line.p2.y == right->edge.line.p2.y)
    {
	return;
    }

    if (slope_compare (left, right) <= 0)
	return;

    if (! edge_intersect (left, right, &intersection))
	return;

    event_insert (sweep_line,
		  EVENT_TYPE_INTERSECTION,
		  left, right,
		  intersection.y);
}

static inline edge_t *
link_to_edge (cairo_list_t *link)
{
    return (edge_t *) link;
}

static void
sweep_line_insert (sweep_line_t	*sweep_line,
		   edge_t	*edge)
{
    cairo_list_t *pos;
    cairo_fixed_t y = sweep_line->current_subrow;

    pos = sweep_line->insert_cursor;
    if (pos == &sweep_line->active)
	pos = sweep_line->active.next;
    if (pos != &sweep_line->active) {
	int cmp;

	cmp = sweep_line_compare_edges (link_to_edge (pos),
					edge,
					y);
	if (cmp < 0) {
	    while (pos->next != &sweep_line->active &&
		   sweep_line_compare_edges (link_to_edge (pos->next),
					     edge,
					     y) < 0)
	    {
		pos = pos->next;
	    }
	} else if (cmp > 0) {
	    do {
		pos = pos->prev;
	    } while (pos != &sweep_line->active &&
		     sweep_line_compare_edges (link_to_edge (pos),
					       edge,
					       y) > 0);
	}
    }
    cairo_list_add (&edge->link, pos);
    sweep_line->insert_cursor = &edge->link;
}

inline static void
coverage_rewind (struct coverage *cells)
{
    cells->cursor = &cells->head;
}

static void
coverage_init (struct coverage *cells)
{
    _cairo_freepool_init (&cells->pool,
			  sizeof (struct cell));
    cells->head.prev = NULL;
    cells->head.next = &cells->tail;
    cells->head.x = INT_MIN;
    cells->tail.prev = &cells->head;
    cells->tail.next = NULL;
    cells->tail.x = INT_MAX;
    cells->count = 0;
    coverage_rewind (cells);
}

static void
coverage_fini (struct coverage *cells)
{
    _cairo_freepool_fini (&cells->pool);
}

inline static void
coverage_reset (struct coverage *cells)
{
    cells->head.next = &cells->tail;
    cells->tail.prev = &cells->head;
    cells->count = 0;
    _cairo_freepool_reset (&cells->pool);
    coverage_rewind (cells);
}

static struct cell *
coverage_alloc (sweep_line_t *sweep_line,
		struct cell *tail,
		int x)
{
    struct cell *cell;

    cell = _cairo_freepool_alloc (&sweep_line->coverage.pool);
    if (unlikely (NULL == cell)) {
	longjmp (sweep_line->unwind,
		 _cairo_error (CAIRO_STATUS_NO_MEMORY));
    }

    tail->prev->next = cell;
    cell->prev = tail->prev;
    cell->next = tail;
    tail->prev = cell;
    cell->x = x;
    cell->uncovered_area = 0;
    cell->covered_height = 0;
    sweep_line->coverage.count++;
    return cell;
}

inline static struct cell *
coverage_find (sweep_line_t *sweep_line, int x)
{
    struct cell *cell;

    cell = sweep_line->coverage.cursor;
    if (unlikely (cell->x > x)) {
	do {
	    if (cell->prev->x < x)
		break;
	    cell = cell->prev;
	} while (TRUE);
    } else {
	if (cell->x == x)
	    return cell;

	do {
	    UNROLL3({
		    cell = cell->next;
		    if (cell->x >= x)
			break;
		    });
	} while (TRUE);
    }

    if (cell->x != x)
	cell = coverage_alloc (sweep_line, cell, x);

    return sweep_line->coverage.cursor = cell;
}

static void
coverage_render_cells (sweep_line_t *sweep_line,
		       cairo_fixed_t left, cairo_fixed_t right,
		       cairo_fixed_t y1, cairo_fixed_t y2,
		       int sign)
{
    int fx1, fx2;
    int ix1, ix2;
    int dx, dy;

    /* Orient the edge left-to-right. */
    dx = right - left;
    if (dx >= 0) {
	ix1 = _cairo_fixed_integer_part (left);
	fx1 = _cairo_fixed_fractional_part (left);

	ix2 = _cairo_fixed_integer_part (right);
	fx2 = _cairo_fixed_fractional_part (right);

	dy = y2 - y1;
    } else {
	ix1 = _cairo_fixed_integer_part (right);
	fx1 = _cairo_fixed_fractional_part (right);

	ix2 = _cairo_fixed_integer_part (left);
	fx2 = _cairo_fixed_fractional_part (left);

	dx = -dx;
	sign = -sign;
	dy = y1 - y2;
	y1 = y2 - dy;
	y2 = y1 + dy;
    }

    /* Add coverage for all pixels [ix1,ix2] on this row crossed
     * by the edge. */
    {
	struct quorem y = floored_divrem ((STEP_X - fx1)*dy, dx);
	struct cell *cell;

	cell = sweep_line->coverage.cursor;
	if (cell->x != ix1) {
	    if (unlikely (cell->x > ix1)) {
		do {
		    if (cell->prev->x < ix1)
			break;
		    cell = cell->prev;
		} while (TRUE);
	    } else do {
		UNROLL3({
			if (cell->x >= ix1)
			    break;
			cell = cell->next;
			});
	    } while (TRUE);

	    if (cell->x != ix1)
		cell = coverage_alloc (sweep_line, cell, ix1);
	}

	cell->uncovered_area += sign * y.quo * (STEP_X + fx1);
	cell->covered_height += sign * y.quo;
	y.quo += y1;

	cell = cell->next;
	if (cell->x != ++ix1)
	    cell = coverage_alloc (sweep_line, cell, ix1);
	if (ix1 < ix2) {
	    struct quorem dydx_full = floored_divrem (STEP_X*dy, dx);

	    do {
		cairo_fixed_t y_skip = dydx_full.quo;
		y.rem += dydx_full.rem;
		if (y.rem >= dx) {
		    ++y_skip;
		    y.rem -= dx;
		}

		y.quo += y_skip;

		y_skip *= sign;
		cell->covered_height += y_skip;
		cell->uncovered_area += y_skip*STEP_X;

		cell = cell->next;
		if (cell->x != ++ix1)
		    cell = coverage_alloc (sweep_line, cell, ix1);
	    } while (ix1 != ix2);
	}
	cell->uncovered_area += sign*(y2 - y.quo)*fx2;
	cell->covered_height += sign*(y2 - y.quo);
	sweep_line->coverage.cursor = cell;
    }
}

inline static void
full_inc_edge (edge_t *edge)
{
    edge->x.quo += edge->dxdy_full.quo;
    edge->x.rem += edge->dxdy_full.rem;
    if (edge->x.rem >= 0) {
	++edge->x.quo;
	edge->x.rem -= edge->dy;
    }
}

static void
full_add_edge (sweep_line_t *sweep_line, edge_t *edge, int sign)
{
    struct cell *cell;
    cairo_fixed_t x1, x2;
    int ix1, ix2;
    int frac;

    edge->current_sign = sign;

    ix1 = _cairo_fixed_integer_part (edge->x.quo);

    if (edge->vertical) {
	frac = _cairo_fixed_fractional_part (edge->x.quo);
	cell = coverage_find (sweep_line, ix1);
	cell->covered_height += sign * STEP_Y;
	cell->uncovered_area += sign * 2 * frac * STEP_Y;
	return;
    }

    x1 = edge->x.quo;
    full_inc_edge (edge);
    x2 = edge->x.quo;

    ix2 = _cairo_fixed_integer_part (edge->x.quo);

    /* Edge is entirely within a column? */
    if (likely (ix1 == ix2)) {
	frac = _cairo_fixed_fractional_part (x1) +
	       _cairo_fixed_fractional_part (x2);
	cell = coverage_find (sweep_line, ix1);
	cell->covered_height += sign * STEP_Y;
	cell->uncovered_area += sign * frac * STEP_Y;
	return;
    }

    coverage_render_cells (sweep_line, x1, x2, 0, STEP_Y, sign);
}

static void
full_nonzero (sweep_line_t *sweep_line)
{
    cairo_list_t *pos;

    sweep_line->is_vertical = TRUE;
    pos = sweep_line->active.next;
    do {
	edge_t *left = link_to_edge (pos), *right;
	int winding = left->edge.dir;

	sweep_line->is_vertical &= left->vertical;

	pos = left->link.next;
	do {
	    if (unlikely (pos == &sweep_line->active)) {
		full_add_edge (sweep_line, left, +1);
		return;
	    }

	    right = link_to_edge (pos);
	    pos = pos->next;
	    sweep_line->is_vertical &= right->vertical;

	    winding += right->edge.dir;
	    if (0 == winding) {
		if (pos == &sweep_line->active ||
		    link_to_edge (pos)->x.quo != right->x.quo)
		{
		    break;
		}
	    }

	    if (! right->vertical)
		full_inc_edge (right);
	} while (TRUE);

	full_add_edge (sweep_line, left,  +1);
	full_add_edge (sweep_line, right, -1);
    } while (pos != &sweep_line->active);
}

static void
full_evenodd (sweep_line_t *sweep_line)
{
    cairo_list_t *pos;

    sweep_line->is_vertical = TRUE;
    pos = sweep_line->active.next;
    do {
	edge_t *left = link_to_edge (pos), *right;
	int winding = 0;

	sweep_line->is_vertical &= left->vertical;

	pos = left->link.next;
	do {
	    if (pos == &sweep_line->active) {
		full_add_edge (sweep_line, left, +1);
		return;
	    }

	    right = link_to_edge (pos);
	    pos = pos->next;
	    sweep_line->is_vertical &= right->vertical;

	    if (++winding & 1) {
		if (pos == &sweep_line->active ||
		    link_to_edge (pos)->x.quo != right->x.quo)
		{
		    break;
		}
	    }

	    if (! right->vertical)
		full_inc_edge (right);
	} while (TRUE);

	full_add_edge (sweep_line, left,  +1);
	full_add_edge (sweep_line, right, -1);
    } while (pos != &sweep_line->active);
}

static void
render_rows (cairo_botor_scan_converter_t *self,
	     sweep_line_t *sweep_line,
	     int y, int height,
	     cairo_span_renderer_t *renderer)
{
    cairo_half_open_span_t spans_stack[CAIRO_STACK_ARRAY_LENGTH (cairo_half_open_span_t)];
    cairo_half_open_span_t *spans = spans_stack;
    struct cell *cell;
    int prev_x, cover;
    int num_spans;
    cairo_status_t status;

    if (unlikely (sweep_line->coverage.count == 0)) {
	status = renderer->render_rows (renderer, y, height, NULL, 0);
	if (unlikely (status))
	    longjmp (sweep_line->unwind, status);
	return;
    }

    /* Allocate enough spans for the row. */

    num_spans = 2*sweep_line->coverage.count+2;
    if (unlikely (num_spans > ARRAY_LENGTH (spans_stack))) {
	spans = _cairo_malloc_ab (num_spans, sizeof (cairo_half_open_span_t));
	if (unlikely (spans == NULL)) {
	    longjmp (sweep_line->unwind,
		     _cairo_error (CAIRO_STATUS_NO_MEMORY));
	}
    }

    /* Form the spans from the coverage and areas. */
    num_spans = 0;
    prev_x = self->xmin;
    cover = 0;
    cell = sweep_line->coverage.head.next;
    do {
	int x = cell->x;
	int area;

	if (x > prev_x) {
	    spans[num_spans].x = prev_x;
	    spans[num_spans].inverse = 0;
	    spans[num_spans].coverage = AREA_TO_ALPHA (cover);
	    ++num_spans;
	}

	cover += cell->covered_height*STEP_X*2;
	area = cover - cell->uncovered_area;

	spans[num_spans].x = x;
	spans[num_spans].coverage = AREA_TO_ALPHA (area);
	++num_spans;

	prev_x = x + 1;
    } while ((cell = cell->next) != &sweep_line->coverage.tail);

    if (prev_x <= self->xmax) {
	spans[num_spans].x = prev_x;
	spans[num_spans].inverse = 0;
	spans[num_spans].coverage = AREA_TO_ALPHA (cover);
	++num_spans;
    }

    if (cover && prev_x < self->xmax) {
	spans[num_spans].x = self->xmax;
	spans[num_spans].inverse = 1;
	spans[num_spans].coverage = 0;
	++num_spans;
    }

    status = renderer->render_rows (renderer, y, height, spans, num_spans);

    if (unlikely (spans != spans_stack))
	free (spans);

    coverage_reset (&sweep_line->coverage);

    if (unlikely (status))
	longjmp (sweep_line->unwind, status);
}

static void
full_repeat (sweep_line_t *sweep)
{
    edge_t *edge;

    cairo_list_foreach_entry (edge, edge_t, &sweep->active, link) {
	if (edge->current_sign)
	    full_add_edge (sweep, edge, edge->current_sign);
	else if (! edge->vertical)
	    full_inc_edge (edge);
    }
}

static void
full_reset (sweep_line_t *sweep)
{
    edge_t *edge;

    cairo_list_foreach_entry (edge, edge_t, &sweep->active, link)
	edge->current_sign = 0;
}

static void
full_step (cairo_botor_scan_converter_t *self,
	   sweep_line_t *sweep_line,
	   cairo_fixed_t row,
	   cairo_span_renderer_t *renderer)
{
    int top, bottom;

    top = _cairo_fixed_integer_part (sweep_line->current_row);
    bottom = _cairo_fixed_integer_part (row);
    if (cairo_list_is_empty (&sweep_line->active)) {
	cairo_status_t  status;

	status = renderer->render_rows (renderer, top, bottom - top, NULL, 0);
	if (unlikely (status))
	    longjmp (sweep_line->unwind, status);

	return;
    }

    if (self->fill_rule == CAIRO_FILL_RULE_WINDING)
	full_nonzero (sweep_line);
    else
	full_evenodd (sweep_line);

    if (sweep_line->is_vertical || bottom == top + 1) {
	render_rows (self, sweep_line, top, bottom - top, renderer);
	full_reset (sweep_line);
	return;
    }

    render_rows (self, sweep_line, top++, 1, renderer);
    do {
	full_repeat (sweep_line);
	render_rows (self, sweep_line, top, 1, renderer);
    } while (++top != bottom);

    full_reset (sweep_line);
}

cairo_always_inline static void
sub_inc_edge (edge_t *edge,
	      cairo_fixed_t height)
{
    if (height == 1) {
	edge->x.quo += edge->dxdy.quo;
	edge->x.rem += edge->dxdy.rem;
	if (edge->x.rem >= 0) {
	    ++edge->x.quo;
	    edge->x.rem -= edge->dy;
	}
    } else {
	edge->x.quo += height * edge->dxdy.quo;
	edge->x.rem += height * edge->dxdy.rem;
	if (edge->x.rem >= 0) {
	    int carry = edge->x.rem / edge->dy + 1;
	    edge->x.quo += carry;
	    edge->x.rem -= carry * edge->dy;
	}
    }
}

static void
sub_add_run (sweep_line_t *sweep_line, edge_t *edge, int y, int sign)
{
    struct run *run;

    run = _cairo_freepool_alloc (&sweep_line->runs);
    if (unlikely (run == NULL))
	longjmp (sweep_line->unwind, _cairo_error (CAIRO_STATUS_NO_MEMORY));

    run->y = y;
    run->sign = sign;
    run->next = edge->runs;
    edge->runs = run;

    edge->current_sign = sign;
}

inline static cairo_bool_t
edges_coincident (edge_t *left, edge_t *right, cairo_fixed_t y)
{
    /* XXX is compare_x_for_y() worth executing during sub steps? */
    return line_equal (&left->edge.line, &right->edge.line);
    //edges_compare_x_for_y (&left->edge, &right->edge, y) >= 0;
}

static void
sub_nonzero (sweep_line_t *sweep_line)
{
    cairo_fixed_t y = sweep_line->current_subrow;
    cairo_fixed_t fy = _cairo_fixed_fractional_part (y);
    cairo_list_t *pos;

    pos = sweep_line->active.next;
    do {
	edge_t *left = link_to_edge (pos), *right;
	int winding = left->edge.dir;

	pos = left->link.next;
	do {
	    if (unlikely (pos == &sweep_line->active)) {
		if (left->current_sign != +1)
		    sub_add_run (sweep_line, left, fy, +1);
		return;
	    }

	    right = link_to_edge (pos);
	    pos = pos->next;

	    winding += right->edge.dir;
	    if (0 == winding) {
		if (pos == &sweep_line->active ||
		    ! edges_coincident (right, link_to_edge (pos), y))
		{
		    break;
		}
	    }

	    if (right->current_sign)
		sub_add_run (sweep_line, right, fy, 0);
	} while (TRUE);

	if (left->current_sign != +1)
	    sub_add_run (sweep_line, left, fy, +1);
	if (right->current_sign != -1)
	    sub_add_run (sweep_line, right, fy, -1);
    } while (pos != &sweep_line->active);
}

static void
sub_evenodd (sweep_line_t *sweep_line)
{
    cairo_fixed_t y = sweep_line->current_subrow;
    cairo_fixed_t fy = _cairo_fixed_fractional_part (y);
    cairo_list_t *pos;

    pos = sweep_line->active.next;
    do {
	edge_t *left = link_to_edge (pos), *right;
	int winding = 0;

	pos = left->link.next;
	do {
	    if (unlikely (pos == &sweep_line->active)) {
		if (left->current_sign != +1)
		    sub_add_run (sweep_line, left, fy, +1);
		return;
	    }

	    right = link_to_edge (pos);
	    pos = pos->next;

	    if (++winding & 1) {
		if (pos == &sweep_line->active ||
		    ! edges_coincident (right, link_to_edge (pos), y))
		{
		    break;
		}
	    }

	    if (right->current_sign)
		sub_add_run (sweep_line, right, fy, 0);
	} while (TRUE);

	if (left->current_sign != +1)
	    sub_add_run (sweep_line, left, fy, +1);
	if (right->current_sign != -1)
	    sub_add_run (sweep_line, right, fy, -1);
    } while (pos != &sweep_line->active);
}

cairo_always_inline static void
sub_step (cairo_botor_scan_converter_t *self,
	  sweep_line_t *sweep_line)
{
    if (cairo_list_is_empty (&sweep_line->active))
	return;

    if (self->fill_rule == CAIRO_FILL_RULE_WINDING)
	sub_nonzero (sweep_line);
    else
	sub_evenodd (sweep_line);
}

static void
coverage_render_runs (sweep_line_t *sweep, edge_t *edge,
		      cairo_fixed_t y1, cairo_fixed_t y2)
{
    struct run tail;
    struct run *run = &tail;

    tail.next = NULL;
    tail.y = y2;

    /* Order the runs top->bottom */
    while (edge->runs) {
	struct run *r;

	r = edge->runs;
	edge->runs = r->next;
	r->next = run;
	run = r;
    }

    if (run->y > y1)
	sub_inc_edge (edge, run->y - y1);

    do {
	cairo_fixed_t x1, x2;

	y1 = run->y;
	y2 = run->next->y;

	x1 = edge->x.quo;
	if (y2 - y1 == STEP_Y)
	    full_inc_edge (edge);
	else
	    sub_inc_edge (edge, y2 - y1);
	x2 = edge->x.quo;

	if (run->sign) {
	    int ix1, ix2;

	    ix1 = _cairo_fixed_integer_part (x1);
	    ix2 = _cairo_fixed_integer_part (x2);

	    /* Edge is entirely within a column? */
	    if (likely (ix1 == ix2)) {
		struct cell *cell;
		int frac;

		frac = _cairo_fixed_fractional_part (x1) +
		       _cairo_fixed_fractional_part (x2);
		cell = coverage_find (sweep, ix1);
		cell->covered_height += run->sign * (y2 - y1);
		cell->uncovered_area += run->sign * (y2 - y1) * frac;
	    } else {
		coverage_render_cells (sweep, x1, x2, y1, y2, run->sign);
	    }
	}

	run = run->next;
    } while (run->next != NULL);
}

static void
coverage_render_vertical_runs (sweep_line_t *sweep, edge_t *edge, cairo_fixed_t y2)
{
    struct cell *cell;
    struct run *run;
    int height = 0;

    for (run = edge->runs; run != NULL; run = run->next) {
	if (run->sign)
	    height += run->sign * (y2 - run->y);
	y2 = run->y;
    }

    cell = coverage_find (sweep, _cairo_fixed_integer_part (edge->x.quo));
    cell->covered_height += height;
    cell->uncovered_area += 2 * _cairo_fixed_fractional_part (edge->x.quo) * height;
}

cairo_always_inline static void
sub_emit (cairo_botor_scan_converter_t *self,
	  sweep_line_t *sweep,
	  cairo_span_renderer_t *renderer)
{
    edge_t *edge;

    sub_step (self, sweep);

    /* convert the runs into coverages */

    cairo_list_foreach_entry (edge, edge_t, &sweep->active, link) {
	if (edge->runs == NULL) {
	    if (! edge->vertical) {
		if (edge->flags & START) {
		    sub_inc_edge (edge,
				  STEP_Y - _cairo_fixed_fractional_part (edge->edge.top));
		    edge->flags &= ~START;
		} else
		    full_inc_edge (edge);
	    }
	} else {
	    if (edge->vertical) {
		coverage_render_vertical_runs (sweep, edge, STEP_Y);
	    } else {
		int y1 = 0;
		if (edge->flags & START) {
		    y1 = _cairo_fixed_fractional_part (edge->edge.top);
		    edge->flags &= ~START;
		}
		coverage_render_runs (sweep, edge, y1, STEP_Y);
	    }
	}
	edge->current_sign = 0;
	edge->runs = NULL;
    }

    cairo_list_foreach_entry (edge, edge_t, &sweep->stopped, link) {
	int y2 = _cairo_fixed_fractional_part (edge->edge.bottom);
	if (edge->vertical) {
	    coverage_render_vertical_runs (sweep, edge, y2);
	} else {
	    int y1 = 0;
	    if (edge->flags & START)
		y1 = _cairo_fixed_fractional_part (edge->edge.top);
	    coverage_render_runs (sweep, edge, y1, y2);
	}
    }
    cairo_list_init (&sweep->stopped);

    _cairo_freepool_reset (&sweep->runs);

    render_rows (self, sweep,
		 _cairo_fixed_integer_part (sweep->current_row), 1,
		 renderer);
}

static void
sweep_line_init (sweep_line_t	 *sweep_line,
		 event_t	**start_events,
		 int		  num_events)
{
    cairo_list_init (&sweep_line->active);
    cairo_list_init (&sweep_line->stopped);
    sweep_line->insert_cursor = &sweep_line->active;

    sweep_line->current_row = INT32_MIN;
    sweep_line->current_subrow = INT32_MIN;

    coverage_init (&sweep_line->coverage);
    _cairo_freepool_init (&sweep_line->runs, sizeof (struct run));

    start_event_sort (start_events, num_events);
    start_events[num_events] = NULL;

    sweep_line->queue.start_events = start_events;

    _cairo_freepool_init (&sweep_line->queue.pool,
			  sizeof (queue_event_t));
    pqueue_init (&sweep_line->queue.pq);
    sweep_line->queue.pq.elements[PQ_FIRST_ENTRY] = NULL;
}

static void
sweep_line_delete (sweep_line_t	*sweep_line,
		   edge_t	*edge)
{
    if (sweep_line->insert_cursor == &edge->link)
	sweep_line->insert_cursor = edge->link.prev;

    cairo_list_del (&edge->link);
    if (edge->runs)
	cairo_list_add_tail (&edge->link, &sweep_line->stopped);
    edge->flags |= STOP;
}

static void
sweep_line_swap (sweep_line_t	*sweep_line,
		 edge_t	*left,
		 edge_t	*right)
{
    right->link.prev = left->link.prev;
    left->link.next = right->link.next;
    right->link.next = &left->link;
    left->link.prev = &right->link;
    left->link.next->prev = &left->link;
    right->link.prev->next = &right->link;
}

static void
sweep_line_fini (sweep_line_t *sweep_line)
{
    pqueue_fini (&sweep_line->queue.pq);
    _cairo_freepool_fini (&sweep_line->queue.pool);
    coverage_fini (&sweep_line->coverage);
    _cairo_freepool_fini (&sweep_line->runs);
}

static cairo_status_t
botor_generate (cairo_botor_scan_converter_t	 *self,
		event_t				**start_events,
		cairo_span_renderer_t		 *renderer)
{
    cairo_status_t status;
    sweep_line_t sweep_line;
    cairo_fixed_t ybot;
    event_t *event;
    cairo_list_t *left, *right;
    edge_t *e1, *e2;
    int bottom;

    sweep_line_init (&sweep_line, start_events, self->num_edges);
    if ((status = setjmp (sweep_line.unwind)))
	goto unwind;

    ybot = self->extents.p2.y;
    sweep_line.current_subrow = self->extents.p1.y;
    sweep_line.current_row = _cairo_fixed_floor (self->extents.p1.y);
    event = *sweep_line.queue.start_events++;
    do {
	/* Can we process a full step in one go? */
	if (event->y >= sweep_line.current_row + STEP_Y) {
	    bottom = _cairo_fixed_floor (event->y);
	    full_step (self, &sweep_line, bottom, renderer);
	    sweep_line.current_row = bottom;
	    sweep_line.current_subrow = bottom;
	}

	do {
	    if (event->y > sweep_line.current_subrow) {
		sub_step (self, &sweep_line);
		sweep_line.current_subrow = event->y;
	    }

	    do {
		/* Update the active list using Bentley-Ottmann */
		switch (event->type) {
		case EVENT_TYPE_START:
		    e1 = ((start_event_t *) event)->edge;

		    sweep_line_insert (&sweep_line, e1);
		    event_insert_stop (&sweep_line, e1);

		    left = e1->link.prev;
		    right = e1->link.next;

		    if (left != &sweep_line.active) {
			event_insert_if_intersect_below_current_y (&sweep_line,
								   link_to_edge (left), e1);
		    }

		    if (right != &sweep_line.active) {
			event_insert_if_intersect_below_current_y (&sweep_line,
								   e1, link_to_edge (right));
		    }

		    break;

		case EVENT_TYPE_STOP:
		    e1 = ((queue_event_t *) event)->e1;
		    event_delete (&sweep_line, event);

		    left = e1->link.prev;
		    right = e1->link.next;

		    sweep_line_delete (&sweep_line, e1);

		    if (left != &sweep_line.active &&
			right != &sweep_line.active)
		    {
			 event_insert_if_intersect_below_current_y (&sweep_line,
								    link_to_edge (left),
								    link_to_edge (right));
		    }

		    break;

		case EVENT_TYPE_INTERSECTION:
		    e1 = ((queue_event_t *) event)->e1;
		    e2 = ((queue_event_t *) event)->e2;

		    event_delete (&sweep_line, event);
		    if (e1->flags & STOP)
			break;
		    if (e2->flags & STOP)
			break;

		    /* skip this intersection if its edges are not adjacent */
		    if (&e2->link != e1->link.next)
			break;

		    left = e1->link.prev;
		    right = e2->link.next;

		    sweep_line_swap (&sweep_line, e1, e2);

		    /* after the swap e2 is left of e1 */
		    if (left != &sweep_line.active) {
			event_insert_if_intersect_below_current_y (&sweep_line,
								   link_to_edge (left), e2);
		    }

		    if (right != &sweep_line.active) {
			event_insert_if_intersect_below_current_y (&sweep_line,
								   e1, link_to_edge (right));
		    }

		    break;
		}

		event = event_next (&sweep_line);
		if (event == NULL)
		    goto end;
	    } while (event->y == sweep_line.current_subrow);
	} while (event->y < sweep_line.current_row + STEP_Y);

	bottom = sweep_line.current_row + STEP_Y;
	sub_emit (self, &sweep_line, renderer);
	sweep_line.current_subrow = bottom;
	sweep_line.current_row = sweep_line.current_subrow;
    } while (TRUE);

  end:
    /* flush any partial spans */
    if (sweep_line.current_subrow != sweep_line.current_row) {
	sub_emit (self, &sweep_line, renderer);
	sweep_line.current_row += STEP_Y;
	sweep_line.current_subrow = sweep_line.current_row;
    }
    /* clear the rest */
    if (sweep_line.current_subrow < ybot) {
	bottom = _cairo_fixed_integer_part (sweep_line.current_row);
	status = renderer->render_rows (renderer,
					bottom, _cairo_fixed_integer_ceil (ybot) - bottom,
					NULL, 0);
    }

 unwind:
    sweep_line_fini (&sweep_line);

    return status;
}

static cairo_status_t
_cairo_botor_scan_converter_generate (void			*converter,
				      cairo_span_renderer_t	*renderer)
{
    cairo_botor_scan_converter_t *self = converter;
    start_event_t stack_events[CAIRO_STACK_ARRAY_LENGTH (start_event_t)];
    start_event_t *events;
    event_t *stack_event_ptrs[ARRAY_LENGTH (stack_events) + 1];
    event_t **event_ptrs;
    struct _cairo_botor_scan_converter_chunk *chunk;
    cairo_status_t status;
    int num_events;
    int i, j;

    num_events = self->num_edges;
    if (unlikely (0 == num_events)) {
	return renderer->render_rows (renderer,
				      _cairo_fixed_integer_floor (self->extents.p1.y),
				      _cairo_fixed_integer_ceil (self->extents.p2.y) -
				      _cairo_fixed_integer_floor (self->extents.p1.y),
				      NULL, 0);
    }

    events = stack_events;
    event_ptrs = stack_event_ptrs;
    if (unlikely (num_events >= ARRAY_LENGTH (stack_events))) {
	events = _cairo_malloc_ab_plus_c (num_events,
					  sizeof (start_event_t) + sizeof (event_t *),
					  sizeof (event_t *));
	if (unlikely (events == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	event_ptrs = (event_t **) (events + num_events);
    }

    j = 0;
    for (chunk = &self->chunks; chunk != NULL; chunk = chunk->next) {
	edge_t *edge;

	edge = chunk->base;
	for (i = 0; i < chunk->count; i++) {
	    event_ptrs[j] = (event_t *) &events[j];

	    events[j].y = edge->edge.top;
	    events[j].type = EVENT_TYPE_START;
	    events[j].edge = edge;

	    edge++, j++;
	}
    }

    status = botor_generate (self, event_ptrs, renderer);

    if (events != stack_events)
	free (events);

    return status;
}

static edge_t *
botor_allocate_edge (cairo_botor_scan_converter_t *self)
{
    struct _cairo_botor_scan_converter_chunk *chunk;

    chunk = self->tail;
    if (chunk->count == chunk->size) {
	int size;

	size = chunk->size * 2;
	chunk->next = _cairo_malloc_ab_plus_c (size,
					       sizeof (edge_t),
					       sizeof (struct _cairo_botor_scan_converter_chunk));
	if (unlikely (chunk->next == NULL))
	    return NULL;

	chunk = chunk->next;
	chunk->next = NULL;
	chunk->count = 0;
	chunk->size = size;
	chunk->base = chunk + 1;
	self->tail = chunk;
    }

    return (edge_t *) chunk->base + chunk->count++;
}

static cairo_status_t
botor_add_edge (cairo_botor_scan_converter_t *self,
		const cairo_edge_t *edge)
{
    edge_t *e;
    cairo_fixed_t dx, dy;

    e = botor_allocate_edge (self);
    if (unlikely (e == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    cairo_list_init (&e->link);
    e->edge = *edge;

    dx = edge->line.p2.x - edge->line.p1.x;
    dy = edge->line.p2.y - edge->line.p1.y;
    e->dy = dy;

    if (dx == 0) {
	e->vertical = TRUE;
	e->x.quo = edge->line.p1.x;
	e->x.rem = 0;
	e->dxdy.quo = 0;
	e->dxdy.rem = 0;
	e->dxdy_full.quo = 0;
	e->dxdy_full.rem = 0;
    } else {
	e->vertical = FALSE;
	e->dxdy = floored_divrem (dx, dy);
	if (edge->top == edge->line.p1.y) {
	    e->x.quo = edge->line.p1.x;
	    e->x.rem = 0;
	} else {
	    e->x = floored_muldivrem (edge->top - edge->line.p1.y,
				      dx, dy);
	    e->x.quo += edge->line.p1.x;
	}

	if (_cairo_fixed_integer_part (edge->bottom) - _cairo_fixed_integer_part (edge->top) > 1) {
	    e->dxdy_full = floored_muldivrem (STEP_Y, dx, dy);
	} else {
	    e->dxdy_full.quo = 0;
	    e->dxdy_full.rem = 0;
	}
    }

    e->x.rem = -e->dy;
    e->current_sign = 0;
    e->runs = NULL;
    e->flags = START;

    self->num_edges++;

    return CAIRO_STATUS_SUCCESS;
}

#if 0
static cairo_status_t
_cairo_botor_scan_converter_add_edge (void		*converter,
				      const cairo_point_t *p1,
				      const cairo_point_t *p2,
				      int top, int bottom,
				      int dir)
{
    cairo_botor_scan_converter_t *self = converter;
    cairo_edge_t edge;

    edge.line.p1 = *p1;
    edge.line.p2 = *p2;
    edge.top = top;
    edge.bottom = bottom;
    edge.dir = dir;

    return botor_add_edge (self, &edge);
}
#endif

cairo_status_t
_cairo_botor_scan_converter_add_polygon (cairo_botor_scan_converter_t *converter,
					 const cairo_polygon_t *polygon)
{
    cairo_botor_scan_converter_t *self = converter;
    cairo_status_t status;
    int i;

    for (i = 0; i < polygon->num_edges; i++) {
	status = botor_add_edge (self, &polygon->edges[i]);
	if (unlikely (status))
	    return status;
    }

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_botor_scan_converter_destroy (void *converter)
{
    cairo_botor_scan_converter_t *self = converter;
    struct _cairo_botor_scan_converter_chunk *chunk, *next;

    for (chunk = self->chunks.next; chunk != NULL; chunk = next) {
	next = chunk->next;
	free (chunk);
    }
}

void
_cairo_botor_scan_converter_init (cairo_botor_scan_converter_t *self,
				  const cairo_box_t *extents,
				  cairo_fill_rule_t fill_rule)
{
    self->base.destroy     = _cairo_botor_scan_converter_destroy;
    self->base.generate    = _cairo_botor_scan_converter_generate;

    self->extents   = *extents;
    self->fill_rule = fill_rule;

    self->xmin = _cairo_fixed_integer_floor (extents->p1.x);
    self->xmax = _cairo_fixed_integer_ceil (extents->p2.x);

    self->chunks.base = self->buf;
    self->chunks.next = NULL;
    self->chunks.count = 0;
    self->chunks.size = sizeof (self->buf) / sizeof (edge_t);
    self->tail = &self->chunks;

    self->num_edges = 0;
}
