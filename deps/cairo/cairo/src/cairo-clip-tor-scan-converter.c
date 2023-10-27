/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* glitter-paths - polygon scan converter
 *
 * Copyright (c) 2008  M Joonas Pihlaja
 * Copyright (c) 2007  David Turner
 *
 * Permission is hereby granted, free of charge, to any person
 * obtaining a copy of this software and associated documentation
 * files (the "Software"), to deal in the Software without
 * restriction, including without limitation the rights to use,
 * copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following
 * conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES
 * OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
 * HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
 * WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
 * OTHER DEALINGS IN THE SOFTWARE.
 */
/* This is the Glitter paths scan converter incorporated into cairo.
 * The source is from commit 734c53237a867a773640bd5b64816249fa1730f8
 * of
 *
 *   https://gitweb.freedesktop.org/?p=users/joonas/glitter-paths
 */
/* Glitter-paths is a stand alone polygon rasteriser derived from
 * David Turner's reimplementation of Tor Anderssons's 15x17
 * supersampling rasteriser from the Apparition graphics library.  The
 * main new feature here is cheaply choosing per-scan line between
 * doing fully analytical coverage computation for an entire row at a
 * time vs. using a supersampling approach.
 *
 * David Turner's code can be found at
 *
 *   http://david.freetype.org/rasterizer-shootout/raster-comparison-20070813.tar.bz2
 *
 * In particular this file incorporates large parts of ftgrays_tor10.h
 * from raster-comparison-20070813.tar.bz2
 */
/* Overview
 *
 * A scan converter's basic purpose to take polygon edges and convert
 * them into an RLE compressed A8 mask.  This one works in two phases:
 * gathering edges and generating spans.
 *
 * 1) As the user feeds the scan converter edges they are vertically
 * clipped and bucketted into a _polygon_ data structure.  The edges
 * are also snapped from the user's coordinates to the subpixel grid
 * coordinates used during scan conversion.
 *
 *     user
 *      |
 *      | edges
 *      V
 *    polygon buckets
 *
 * 2) Generating spans works by performing a vertical sweep of pixel
 * rows from top to bottom and maintaining an _active_list_ of edges
 * that intersect the row.  From the active list the fill rule
 * determines which edges are the left and right edges of the start of
 * each span, and their contribution is then accumulated into a pixel
 * coverage list (_cell_list_) as coverage deltas.  Once the coverage
 * deltas of all edges are known we can form spans of constant pixel
 * coverage by summing the deltas during a traversal of the cell list.
 * At the end of a pixel row the cell list is sent to a coverage
 * blitter for rendering to some target surface.
 *
 * The pixel coverages are computed by either supersampling the row
 * and box filtering a mono rasterisation, or by computing the exact
 * coverages of edges in the active list.  The supersampling method is
 * used whenever some edge starts or stops within the row or there are
 * edge intersections in the row.
 *
 *   polygon bucket for       \
 *   current pixel row        |
 *      |                     |
 *      | activate new edges  |  Repeat GRID_Y times if we
 *      V                     \  are supersampling this row,
 *   active list              /  or just once if we're computing
 *      |                     |  analytical coverage.
 *      | coverage deltas     |
 *      V                     |
 *   pixel coverage list     /
 *      |
 *      V
 *   coverage blitter
 */
#include "cairoint.h"
#include "cairo-spans-private.h"
#include "cairo-error-private.h"

#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>

/* The input coordinate scale and the rasterisation grid scales. */
#define GLITTER_INPUT_BITS CAIRO_FIXED_FRAC_BITS
#define GRID_X_BITS CAIRO_FIXED_FRAC_BITS
#define GRID_Y 15

/* Set glitter up to use a cairo span renderer to do the coverage
 * blitting. */
struct pool;
struct cell_list;

/*-------------------------------------------------------------------------
 * glitter-paths.h
 */

/* "Input scaled" numbers are fixed precision reals with multiplier
 * 2**GLITTER_INPUT_BITS.  Input coordinates are given to glitter as
 * pixel scaled numbers.  These get converted to the internal grid
 * scaled numbers as soon as possible. Internal overflow is possible
 * if GRID_X/Y inside glitter-paths.c is larger than
 * 1<<GLITTER_INPUT_BITS. */
#ifndef GLITTER_INPUT_BITS
#  define GLITTER_INPUT_BITS 8
#endif
#define GLITTER_INPUT_SCALE (1<<GLITTER_INPUT_BITS)
typedef int glitter_input_scaled_t;

/* Opaque type for scan converting. */
typedef struct glitter_scan_converter glitter_scan_converter_t;

/*-------------------------------------------------------------------------
 * glitter-paths.c: Implementation internal types
 */
#include <stdlib.h>
#include <string.h>
#include <limits.h>

/* All polygon coordinates are snapped onto a subsample grid. "Grid
 * scaled" numbers are fixed precision reals with multiplier GRID_X or
 * GRID_Y. */
typedef int grid_scaled_t;
typedef int grid_scaled_x_t;
typedef int grid_scaled_y_t;

/* Default x/y scale factors.
 *  You can either define GRID_X/Y_BITS to get a power-of-two scale
 *  or define GRID_X/Y separately. */
#if !defined(GRID_X) && !defined(GRID_X_BITS)
#  define GRID_X_BITS 8
#endif
#if !defined(GRID_Y) && !defined(GRID_Y_BITS)
#  define GRID_Y 15
#endif

/* Use GRID_X/Y_BITS to define GRID_X/Y if they're available. */
#ifdef GRID_X_BITS
#  define GRID_X (1 << GRID_X_BITS)
#endif
#ifdef GRID_Y_BITS
#  define GRID_Y (1 << GRID_Y_BITS)
#endif

/* The GRID_X_TO_INT_FRAC macro splits a grid scaled coordinate into
 * integer and fractional parts. The integer part is floored. */
#if defined(GRID_X_TO_INT_FRAC)
  /* do nothing */
#elif defined(GRID_X_BITS)
#  define GRID_X_TO_INT_FRAC(x, i, f) \
	_GRID_TO_INT_FRAC_shift(x, i, f, GRID_X_BITS)
#else
#  define GRID_X_TO_INT_FRAC(x, i, f) \
	_GRID_TO_INT_FRAC_general(x, i, f, GRID_X)
#endif

#define _GRID_TO_INT_FRAC_general(t, i, f, m) do {	\
    (i) = (t) / (m);					\
    (f) = (t) % (m);					\
    if ((f) < 0) {					\
	--(i);						\
	(f) += (m);					\
    }							\
} while (0)

#define _GRID_TO_INT_FRAC_shift(t, i, f, b) do {	\
    (f) = (t) & ((1 << (b)) - 1);			\
    (i) = (t) >> (b);					\
} while (0)

/* A grid area is a real in [0,1] scaled by 2*GRID_X*GRID_Y.  We want
 * to be able to represent exactly areas of subpixel trapezoids whose
 * vertices are given in grid scaled coordinates.  The scale factor
 * comes from needing to accurately represent the area 0.5*dx*dy of a
 * triangle with base dx and height dy in grid scaled numbers. */
typedef int grid_area_t;
#define GRID_XY (2*GRID_X*GRID_Y) /* Unit area on the grid. */

/* GRID_AREA_TO_ALPHA(area): map [0,GRID_XY] to [0,255]. */
#if GRID_XY == 510
#  define GRID_AREA_TO_ALPHA(c)	  (((c)+1) >> 1)
#elif GRID_XY == 255
#  define  GRID_AREA_TO_ALPHA(c)  (c)
#elif GRID_XY == 64
#  define  GRID_AREA_TO_ALPHA(c)  (((c) << 2) | -(((c) & 0x40) >> 6))
#elif GRID_XY == 128
#  define  GRID_AREA_TO_ALPHA(c)  ((((c) << 1) | -((c) >> 7)) & 255)
#elif GRID_XY == 256
#  define  GRID_AREA_TO_ALPHA(c)  (((c) | -((c) >> 8)) & 255)
#elif GRID_XY == 15
#  define  GRID_AREA_TO_ALPHA(c)  (((c) << 4) + (c))
#elif GRID_XY == 2*256*15
#  define  GRID_AREA_TO_ALPHA(c)  (((c) + ((c)<<4) + 256) >> 9)
#else
#  define  GRID_AREA_TO_ALPHA(c)  (((c)*255 + GRID_XY/2) / GRID_XY)
#endif

#define UNROLL3(x) x x x

struct quorem {
    int32_t quo;
    int32_t rem;
};

/* Header for a chunk of memory in a memory pool. */
struct _pool_chunk {
    /* # bytes used in this chunk. */
    size_t size;

    /* # bytes total in this chunk */
    size_t capacity;

    /* Pointer to the previous chunk or %NULL if this is the sentinel
     * chunk in the pool header. */
    struct _pool_chunk *prev_chunk;

    /* Actual data starts here.	 Well aligned for pointers. */
};

/* A memory pool.  This is supposed to be embedded on the stack or
 * within some other structure.	 It may optionally be followed by an
 * embedded array from which requests are fulfilled until
 * malloc needs to be called to allocate a first real chunk. */
struct pool {
    /* Chunk we're allocating from. */
    struct _pool_chunk *current;

    jmp_buf *jmp;

    /* Free list of previously allocated chunks.  All have >= default
     * capacity. */
    struct _pool_chunk *first_free;

    /* The default capacity of a chunk. */
    size_t default_capacity;

    /* Header for the sentinel chunk.  Directly following the pool
     * struct should be some space for embedded elements from which
     * the sentinel chunk allocates from. */
    struct _pool_chunk sentinel[1];
};

/* A polygon edge. */
struct edge {
    /* Next in y-bucket or active list. */
    struct edge *next;

    /* Current x coordinate while the edge is on the active
     * list. Initialised to the x coordinate of the top of the
     * edge. The quotient is in grid_scaled_x_t units and the
     * remainder is mod dy in grid_scaled_y_t units.*/
    struct quorem x;

    /* Advance of the current x when moving down a subsample line. */
    struct quorem dxdy;

    /* Advance of the current x when moving down a full pixel
     * row. Only initialised when the height of the edge is large
     * enough that there's a chance the edge could be stepped by a
     * full row's worth of subsample rows at a time. */
    struct quorem dxdy_full;

    /* The clipped y of the top of the edge. */
    grid_scaled_y_t ytop;

    /* y2-y1 after orienting the edge downwards.  */
    grid_scaled_y_t dy;

    /* Number of subsample rows remaining to scan convert of this
     * edge. */
    grid_scaled_y_t height_left;

    /* Original sign of the edge: +1 for downwards, -1 for upwards
     * edges.  */
    int dir;
    int vertical;
    int clip;
};

/* Number of subsample rows per y-bucket. Must be GRID_Y. */
#define EDGE_Y_BUCKET_HEIGHT GRID_Y

#define EDGE_Y_BUCKET_INDEX(y, ymin) (((y) - (ymin))/EDGE_Y_BUCKET_HEIGHT)

/* A collection of sorted and vertically clipped edges of the polygon.
 * Edges are moved from the polygon to an active list while scan
 * converting. */
struct polygon {
    /* The vertical clip extents. */
    grid_scaled_y_t ymin, ymax;

    /* Array of edges all starting in the same bucket.	An edge is put
     * into bucket EDGE_BUCKET_INDEX(edge->ytop, polygon->ymin) when
     * it is added to the polygon. */
    struct edge **y_buckets;
    struct edge *y_buckets_embedded[64];

    struct {
	struct pool base[1];
	struct edge embedded[32];
    } edge_pool;
};

/* A cell records the effect on pixel coverage of polygon edges
 * passing through a pixel.  It contains two accumulators of pixel
 * coverage.
 *
 * Consider the effects of a polygon edge on the coverage of a pixel
 * it intersects and that of the following one.  The coverage of the
 * following pixel is the height of the edge multiplied by the width
 * of the pixel, and the coverage of the pixel itself is the area of
 * the trapezoid formed by the edge and the right side of the pixel.
 *
 * +-----------------------+-----------------------+
 * |                       |                       |
 * |                       |                       |
 * |_______________________|_______________________|
 * |   \...................|.......................|\
 * |    \..................|.......................| |
 * |     \.................|.......................| |
 * |      \....covered.....|.......................| |
 * |       \....area.......|.......................| } covered height
 * |        \..............|.......................| |
 * |uncovered\.............|.......................| |
 * |  area    \............|.......................| |
 * |___________\...........|.......................|/
 * |                       |                       |
 * |                       |                       |
 * |                       |                       |
 * +-----------------------+-----------------------+
 *
 * Since the coverage of the following pixel will always be a multiple
 * of the width of the pixel, we can store the height of the covered
 * area instead.  The coverage of the pixel itself is the total
 * coverage minus the area of the uncovered area to the left of the
 * edge.  As it's faster to compute the uncovered area we only store
 * that and subtract it from the total coverage later when forming
 * spans to blit.
 *
 * The heights and areas are signed, with left edges of the polygon
 * having positive sign and right edges having negative sign.  When
 * two edges intersect they swap their left/rightness so their
 * contribution above and below the intersection point must be
 * computed separately. */
struct cell {
    struct cell		*next;
    int			 x;
    grid_area_t		 uncovered_area;
    grid_scaled_y_t	 covered_height;
    grid_scaled_y_t	 clipped_height;
};

/* A cell list represents the scan line sparsely as cells ordered by
 * ascending x.  It is geared towards scanning the cells in order
 * using an internal cursor. */
struct cell_list {
    /* Sentinel nodes */
    struct cell head, tail;

    /* Cursor state for iterating through the cell list. */
    struct cell *cursor;

    /* Cells in the cell list are owned by the cell list and are
     * allocated from this pool.  */
    struct {
	struct pool base[1];
	struct cell embedded[32];
    } cell_pool;
};

struct cell_pair {
    struct cell *cell1;
    struct cell *cell2;
};

/* The active list contains edges in the current scan line ordered by
 * the x-coordinate of the intercept of the edge and the scan line. */
struct active_list {
    /* Leftmost edge on the current scan line. */
    struct edge *head;

    /* A lower bound on the height of the active edges is used to
     * estimate how soon some active edge ends.	 We can't advance the
     * scan conversion by a full pixel row if an edge ends somewhere
     * within it. */
    grid_scaled_y_t min_height;
};

struct glitter_scan_converter {
    struct polygon	polygon[1];
    struct active_list	active[1];
    struct cell_list	coverages[1];

    /* Clip box. */
    grid_scaled_y_t ymin, ymax;
};

/* Compute the floored division a/b. Assumes / and % perform symmetric
 * division. */
inline static struct quorem
floored_divrem(int a, int b)
{
    struct quorem qr;
    qr.quo = a/b;
    qr.rem = a%b;
    if ((a^b)<0 && qr.rem) {
	qr.quo -= 1;
	qr.rem += b;
    }
    return qr;
}

/* Compute the floored division (x*a)/b. Assumes / and % perform symmetric
 * division. */
static struct quorem
floored_muldivrem(int x, int a, int b)
{
    struct quorem qr;
    long long xa = (long long)x*a;
    qr.quo = xa/b;
    qr.rem = xa%b;
    if ((xa>=0) != (b>=0) && qr.rem) {
	qr.quo -= 1;
	qr.rem += b;
    }
    return qr;
}

static struct _pool_chunk *
_pool_chunk_init(
    struct _pool_chunk *p,
    struct _pool_chunk *prev_chunk,
    size_t capacity)
{
    p->prev_chunk = prev_chunk;
    p->size = 0;
    p->capacity = capacity;
    return p;
}

static struct _pool_chunk *
_pool_chunk_create(struct pool *pool, size_t size)
{
    struct _pool_chunk *p;

    p = _cairo_malloc (size + sizeof(struct _pool_chunk));
    if (unlikely (NULL == p))
	longjmp (*pool->jmp, _cairo_error (CAIRO_STATUS_NO_MEMORY));

    return _pool_chunk_init(p, pool->current, size);
}

static void
pool_init(struct pool *pool,
	  jmp_buf *jmp,
	  size_t default_capacity,
	  size_t embedded_capacity)
{
    pool->jmp = jmp;
    pool->current = pool->sentinel;
    pool->first_free = NULL;
    pool->default_capacity = default_capacity;
    _pool_chunk_init(pool->sentinel, NULL, embedded_capacity);
}

static void
pool_fini(struct pool *pool)
{
    struct _pool_chunk *p = pool->current;
    do {
	while (NULL != p) {
	    struct _pool_chunk *prev = p->prev_chunk;
	    if (p != pool->sentinel)
		free(p);
	    p = prev;
	}
	p = pool->first_free;
	pool->first_free = NULL;
    } while (NULL != p);
}

/* Satisfy an allocation by first allocating a new large enough chunk
 * and adding it to the head of the pool's chunk list. This function
 * is called as a fallback if pool_alloc() couldn't do a quick
 * allocation from the current chunk in the pool. */
static void *
_pool_alloc_from_new_chunk(
    struct pool *pool,
    size_t size)
{
    struct _pool_chunk *chunk;
    void *obj;
    size_t capacity;

    /* If the allocation is smaller than the default chunk size then
     * try getting a chunk off the free list.  Force alloc of a new
     * chunk for large requests. */
    capacity = size;
    chunk = NULL;
    if (size < pool->default_capacity) {
	capacity = pool->default_capacity;
	chunk = pool->first_free;
	if (chunk) {
	    pool->first_free = chunk->prev_chunk;
	    _pool_chunk_init(chunk, pool->current, chunk->capacity);
	}
    }

    if (NULL == chunk)
	chunk = _pool_chunk_create (pool, capacity);
    pool->current = chunk;

    obj = ((unsigned char*)chunk + sizeof(*chunk) + chunk->size);
    chunk->size += size;
    return obj;
}

/* Allocate size bytes from the pool.  The first allocated address
 * returned from a pool is aligned to sizeof(void*).  Subsequent
 * addresses will maintain alignment as long as multiples of void* are
 * allocated.  Returns the address of a new memory area or %NULL on
 * allocation failures.	 The pool retains ownership of the returned
 * memory. */
inline static void *
pool_alloc (struct pool *pool, size_t size)
{
    struct _pool_chunk *chunk = pool->current;

    if (size <= chunk->capacity - chunk->size) {
	void *obj = ((unsigned char*)chunk + sizeof(*chunk) + chunk->size);
	chunk->size += size;
	return obj;
    } else {
	return _pool_alloc_from_new_chunk(pool, size);
    }
}

/* Relinquish all pool_alloced memory back to the pool. */
static void
pool_reset (struct pool *pool)
{
    /* Transfer all used chunks to the chunk free list. */
    struct _pool_chunk *chunk = pool->current;
    if (chunk != pool->sentinel) {
	while (chunk->prev_chunk != pool->sentinel) {
	    chunk = chunk->prev_chunk;
	}
	chunk->prev_chunk = pool->first_free;
	pool->first_free = pool->current;
    }
    /* Reset the sentinel as the current chunk. */
    pool->current = pool->sentinel;
    pool->sentinel->size = 0;
}

/* Rewinds the cell list's cursor to the beginning.  After rewinding
 * we're good to cell_list_find() the cell any x coordinate. */
inline static void
cell_list_rewind (struct cell_list *cells)
{
    cells->cursor = &cells->head;
}

/* Rewind the cell list if its cursor has been advanced past x. */
inline static void
cell_list_maybe_rewind (struct cell_list *cells, int x)
{
    struct cell *tail = cells->cursor;
    if (tail->x > x)
	cell_list_rewind (cells);
}

static void
cell_list_init(struct cell_list *cells, jmp_buf *jmp)
{
    pool_init(cells->cell_pool.base, jmp,
	      256*sizeof(struct cell),
	      sizeof(cells->cell_pool.embedded));
    cells->tail.next = NULL;
    cells->tail.x = INT_MAX;
    cells->head.x = INT_MIN;
    cells->head.next = &cells->tail;
    cell_list_rewind (cells);
}

static void
cell_list_fini(struct cell_list *cells)
{
    pool_fini (cells->cell_pool.base);
}

/* Empty the cell list.  This is called at the start of every pixel
 * row. */
inline static void
cell_list_reset (struct cell_list *cells)
{
    cell_list_rewind (cells);
    cells->head.next = &cells->tail;
    pool_reset (cells->cell_pool.base);
}

static struct cell *
cell_list_alloc (struct cell_list *cells,
		 struct cell *tail,
		 int x)
{
    struct cell *cell;

    cell = pool_alloc (cells->cell_pool.base, sizeof (struct cell));
    cell->next = tail->next;
    tail->next = cell;
    cell->x = x;
    cell->uncovered_area = 0;
    cell->covered_height = 0;
    cell->clipped_height = 0;
    return cell;
}

/* Find a cell at the given x-coordinate.  Returns %NULL if a new cell
 * needed to be allocated but couldn't be.  Cells must be found with
 * non-decreasing x-coordinate until the cell list is rewound using
 * cell_list_rewind(). Ownership of the returned cell is retained by
 * the cell list. */
inline static struct cell *
cell_list_find (struct cell_list *cells, int x)
{
    struct cell *tail = cells->cursor;

    while (1) {
	UNROLL3({
	    if (tail->next->x > x)
		break;
	    tail = tail->next;
	});
    }

    if (tail->x != x)
	tail = cell_list_alloc (cells, tail, x);
    return cells->cursor = tail;

}

/* Find two cells at x1 and x2.	 This is exactly equivalent
 * to
 *
 *   pair.cell1 = cell_list_find(cells, x1);
 *   pair.cell2 = cell_list_find(cells, x2);
 *
 * except with less function call overhead. */
inline static struct cell_pair
cell_list_find_pair(struct cell_list *cells, int x1, int x2)
{
    struct cell_pair pair;

    pair.cell1 = cells->cursor;
    while (1) {
	UNROLL3({
	    if (pair.cell1->next->x > x1)
		break;
	    pair.cell1 = pair.cell1->next;
	});
    }
    if (pair.cell1->x != x1) {
	struct cell *cell = pool_alloc (cells->cell_pool.base,
					sizeof (struct cell));
	cell->x = x1;
	cell->uncovered_area = 0;
	cell->covered_height = 0;
	cell->clipped_height = 0;
	cell->next = pair.cell1->next;
	pair.cell1->next = cell;
	pair.cell1 = cell;
    }

    pair.cell2 = pair.cell1;
    while (1) {
	UNROLL3({
	    if (pair.cell2->next->x > x2)
		break;
	    pair.cell2 = pair.cell2->next;
	});
    }
    if (pair.cell2->x != x2) {
	struct cell *cell = pool_alloc (cells->cell_pool.base,
					sizeof (struct cell));
	cell->uncovered_area = 0;
	cell->covered_height = 0;
	cell->clipped_height = 0;
	cell->x = x2;
	cell->next = pair.cell2->next;
	pair.cell2->next = cell;
	pair.cell2 = cell;
    }

    cells->cursor = pair.cell2;
    return pair;
}

/* Add a subpixel span covering [x1, x2) to the coverage cells. */
inline static void
cell_list_add_subspan(struct cell_list *cells,
		      grid_scaled_x_t x1,
		      grid_scaled_x_t x2)
{
    int ix1, fx1;
    int ix2, fx2;

    GRID_X_TO_INT_FRAC(x1, ix1, fx1);
    GRID_X_TO_INT_FRAC(x2, ix2, fx2);

    if (ix1 != ix2) {
	struct cell_pair p;
	p = cell_list_find_pair(cells, ix1, ix2);
	p.cell1->uncovered_area += 2*fx1;
	++p.cell1->covered_height;
	p.cell2->uncovered_area -= 2*fx2;
	--p.cell2->covered_height;
    } else {
	struct cell *cell = cell_list_find(cells, ix1);
	cell->uncovered_area += 2*(fx1-fx2);
    }
}

/* Adds the analytical coverage of an edge crossing the current pixel
 * row to the coverage cells and advances the edge's x position to the
 * following row.
 *
 * This function is only called when we know that during this pixel row:
 *
 * 1) The relative order of all edges on the active list doesn't
 * change.  In particular, no edges intersect within this row to pixel
 * precision.
 *
 * 2) No new edges start in this row.
 *
 * 3) No existing edges end mid-row.
 *
 * This function depends on being called with all edges from the
 * active list in the order they appear on the list (i.e. with
 * non-decreasing x-coordinate.)  */
static void
cell_list_render_edge(
    struct cell_list *cells,
    struct edge *edge,
    int sign)
{
    grid_scaled_y_t y1, y2, dy;
    grid_scaled_x_t dx;
    int ix1, ix2;
    grid_scaled_x_t fx1, fx2;

    struct quorem x1 = edge->x;
    struct quorem x2 = x1;

    if (! edge->vertical) {
	x2.quo += edge->dxdy_full.quo;
	x2.rem += edge->dxdy_full.rem;
	if (x2.rem >= 0) {
	    ++x2.quo;
	    x2.rem -= edge->dy;
	}

	edge->x = x2;
    }

    GRID_X_TO_INT_FRAC(x1.quo, ix1, fx1);
    GRID_X_TO_INT_FRAC(x2.quo, ix2, fx2);

    /* Edge is entirely within a column? */
    if (ix1 == ix2) {
	/* We always know that ix1 is >= the cell list cursor in this
	 * case due to the no-intersections precondition.  */
	struct cell *cell = cell_list_find(cells, ix1);
	cell->covered_height += sign*GRID_Y;
	cell->uncovered_area += sign*(fx1 + fx2)*GRID_Y;
	return;
    }

    /* Orient the edge left-to-right. */
    dx = x2.quo - x1.quo;
    if (dx >= 0) {
	y1 = 0;
	y2 = GRID_Y;
    } else {
	int tmp;
	tmp = ix1; ix1 = ix2; ix2 = tmp;
	tmp = fx1; fx1 = fx2; fx2 = tmp;
	dx = -dx;
	sign = -sign;
	y1 = GRID_Y;
	y2 = 0;
    }
    dy = y2 - y1;

    /* Add coverage for all pixels [ix1,ix2] on this row crossed
     * by the edge. */
    {
	struct cell_pair pair;
	struct quorem y = floored_divrem((GRID_X - fx1)*dy, dx);

	/* When rendering a previous edge on the active list we may
	 * advance the cell list cursor past the leftmost pixel of the
	 * current edge even though the two edges don't intersect.
	 * e.g. consider two edges going down and rightwards:
	 *
	 *  --\_+---\_+-----+-----+----
	 *      \_    \_    |     |
	 *      | \_  | \_  |     |
	 *      |   \_|   \_|     |
	 *      |     \_    \_    |
	 *  ----+-----+-\---+-\---+----
	 *
	 * The left edge touches cells past the starting cell of the
	 * right edge.  Fortunately such cases are rare.
	 *
	 * The rewinding is never necessary if the current edge stays
	 * within a single column because we've checked before calling
	 * this function that the active list order won't change. */
	cell_list_maybe_rewind(cells, ix1);

	pair = cell_list_find_pair(cells, ix1, ix1+1);
	pair.cell1->uncovered_area += sign*y.quo*(GRID_X + fx1);
	pair.cell1->covered_height += sign*y.quo;
	y.quo += y1;

	if (ix1+1 < ix2) {
	    struct quorem dydx_full = floored_divrem(GRID_X*dy, dx);
	    struct cell *cell = pair.cell2;

	    ++ix1;
	    do {
		grid_scaled_y_t y_skip = dydx_full.quo;
		y.rem += dydx_full.rem;
		if (y.rem >= dx) {
		    ++y_skip;
		    y.rem -= dx;
		}

		y.quo += y_skip;

		y_skip *= sign;
		cell->uncovered_area += y_skip*GRID_X;
		cell->covered_height += y_skip;

		++ix1;
		cell = cell_list_find(cells, ix1);
	    } while (ix1 != ix2);

	    pair.cell2 = cell;
	}
	pair.cell2->uncovered_area += sign*(y2 - y.quo)*fx2;
	pair.cell2->covered_height += sign*(y2 - y.quo);
    }
}

static void
polygon_init (struct polygon *polygon, jmp_buf *jmp)
{
    polygon->ymin = polygon->ymax = 0;
    polygon->y_buckets = polygon->y_buckets_embedded;
    pool_init (polygon->edge_pool.base, jmp,
	       8192 - sizeof (struct _pool_chunk),
	       sizeof (polygon->edge_pool.embedded));
}

static void
polygon_fini (struct polygon *polygon)
{
    if (polygon->y_buckets != polygon->y_buckets_embedded)
	free (polygon->y_buckets);

    pool_fini (polygon->edge_pool.base);
}

/* Empties the polygon of all edges. The polygon is then prepared to
 * receive new edges and clip them to the vertical range
 * [ymin,ymax). */
static cairo_status_t
polygon_reset (struct polygon *polygon,
	       grid_scaled_y_t ymin,
	       grid_scaled_y_t ymax)
{
    unsigned h = ymax - ymin;
    unsigned num_buckets = EDGE_Y_BUCKET_INDEX(ymax + EDGE_Y_BUCKET_HEIGHT-1,
					       ymin);

    pool_reset(polygon->edge_pool.base);

    if (unlikely (h > 0x7FFFFFFFU - EDGE_Y_BUCKET_HEIGHT))
	goto bail_no_mem; /* even if you could, you wouldn't want to. */

    if (polygon->y_buckets != polygon->y_buckets_embedded)
	free (polygon->y_buckets);

    polygon->y_buckets =  polygon->y_buckets_embedded;
    if (num_buckets > ARRAY_LENGTH (polygon->y_buckets_embedded)) {
	polygon->y_buckets = _cairo_malloc_ab (num_buckets,
					       sizeof (struct edge *));
	if (unlikely (NULL == polygon->y_buckets))
	    goto bail_no_mem;
    }
    memset (polygon->y_buckets, 0, num_buckets * sizeof (struct edge *));

    polygon->ymin = ymin;
    polygon->ymax = ymax;
    return CAIRO_STATUS_SUCCESS;

 bail_no_mem:
    polygon->ymin = 0;
    polygon->ymax = 0;
    return CAIRO_STATUS_NO_MEMORY;
}

static void
_polygon_insert_edge_into_its_y_bucket(
    struct polygon *polygon,
    struct edge *e)
{
    unsigned ix = EDGE_Y_BUCKET_INDEX(e->ytop, polygon->ymin);
    struct edge **ptail = &polygon->y_buckets[ix];
    e->next = *ptail;
    *ptail = e;
}

inline static void
polygon_add_edge (struct polygon *polygon,
		  const cairo_edge_t *edge,
		  int clip)
{
    struct edge *e;
    grid_scaled_x_t dx;
    grid_scaled_y_t dy;
    grid_scaled_y_t ytop, ybot;
    grid_scaled_y_t ymin = polygon->ymin;
    grid_scaled_y_t ymax = polygon->ymax;

    assert (edge->bottom > edge->top);

    if (unlikely (edge->top >= ymax || edge->bottom <= ymin))
	return;

    e = pool_alloc (polygon->edge_pool.base, sizeof (struct edge));

    dx = edge->line.p2.x - edge->line.p1.x;
    dy = edge->line.p2.y - edge->line.p1.y;
    e->dy = dy;
    e->dir = edge->dir;
    e->clip = clip;

    ytop = edge->top >= ymin ? edge->top : ymin;
    ybot = edge->bottom <= ymax ? edge->bottom : ymax;
    e->ytop = ytop;
    e->height_left = ybot - ytop;

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
	if (ytop == edge->line.p1.y) {
	    e->x.quo = edge->line.p1.x;
	    e->x.rem = 0;
	} else {
	    e->x = floored_muldivrem (ytop - edge->line.p1.y, dx, dy);
	    e->x.quo += edge->line.p1.x;
	}

	if (e->height_left >= GRID_Y) {
	    e->dxdy_full = floored_muldivrem (GRID_Y, dx, dy);
	} else {
	    e->dxdy_full.quo = 0;
	    e->dxdy_full.rem = 0;
	}
    }

    _polygon_insert_edge_into_its_y_bucket (polygon, e);

    e->x.rem -= dy;		/* Bias the remainder for faster
				 * edge advancement. */
}

static void
active_list_reset (struct active_list *active)
{
    active->head = NULL;
    active->min_height = 0;
}

static void
active_list_init(struct active_list *active)
{
    active_list_reset(active);
}

/*
 * Merge two sorted edge lists.
 * Input:
 *  - head_a: The head of the first list.
 *  - head_b: The head of the second list; head_b cannot be NULL.
 * Output:
 * Returns the head of the merged list.
 *
 * Implementation notes:
 * To make it fast (in particular, to reduce to an insertion sort whenever
 * one of the two input lists only has a single element) we iterate through
 * a list until its head becomes greater than the head of the other list,
 * then we switch their roles. As soon as one of the two lists is empty, we
 * just attach the other one to the current list and exit.
 * Writes to memory are only needed to "switch" lists (as it also requires
 * attaching to the output list the list which we will be iterating next) and
 * to attach the last non-empty list.
 */
static struct edge *
merge_sorted_edges (struct edge *head_a, struct edge *head_b)
{
    struct edge *head, **next;
    int32_t x;

    if (head_a == NULL)
	return head_b;

    next = &head;
    if (head_a->x.quo <= head_b->x.quo) {
	head = head_a;
    } else {
	head = head_b;
	goto start_with_b;
    }

    do {
	x = head_b->x.quo;
	while (head_a != NULL && head_a->x.quo <= x) {
	    next = &head_a->next;
	    head_a = head_a->next;
	}

	*next = head_b;
	if (head_a == NULL)
	    return head;

start_with_b:
	x = head_a->x.quo;
	while (head_b != NULL && head_b->x.quo <= x) {
	    next = &head_b->next;
	    head_b = head_b->next;
	}

	*next = head_a;
	if (head_b == NULL)
	    return head;
    } while (1);
}

/*
 * Sort (part of) a list.
 * Input:
 *  - list: The list to be sorted; list cannot be NULL.
 *  - limit: Recursion limit.
 * Output:
 *  - head_out: The head of the sorted list containing the first 2^(level+1) elements of the
 *              input list; if the input list has fewer elements, head_out be a sorted list
 *              containing all the elements of the input list.
 * Returns the head of the list of unprocessed elements (NULL if the sorted list contains
 * all the elements of the input list).
 *
 * Implementation notes:
 * Special case single element list, unroll/inline the sorting of the first two elements.
 * Some tail recursion is used since we iterate on the bottom-up solution of the problem
 * (we start with a small sorted list and keep merging other lists of the same size to it).
 */
static struct edge *
sort_edges (struct edge  *list,
	    unsigned int  level,
	    struct edge **head_out)
{
    struct edge *head_other, *remaining;
    unsigned int i;

    head_other = list->next;

    /* Single element list -> return */
    if (head_other == NULL) {
	*head_out = list;
	return NULL;
    }

    /* Unroll the first iteration of the following loop (halves the number of calls to merge_sorted_edges):
     *  - Initialize remaining to be the list containing the elements after the second in the input list.
     *  - Initialize *head_out to be the sorted list containing the first two element.
     */
    remaining = head_other->next;
    if (list->x.quo <= head_other->x.quo) {
	*head_out = list;
	/* list->next = head_other; */ /* The input list is already like this. */
	head_other->next = NULL;
    } else {
	*head_out = head_other;
	head_other->next = list;
	list->next = NULL;
    }

    for (i = 0; i < level && remaining; i++) {
	/* Extract a sorted list of the same size as *head_out
	 * (2^(i+1) elements) from the list of remaining elements. */
	remaining = sort_edges (remaining, i, &head_other);
	*head_out = merge_sorted_edges (*head_out, head_other);
    }

    /* *head_out now contains (at most) 2^(level+1) elements. */

    return remaining;
}

/* Test if the edges on the active list can be safely advanced by a
 * full row without intersections or any edges ending. */
inline static int
active_list_can_step_full_row (struct active_list *active)
{
    const struct edge *e;
    int prev_x = INT_MIN;

    /* Recomputes the minimum height of all edges on the active
     * list if we have been dropping edges. */
    if (active->min_height <= 0) {
	int min_height = INT_MAX;

	e = active->head;
	while (NULL != e) {
	    if (e->height_left < min_height)
		min_height = e->height_left;
	    e = e->next;
	}

	active->min_height = min_height;
    }

    if (active->min_height < GRID_Y)
	return 0;

    /* Check for intersections as no edges end during the next row. */
    e = active->head;
    while (NULL != e) {
	struct quorem x = e->x;

	if (! e->vertical) {
	    x.quo += e->dxdy_full.quo;
	    x.rem += e->dxdy_full.rem;
	    if (x.rem >= 0)
		++x.quo;
	}

	if (x.quo <= prev_x)
	    return 0;

	prev_x = x.quo;
	e = e->next;
    }

    return 1;
}

/* Merges edges on the given subpixel row from the polygon to the
 * active_list. */
inline static void
active_list_merge_edges_from_polygon(struct active_list *active,
				     struct edge **ptail,
				     grid_scaled_y_t y,
				     struct polygon *polygon)
{
    /* Split off the edges on the current subrow and merge them into
     * the active list. */
    int min_height = active->min_height;
    struct edge *subrow_edges = NULL;
    struct edge *tail = *ptail;

    do {
	struct edge *next = tail->next;

	if (y == tail->ytop) {
	    tail->next = subrow_edges;
	    subrow_edges = tail;

	    if (tail->height_left < min_height)
		min_height = tail->height_left;

	    *ptail = next;
	} else
	    ptail = &tail->next;

	tail = next;
    } while (tail);

    if (subrow_edges) {
	sort_edges (subrow_edges, UINT_MAX, &subrow_edges);
	active->head = merge_sorted_edges (active->head, subrow_edges);
	active->min_height = min_height;
    }
}

/* Advance the edges on the active list by one subsample row by
 * updating their x positions.  Drop edges from the list that end. */
inline static void
active_list_substep_edges(struct active_list *active)
{
    struct edge **cursor = &active->head;
    grid_scaled_x_t prev_x = INT_MIN;
    struct edge *unsorted = NULL;
    struct edge *edge = *cursor;

    do {
	UNROLL3({
	    struct edge *next;

	    if (NULL == edge)
		break;

	    next = edge->next;
	    if (--edge->height_left) {
		edge->x.quo += edge->dxdy.quo;
		edge->x.rem += edge->dxdy.rem;
		if (edge->x.rem >= 0) {
		    ++edge->x.quo;
		    edge->x.rem -= edge->dy;
		}

		if (edge->x.quo < prev_x) {
		    *cursor = next;
		    edge->next = unsorted;
		    unsorted = edge;
		} else {
		    prev_x = edge->x.quo;
		    cursor = &edge->next;
		}
	    } else {
		 *cursor = next;
	    }
	    edge = next;
	})
    } while (1);

    if (unsorted) {
	sort_edges (unsorted, UINT_MAX, &unsorted);
	active->head = merge_sorted_edges (active->head, unsorted);
    }
}

inline static void
apply_nonzero_fill_rule_for_subrow (struct active_list *active,
				    struct cell_list *coverages)
{
    struct edge *edge = active->head;
    int winding = 0;
    int xstart;
    int xend;

    cell_list_rewind (coverages);

    while (NULL != edge) {
	xstart = edge->x.quo;
	winding = edge->dir;
	while (1) {
	    edge = edge->next;
	    if (NULL == edge) {
		ASSERT_NOT_REACHED;
		return;
	    }

	    winding += edge->dir;
	    if (0 == winding) {
		if (edge->next == NULL || edge->next->x.quo != edge->x.quo)
		    break;
	    }
	}

	xend = edge->x.quo;
	cell_list_add_subspan (coverages, xstart, xend);

	edge = edge->next;
    }
}

static void
apply_evenodd_fill_rule_for_subrow (struct active_list *active,
				    struct cell_list *coverages)
{
    struct edge *edge = active->head;
    int xstart;
    int xend;

    cell_list_rewind (coverages);

    while (NULL != edge) {
	xstart = edge->x.quo;

	while (1) {
	    edge = edge->next;
	    if (NULL == edge) {
		ASSERT_NOT_REACHED;
		return;
	    }

	    if (edge->next == NULL || edge->next->x.quo != edge->x.quo)
		break;

	    edge = edge->next;
	}

	xend = edge->x.quo;
	cell_list_add_subspan (coverages, xstart, xend);

	edge = edge->next;
    }
}

static void
apply_nonzero_fill_rule_and_step_edges (struct active_list *active,
					struct cell_list *coverages)
{
    struct edge **cursor = &active->head;
    struct edge *left_edge;

    left_edge = *cursor;
    while (NULL != left_edge) {
	struct edge *right_edge;
	int winding = left_edge->dir;

	left_edge->height_left -= GRID_Y;
	if (left_edge->height_left)
	    cursor = &left_edge->next;
	else
	    *cursor = left_edge->next;

	while (1) {
	    right_edge = *cursor;
	    if (NULL == right_edge) {
		cell_list_render_edge (coverages, left_edge, +1);
		return;
	    }

	    right_edge->height_left -= GRID_Y;
	    if (right_edge->height_left)
		cursor = &right_edge->next;
	    else
		*cursor = right_edge->next;

	    winding += right_edge->dir;
	    if (0 == winding) {
		if (right_edge->next == NULL ||
		    right_edge->next->x.quo != right_edge->x.quo)
		{
		    break;
		}
	    }

	    if (! right_edge->vertical) {
		right_edge->x.quo += right_edge->dxdy_full.quo;
		right_edge->x.rem += right_edge->dxdy_full.rem;
		if (right_edge->x.rem >= 0) {
		    ++right_edge->x.quo;
		    right_edge->x.rem -= right_edge->dy;
		}
	    }
	}

	cell_list_render_edge (coverages, left_edge, +1);
	cell_list_render_edge (coverages, right_edge, -1);

	left_edge = *cursor;
    }
}

static void
apply_evenodd_fill_rule_and_step_edges (struct active_list *active,
					struct cell_list *coverages)
{
    struct edge **cursor = &active->head;
    struct edge *left_edge;

    left_edge = *cursor;
    while (NULL != left_edge) {
	struct edge *right_edge;

	left_edge->height_left -= GRID_Y;
	if (left_edge->height_left)
	    cursor = &left_edge->next;
	else
	    *cursor = left_edge->next;

	while (1) {
	    right_edge = *cursor;
	    if (NULL == right_edge) {
		cell_list_render_edge (coverages, left_edge, +1);
		return;
	    }

	    right_edge->height_left -= GRID_Y;
	    if (right_edge->height_left)
		cursor = &right_edge->next;
	    else
		*cursor = right_edge->next;

	    if (right_edge->next == NULL ||
		right_edge->next->x.quo != right_edge->x.quo)
	    {
		break;
	    }

	    if (! right_edge->vertical) {
		right_edge->x.quo += right_edge->dxdy_full.quo;
		right_edge->x.rem += right_edge->dxdy_full.rem;
		if (right_edge->x.rem >= 0) {
		    ++right_edge->x.quo;
		    right_edge->x.rem -= right_edge->dy;
		}
	    }
	}

	cell_list_render_edge (coverages, left_edge, +1);
	cell_list_render_edge (coverages, right_edge, -1);

	left_edge = *cursor;
    }
}

static void
_glitter_scan_converter_init(glitter_scan_converter_t *converter, jmp_buf *jmp)
{
    polygon_init(converter->polygon, jmp);
    active_list_init(converter->active);
    cell_list_init(converter->coverages, jmp);
    converter->ymin=0;
    converter->ymax=0;
}

static void
_glitter_scan_converter_fini(glitter_scan_converter_t *converter)
{
    polygon_fini(converter->polygon);
    cell_list_fini(converter->coverages);
    converter->ymin=0;
    converter->ymax=0;
}

static grid_scaled_t
int_to_grid_scaled(int i, int scale)
{
    /* Clamp to max/min representable scaled number. */
    if (i >= 0) {
	if (i >= INT_MAX/scale)
	    i = INT_MAX/scale;
    }
    else {
	if (i <= INT_MIN/scale)
	    i = INT_MIN/scale;
    }
    return i*scale;
}

#define int_to_grid_scaled_x(x) int_to_grid_scaled((x), GRID_X)
#define int_to_grid_scaled_y(x) int_to_grid_scaled((x), GRID_Y)

static cairo_status_t
glitter_scan_converter_reset(glitter_scan_converter_t *converter,
			     int ymin, int ymax)
{
    cairo_status_t status;

    converter->ymin = 0;
    converter->ymax = 0;

    ymin = int_to_grid_scaled_y(ymin);
    ymax = int_to_grid_scaled_y(ymax);

    active_list_reset(converter->active);
    cell_list_reset(converter->coverages);
    status = polygon_reset(converter->polygon, ymin, ymax);
    if (status)
	return status;

    converter->ymin = ymin;
    converter->ymax = ymax;
    return CAIRO_STATUS_SUCCESS;
}

/* INPUT_TO_GRID_X/Y (in_coord, out_grid_scaled, grid_scale)
 *   These macros convert an input coordinate in the client's
 *   device space to the rasterisation grid.
 */
/* Gah.. this bit of ugly defines INPUT_TO_GRID_X/Y so as to use
 * shifts if possible, and something saneish if not.
 */
#if !defined(INPUT_TO_GRID_Y) && defined(GRID_Y_BITS) && GRID_Y_BITS <= GLITTER_INPUT_BITS
#  define INPUT_TO_GRID_Y(in, out) (out) = (in) >> (GLITTER_INPUT_BITS - GRID_Y_BITS)
#else
#  define INPUT_TO_GRID_Y(in, out) INPUT_TO_GRID_general(in, out, GRID_Y)
#endif

#if !defined(INPUT_TO_GRID_X) && defined(GRID_X_BITS) && GRID_X_BITS <= GLITTER_INPUT_BITS
#  define INPUT_TO_GRID_X(in, out) (out) = (in) >> (GLITTER_INPUT_BITS - GRID_X_BITS)
#else
#  define INPUT_TO_GRID_X(in, out) INPUT_TO_GRID_general(in, out, GRID_X)
#endif

#define INPUT_TO_GRID_general(in, out, grid_scale) do {		\
	long long tmp__ = (long long)(grid_scale) * (in);	\
	tmp__ >>= GLITTER_INPUT_BITS;				\
	(out) = tmp__;						\
} while (0)

static void
glitter_scan_converter_add_edge (glitter_scan_converter_t *converter,
				 const cairo_edge_t *edge,
				 int clip)
{
    cairo_edge_t e;

    INPUT_TO_GRID_Y (edge->top, e.top);
    INPUT_TO_GRID_Y (edge->bottom, e.bottom);
    if (e.top >= e.bottom)
	return;

    /* XXX: possible overflows if GRID_X/Y > 2**GLITTER_INPUT_BITS */
    INPUT_TO_GRID_Y (edge->line.p1.y, e.line.p1.y);
    INPUT_TO_GRID_Y (edge->line.p2.y, e.line.p2.y);
    if (e.line.p1.y == e.line.p2.y)
	return;

    INPUT_TO_GRID_X (edge->line.p1.x, e.line.p1.x);
    INPUT_TO_GRID_X (edge->line.p2.x, e.line.p2.x);

    e.dir = edge->dir;

    polygon_add_edge (converter->polygon, &e, clip);
}

static cairo_bool_t
active_list_is_vertical (struct active_list *active)
{
    struct edge *e;

    for (e = active->head; e != NULL; e = e->next) {
	if (! e->vertical)
	    return FALSE;
    }

    return TRUE;
}

static void
step_edges (struct active_list *active, int count)
{
    struct edge **cursor = &active->head;
    struct edge *edge;

    for (edge = *cursor; edge != NULL; edge = *cursor) {
	edge->height_left -= GRID_Y * count;
	if (edge->height_left)
	    cursor = &edge->next;
	else
	    *cursor = edge->next;
    }
}

static cairo_status_t
blit_coverages (struct cell_list *cells,
		cairo_span_renderer_t *renderer,
		struct pool *span_pool,
		int y, int height)
{
    struct cell *cell = cells->head.next;
    int prev_x = -1;
    int cover = 0, last_cover = 0;
    int clip = 0;
    cairo_half_open_span_t *spans;
    unsigned num_spans;

    assert (cell != &cells->tail);

    /* Count number of cells remaining. */
    {
	struct cell *next = cell;
	num_spans = 2;
	while (next->next) {
	    next = next->next;
	    ++num_spans;
	}
	num_spans = 2*num_spans;
    }

    /* Allocate enough spans for the row. */
    pool_reset (span_pool);
    spans = pool_alloc (span_pool, sizeof(spans[0])*num_spans);
    num_spans = 0;

    /* Form the spans from the coverages and areas. */
    for (; cell->next; cell = cell->next) {
	int x = cell->x;
	int area;

	if (x > prev_x && cover != last_cover) {
	    spans[num_spans].x = prev_x;
	    spans[num_spans].coverage = GRID_AREA_TO_ALPHA (cover);
	    spans[num_spans].inverse = 0;
	    last_cover = cover;
	    ++num_spans;
	}

	cover += cell->covered_height*GRID_X*2;
	clip += cell->covered_height*GRID_X*2;
	area = cover - cell->uncovered_area;

	if (area != last_cover) {
	    spans[num_spans].x = x;
	    spans[num_spans].coverage = GRID_AREA_TO_ALPHA (area);
	    spans[num_spans].inverse = 0;
	    last_cover = area;
	    ++num_spans;
	}

	prev_x = x+1;
    }

    /* Dump them into the renderer. */
    return renderer->render_rows (renderer, y, height, spans, num_spans);
}

static void
glitter_scan_converter_render(glitter_scan_converter_t *converter,
			      int nonzero_fill,
			      cairo_span_renderer_t *span_renderer,
			      struct pool *span_pool)
{
    int i, j;
    int ymax_i = converter->ymax / GRID_Y;
    int ymin_i = converter->ymin / GRID_Y;
    int h = ymax_i - ymin_i;
    struct polygon *polygon = converter->polygon;
    struct cell_list *coverages = converter->coverages;
    struct active_list *active = converter->active;

    /* Render each pixel row. */
    for (i = 0; i < h; i = j) {
	int do_full_step = 0;

	j = i + 1;

	/* Determine if we can ignore this row or use the full pixel
	 * stepper. */
	if (GRID_Y == EDGE_Y_BUCKET_HEIGHT && ! polygon->y_buckets[i]) {
	    if (! active->head) {
		for (; j < h && ! polygon->y_buckets[j]; j++)
		    ;
		continue;
	    }

	    do_full_step = active_list_can_step_full_row (active);
	}

	if (do_full_step) {
	    /* Step by a full pixel row's worth. */
	    if (nonzero_fill)
		apply_nonzero_fill_rule_and_step_edges (active, coverages);
	    else
		apply_evenodd_fill_rule_and_step_edges (active, coverages);

	    if (active_list_is_vertical (active)) {
		while (j < h &&
		       polygon->y_buckets[j] == NULL &&
		       active->min_height >= 2*GRID_Y)
		{
		    active->min_height -= GRID_Y;
		    j++;
		}
		if (j != i + 1)
		    step_edges (active, j - (i + 1));
	    }
	} else {
	    grid_scaled_y_t suby;

	    /* Subsample this row. */
	    for (suby = 0; suby < GRID_Y; suby++) {
		grid_scaled_y_t y = (i+ymin_i)*GRID_Y + suby;

		if (polygon->y_buckets[i]) {
		    active_list_merge_edges_from_polygon (active,
							  &polygon->y_buckets[i], y,
							  polygon);
		}

		if (nonzero_fill)
		    apply_nonzero_fill_rule_for_subrow (active, coverages);
		else
		    apply_evenodd_fill_rule_for_subrow (active, coverages);

		active_list_substep_edges(active);
	    }
	}

	blit_coverages (coverages, span_renderer, span_pool, i+ymin_i, j -i);
	cell_list_reset (coverages);

	if (! active->head)
	    active->min_height = INT_MAX;
	else
	    active->min_height -= GRID_Y;
    }
}

struct _cairo_clip_tor_scan_converter {
    cairo_scan_converter_t base;

    glitter_scan_converter_t converter[1];
    cairo_fill_rule_t fill_rule;
    cairo_antialias_t antialias;

    cairo_fill_rule_t clip_fill_rule;
    cairo_antialias_t clip_antialias;

    jmp_buf jmp;

    struct {
	struct pool base[1];
	cairo_half_open_span_t embedded[32];
    } span_pool;
};

typedef struct _cairo_clip_tor_scan_converter cairo_clip_tor_scan_converter_t;

static void
_cairo_clip_tor_scan_converter_destroy (void *converter)
{
    cairo_clip_tor_scan_converter_t *self = converter;
    if (self == NULL) {
	return;
    }
    _glitter_scan_converter_fini (self->converter);
    pool_fini (self->span_pool.base);
    free(self);
}

static cairo_status_t
_cairo_clip_tor_scan_converter_generate (void			*converter,
				    cairo_span_renderer_t	*renderer)
{
    cairo_clip_tor_scan_converter_t *self = converter;
    cairo_status_t status;

    if ((status = setjmp (self->jmp)))
	return _cairo_scan_converter_set_error (self, _cairo_error (status));

    glitter_scan_converter_render (self->converter,
				   self->fill_rule == CAIRO_FILL_RULE_WINDING,
				   renderer,
				   self->span_pool.base);
    return CAIRO_STATUS_SUCCESS;
}

cairo_scan_converter_t *
_cairo_clip_tor_scan_converter_create (cairo_clip_t *clip,
				       cairo_polygon_t *polygon,
				       cairo_fill_rule_t fill_rule,
				       cairo_antialias_t antialias)
{
    cairo_clip_tor_scan_converter_t *self;
    cairo_polygon_t clipper;
    cairo_status_t status;
    int i;

    self = calloc (1, sizeof(struct _cairo_clip_tor_scan_converter));
    if (unlikely (self == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto bail_nomem;
    }

    self->base.destroy = _cairo_clip_tor_scan_converter_destroy;
    self->base.generate = _cairo_clip_tor_scan_converter_generate;

    pool_init (self->span_pool.base, &self->jmp,
	       250 * sizeof(self->span_pool.embedded[0]),
	       sizeof(self->span_pool.embedded));

    _glitter_scan_converter_init (self->converter, &self->jmp);
    status = glitter_scan_converter_reset (self->converter,
					   clip->extents.y,
					   clip->extents.y + clip->extents.height);
    if (unlikely (status))
	goto bail;

    self->fill_rule = fill_rule;
    self->antialias = antialias;

    for (i = 0; i < polygon->num_edges; i++)
	 glitter_scan_converter_add_edge (self->converter,
					  &polygon->edges[i],
					  FALSE);

    status = _cairo_clip_get_polygon (clip,
				      &clipper,
				      &self->clip_fill_rule,
				      &self->clip_antialias);
    if (unlikely (status))
	goto bail;

    for (i = 0; i < clipper.num_edges; i++)
	 glitter_scan_converter_add_edge (self->converter,
					  &clipper.edges[i],
					  TRUE);
    _cairo_polygon_fini (&clipper);

    return &self->base;

 bail:
    self->base.destroy(&self->base);
 bail_nomem:
    return _cairo_scan_converter_create_in_error (status);
}

