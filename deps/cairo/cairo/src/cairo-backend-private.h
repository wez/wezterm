/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2010 Intel Corporation
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
 * The Initial Developer of the Original Code is Intel Corporation
 *
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_BACKEND_PRIVATE_H
#define CAIRO_BACKEND_PRIVATE_H

#include "cairo-types-private.h"
#include "cairo-private.h"

typedef enum _cairo_backend_type {
    CAIRO_TYPE_DEFAULT,
    CAIRO_TYPE_SKIA,
} cairo_backend_type_t;

struct _cairo_backend {
    cairo_backend_type_t type;
    void (*destroy) (void *cr);

    cairo_surface_t *(*get_original_target) (void *cr);
    cairo_surface_t *(*get_current_target) (void *cr);

    cairo_status_t (*save) (void *cr);
    cairo_status_t (*restore) (void *cr);

    cairo_status_t (*push_group) (void *cr, cairo_content_t content);
    cairo_pattern_t *(*pop_group) (void *cr);

    cairo_status_t (*set_source_rgba) (void *cr, double red, double green, double blue, double alpha);
    cairo_status_t (*set_source_surface) (void *cr, cairo_surface_t *surface, double x, double y);
    cairo_status_t (*set_source) (void *cr, cairo_pattern_t *source);
    cairo_pattern_t *(*get_source) (void *cr);

    cairo_status_t (*set_antialias) (void *cr, cairo_antialias_t antialias);
    cairo_status_t (*set_dash) (void *cr, const double *dashes, int num_dashes, double offset);
    cairo_status_t (*set_fill_rule) (void *cr, cairo_fill_rule_t fill_rule);
    cairo_status_t (*set_line_cap) (void *cr, cairo_line_cap_t line_cap);
    cairo_status_t (*set_line_join) (void *cr, cairo_line_join_t line_join);
    cairo_status_t (*set_line_width) (void *cr, double line_width);
    cairo_status_t (*set_hairline) (void *cr, cairo_bool_t set_hairline);
    cairo_status_t (*set_miter_limit) (void *cr, double limit);
    cairo_status_t (*set_opacity) (void *cr, double opacity);
    cairo_status_t (*set_operator) (void *cr, cairo_operator_t op);
    cairo_status_t (*set_tolerance) (void *cr, double tolerance);

    cairo_antialias_t (*get_antialias) (void *cr);
    void (*get_dash) (void *cr, double *dashes, int *num_dashes, double *offset);
    cairo_fill_rule_t (*get_fill_rule) (void *cr);
    cairo_line_cap_t (*get_line_cap) (void *cr);
    cairo_line_join_t (*get_line_join) (void *cr);
    double (*get_line_width) (void *cr);
    cairo_bool_t (*get_hairline) (void *cr);
    double (*get_miter_limit) (void *cr);
    double (*get_opacity) (void *cr);
    cairo_operator_t (*get_operator) (void *cr);
    double (*get_tolerance) (void *cr);

    cairo_status_t (*translate) (void *cr, double tx, double ty);
    cairo_status_t (*scale) (void *cr, double sx, double sy);
    cairo_status_t (*rotate) (void *cr, double theta);
    cairo_status_t (*transform) (void *cr, const cairo_matrix_t *matrix);
    cairo_status_t (*set_matrix) (void *cr, const cairo_matrix_t *matrix);
    cairo_status_t (*set_identity_matrix) (void *cr);
    void (*get_matrix) (void *cr, cairo_matrix_t *matrix);

    void (*user_to_device) (void *cr, double *x, double *y);
    void (*user_to_device_distance) (void *cr, double *x, double *y);
    void (*device_to_user) (void *cr, double *x, double *y);
    void (*device_to_user_distance) (void *cr, double *x, double *y);

    void (*user_to_backend) (void *cr, double *x, double *y);
    void (*user_to_backend_distance) (void *cr, double *x, double *y);
    void (*backend_to_user) (void *cr, double *x, double *y);
    void (*backend_to_user_distance) (void *cr, double *x, double *y);

    cairo_status_t (*new_path) (void *cr);
    cairo_status_t (*new_sub_path) (void *cr);
    cairo_status_t (*move_to) (void *cr, double x, double y);
    cairo_status_t (*rel_move_to) (void *cr, double dx, double dy);
    cairo_status_t (*line_to) (void *cr, double x, double y);
    cairo_status_t (*rel_line_to) (void *cr, double dx, double dy);
    cairo_status_t (*curve_to) (void *cr, double x1, double y1, double x2, double y2, double x3, double y3);
    cairo_status_t (*rel_curve_to) (void *cr, double dx1, double dy1, double dx2, double dy2, double dx3, double dy3);
    cairo_status_t (*arc_to) (void *cr, double x1, double y1, double x2, double y2, double radius);
    cairo_status_t (*rel_arc_to) (void *cr, double dx1, double dy1, double dx2, double dy2, double radius);
    cairo_status_t (*close_path) (void *cr);

    cairo_status_t (*arc) (void *cr, double xc, double yc, double radius, double angle1, double angle2, cairo_bool_t forward);
    cairo_status_t (*rectangle) (void *cr, double x, double y, double width, double height);

    void (*path_extents) (void *cr, double *x1, double *y1, double *x2, double *y2);
    cairo_bool_t (*has_current_point) (void *cr);
    cairo_bool_t (*get_current_point) (void *cr, double *x, double *y);

    cairo_path_t *(*copy_path) (void *cr);
    cairo_path_t *(*copy_path_flat) (void *cr);
    cairo_status_t (*append_path) (void *cr, const cairo_path_t *path);

    cairo_status_t (*stroke_to_path) (void *cr);

    cairo_status_t (*clip) (void *cr);
    cairo_status_t (*clip_preserve) (void *cr);
    cairo_status_t (*in_clip) (void *cr, double x, double y, cairo_bool_t *inside);
    cairo_status_t (*clip_extents) (void *cr, double *x1, double *y1, double *x2, double *y2);
    cairo_status_t (*reset_clip) (void *cr);
    cairo_rectangle_list_t *(*clip_copy_rectangle_list) (void *cr);

    cairo_status_t (*paint) (void *cr);
    cairo_status_t (*paint_with_alpha) (void *cr, double opacity);
    cairo_status_t (*mask) (void *cr, cairo_pattern_t *pattern);

    cairo_status_t (*stroke) (void *cr);
    cairo_status_t (*stroke_preserve) (void *cr);
    cairo_status_t (*in_stroke) (void *cr, double x, double y, cairo_bool_t *inside);
    cairo_status_t (*stroke_extents) (void *cr, double *x1, double *y1, double *x2, double *y2);

    cairo_status_t (*fill) (void *cr);
    cairo_status_t (*fill_preserve) (void *cr);
    cairo_status_t (*in_fill) (void *cr, double x, double y, cairo_bool_t *inside);
    cairo_status_t (*fill_extents) (void *cr, double *x1, double *y1, double *x2, double *y2);

    cairo_status_t (*set_font_face) (void *cr, cairo_font_face_t *font_face);
    cairo_font_face_t *(*get_font_face) (void *cr);
    cairo_status_t (*set_font_size) (void *cr, double size);
    cairo_status_t (*set_font_matrix) (void *cr, const cairo_matrix_t *matrix);
    void (*get_font_matrix) (void *cr, cairo_matrix_t *matrix);
    cairo_status_t (*set_font_options) (void *cr, const cairo_font_options_t *options);
    void (*get_font_options) (void *cr, cairo_font_options_t *options);
    cairo_status_t (*set_scaled_font) (void *cr, cairo_scaled_font_t *scaled_font);
    cairo_scaled_font_t *(*get_scaled_font) (void *cr);
    cairo_status_t (*font_extents) (void *cr, cairo_font_extents_t *extents);

    cairo_status_t (*glyphs) (void *cr,
			      const cairo_glyph_t *glyphs, int num_glyphs,
			      cairo_glyph_text_info_t *info);
    cairo_status_t (*glyph_path) (void *cr,
				  const cairo_glyph_t *glyphs, int num_glyphs);

    cairo_status_t (*glyph_extents) (void *cr,
				     const cairo_glyph_t *glyphs,
				     int num_glyphs,
				     cairo_text_extents_t *extents);

    cairo_status_t (*copy_page) (void *cr);
    cairo_status_t (*show_page) (void *cr);

    cairo_status_t (*tag_begin) (void *cr, const char *tag_name, const char *attributes);
    cairo_status_t (*tag_end) (void *cr, const char *tag_name);
};

static inline void
_cairo_backend_to_user (cairo_t *cr, double *x, double *y)
{
    cr->backend->backend_to_user (cr, x, y);
}

static inline void
_cairo_backend_to_user_distance (cairo_t *cr, double *x, double *y)
{
    cr->backend->backend_to_user_distance (cr, x, y);
}

static inline void
_cairo_user_to_backend (cairo_t *cr, double *x, double *y)
{
    cr->backend->user_to_backend (cr, x, y);
}

static inline void
_cairo_user_to_backend_distance (cairo_t *cr, double *x, double *y)
{
    cr->backend->user_to_backend_distance (cr, x, y);
}

#endif /* CAIRO_BACKEND_PRIVATE_H */
