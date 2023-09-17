/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2005 Red Hat, Inc.
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

#ifndef CAIRO_TYPES_PRIVATE_H
#define CAIRO_TYPES_PRIVATE_H

#include "cairo.h"
#include "cairo-fixed-type-private.h"
#include "cairo-list-private.h"
#include "cairo-reference-count-private.h"

CAIRO_BEGIN_DECLS

/**
 * SECTION:cairo-types
 * @Title: Types
 * @Short_Description: Generic data types
 *
 * This section lists generic data types used in the cairo API.
 **/

typedef struct _cairo_array cairo_array_t;
typedef struct _cairo_backend cairo_backend_t;
typedef struct _cairo_boxes_t cairo_boxes_t;
typedef struct _cairo_cache cairo_cache_t;
typedef struct _cairo_composite_rectangles cairo_composite_rectangles_t;
typedef struct _cairo_clip cairo_clip_t;
typedef struct _cairo_clip_path cairo_clip_path_t;
typedef struct _cairo_color cairo_color_t;
typedef struct _cairo_color_stop cairo_color_stop_t;
typedef struct _cairo_contour cairo_contour_t;
typedef struct _cairo_contour_chain cairo_contour_chain_t;
typedef struct _cairo_contour_iter cairo_contour_iter_t;
typedef struct _cairo_damage cairo_damage_t;
typedef struct _cairo_device_backend cairo_device_backend_t;
typedef struct _cairo_font_face_backend     cairo_font_face_backend_t;
typedef struct _cairo_gstate cairo_gstate_t;
typedef struct _cairo_gstate_backend cairo_gstate_backend_t;
typedef struct _cairo_glyph_text_info cairo_glyph_text_info_t;
typedef struct _cairo_hash_entry cairo_hash_entry_t;
typedef struct _cairo_hash_table cairo_hash_table_t;
typedef struct _cairo_image_surface cairo_image_surface_t;
typedef struct _cairo_mime_data cairo_mime_data_t;
typedef struct _cairo_observer cairo_observer_t;
typedef struct _cairo_output_stream cairo_output_stream_t;
typedef struct _cairo_paginated_surface_backend cairo_paginated_surface_backend_t;
typedef struct _cairo_path_fixed cairo_path_fixed_t;
typedef struct _cairo_rectangle_int16 cairo_glyph_size_t;
typedef struct _cairo_scaled_font_subsets cairo_scaled_font_subsets_t;
typedef struct _cairo_solid_pattern cairo_solid_pattern_t;
typedef struct _cairo_surface_attributes cairo_surface_attributes_t;
typedef struct _cairo_surface_backend cairo_surface_backend_t;
typedef struct _cairo_surface_observer cairo_surface_observer_t;
typedef struct _cairo_surface_snapshot cairo_surface_snapshot_t;
typedef struct _cairo_surface_subsurface cairo_surface_subsurface_t;
typedef struct _cairo_surface_wrapper cairo_surface_wrapper_t;
typedef struct _cairo_traps cairo_traps_t;
typedef struct _cairo_tristrip cairo_tristrip_t;
typedef struct _cairo_unscaled_font_backend cairo_unscaled_font_backend_t;
typedef struct _cairo_xlib_screen_info cairo_xlib_screen_info_t;

typedef cairo_array_t cairo_user_data_array_t;

typedef struct _cairo_scaled_font_private cairo_scaled_font_private_t;
typedef struct _cairo_scaled_font_backend   cairo_scaled_font_backend_t;
typedef struct _cairo_scaled_glyph cairo_scaled_glyph_t;
typedef struct _cairo_scaled_glyph_private cairo_scaled_glyph_private_t;

typedef struct cairo_compositor cairo_compositor_t;
typedef struct cairo_fallback_compositor cairo_fallback_compositor_t;
typedef struct cairo_mask_compositor cairo_mask_compositor_t;
typedef struct cairo_traps_compositor cairo_traps_compositor_t;
typedef struct cairo_spans_compositor cairo_spans_compositor_t;

struct _cairo_observer {
    cairo_list_t link;
    void (*callback) (cairo_observer_t *self, void *arg);
};

/**
 * _cairo_hash_entry:
 *
 * A #cairo_hash_entry_t contains both a key and a value for
 * #cairo_hash_table_t. User-derived types for #cairo_hash_entry_t must
 * be type-compatible with this structure (eg. they must have a
 * uintptr_t as the first parameter. The easiest way to get this
 * is to use:
 *
 * 	typedef _my_entry {
 *	    cairo_hash_entry_t base;
 *	    ... Remainder of key and value fields here ..
 *	} my_entry_t;
 *
 * which then allows a pointer to my_entry_t to be passed to any of
 * the #cairo_hash_table_t functions as follows without requiring a cast:
 *
 *	_cairo_hash_table_insert (hash_table, &my_entry->base);
 *
 * IMPORTANT: The caller is responsible for initializing
 * my_entry->base.hash with a hash code derived from the key. The
 * essential property of the hash code is that keys_equal must never
 * return %TRUE for two keys that have different hashes. The best hash
 * code will reduce the frequency of two keys with the same code for
 * which keys_equal returns %FALSE.
 *
 * Which parts of the entry make up the "key" and which part make up
 * the value are entirely up to the caller, (as determined by the
 * computation going into base.hash as well as the keys_equal
 * function). A few of the #cairo_hash_table_t functions accept an entry
 * which will be used exclusively as a "key", (indicated by a
 * parameter name of key). In these cases, the value-related fields of
 * the entry need not be initialized if so desired.
 **/
struct _cairo_hash_entry {
    uintptr_t hash;
};

struct _cairo_array {
    unsigned int size;
    unsigned int num_elements;
    unsigned int element_size;
    char *elements;
};

/**
 * _cairo_lcd_filter:
 * @CAIRO_LCD_FILTER_DEFAULT: Use the default LCD filter for
 *   font backend and target device
 * @CAIRO_LCD_FILTER_NONE: Do not perform LCD filtering
 * @CAIRO_LCD_FILTER_INTRA_PIXEL: Intra-pixel filter
 * @CAIRO_LCD_FILTER_FIR3: FIR filter with a 3x3 kernel
 * @CAIRO_LCD_FILTER_FIR5: FIR filter with a 5x5 kernel
 *
 * The LCD filter specifies the low-pass filter applied to LCD-optimized
 * bitmaps generated with an antialiasing mode of %CAIRO_ANTIALIAS_SUBPIXEL.
 *
 * Note: This API was temporarily made available in the public
 * interface during the 1.7.x development series, but was made private
 * before 1.8.
 **/
typedef enum _cairo_lcd_filter {
    CAIRO_LCD_FILTER_DEFAULT,
    CAIRO_LCD_FILTER_NONE,
    CAIRO_LCD_FILTER_INTRA_PIXEL,
    CAIRO_LCD_FILTER_FIR3,
    CAIRO_LCD_FILTER_FIR5
} cairo_lcd_filter_t;

typedef enum _cairo_round_glyph_positions {
    CAIRO_ROUND_GLYPH_POS_DEFAULT,
    CAIRO_ROUND_GLYPH_POS_ON,
    CAIRO_ROUND_GLYPH_POS_OFF
} cairo_round_glyph_positions_t;

typedef struct {
    unsigned int index;
    double red, green, blue, alpha;
} cairo_palette_color_t;

struct _cairo_font_options {
    cairo_antialias_t antialias;
    cairo_subpixel_order_t subpixel_order;
    cairo_lcd_filter_t lcd_filter;
    cairo_hint_style_t hint_style;
    cairo_hint_metrics_t hint_metrics;
    cairo_round_glyph_positions_t round_glyph_positions;
    char *variations;
    cairo_color_mode_t color_mode;
    unsigned int palette_index;
    cairo_palette_color_t *custom_palette;
    unsigned int custom_palette_size;
};

struct _cairo_glyph_text_info {
    const char *utf8;
    int utf8_len;

    const cairo_text_cluster_t *clusters;
    int num_clusters;
    cairo_text_cluster_flags_t cluster_flags;
};


/* XXX: Right now, the _cairo_color structure puts unpremultiplied
   color in the doubles and premultiplied color in the shorts. Yes,
   this is crazy insane, (but at least we don't export this
   madness). I'm still working on a cleaner API, but in the meantime,
   at least this does prevent precision loss in color when changing
   alpha. */
struct _cairo_color {
    double red;
    double green;
    double blue;
    double alpha;

    unsigned short red_short;
    unsigned short green_short;
    unsigned short blue_short;
    unsigned short alpha_short;
};

struct _cairo_color_stop {
    /* unpremultiplied */
    double red;
    double green;
    double blue;
    double alpha;

    /* unpremultipled, for convenience */
    uint16_t red_short;
    uint16_t green_short;
    uint16_t blue_short;
    uint16_t alpha_short;
};

typedef enum _cairo_paginated_mode {
    CAIRO_PAGINATED_MODE_ANALYZE,	/* analyze page regions */
    CAIRO_PAGINATED_MODE_RENDER,	/* render page contents */
    CAIRO_PAGINATED_MODE_FALLBACK	/* paint fallback images */
} cairo_paginated_mode_t;

typedef enum _cairo_internal_surface_type {
    CAIRO_INTERNAL_SURFACE_TYPE_SNAPSHOT = 0x1000,
    CAIRO_INTERNAL_SURFACE_TYPE_PAGINATED,
    CAIRO_INTERNAL_SURFACE_TYPE_ANALYSIS,
    CAIRO_INTERNAL_SURFACE_TYPE_OBSERVER,
    CAIRO_INTERNAL_SURFACE_TYPE_TEST_FALLBACK,
    CAIRO_INTERNAL_SURFACE_TYPE_TEST_PAGINATED,
    CAIRO_INTERNAL_SURFACE_TYPE_TEST_WRAPPING,
    CAIRO_INTERNAL_SURFACE_TYPE_NULL,
    CAIRO_INTERNAL_SURFACE_TYPE_TYPE3_GLYPH,
    CAIRO_INTERNAL_SURFACE_TYPE_QUARTZ_SNAPSHOT
} cairo_internal_surface_type_t;

typedef enum _cairo_internal_device_type {
    CAIRO_INTERNAL_DEVICE_TYPE_OBSERVER = 0x1000,
} cairo_device_surface_type_t;

#define CAIRO_HAS_TEST_PAGINATED_SURFACE 1

typedef struct _cairo_slope {
    cairo_fixed_t dx;
    cairo_fixed_t dy;
} cairo_slope_t, cairo_distance_t;

typedef struct _cairo_point_double {
    double x;
    double y;
} cairo_point_double_t;

typedef struct _cairo_circle_double {
    cairo_point_double_t center;
    double               radius;
} cairo_circle_double_t;

typedef struct _cairo_distance_double {
    double dx;
    double dy;
} cairo_distance_double_t;

typedef struct _cairo_box_double {
    cairo_point_double_t p1;
    cairo_point_double_t p2;
} cairo_box_double_t;

typedef struct _cairo_line {
    cairo_point_t p1;
    cairo_point_t p2;
} cairo_line_t, cairo_box_t;

typedef struct _cairo_trapezoid {
    cairo_fixed_t top, bottom;
    cairo_line_t left, right;
} cairo_trapezoid_t;

typedef struct _cairo_point_int {
    int x, y;
} cairo_point_int_t;

#define CAIRO_RECT_INT_MIN (INT_MIN >> CAIRO_FIXED_FRAC_BITS)
#define CAIRO_RECT_INT_MAX (INT_MAX >> CAIRO_FIXED_FRAC_BITS)

typedef enum _cairo_direction {
    CAIRO_DIRECTION_FORWARD,
    CAIRO_DIRECTION_REVERSE
} cairo_direction_t;

typedef struct _cairo_edge {
    cairo_line_t line;
    int top, bottom;
    int dir;
} cairo_edge_t;

typedef struct _cairo_polygon {
    cairo_status_t status;

    cairo_box_t extents;
    cairo_box_t limit;
    const cairo_box_t *limits;
    int num_limits;

    int num_edges;
    int edges_size;
    cairo_edge_t *edges;
    cairo_edge_t  edges_embedded[32];
} cairo_polygon_t;

typedef cairo_warn cairo_status_t
(*cairo_spline_add_point_func_t) (void *closure,
				  const cairo_point_t *point,
				  const cairo_slope_t *tangent);

typedef struct _cairo_spline_knots {
    cairo_point_t a, b, c, d;
} cairo_spline_knots_t;

typedef struct _cairo_spline {
    cairo_spline_add_point_func_t add_point_func;
    void *closure;

    cairo_spline_knots_t knots;

    cairo_slope_t initial_slope;
    cairo_slope_t final_slope;

    cairo_bool_t has_point;
    cairo_point_t last_point;
} cairo_spline_t;

typedef struct _cairo_pen_vertex {
    cairo_point_t point;

    cairo_slope_t slope_ccw;
    cairo_slope_t slope_cw;
} cairo_pen_vertex_t;

typedef struct _cairo_pen {
    double radius;
    double tolerance;

    int num_vertices;
    cairo_pen_vertex_t *vertices;
    cairo_pen_vertex_t  vertices_embedded[32];
} cairo_pen_t;

typedef struct _cairo_stroke_style {
    double		 line_width;
    cairo_line_cap_t	 line_cap;
    cairo_line_join_t	 line_join;
    double		 miter_limit;
    double		*dash;
    unsigned int	 num_dashes;
    double		 dash_offset;
    cairo_bool_t	 is_hairline;
    double      pre_hairline_line_width;
} cairo_stroke_style_t;

typedef struct _cairo_format_masks {
    int bpp;
    unsigned long alpha_mask;
    unsigned long red_mask;
    unsigned long green_mask;
    unsigned long blue_mask;
} cairo_format_masks_t;

typedef enum {
    CAIRO_STOCK_WHITE,
    CAIRO_STOCK_BLACK,
    CAIRO_STOCK_TRANSPARENT,
    CAIRO_STOCK_NUM_COLORS,
} cairo_stock_t;

typedef enum _cairo_image_transparency {
    CAIRO_IMAGE_IS_OPAQUE,
    CAIRO_IMAGE_HAS_BILEVEL_ALPHA,
    CAIRO_IMAGE_HAS_ALPHA,
    CAIRO_IMAGE_UNKNOWN
} cairo_image_transparency_t;

typedef enum _cairo_image_color {
    CAIRO_IMAGE_IS_COLOR,
    CAIRO_IMAGE_IS_GRAYSCALE,
    CAIRO_IMAGE_IS_MONOCHROME,
    CAIRO_IMAGE_UNKNOWN_COLOR
} cairo_image_color_t;


struct _cairo_mime_data {
    cairo_reference_count_t ref_count;
    unsigned char *data;
    unsigned long length;
    cairo_destroy_func_t destroy;
    void *closure;
};

/*
 * A #cairo_unscaled_font_t is just an opaque handle we use in the
 * glyph cache.
 */
typedef struct _cairo_unscaled_font {
    cairo_hash_entry_t			 hash_entry;
    cairo_reference_count_t		 ref_count;
    const cairo_unscaled_font_backend_t	*backend;
} cairo_unscaled_font_t;
CAIRO_END_DECLS

#endif /* CAIRO_TYPES_PRIVATE_H */
