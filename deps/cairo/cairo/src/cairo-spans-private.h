/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright (c) 2008  M Joonas Pihlaja
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
#ifndef CAIRO_SPANS_PRIVATE_H
#define CAIRO_SPANS_PRIVATE_H
#include "cairo-types-private.h"
#include "cairo-compiler-private.h"

/* Number of bits of precision used for alpha. */
#define CAIRO_SPANS_UNIT_COVERAGE_BITS 8
#define CAIRO_SPANS_UNIT_COVERAGE ((1 << CAIRO_SPANS_UNIT_COVERAGE_BITS)-1)

/* A structure representing an open-ended horizontal span of constant
 * pixel coverage. */
typedef struct _cairo_half_open_span {
    int32_t x; /* The inclusive x-coordinate of the start of the span. */
    uint8_t coverage; /* The pixel coverage for the pixels to the right. */
    uint8_t inverse; /* between regular mask and clip */
} cairo_half_open_span_t;

/* Span renderer interface. Instances of renderers are provided by
 * surfaces if they want to composite spans instead of trapezoids. */
typedef struct _cairo_span_renderer cairo_span_renderer_t;
struct _cairo_span_renderer {
    /* Private status variable. */
    cairo_status_t status;

    /* Called to destroy the renderer. */
    cairo_destroy_func_t	destroy;

    /* Render the spans on row y of the destination by whatever compositing
     * method is required. */
    cairo_status_t
    (*render_rows) (void *abstract_renderer,
		    int y, int height,
		    const cairo_half_open_span_t	*coverages,
		    unsigned num_coverages);

    /* Called after all rows have been rendered to perform whatever
     * final rendering step is required.  This function is called just
     * once before the renderer is destroyed. */
    cairo_status_t (*finish) (void *abstract_renderer);
};

/* Scan converter interface. */
typedef struct _cairo_scan_converter cairo_scan_converter_t;
struct _cairo_scan_converter {
    /* Destroy this scan converter. */
    cairo_destroy_func_t	destroy;

    /* Generates coverage spans for rows for the added edges and calls
     * the renderer function for each row. After generating spans the
     * only valid thing to do with the converter is to destroy it. */
    cairo_status_t (*generate) (void			*abstract_converter,
				cairo_span_renderer_t	*renderer);

    /* Private status. Read with _cairo_scan_converter_status(). */
    cairo_status_t status;
};

/* Scan converter constructors. */

cairo_private cairo_scan_converter_t *
_cairo_tor_scan_converter_create (int			xmin,
				  int			ymin,
				  int			xmax,
				  int			ymax,
				  cairo_fill_rule_t	fill_rule,
				  cairo_antialias_t	antialias);
cairo_private cairo_status_t
_cairo_tor_scan_converter_add_polygon (void		*converter,
				       const cairo_polygon_t *polygon);

cairo_private cairo_scan_converter_t *
_cairo_tor22_scan_converter_create (int			xmin,
				    int			ymin,
				    int			xmax,
				    int			ymax,
				    cairo_fill_rule_t	fill_rule,
				    cairo_antialias_t	antialias);
cairo_private cairo_status_t
_cairo_tor22_scan_converter_add_polygon (void		*converter,
					 const cairo_polygon_t *polygon);

cairo_private cairo_scan_converter_t *
_cairo_mono_scan_converter_create (int			xmin,
				   int			ymin,
				   int			xmax,
				   int			ymax,
				   cairo_fill_rule_t	fill_rule);
cairo_private cairo_status_t
_cairo_mono_scan_converter_add_polygon (void		*converter,
					const cairo_polygon_t *polygon);

cairo_private cairo_scan_converter_t *
_cairo_clip_tor_scan_converter_create (cairo_clip_t *clip,
				       cairo_polygon_t *polygon,
				       cairo_fill_rule_t fill_rule,
				       cairo_antialias_t antialias);

typedef struct _cairo_rectangular_scan_converter {
    cairo_scan_converter_t base;

    cairo_box_t extents;

    struct _cairo_rectangular_scan_converter_chunk {
	struct _cairo_rectangular_scan_converter_chunk *next;
	void *base;
	int count;
	int size;
    } chunks, *tail;
    char buf[CAIRO_STACK_BUFFER_SIZE];
    int num_rectangles;
} cairo_rectangular_scan_converter_t;

cairo_private void
_cairo_rectangular_scan_converter_init (cairo_rectangular_scan_converter_t *self,
					const cairo_rectangle_int_t *extents);

cairo_private cairo_status_t
_cairo_rectangular_scan_converter_add_box (cairo_rectangular_scan_converter_t *self,
					   const cairo_box_t *box,
					   int dir);

typedef struct _cairo_botor_scan_converter {
    cairo_scan_converter_t base;

    cairo_box_t extents;
    cairo_fill_rule_t fill_rule;

    int xmin, xmax;

    struct _cairo_botor_scan_converter_chunk {
	struct _cairo_botor_scan_converter_chunk *next;
	void *base;
	int count;
	int size;
    } chunks, *tail;
    char buf[CAIRO_STACK_BUFFER_SIZE];
    int num_edges;
} cairo_botor_scan_converter_t;

cairo_private void
_cairo_botor_scan_converter_init (cairo_botor_scan_converter_t *self,
				  const cairo_box_t *extents,
				  cairo_fill_rule_t fill_rule);

cairo_private cairo_status_t
_cairo_botor_scan_converter_add_polygon (cairo_botor_scan_converter_t *converter,
					const cairo_polygon_t *polygon);

/* cairo-spans.c: */

cairo_private cairo_scan_converter_t *
_cairo_scan_converter_create_in_error (cairo_status_t error);

cairo_private cairo_status_t
_cairo_scan_converter_status (void *abstract_converter);

cairo_private cairo_status_t
_cairo_scan_converter_set_error (void *abstract_converter,
				 cairo_status_t error);

cairo_private cairo_span_renderer_t *
_cairo_span_renderer_create_in_error (cairo_status_t error);

cairo_private cairo_status_t
_cairo_span_renderer_status (void *abstract_renderer);

/* Set the renderer into an error state.  This sets all the method
 * pointers except ->destroy() of the renderer to no-op
 * implementations that just return the error status. */
cairo_private cairo_status_t
_cairo_span_renderer_set_error (void *abstract_renderer,
				cairo_status_t error);

cairo_private cairo_status_t
_cairo_surface_composite_polygon (cairo_surface_t	*surface,
				  cairo_operator_t	 op,
				  const cairo_pattern_t	*pattern,
				  cairo_fill_rule_t	fill_rule,
				  cairo_antialias_t	antialias,
				  const cairo_composite_rectangles_t *rects,
				  cairo_polygon_t	*polygon,
				  cairo_region_t	*clip_region);

#endif /* CAIRO_SPANS_PRIVATE_H */
