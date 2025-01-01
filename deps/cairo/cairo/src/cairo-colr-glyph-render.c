/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2022 Matthias Clasen
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
 * Contributor(s):
 *      Matthias Clasen <mclasen@redhat.com>
 */

#include "cairoint.h"
#include "cairo-array-private.h"
#include "cairo-ft-private.h"
#include "cairo-path-private.h"
#include "cairo-pattern-private.h"

#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

#if HAVE_FT_COLR_V1

#include <ft2build.h>
#include FT_CONFIG_OPTIONS_H
#include FT_COLOR_H
#include FT_GLYPH_H
#include FT_OUTLINE_H
#include FT_SIZES_H

/* #define DEBUG_COLR 1 */

typedef struct _cairo_colr_glyph_render {
    FT_Face face;
    FT_Color *palette;
    unsigned int num_palette_entries;
    cairo_pattern_t *foreground_marker;
    cairo_pattern_t *foreground_source;
    cairo_bool_t foreground_source_used;
    int level;
} cairo_colr_glyph_render_t;

static cairo_status_t
draw_paint (cairo_colr_glyph_render_t *render,
	    FT_OpaquePaint            *paint,
	    cairo_t                   *cr);


static inline double
double_from_16_16 (FT_Fixed f)
{
    return f / (double) (1 << 16);
}

static inline double
double_from_26_6 (FT_F26Dot6 f)
{
    return f / (double) (1 << 6);
}

static inline double
double_from_2_14 (FT_F2Dot14 f)
{
    return f / (double) (1 << 14);
}

static inline double
interpolate (double f0, double f1, double f)
{
    return f0 + f * (f1 - f0);
}

static inline void
interpolate_points (cairo_point_double_t *p0,
		    cairo_point_double_t *p1,
		    double                f,
		    cairo_point_double_t *out)
{
  out->x = interpolate (p0->x, p1->x, f);
  out->y = interpolate (p0->y, p1->y, f);
}

static inline void
interpolate_colors (cairo_color_t *c0,
		    cairo_color_t *c1,
		    double         f,
		    cairo_color_t *out)
{
    out->red = interpolate (c0->red, c1->red, f);
    out->green = interpolate (c0->green, c1->green, f);
    out->blue = interpolate (c0->blue, c1->blue, f);
    out->alpha = interpolate (c0->alpha, c1->alpha, f);
}

static inline double
dot (cairo_point_double_t p, cairo_point_double_t q)
{
    return p.x * q.x + p.y * q.y;
}

static inline cairo_point_double_t
normalize (cairo_point_double_t p)
{
    double len = sqrt (dot (p, p));

    return (cairo_point_double_t) { p.x / len, p.y / len };
}

static inline cairo_point_double_t
sum (cairo_point_double_t p, cairo_point_double_t q)
{
    return (cairo_point_double_t) { p.x + q.x, p.y + q.y };
}

static inline cairo_point_double_t
difference (cairo_point_double_t p, cairo_point_double_t q)
{
    return (cairo_point_double_t) { p.x - q.x, p.y - q.y };
}

static inline cairo_point_double_t
scale (cairo_point_double_t p, double f)
{
    return (cairo_point_double_t) { p.x * f, p.y * f };
}

static cairo_operator_t
cairo_operator_from_ft_composite_mode (FT_Composite_Mode mode)
{
    switch (mode)
    {
	case FT_COLR_COMPOSITE_CLEAR: return CAIRO_OPERATOR_CLEAR;
	case FT_COLR_COMPOSITE_SRC: return CAIRO_OPERATOR_SOURCE;
	case FT_COLR_COMPOSITE_DEST: return CAIRO_OPERATOR_DEST;
	case FT_COLR_COMPOSITE_SRC_OVER: return CAIRO_OPERATOR_OVER;
	case FT_COLR_COMPOSITE_DEST_OVER: return CAIRO_OPERATOR_DEST_OVER;
	case FT_COLR_COMPOSITE_SRC_IN: return CAIRO_OPERATOR_IN;
	case FT_COLR_COMPOSITE_DEST_IN: return CAIRO_OPERATOR_DEST_IN;
	case FT_COLR_COMPOSITE_SRC_OUT: return CAIRO_OPERATOR_OUT;
	case FT_COLR_COMPOSITE_DEST_OUT: return CAIRO_OPERATOR_DEST_OUT;
	case FT_COLR_COMPOSITE_SRC_ATOP: return CAIRO_OPERATOR_ATOP;
	case FT_COLR_COMPOSITE_DEST_ATOP: return CAIRO_OPERATOR_DEST_ATOP;
	case FT_COLR_COMPOSITE_XOR: return CAIRO_OPERATOR_XOR;
	case FT_COLR_COMPOSITE_PLUS: return CAIRO_OPERATOR_ADD;
	case FT_COLR_COMPOSITE_SCREEN: return CAIRO_OPERATOR_SCREEN;
	case FT_COLR_COMPOSITE_OVERLAY: return CAIRO_OPERATOR_OVERLAY;
	case FT_COLR_COMPOSITE_DARKEN: return CAIRO_OPERATOR_DARKEN;
	case FT_COLR_COMPOSITE_LIGHTEN: return CAIRO_OPERATOR_LIGHTEN;
	case FT_COLR_COMPOSITE_COLOR_DODGE: return CAIRO_OPERATOR_COLOR_DODGE;
	case FT_COLR_COMPOSITE_COLOR_BURN: return CAIRO_OPERATOR_COLOR_BURN;
	case FT_COLR_COMPOSITE_HARD_LIGHT: return CAIRO_OPERATOR_HARD_LIGHT;
	case FT_COLR_COMPOSITE_SOFT_LIGHT: return CAIRO_OPERATOR_SOFT_LIGHT;
	case FT_COLR_COMPOSITE_DIFFERENCE: return CAIRO_OPERATOR_DIFFERENCE;
	case FT_COLR_COMPOSITE_EXCLUSION: return CAIRO_OPERATOR_EXCLUSION;
	case FT_COLR_COMPOSITE_MULTIPLY: return CAIRO_OPERATOR_MULTIPLY;
	case FT_COLR_COMPOSITE_HSL_HUE: return CAIRO_OPERATOR_HSL_HUE;
	case FT_COLR_COMPOSITE_HSL_SATURATION: return CAIRO_OPERATOR_HSL_SATURATION;
	case FT_COLR_COMPOSITE_HSL_COLOR: return CAIRO_OPERATOR_HSL_COLOR;
	case FT_COLR_COMPOSITE_HSL_LUMINOSITY: return CAIRO_OPERATOR_HSL_LUMINOSITY;
	case FT_COLR_COMPOSITE_MAX:
	default:
	    ASSERT_NOT_REACHED;
    }
}

static cairo_extend_t
cairo_extend_from_ft_paint_extend (FT_PaintExtend extend)
{
    switch (extend)
    {
	case FT_COLR_PAINT_EXTEND_PAD: return CAIRO_EXTEND_PAD;
	case FT_COLR_PAINT_EXTEND_REPEAT: return CAIRO_EXTEND_REPEAT;
	case FT_COLR_PAINT_EXTEND_REFLECT: return CAIRO_EXTEND_REFLECT;
	default:
	    ASSERT_NOT_REACHED;
    }
}

static cairo_status_t
draw_paint_colr_layers (cairo_colr_glyph_render_t *render,
                        FT_PaintColrLayers        *colr_layers,
                        cairo_t                   *cr)
{
    FT_OpaquePaint paint;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

#if DEBUG_COLR
    printf ("%*sDraw PaintColrLayers\n", 2 * render->level, "");
#endif

    while (FT_Get_Paint_Layers (render->face, &colr_layers->layer_iterator, &paint)) {
	cairo_push_group (cr);
	status = draw_paint (render, &paint, cr);
	cairo_pop_group_to_source (cr);
	cairo_set_operator (cr, CAIRO_OPERATOR_OVER);
	cairo_paint (cr);

	if (unlikely (status))
	    break;
    }

    return status;
}

static void
get_palette_color (cairo_colr_glyph_render_t *render,
		   FT_ColorIndex             *ci,
		   cairo_color_t             *color,
		   double                    *colr_alpha,
		   cairo_bool_t              *is_foreground_color)
{
    cairo_bool_t foreground = FALSE;

    if (ci->palette_index == 0xffff || ci->palette_index >= render->num_palette_entries) {
	color->red = 0;
	color->green = 0;
	color->blue = 0;
	color->alpha = 1;
	foreground = TRUE;
    } else {
	FT_Color c = render->palette[ci->palette_index];
	color->red = c.red / 255.0;
	color->green = c.green / 255.0;
	color->blue = c.blue / 255.0;
	color->alpha = c.alpha / 255.0;
    }

    *colr_alpha = double_from_2_14 (ci->alpha);
    *is_foreground_color = foreground;
}

static cairo_status_t
draw_paint_solid (cairo_colr_glyph_render_t *render,
                  FT_PaintSolid             *solid,
                  cairo_t                   *cr)
{
    cairo_color_t color;
    double colr_alpha;
    cairo_bool_t is_foreground_color;

#if DEBUG_COLR
    printf ("%*sDraw PaintSolid\n", 2 * render->level, "");
#endif

    get_palette_color (render, &solid->color, &color, &colr_alpha, &is_foreground_color);
    if (is_foreground_color) {
	cairo_set_source (cr, render->foreground_marker);
	cairo_paint_with_alpha (cr, colr_alpha);
    } else {
	cairo_set_source_rgba (cr, color.red, color.green, color.blue, color.alpha * colr_alpha);
	cairo_paint (cr);
    }

    return CAIRO_STATUS_SUCCESS;
}

typedef struct _cairo_colr_color_stop {
    cairo_color_t color;
    double position;
} cairo_colr_color_stop_t;

typedef struct _cairo_colr_color_line {
    int n_stops;
    cairo_colr_color_stop_t *stops;
} cairo_colr_color_line_t;

static void
free_colorline (cairo_colr_color_line_t *cl)
{
    free (cl->stops);
    free (cl);
}

static int
_compare_stops (const void *p1, const void *p2)
{
    const cairo_colr_color_stop_t *c1 = p1;
    const cairo_colr_color_stop_t *c2 = p2;

    if (c1->position < c2->position)
	return -1;
    else if (c1->position > c2->position)
	return 1;
    else
	return 0;
}

static cairo_colr_color_line_t *
read_colorline (cairo_colr_glyph_render_t *render,
                FT_ColorLine              *colorline)
{
    cairo_colr_color_line_t *cl;
    FT_ColorStop stop;
    int i;
    double colr_alpha;
    cairo_bool_t is_foreground_color;

    cl = calloc (1, sizeof (cairo_colr_color_line_t));
    if (unlikely (cl == NULL))
	return NULL;

    cl->n_stops = colorline->color_stop_iterator.num_color_stops;
    cl->stops = calloc (cl->n_stops, sizeof (cairo_colr_color_stop_t));
    if (unlikely (cl->stops == NULL)) {
	free (cl);
	return NULL;
    }

    i = 0;
    while (FT_Get_Colorline_Stops (render->face, &stop, &colorline->color_stop_iterator)) {
	cl->stops[i].position = double_from_16_16 (stop.stop_offset);
	get_palette_color (render, &stop.color, &cl->stops[i].color, &colr_alpha, &is_foreground_color);
	if (is_foreground_color) {
	    double red, green, blue, alpha;
	    if (cairo_pattern_get_rgba (render->foreground_source,
					&red, &green, &blue, &alpha) == CAIRO_STATUS_SUCCESS)
	    {
		cl->stops[i].color.red = red;
		cl->stops[i].color.green = green;
		cl->stops[i].color.blue = blue;
		cl->stops[i].color.alpha = alpha * colr_alpha;
		render->foreground_source_used = TRUE;
	    }
	    else
	    {
		cl->stops[i].color.red = 0;
		cl->stops[i].color.green = 0;
		cl->stops[i].color.blue = 0;
		cl->stops[i].color.alpha = colr_alpha;
	    }
	} else {
	    cl->stops[i].color.alpha *= colr_alpha;
	}
	i++;
    }

    qsort (cl->stops, cl->n_stops, sizeof (cairo_colr_color_stop_t), _compare_stops);

    return cl;
}

static void
reduce_anchors (FT_PaintLinearGradient *gradient,
                cairo_point_double_t   *pp0,
                cairo_point_double_t   *pp1)
{
    cairo_point_double_t p0, p1, p2;
    cairo_point_double_t q1, q2;
    double s;
    double k;

    p0.x = double_from_16_16 (gradient->p0.x);
    p0.y = double_from_16_16 (gradient->p0.y);
    p1.x = double_from_16_16 (gradient->p1.x);
    p1.y = double_from_16_16 (gradient->p1.y);
    p2.x = double_from_16_16 (gradient->p2.x);
    p2.y = double_from_16_16 (gradient->p2.y);

    q2.x = p2.x - p0.x;
    q2.y = p2.y - p0.y;
    q1.x = p1.x - p0.x;
    q1.y = p1.y - p0.y;

    s = q2.x * q2.x + q2.y * q2.y;
    if (s < 0.000001)
    {
	pp0->x = p0.x; pp0->y = p0.y;
	pp1->x = p1.x; pp1->y = p1.y;
	return;
    }

    k = (q2.x * q1.x + q2.y * q1.y) / s;
    pp0->x = p0.x;
    pp0->y = p0.y;
    pp1->x = p1.x - k * q2.x;
    pp1->y = p1.y - k * q2.y;
}

static void
normalize_colorline (cairo_colr_color_line_t *cl,
                     double                  *out_min,
                     double                  *out_max)
{
    double min, max;

    *out_min = 0.;
    *out_max = 1.;

    min = max = cl->stops[0].position;
    for (int i = 0; i < cl->n_stops; i++) {
	cairo_colr_color_stop_t *stop = &cl->stops[i];
	min = MIN (min, stop->position);
	max = MAX (max, stop->position);
    }

    if (min != max) {
	for (int i = 0; i < cl->n_stops; i++) {
	    cairo_colr_color_stop_t *stop = &cl->stops[i];
	    stop->position = (stop->position - min) / (max - min);
        }
	*out_min = min;
	*out_max = max;
    }
}

static cairo_status_t
draw_paint_linear_gradient (cairo_colr_glyph_render_t *render,
                            FT_PaintLinearGradient    *gradient,
                            cairo_t                   *cr)
{
    cairo_colr_color_line_t *cl;
    cairo_point_double_t p0, p1;
    cairo_point_double_t pp0, pp1;
    cairo_pattern_t *pattern;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;
    double min, max;

#if DEBUG_COLR
    printf ("%*sDraw PaintLinearGradient\n", 2 * render->level, "");
#endif

    cl = read_colorline (render, &gradient->colorline);
    if (unlikely (cl == NULL))
	return CAIRO_STATUS_NO_MEMORY;

    /* cairo only allows stop positions between 0 and 1 */
    normalize_colorline (cl, &min, &max);
    reduce_anchors (gradient, &p0, &p1);
    interpolate_points (&p0, &p1, min, &pp0);
    interpolate_points (&p0, &p1, max, &pp1);

    pattern = cairo_pattern_create_linear (pp0.x, pp0.y, pp1.x, pp1.y);

    cairo_pattern_set_extend (pattern, cairo_extend_from_ft_paint_extend (gradient->colorline.extend));

    for (int i = 0; i < cl->n_stops; i++) {
	cairo_colr_color_stop_t *stop = &cl->stops[i];
	cairo_pattern_add_color_stop_rgba (pattern, stop->position,
					   stop->color.red, stop->color.green, stop->color.blue, stop->color.alpha);
    }

    cairo_set_source (cr, pattern);
    cairo_paint (cr);

    cairo_pattern_destroy (pattern);

    free_colorline (cl);

    return status;
}

static cairo_status_t
draw_paint_radial_gradient (cairo_colr_glyph_render_t *render,
                            FT_PaintRadialGradient *gradient,
                            cairo_t *cr)
{
    cairo_colr_color_line_t *cl;
    cairo_point_double_t start, end;
    cairo_point_double_t start1, end1;
    double start_radius, end_radius;
    double start_radius1, end_radius1;
    double min, max;
    cairo_pattern_t *pattern;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

#if DEBUG_COLR
    printf ("%*sDraw PaintRadialGradient\n", 2 * render->level, "");
#endif

    cl = read_colorline (render, &gradient->colorline);
    if (unlikely (cl == NULL))
	return CAIRO_STATUS_NO_MEMORY;

    start.x = double_from_16_16 (gradient->c0.x);
    start.y = double_from_16_16 (gradient->c0.y);
    end.x = double_from_16_16 (gradient->c1.x);
    end.y = double_from_16_16 (gradient->c1.y);

    start_radius = double_from_16_16 (gradient->r0);
    end_radius = double_from_16_16 (gradient->r1);

    /* cairo only allows stop positions between 0 and 1 */
    normalize_colorline (cl, &min, &max);
    interpolate_points (&start, &end, min, &start1);
    interpolate_points (&start, &end, max, &end1);
    start_radius1 = interpolate (start_radius, end_radius, min);
    end_radius1 = interpolate (start_radius, end_radius, max);

    pattern = cairo_pattern_create_radial (start1.x, start1.y, start_radius1,
					   end1.x, end1.y, end_radius1);

    cairo_pattern_set_extend (pattern, cairo_extend_from_ft_paint_extend (gradient->colorline.extend));

    for (int i = 0; i < cl->n_stops; i++) {
	cairo_colr_color_stop_t *stop = &cl->stops[i];
	cairo_pattern_add_color_stop_rgba (pattern, stop->position,
					   stop->color.red, stop->color.green, stop->color.blue, stop->color.alpha);
    }

    cairo_set_source (cr, pattern);
    cairo_paint (cr);

    cairo_pattern_destroy (pattern);

    free_colorline (cl);

    return status;
}

typedef struct {
    cairo_point_double_t center, p0, c0, c1, p1;
    cairo_color_t color0, color1;
} cairo_colr_gradient_patch_t;

static void
add_patch (cairo_pattern_t             *pattern,
	   cairo_point_double_t        *center,
	   cairo_colr_gradient_patch_t *p)
{
    cairo_mesh_pattern_begin_patch (pattern);
    cairo_mesh_pattern_move_to (pattern, center->x, center->y);
    cairo_mesh_pattern_line_to (pattern, p->p0.x, p->p0.y);
    cairo_mesh_pattern_curve_to (pattern,
				 p->c0.x, p->c0.y,
				 p->c1.x, p->c1.y,
				 p->p1.x, p->p1.y);
    cairo_mesh_pattern_line_to (pattern, center->x, center->y);
    cairo_mesh_pattern_set_corner_color_rgba (pattern, 0,
					      p->color0.red,
					      p->color0.green,
					      p->color0.blue,
					      p->color0.alpha);
    cairo_mesh_pattern_set_corner_color_rgba (pattern, 1,
					      p->color0.red,
					      p->color0.green,
					      p->color0.blue,
					      p->color0.alpha);
    cairo_mesh_pattern_set_corner_color_rgba (pattern, 2,
					      p->color1.red,
					      p->color1.green,
					      p->color1.blue,
					      p->color1.alpha);
    cairo_mesh_pattern_set_corner_color_rgba (pattern, 3,
					      p->color1.red,
					      p->color1.green,
					      p->color1.blue,
					      p->color1.alpha);
    cairo_mesh_pattern_end_patch (pattern);
}

#define MAX_ANGLE (M_PI / 8.)

static void
add_sweep_gradient_patches1 (cairo_point_double_t *center,
			     double                radius,
                             double                a0,
			     cairo_color_t        *c0,
                             double                a1,
			     cairo_color_t        *c1,
                             cairo_pattern_t      *pattern)
{

    int num_splits;
    cairo_point_double_t p0;
    cairo_color_t color0, color1;

    num_splits = ceilf (fabs (a1 - a0) / MAX_ANGLE);
    p0 = (cairo_point_double_t) { cosf (a0), sinf (a0) };
    color0 = *c0;

    for (int a = 0; a < num_splits; a++) {
	double k = (a + 1.) / num_splits;
	double angle1;
	cairo_point_double_t p1;
	cairo_point_double_t A, U;
	cairo_point_double_t C0, C1;
	cairo_colr_gradient_patch_t patch;

	angle1 = interpolate (a0, a1, k);
	interpolate_colors (c0, c1, k, &color1);

	patch.color0 = color0;
	patch.color1 = color1;

	p1 = (cairo_point_double_t) { cosf (angle1), sinf (angle1) };
	patch.p0 = sum (*center, scale (p0, radius));
	patch.p1 = sum (*center, scale (p1, radius));

	A = normalize (sum (p0, p1));
	U = (cairo_point_double_t) { -A.y, A.x };
	C0 = sum (A, scale (U, dot (difference (p0, A), p0) / dot (U, p0)));
	C1 = sum (A, scale (U, dot (difference (p1, A), p1) / dot (U, p1)));
	patch.c0 = sum (*center, scale (sum (C0, scale (difference (C0, p0), 0.33333)), radius));
	patch.c1 = sum (*center, scale (sum (C1, scale (difference (C1, p1), 0.33333)), radius));

	add_patch (pattern, center, &patch);

	p0 = p1;
	color0 = color1;
    }
}

static void
add_sweep_gradient_patches (cairo_colr_color_line_t *cl,
                            cairo_extend_t           extend,
                            cairo_point_double_t    *center,
                            double                   radius,
                            double                   start_angle,
                            double                   end_angle,
                            cairo_pattern_t         *pattern)
{
    double *angles;
    cairo_color_t color0, color1;

    if (start_angle == end_angle) {
	if (extend == CAIRO_EXTEND_PAD) {
	    if (start_angle > 0)
		add_sweep_gradient_patches1 (center, radius,
					     0.,          &cl->stops[0].color,
					     start_angle, &cl->stops[0].color,
					     pattern);
	    if (end_angle < 2 * M_PI)
		add_sweep_gradient_patches1 (center, radius,
					     end_angle, &cl->stops[cl->n_stops - 1].color,
					     2 * M_PI,  &cl->stops[cl->n_stops - 1].color,
					     pattern);
        }
	return;
    }

    assert (start_angle != end_angle);

    angles = alloca (sizeof (double) * cl->n_stops);

    for (int i = 0; i < cl->n_stops; i++)
	angles[i] = start_angle + cl->stops[i].position * (end_angle - start_angle);

    /* handle directions */
    if (end_angle < start_angle) {
	for (int i = 0; i < cl->n_stops - 1 - i; i++) {
	    cairo_colr_color_stop_t stop = cl->stops[i];
	    double a = angles[i];
	    cl->stops[i] = cl->stops[cl->n_stops - 1 - i];
	    cl->stops[cl->n_stops - 1 - i] = stop;
	    angles[i] = angles[cl->n_stops - 1 - i];
	    angles[cl->n_stops - 1 - i] = a;
        }
    }

    if (extend == CAIRO_EXTEND_PAD)
    {
	int pos;

	color0 = cl->stops[0].color;
	for (pos = 0; pos < cl->n_stops; pos++) {
	    if (angles[pos] >= 0) {
		if (pos > 0) {
		    double k = (0 - angles[pos - 1]) / (angles[pos] - angles[pos - 1]);
		    interpolate_colors (&cl->stops[pos - 1].color, &cl->stops[pos].color, k, &color0);
                }
		break;
            }
        }
	if (pos == cl->n_stops) {
	    /* everything is below 0 */
	    color0 = cl->stops[cl->n_stops - 1].color;
	    add_sweep_gradient_patches1 (center, radius,
					 0.,       &color0,
					 2 * M_PI, &color0,
					 pattern);
	    return;
        }

	add_sweep_gradient_patches1 (center, radius,
				     0.,          &color0,
				     angles[pos], &cl->stops[pos].color,
				     pattern);

	for (pos++; pos < cl->n_stops; pos++) {
	    if (angles[pos] <= 2 * M_PI) {
		add_sweep_gradient_patches1 (center, radius,
					     angles[pos - 1], &cl->stops[pos - 1].color,
					     angles[pos],     &cl->stops[pos].color,
					     pattern);
            } else {
		double k = (2 * M_PI - angles[pos - 1]) / (angles[pos] - angles[pos - 1]);
		interpolate_colors (&cl->stops[pos - 1].color, &cl->stops[pos].color, k, &color1);
		add_sweep_gradient_patches1 (center, radius,
					     angles[pos - 1], &cl->stops[pos - 1].color,
					     2 * M_PI,        &color1,
					     pattern);
		break;
            }
        }

	if (pos == cl->n_stops) {
	    /* everything is below 2*M_PI */
	    color0 = cl->stops[cl->n_stops - 1].color;
	    add_sweep_gradient_patches1 (center, radius,
					 angles[cl->n_stops - 1], &color0,
					 2 * M_PI,                &color0,
					 pattern);
	    return;
        }
    } else {
	int k;
	double span;

	span = angles[cl->n_stops - 1] - angles[0];
	k = 0;
	if (angles[0] >= 0) {
	    double ss = angles[0];
	    while (ss > 0) {
		if (span > 0) {
		    ss -= span;
		    k--;
                } else {
		    ss += span;
		    k++;
                }
            }
        }
	else if (angles[0] < 0)
        {
	    double ee = angles[cl->n_stops - 1];
	    while (ee < 0) {
		if (span > 0) {
		    ee += span;
		    k++;
                } else {
		    ee -= span;
		    k--;
                }
            }
        }

	//assert (angles[0] + k * span <= 0 && 0 < angles[cl->n_stops - 1] + k * span);

	for (int l = k; TRUE; l++) {
	    for (int i = 1; i < cl->n_stops; i++) {
		double a0, a1;
		cairo_color_t *c0, *c1;

		if ((l % 2 != 0) && (extend == CAIRO_EXTEND_REFLECT)) {
		    a0 = angles[0] + angles[cl->n_stops - 1] - angles[cl->n_stops - 1 - (i-1)] + l * span;
		    a1 = angles[0] + angles[cl->n_stops - 1] - angles[cl->n_stops - 1 - i] + l * span;
		    c0 = &cl->stops[cl->n_stops - 1 - (i-1)].color;
		    c1 = &cl->stops[cl->n_stops - 1 - i].color;
                } else {
		    a0 = angles[i-1] + l * span;
		    a1 = angles[i] + l * span;
		    c0 = &cl->stops[i-1].color;
		    c1 = &cl->stops[i].color;
                }

		if (a1 < 0)
		    continue;

		if (a0 < 0) {
		    cairo_color_t color;
		    double f = (0 - a0)/(a1 - a0);
		    interpolate_colors (c0, c1, f, &color);
		    add_sweep_gradient_patches1 (center, radius,
						 0,  &color,
						 a1, c1,
						 pattern);
                } else if (a1 >= 2 * M_PI) {
		    cairo_color_t color;
		    double f = (2 * M_PI - a0)/(a1 - a0);
		    interpolate_colors (c0, c1, f, &color);
		    add_sweep_gradient_patches1 (center, radius,
						 a0,       c0,
						 2 * M_PI, &color,
						 pattern);
		    return;
                } else {
		    add_sweep_gradient_patches1 (center, radius,
						 a0, c0,
						 a1, c1,
						 pattern);
                }
            }
        }
    }
}

static cairo_status_t
draw_paint_sweep_gradient (cairo_colr_glyph_render_t *render,
                           FT_PaintSweepGradient     *gradient,
                           cairo_t                   *cr)
{
    cairo_colr_color_line_t *cl;
    cairo_point_double_t center;
    double start_angle, end_angle;
    double x1, y1, x2, y2;
    double max_x, max_y, R;
    cairo_pattern_t *pattern;
    cairo_extend_t extend;

#if DEBUG_COLR
    printf ("%*sDraw PaintSweepGradient\n", 2 * render->level, "");
#endif

    cl = read_colorline (render, &gradient->colorline);
    if (unlikely (cl == NULL))
	return CAIRO_STATUS_NO_MEMORY;

    center.x = double_from_16_16 (gradient->center.x);
    center.y = double_from_16_16 (gradient->center.y);
    start_angle = (double_from_16_16 (gradient->start_angle) + 1) * M_PI;
    end_angle = (double_from_16_16 (gradient->end_angle) + 1) * M_PI;

    pattern = cairo_pattern_create_mesh ();

    cairo_clip_extents (cr, &x1, &y1, &x2, &y2);
    max_x = MAX ((x1 - center.x) * (x1 - center.x), (x2 - center.x) * (x2 - center.x));
    max_y = MAX ((y1 - center.y) * (y1 - center.y), (y2 - center.y) * (y2 - center.y));
    R = sqrt (max_x + max_y);

    extend = cairo_extend_from_ft_paint_extend (gradient->colorline.extend);

    add_sweep_gradient_patches (cl, extend, &center, R, start_angle, end_angle, pattern);

    cairo_set_source (cr, pattern);
    cairo_paint (cr);

    cairo_pattern_destroy (pattern);

    free_colorline (cl);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
draw_paint_glyph (cairo_colr_glyph_render_t *render,
                  FT_PaintGlyph             *glyph,
                  cairo_t                   *cr)
{
    cairo_path_fixed_t *path_fixed;
    cairo_path_t *path;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;
    FT_Error error;

#if DEBUG_COLR
    printf ("%*sDraw PaintGlyph\n", 2 * render->level, "");
#endif

    error = FT_Load_Glyph (render->face, glyph->glyphID, FT_LOAD_DEFAULT);
    status = _cairo_ft_to_cairo_error (error);
    if (unlikely (status))
        return status;

    status = _cairo_ft_face_decompose_glyph_outline (render->face, &path_fixed);
    if (unlikely (status))
        return status;

    cairo_save (cr);
    cairo_identity_matrix (cr);
    path = _cairo_path_create (path_fixed, cr);
    _cairo_path_fixed_destroy (path_fixed);
    cairo_restore (cr);

    cairo_save (cr);

    cairo_new_path (cr);
    cairo_append_path (cr, path);
    cairo_path_destroy (path);
    cairo_clip (cr);

    status = draw_paint (render, &glyph->paint, cr);

    cairo_restore (cr);

    return status;
}

static cairo_status_t draw_colr_glyph (cairo_colr_glyph_render_t *render,
				       unsigned long              glyph,
                                       FT_Color_Root_Transform    root,
                                       cairo_t                   *cr);

static cairo_status_t
draw_paint_colr_glyph (cairo_colr_glyph_render_t *render,
                       FT_PaintColrGlyph *colr_glyph,
                       cairo_t *cr)
{
#if DEBUG_COLR
    printf ("%*sDraw PaintColrGlyph\n", 2 * render->level, "");
#endif

    return draw_colr_glyph (render, colr_glyph->glyphID, FT_COLOR_NO_ROOT_TRANSFORM, cr);
}

static cairo_status_t
draw_paint_transform (cairo_colr_glyph_render_t *render,
                      FT_PaintTransform *transform,
                      cairo_t *cr)
{
    cairo_matrix_t t;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

#if DEBUG_COLR
    printf ("%*sDraw PaintTransform\n", 2 * render->level, "");
#endif

    cairo_matrix_init (&t,
		       double_from_16_16 (transform->affine.xx),
		       double_from_16_16 (transform->affine.yx),
		       double_from_16_16 (transform->affine.xy),
		       double_from_16_16 (transform->affine.yy),
		       double_from_16_16 (transform->affine.dx),
		       double_from_16_16 (transform->affine.dy));

    cairo_save (cr);

    cairo_transform (cr, &t);
    status = draw_paint (render, &transform->paint, cr);

    cairo_restore (cr);

    return status;
}

static cairo_status_t
draw_paint_translate (cairo_colr_glyph_render_t *render,
                      FT_PaintTranslate *translate,
                      cairo_t *cr)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

#if DEBUG_COLR
    printf ("%*sDraw PaintTranslate\n", 2 * render->level, "");
#endif

    cairo_save (cr);

    cairo_translate (cr, double_from_16_16 (translate->dx), double_from_16_16 (translate->dy));
    status = draw_paint (render, &translate->paint, cr);

    cairo_restore (cr);

    return status;
}

static cairo_status_t
draw_paint_rotate (cairo_colr_glyph_render_t *render,
                   FT_PaintRotate *rotate,
                   cairo_t *cr)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

#if DEBUG_COLR
    printf ("%*sDraw PaintRotate\n", 2 * render->level, "");
#endif

    cairo_save (cr);

    cairo_translate (cr, double_from_16_16 (rotate->center_x), double_from_16_16 (rotate->center_y));
    cairo_rotate (cr, double_from_16_16 (rotate->angle) * M_PI);
    cairo_translate (cr, - double_from_16_16 (rotate->center_x), - double_from_16_16 (rotate->center_y));
    status = draw_paint (render, &rotate->paint, cr);

    cairo_restore (cr);

    return status;
}

static cairo_status_t
draw_paint_scale (cairo_colr_glyph_render_t *render,
                  FT_PaintScale *scale,
                  cairo_t *cr)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

#if DEBUG_COLR
    printf ("%*sDraw PaintScale\n", 2 * render->level, "");
#endif

    cairo_save (cr);

    cairo_translate (cr, double_from_16_16 (scale->center_x), double_from_16_16 (scale->center_y));
    cairo_scale (cr, double_from_16_16 (scale->scale_x), double_from_16_16 (scale->scale_y));
    cairo_translate (cr, - double_from_16_16 (scale->center_x), - double_from_16_16 (scale->center_y));
    status = draw_paint (render, &scale->paint, cr);

    cairo_restore (cr);

    return status;
}

static cairo_status_t
draw_paint_skew (cairo_colr_glyph_render_t *render,
                 FT_PaintSkew              *skew,
                 cairo_t                   *cr)
{
    cairo_matrix_t s;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

#if DEBUG_COLR
    printf ("%*sDraw PaintSkew\n", 2 * render->level, "");
#endif

    cairo_save (cr);

    cairo_translate (cr, double_from_16_16 (skew->center_x), double_from_16_16 (skew->center_y));
    cairo_matrix_init (&s, 1., tan (double_from_16_16 (skew->y_skew_angle) * M_PI), - tan (double_from_16_16 (skew->x_skew_angle) * M_PI), 1., 0., 0.);
    cairo_transform (cr, &s);
    cairo_translate (cr, - double_from_16_16 (skew->center_x), - double_from_16_16 (skew->center_y));
    status = draw_paint (render, &skew->paint, cr);

    cairo_restore (cr);

    return status;
}

static cairo_status_t
draw_paint_composite (cairo_colr_glyph_render_t *render,
                      FT_PaintComposite         *composite,
                      cairo_t                   *cr)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;


#if DEBUG_COLR
    printf ("%*sDraw PaintComposite\n", 2 * render->level, "");
#endif

    cairo_save (cr);

    status = draw_paint (render, &composite->backdrop_paint, cr);
    if (unlikely (status)) {
	cairo_pattern_destroy (cairo_pop_group (cr));
	goto cleanup;
    }

    cairo_push_group (cr);
    status = draw_paint (render, &composite->source_paint, cr);
    if (unlikely (status)) {
	cairo_pattern_destroy (cairo_pop_group (cr));
	cairo_pattern_destroy (cairo_pop_group (cr));
	goto cleanup;
    }

    cairo_pop_group_to_source (cr);
    cairo_set_operator (cr, cairo_operator_from_ft_composite_mode (composite->composite_mode));
    cairo_paint (cr);

  cleanup:
    cairo_restore (cr);

    return status;
}

static cairo_status_t
draw_paint (cairo_colr_glyph_render_t *render,
            FT_OpaquePaint *paint,
            cairo_t *cr)
{
    FT_COLR_Paint p;
    FT_Size orig_size;
    FT_Size unscaled_size;
    FT_Matrix orig_transform;
    FT_Vector orig_delta;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    assert (cairo_status (cr) == CAIRO_STATUS_SUCCESS);

    if (!FT_Get_Paint (render->face, *paint, &p))
	return CAIRO_STATUS_NO_MEMORY;

    if (render->level == 0) {
	/* Now that the FT_Get_Paint call has applied the root transform,
	 * make the face unscaled and untransformed, so we can load glyph
	 * contours.
	 */

	FT_Matrix transform;
	FT_Vector delta;

	orig_size = render->face->size;
	FT_New_Size (render->face, &unscaled_size);
	FT_Activate_Size (unscaled_size);
	FT_Set_Char_Size (render->face, render->face->units_per_EM << 6, 0, 0, 0);

	transform.xx = transform.yy = 1 << 16;
	transform.xy = transform.yx = 0;
	delta.x = delta.y = 0;

	FT_Get_Transform (render->face, &orig_transform, &orig_delta);
	FT_Set_Transform (render->face, &transform, &delta);
    }

    render->level++;

    switch (p.format) {
	case FT_COLR_PAINTFORMAT_COLR_LAYERS:
	    status = draw_paint_colr_layers (render, &p.u.colr_layers, cr);
	    break;
	case FT_COLR_PAINTFORMAT_SOLID:
	    status = draw_paint_solid (render, &p.u.solid, cr);
	    break;
	case FT_COLR_PAINTFORMAT_LINEAR_GRADIENT:
	    status = draw_paint_linear_gradient (render, &p.u.linear_gradient, cr);
	    break;
	case FT_COLR_PAINTFORMAT_RADIAL_GRADIENT:
	    status = draw_paint_radial_gradient (render, &p.u.radial_gradient, cr);
	    break;
	case FT_COLR_PAINTFORMAT_SWEEP_GRADIENT:
	    status = draw_paint_sweep_gradient (render, &p.u.sweep_gradient, cr);
	    break;
	case FT_COLR_PAINTFORMAT_GLYPH:
	    status = draw_paint_glyph (render, &p.u.glyph, cr);
	    break;
	case FT_COLR_PAINTFORMAT_COLR_GLYPH:
	    status = draw_paint_colr_glyph (render, &p.u.colr_glyph, cr);
	    break;
	case FT_COLR_PAINTFORMAT_TRANSFORM:
	    status = draw_paint_transform (render, &p.u.transform, cr);
	    break;
	case FT_COLR_PAINTFORMAT_TRANSLATE:
	    status = draw_paint_translate (render, &p.u.translate, cr);
	    break;
	case FT_COLR_PAINTFORMAT_ROTATE:
	    status = draw_paint_rotate (render, &p.u.rotate, cr);
	    break;
	case FT_COLR_PAINTFORMAT_SCALE:
	    status = draw_paint_scale (render, &p.u.scale, cr);
	    break;
	case FT_COLR_PAINTFORMAT_SKEW:
	    status = draw_paint_skew (render, &p.u.skew, cr);
	    break;
	case FT_COLR_PAINTFORMAT_COMPOSITE:
	    status = draw_paint_composite (render, &p.u.composite, cr);
	    break;
	case FT_COLR_PAINT_FORMAT_MAX:
	case FT_COLR_PAINTFORMAT_UNSUPPORTED:
	default:
	    ASSERT_NOT_REACHED;
    }

    render->level--;

    if (render->level == 0) {
	FT_Set_Transform (render->face, &orig_transform, &orig_delta);
	FT_Activate_Size (orig_size);
	FT_Done_Size (unscaled_size);
    }

    return status;
}

static cairo_status_t
draw_colr_glyph (cairo_colr_glyph_render_t *render,
		 unsigned long              glyph,
                 FT_Color_Root_Transform    root,
                 cairo_t                   *cr)
{
    FT_OpaquePaint paint = { NULL, 0 };
    FT_ClipBox box;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    cairo_save (cr);

    if (FT_Get_Color_Glyph_ClipBox (render->face, glyph, &box)) {
	double xmin, ymin, xmax, ymax;

	xmin = double_from_26_6 (box.bottom_left.x);
	ymin = double_from_26_6 (box.bottom_left.y);
	xmax = double_from_26_6 (box.top_right.x);
	ymax = double_from_26_6 (box.top_right.y);

	cairo_new_path (cr);
	cairo_rectangle (cr, xmin, ymin, xmax - xmin, ymax - ymin);
	cairo_clip (cr);
    }

    if (FT_Get_Color_Glyph_Paint (render->face, glyph, root, &paint))
	status = draw_paint (render, &paint, cr);

    cairo_restore (cr);

    return status;
}

/* Create an image surface and render the glyph onto it,
 * using the given colors.
 */
cairo_status_t
_cairo_render_colr_v1_glyph (FT_Face               face,
                             unsigned long         glyph,
                             FT_Color             *palette,
                             int                   num_palette_entries,
                             cairo_t              *cr,
                             cairo_pattern_t      *foreground_source,
                             cairo_bool_t         *foreground_source_used)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;
    cairo_colr_glyph_render_t colr_render;

#if DEBUG_COLR
    printf ("_cairo_render_colr_glyph  glyph index: %ld\n", glyph);
#endif

    colr_render.face = face;
    colr_render.palette = palette;
    colr_render.num_palette_entries = num_palette_entries;
    colr_render.foreground_marker = _cairo_pattern_create_foreground_marker ();
    colr_render.foreground_source = cairo_pattern_reference (foreground_source);;
    colr_render.foreground_source_used = FALSE;
    colr_render.level = 0;

    status = draw_colr_glyph (&colr_render,
			      glyph,
			      FT_COLOR_INCLUDE_ROOT_TRANSFORM,
			      cr);
  
    cairo_pattern_destroy (colr_render.foreground_marker);
    cairo_pattern_destroy (colr_render.foreground_source);
    *foreground_source_used = colr_render.foreground_source_used;

    return status;
}

#endif /* HAVE_FT_COLR_V1 */
