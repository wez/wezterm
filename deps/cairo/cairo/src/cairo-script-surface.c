/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
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
 * The Initial Developer of the Original Code is Chris Wilson.
 *
 * Contributor(s):
 *      Chris Wilson <chris@chris-wilson.co.uk>
 */

/* The script surface is one that records all operations performed on
 * it in the form of a procedural script, similar in fashion to
 * PostScript but using Cairo's imaging model. In essence, this is
 * equivalent to the recording-surface, but as there is no impedance mismatch
 * between Cairo and CairoScript, we can generate output immediately
 * without having to copy and hold the data in memory.
 */

/**
 * SECTION:cairo-script
 * @Title: Script Surfaces
 * @Short_Description: Rendering to replayable scripts
 * @See_Also: #cairo_surface_t
 *
 * The script surface provides the ability to render to a native
 * script that matches the cairo drawing model. The scripts can
 * be replayed using tools under the util/cairo-script directory,
 * or with cairo-perf-trace.
 **/

/**
 * CAIRO_HAS_SCRIPT_SURFACE:
 *
 * Defined if the script surface backend is available.
 * The script surface backend is always built in since 1.12.
 *
 * Since: 1.12
 **/


#include "cairoint.h"

#include "cairo-script.h"
#include "cairo-script-private.h"

#include "cairo-analysis-surface-private.h"
#include "cairo-default-context-private.h"
#include "cairo-device-private.h"
#include "cairo-error-private.h"
#include "cairo-list-inline.h"
#include "cairo-image-surface-private.h"
#include "cairo-output-stream-private.h"
#include "cairo-pattern-private.h"
#include "cairo-recording-surface-inline.h"
#include "cairo-scaled-font-private.h"
#include "cairo-surface-clipper-private.h"
#include "cairo-surface-snapshot-inline.h"
#include "cairo-surface-subsurface-private.h"
#include "cairo-surface-wrapper-private.h"

#if CAIRO_HAS_FT_FONT
#include "cairo-ft-private.h"
#endif

#include <ctype.h>

#ifdef WORDS_BIGENDIAN
#define to_be32(x) x
#else
#define to_be32(x) bswap_32(x)
#endif

#define _cairo_output_stream_puts(S, STR) \
    _cairo_output_stream_write ((S), (STR), strlen (STR))

#define static cairo_warn static

typedef struct _cairo_script_context cairo_script_context_t;
typedef struct _cairo_script_surface cairo_script_surface_t;
typedef struct _cairo_script_implicit_context cairo_script_implicit_context_t;
typedef struct _cairo_script_font cairo_script_font_t;

typedef struct _operand {
    enum {
	SURFACE,
	DEFERRED,
    } type;
    cairo_list_t link;
} operand_t;


struct deferred_finish {
    cairo_list_t link;
    operand_t operand;
};

struct _cairo_script_context {
    cairo_device_t base;

    int active;
    int attach_snapshots;

    cairo_bool_t owns_stream;
    cairo_output_stream_t *stream;
    cairo_script_mode_t mode;

    struct _bitmap {
	unsigned long min;
	unsigned long count;
	unsigned int map[64];
	struct _bitmap *next;
    } surface_id, font_id;

    cairo_list_t operands;
    cairo_list_t deferred;

    cairo_list_t fonts;
    cairo_list_t defines;
};

struct _cairo_script_font {
    cairo_scaled_font_private_t base;

    cairo_bool_t has_sfnt;
    unsigned long id;
    unsigned long subset_glyph_index;
    cairo_list_t link;
    cairo_scaled_font_t *parent;
};

struct _cairo_script_implicit_context {
    cairo_operator_t current_operator;
    cairo_fill_rule_t current_fill_rule;
    double current_tolerance;
    cairo_antialias_t current_antialias;
    cairo_stroke_style_t current_style;
    cairo_pattern_union_t current_source;
    cairo_matrix_t current_ctm;
    cairo_matrix_t current_stroke_matrix;
    cairo_matrix_t current_font_matrix;
    cairo_font_options_t current_font_options;
    cairo_scaled_font_t *current_scaled_font;
    cairo_path_fixed_t current_path;
    cairo_bool_t has_clip;
};

struct _cairo_script_surface {
    cairo_surface_t base;

    cairo_surface_wrapper_t wrapper;

    cairo_surface_clipper_t clipper;

    operand_t operand;
    cairo_bool_t emitted;
    cairo_bool_t defined;
    cairo_bool_t active;

    double width, height;

    /* implicit flattened context */
    cairo_script_implicit_context_t cr;
};

static const cairo_surface_backend_t _cairo_script_surface_backend;

static cairo_script_surface_t *
_cairo_script_surface_create_internal (cairo_script_context_t *ctx,
				       cairo_content_t content,
				       cairo_rectangle_t *extents,
				       cairo_surface_t *passthrough);

static void
_cairo_script_scaled_font_fini (cairo_scaled_font_private_t *abstract_private,
				cairo_scaled_font_t *scaled_font);

static void
_cairo_script_implicit_context_init (cairo_script_implicit_context_t *cr);

static void
_cairo_script_implicit_context_reset (cairo_script_implicit_context_t *cr);

static void
_bitmap_release_id (struct _bitmap *b, unsigned long token)
{
    struct _bitmap **prev = NULL;

    do {
	if (token < b->min + sizeof (b->map) * CHAR_BIT) {
	    unsigned int bit, elem;

	    token -= b->min;
	    elem = token / (sizeof (b->map[0]) * CHAR_BIT);
	    bit  = token % (sizeof (b->map[0]) * CHAR_BIT);
	    b->map[elem] &= ~(1 << bit);
	    if (! --b->count && prev) {
		*prev = b->next;
		free (b);
	    }
	    return;
	}
	prev = &b->next;
	b = b->next;
    } while (b != NULL);
}

static cairo_status_t
_bitmap_next_id (struct _bitmap *b,
		 unsigned long *id)
{
    struct _bitmap *bb, **prev = NULL;
    unsigned long min = 0;

    do {
	if (b->min != min)
	    break;

	if (b->count < sizeof (b->map) * CHAR_BIT) {
	    unsigned int n, m, bit;
	    for (n = 0; n < ARRAY_LENGTH (b->map); n++) {
		if (b->map[n] == (unsigned int) -1)
		    continue;

		for (m=0, bit=1; m<sizeof (b->map[0])*CHAR_BIT; m++, bit<<=1) {
		    if ((b->map[n] & bit) == 0) {
			b->map[n] |= bit;
			b->count++;
			*id = n * sizeof (b->map[0])*CHAR_BIT + m + b->min;
			return CAIRO_STATUS_SUCCESS;
		    }
		}
	    }
	}
	min += sizeof (b->map) * CHAR_BIT;

	prev = &b->next;
	b = b->next;
    } while (b != NULL);
    assert (prev != NULL);

    bb = _cairo_malloc (sizeof (struct _bitmap));
    if (unlikely (bb == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    *prev = bb;
    bb->next = b;
    bb->min = min;
    bb->count = 1;
    bb->map[0] = 0x1;
    memset (bb->map + 1, 0, sizeof (bb->map) - sizeof (bb->map[0]));
    *id = min;

    return CAIRO_STATUS_SUCCESS;
}

static void
_bitmap_fini (struct _bitmap *b)
{
    while (b != NULL) {
	struct _bitmap *next = b->next;
	free (b);
	b = next;
    }
}

static const char *
_direction_to_string (cairo_bool_t backward)
{
    static const char *names[] = {
	"FORWARD",
	"BACKWARD"
    };
    assert (backward < ARRAY_LENGTH (names));
    return names[backward];
}

static const char *
_operator_to_string (cairo_operator_t op)
{
    static const char *names[] = {
	"CLEAR",	/* CAIRO_OPERATOR_CLEAR */

	"SOURCE",	/* CAIRO_OPERATOR_SOURCE */
	"OVER",		/* CAIRO_OPERATOR_OVER */
	"IN",		/* CAIRO_OPERATOR_IN */
	"OUT",		/* CAIRO_OPERATOR_OUT */
	"ATOP",		/* CAIRO_OPERATOR_ATOP */

	"DEST",		/* CAIRO_OPERATOR_DEST */
	"DEST_OVER",	/* CAIRO_OPERATOR_DEST_OVER */
	"DEST_IN",	/* CAIRO_OPERATOR_DEST_IN */
	"DEST_OUT",	/* CAIRO_OPERATOR_DEST_OUT */
	"DEST_ATOP",	/* CAIRO_OPERATOR_DEST_ATOP */

	"XOR",		/* CAIRO_OPERATOR_XOR */
	"ADD",		/* CAIRO_OPERATOR_ADD */
	"SATURATE",	/* CAIRO_OPERATOR_SATURATE */

	"MULTIPLY",	/* CAIRO_OPERATOR_MULTIPLY */
	"SCREEN",	/* CAIRO_OPERATOR_SCREEN */
	"OVERLAY",	/* CAIRO_OPERATOR_OVERLAY */
	"DARKEN",	/* CAIRO_OPERATOR_DARKEN */
	"LIGHTEN",	/* CAIRO_OPERATOR_LIGHTEN */
	"DODGE",	/* CAIRO_OPERATOR_COLOR_DODGE */
	"BURN",		/* CAIRO_OPERATOR_COLOR_BURN */
	"HARD_LIGHT",	/* CAIRO_OPERATOR_HARD_LIGHT */
	"SOFT_LIGHT",	/* CAIRO_OPERATOR_SOFT_LIGHT */
	"DIFFERENCE",	/* CAIRO_OPERATOR_DIFFERENCE */
	"EXCLUSION",	/* CAIRO_OPERATOR_EXCLUSION */
	"HSL_HUE",	/* CAIRO_OPERATOR_HSL_HUE */
	"HSL_SATURATION", /* CAIRO_OPERATOR_HSL_SATURATION */
	"HSL_COLOR",	/* CAIRO_OPERATOR_HSL_COLOR */
	"HSL_LUMINOSITY" /* CAIRO_OPERATOR_HSL_LUMINOSITY */
    };
    assert (op < ARRAY_LENGTH (names));
    return names[op];
}

static const char *
_extend_to_string (cairo_extend_t extend)
{
    static const char *names[] = {
	"EXTEND_NONE",		/* CAIRO_EXTEND_NONE */
	"EXTEND_REPEAT",	/* CAIRO_EXTEND_REPEAT */
	"EXTEND_REFLECT",	/* CAIRO_EXTEND_REFLECT */
	"EXTEND_PAD"		/* CAIRO_EXTEND_PAD */
    };
    assert (extend < ARRAY_LENGTH (names));
    return names[extend];
}

static const char *
_filter_to_string (cairo_filter_t filter)
{
    static const char *names[] = {
	"FILTER_FAST",		/* CAIRO_FILTER_FAST */
	"FILTER_GOOD",		/* CAIRO_FILTER_GOOD */
	"FILTER_BEST",		/* CAIRO_FILTER_BEST */
	"FILTER_NEAREST",	/* CAIRO_FILTER_NEAREST */
	"FILTER_BILINEAR",	/* CAIRO_FILTER_BILINEAR */
	"FILTER_GAUSSIAN",	/* CAIRO_FILTER_GAUSSIAN */
    };
    assert (filter < ARRAY_LENGTH (names));
    return names[filter];
}

static const char *
_fill_rule_to_string (cairo_fill_rule_t rule)
{
    static const char *names[] = {
	"WINDING",	/* CAIRO_FILL_RULE_WINDING */
	"EVEN_ODD"	/* CAIRO_FILL_RILE_EVEN_ODD */
    };
    assert (rule < ARRAY_LENGTH (names));
    return names[rule];
}

static const char *
_antialias_to_string (cairo_antialias_t antialias)
{
    static const char *names[] = {
	"ANTIALIAS_DEFAULT",	/* CAIRO_ANTIALIAS_DEFAULT */
	"ANTIALIAS_NONE",	/* CAIRO_ANTIALIAS_NONE */
	"ANTIALIAS_GRAY",	/* CAIRO_ANTIALIAS_GRAY */
	"ANTIALIAS_SUBPIXEL",	/* CAIRO_ANTIALIAS_SUBPIXEL */
	"ANTIALIAS_FAST",	/* CAIRO_ANTIALIAS_FAST */
	"ANTIALIAS_GOOD",	/* CAIRO_ANTIALIAS_GOOD */
	"ANTIALIAS_BEST"	/* CAIRO_ANTIALIAS_BEST */
    };
    assert (antialias < ARRAY_LENGTH (names));
    return names[antialias];
}

static const char *
_line_cap_to_string (cairo_line_cap_t line_cap)
{
    static const char *names[] = {
	"LINE_CAP_BUTT",	/* CAIRO_LINE_CAP_BUTT */
	"LINE_CAP_ROUND",	/* CAIRO_LINE_CAP_ROUND */
	"LINE_CAP_SQUARE"	/* CAIRO_LINE_CAP_SQUARE */
    };
    assert (line_cap < ARRAY_LENGTH (names));
    return names[line_cap];
}

static const char *
_line_join_to_string (cairo_line_join_t line_join)
{
    static const char *names[] = {
	"LINE_JOIN_MITER",	/* CAIRO_LINE_JOIN_MITER */
	"LINE_JOIN_ROUND",	/* CAIRO_LINE_JOIN_ROUND */
	"LINE_JOIN_BEVEL",	/* CAIRO_LINE_JOIN_BEVEL */
    };
    assert (line_join < ARRAY_LENGTH (names));
    return names[line_join];
}

static inline cairo_script_context_t *
to_context (cairo_script_surface_t *surface)
{
    return (cairo_script_context_t *) surface->base.device;
}

static cairo_bool_t
target_is_active (cairo_script_surface_t *surface)
{
    return cairo_list_is_first (&surface->operand.link,
				&to_context (surface)->operands);
}

static void
target_push (cairo_script_surface_t *surface)
{
    cairo_list_move (&surface->operand.link, &to_context (surface)->operands);
}

static int
target_depth (cairo_script_surface_t *surface)
{
    cairo_list_t *link;
    int depth = 0;

    cairo_list_foreach (link, &to_context (surface)->operands) {
	if (link == &surface->operand.link)
	    break;
	depth++;
    }

    return depth;
}

static void
_get_target (cairo_script_surface_t *surface)
{
    cairo_script_context_t *ctx = to_context (surface);

    if (target_is_active (surface)) {
	_cairo_output_stream_puts (ctx->stream, "dup ");
	return;
    }

    if (surface->defined) {
	_cairo_output_stream_printf (ctx->stream, "s%u ",
				     surface->base.unique_id);
    } else {
	int depth = target_depth (surface);

	assert (! cairo_list_is_empty (&surface->operand.link));
	assert (! target_is_active (surface));

	if (ctx->active) {
	    _cairo_output_stream_printf (ctx->stream, "%d index ", depth);
	    _cairo_output_stream_puts (ctx->stream, "/target get exch pop ");
	} else {
	    if (depth == 1) {
		_cairo_output_stream_puts (ctx->stream, "exch ");
	    } else {
		_cairo_output_stream_printf (ctx->stream,
					     "%d -1 roll ", depth);
	    }
	    target_push (surface);
	    _cairo_output_stream_puts (ctx->stream, "dup ");
	}
    }
}

static const char *
_content_to_string (cairo_content_t content)
{
    switch (content) {
    case CAIRO_CONTENT_ALPHA: return "ALPHA";
    case CAIRO_CONTENT_COLOR: return "COLOR";
    default:
    case CAIRO_CONTENT_COLOR_ALPHA: return "COLOR_ALPHA";
    }
}

static cairo_status_t
_emit_surface (cairo_script_surface_t *surface)
{
    cairo_script_context_t *ctx = to_context (surface);

    _cairo_output_stream_printf (ctx->stream,
				 "<< /content //%s",
				 _content_to_string (surface->base.content));
    if (surface->width != -1 && surface->height != -1) {
	_cairo_output_stream_printf (ctx->stream,
				     " /width %f /height %f",
				     surface->width,
				     surface->height);
    }

    if (surface->base.x_fallback_resolution !=
	CAIRO_SURFACE_FALLBACK_RESOLUTION_DEFAULT ||
	surface->base.y_fallback_resolution !=
	CAIRO_SURFACE_FALLBACK_RESOLUTION_DEFAULT)
    {
	_cairo_output_stream_printf (ctx->stream,
				     " /fallback-resolution [%f %f]",
				     surface->base.x_fallback_resolution,
				     surface->base.y_fallback_resolution);
    }

    if (surface->base.device_transform.x0 != 0. ||
	surface->base.device_transform.y0 != 0.)
    {
	/* XXX device offset is encoded into the pattern matrices etc. */
	if (0) {
	_cairo_output_stream_printf (ctx->stream,
				     " /device-offset [%f %f]",
				     surface->base.device_transform.x0,
				     surface->base.device_transform.y0);
	}
    }

    _cairo_output_stream_puts (ctx->stream, " >> surface context\n");
    surface->emitted = TRUE;
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_context (cairo_script_surface_t *surface)
{
    cairo_script_context_t *ctx = to_context (surface);

    if (target_is_active (surface))
	return CAIRO_STATUS_SUCCESS;

    while (! cairo_list_is_empty (&ctx->operands)) {
	operand_t *op;
	cairo_script_surface_t *old;

	op = cairo_list_first_entry (&ctx->operands,
				     operand_t,
				     link);
	if (op->type == DEFERRED)
	    break;

	old = cairo_container_of (op, cairo_script_surface_t, operand);
	if (old == surface)
	    break;
	if (old->active)
	    break;

	if (! old->defined) {
	    assert (old->emitted);
	    _cairo_output_stream_printf (ctx->stream,
					 "/target get /s%u exch def pop\n",
					 old->base.unique_id);
	    old->defined = TRUE;
	} else {
	    _cairo_output_stream_puts (ctx->stream, "pop\n");
	}

	cairo_list_del (&old->operand.link);
    }

    if (target_is_active (surface))
	return CAIRO_STATUS_SUCCESS;

    if (! surface->emitted) {
	cairo_status_t status;

	status = _emit_surface (surface);
	if (unlikely (status))
	    return status;
    } else if (cairo_list_is_empty (&surface->operand.link)) {
	assert (surface->defined);
	_cairo_output_stream_printf (ctx->stream,
				     "s%u context\n",
				     surface->base.unique_id);
	_cairo_script_implicit_context_reset (&surface->cr);
	_cairo_surface_clipper_reset (&surface->clipper);
    } else {
	int depth = target_depth (surface);
	if (depth == 1) {
	    _cairo_output_stream_puts (ctx->stream, "exch\n");
	} else {
	    _cairo_output_stream_printf (ctx->stream,
					 "%d -1 roll\n",
					 depth);
	}
    }
    target_push (surface);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_operator (cairo_script_surface_t *surface,
		cairo_operator_t op)
{
    assert (target_is_active (surface));

    if (surface->cr.current_operator == op)
	return CAIRO_STATUS_SUCCESS;

    surface->cr.current_operator = op;

    _cairo_output_stream_printf (to_context (surface)->stream,
				 "//%s set-operator\n",
				 _operator_to_string (op));
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_fill_rule (cairo_script_surface_t *surface,
		 cairo_fill_rule_t fill_rule)
{
    assert (target_is_active (surface));

    if (surface->cr.current_fill_rule == fill_rule)
	return CAIRO_STATUS_SUCCESS;

    surface->cr.current_fill_rule = fill_rule;

    _cairo_output_stream_printf (to_context (surface)->stream,
				 "//%s set-fill-rule\n",
				 _fill_rule_to_string (fill_rule));
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_tolerance (cairo_script_surface_t *surface,
		 double tolerance,
		 cairo_bool_t force)
{
    assert (target_is_active (surface));

    if ((! force ||
	 fabs (tolerance - CAIRO_GSTATE_TOLERANCE_DEFAULT) < 1e-5) &&
	surface->cr.current_tolerance == tolerance)
    {
	return CAIRO_STATUS_SUCCESS;
    }

    surface->cr.current_tolerance = tolerance;

    _cairo_output_stream_printf (to_context (surface)->stream,
				 "%f set-tolerance\n",
				 tolerance);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_antialias (cairo_script_surface_t *surface,
		 cairo_antialias_t antialias)
{
    assert (target_is_active (surface));

    if (surface->cr.current_antialias == antialias)
	return CAIRO_STATUS_SUCCESS;

    surface->cr.current_antialias = antialias;

    _cairo_output_stream_printf (to_context (surface)->stream,
				 "//%s set-antialias\n",
				 _antialias_to_string (antialias));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_line_width (cairo_script_surface_t *surface,
		 double line_width,
		 cairo_bool_t force)
{
    assert (target_is_active (surface));

    if ((! force ||
	 fabs (line_width - CAIRO_GSTATE_LINE_WIDTH_DEFAULT) < 1e-5) &&
	surface->cr.current_style.line_width == line_width)
    {
	return CAIRO_STATUS_SUCCESS;
    }

    surface->cr.current_style.line_width = line_width;

    _cairo_output_stream_printf (to_context (surface)->stream,
				 "%f set-line-width\n",
				 line_width);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_hairline (cairo_script_surface_t *surface, cairo_bool_t set_hairline)
{
    assert (target_is_active (surface));

    if (surface->cr.current_style.is_hairline == set_hairline)
    {
	return CAIRO_STATUS_SUCCESS;
    }

    surface->cr.current_style.is_hairline = set_hairline;

    _cairo_output_stream_printf (to_context (surface)->stream, 
					"%d set-hairline\n",
					set_hairline);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_line_cap (cairo_script_surface_t *surface,
		cairo_line_cap_t line_cap)
{
    assert (target_is_active (surface));

    if (surface->cr.current_style.line_cap == line_cap)
	return CAIRO_STATUS_SUCCESS;

    surface->cr.current_style.line_cap = line_cap;

    _cairo_output_stream_printf (to_context (surface)->stream,
				 "//%s set-line-cap\n",
				 _line_cap_to_string (line_cap));
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_line_join (cairo_script_surface_t *surface,
		 cairo_line_join_t line_join)
{
    assert (target_is_active (surface));

    if (surface->cr.current_style.line_join == line_join)
	return CAIRO_STATUS_SUCCESS;

    surface->cr.current_style.line_join = line_join;

    _cairo_output_stream_printf (to_context (surface)->stream,
				 "//%s set-line-join\n",
				 _line_join_to_string (line_join));
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_miter_limit (cairo_script_surface_t *surface,
		   double miter_limit,
		   cairo_bool_t force)
{
    assert (target_is_active (surface));

    if ((! force ||
	 fabs (miter_limit - CAIRO_GSTATE_MITER_LIMIT_DEFAULT) < 1e-5) &&
	surface->cr.current_style.miter_limit == miter_limit)
    {
	return CAIRO_STATUS_SUCCESS;
    }

    surface->cr.current_style.miter_limit = miter_limit;

    _cairo_output_stream_printf (to_context (surface)->stream,
				 "%f set-miter-limit\n",
				 miter_limit);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
_dashes_equal (const double *a, const double *b, int num_dashes)
{
    while (num_dashes--) {
	if (fabs (*a - *b) > 1e-5)
	    return FALSE;
	a++, b++;
    }

    return TRUE;
}

static cairo_status_t
_emit_dash (cairo_script_surface_t *surface,
	    const double *dash,
	    unsigned int num_dashes,
	    double offset,
	    cairo_bool_t force)
{
    unsigned int n;

    assert (target_is_active (surface));

    if (force &&
	num_dashes == 0 &&
	surface->cr.current_style.num_dashes == 0)
    {
	return CAIRO_STATUS_SUCCESS;
    }

    if (! force &&
	(surface->cr.current_style.num_dashes == num_dashes &&
	 (num_dashes == 0 ||
	  (fabs (surface->cr.current_style.dash_offset - offset) < 1e-5 &&
	   _dashes_equal (surface->cr.current_style.dash, dash, num_dashes)))))
    {
	return CAIRO_STATUS_SUCCESS;
    }


    if (num_dashes) {
	surface->cr.current_style.dash = _cairo_realloc_ab
	    (surface->cr.current_style.dash, num_dashes, sizeof (double));
	if (unlikely (surface->cr.current_style.dash == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	memcpy (surface->cr.current_style.dash, dash,
		sizeof (double) * num_dashes);
    } else {
	free (surface->cr.current_style.dash);
	surface->cr.current_style.dash = NULL;
    }

    surface->cr.current_style.num_dashes = num_dashes;
    surface->cr.current_style.dash_offset = offset;

    _cairo_output_stream_puts (to_context (surface)->stream, "[");
    for (n = 0; n < num_dashes; n++) {
	_cairo_output_stream_printf (to_context (surface)->stream, "%f", dash[n]);
	if (n < num_dashes-1)
	    _cairo_output_stream_puts (to_context (surface)->stream, " ");
    }
    _cairo_output_stream_printf (to_context (surface)->stream,
				 "] %f set-dash\n",
				 offset);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_stroke_style (cairo_script_surface_t *surface,
		    const cairo_stroke_style_t *style,
		    cairo_bool_t force)
{
    cairo_status_t status;

    assert (target_is_active (surface));

    status = _emit_line_width (surface, style->line_width, force);
    if (unlikely (status))
	return status;

    status = _emit_line_cap (surface, style->line_cap);
    if (unlikely (status))
	return status;

    status = _emit_line_join (surface, style->line_join);
    if (unlikely (status))
	return status;

    status = _emit_miter_limit (surface, style->miter_limit, force);
    if (unlikely (status))
	return status;

    status = _emit_hairline (surface, style->is_hairline);
    if (unlikely (status))
	return status;

    status = _emit_dash (surface,
			 style->dash, style->num_dashes, style->dash_offset,
			 force);
    if (unlikely (status))
	return status;

    return CAIRO_STATUS_SUCCESS;
}

static const char *
_format_to_string (cairo_format_t format)
{
    switch (format) {
    case CAIRO_FORMAT_RGBA128F: return "RGBA128F";
    case CAIRO_FORMAT_RGB96F: return "RGB96F";
    case CAIRO_FORMAT_ARGB32:  return "ARGB32";
    case CAIRO_FORMAT_RGB30:   return "RGB30";
    case CAIRO_FORMAT_RGB24:   return "RGB24";
    case CAIRO_FORMAT_RGB16_565: return "RGB16_565";
    case CAIRO_FORMAT_A8:      return "A8";
    case CAIRO_FORMAT_A1:      return "A1";
    case CAIRO_FORMAT_INVALID: return "INVALID";
    }
    ASSERT_NOT_REACHED;
    return "INVALID";
}

static cairo_status_t
_emit_solid_pattern (cairo_script_surface_t *surface,
		     const cairo_pattern_t *pattern)
{
    cairo_solid_pattern_t *solid = (cairo_solid_pattern_t *) pattern;
    cairo_script_context_t *ctx = to_context (surface);

    if (! CAIRO_COLOR_IS_OPAQUE (&solid->color))
    {
	if (! (surface->base.content & CAIRO_CONTENT_COLOR) ||
	    ((solid->color.red_short   == 0 || solid->color.red_short   == 0xffff) &&
	     (solid->color.green_short == 0 || solid->color.green_short == 0xffff) &&
	     (solid->color.blue_short  == 0 || solid->color.blue_short  == 0xffff) ))
	{
	    _cairo_output_stream_printf (ctx->stream,
					 "%f a",
					 solid->color.alpha);
	}
	else
	{
	    _cairo_output_stream_printf (ctx->stream,
					 "%f %f %f %f rgba",
					 solid->color.red,
					 solid->color.green,
					 solid->color.blue,
					 solid->color.alpha);
	}
    }
    else
    {
	if (solid->color.red_short == solid->color.green_short &&
	    solid->color.red_short == solid->color.blue_short)
	{
	    _cairo_output_stream_printf (ctx->stream,
					 "%f g",
					 solid->color.red);
	}
	else
	{
	    _cairo_output_stream_printf (ctx->stream,
					 "%f %f %f rgb",
					 solid->color.red,
					 solid->color.green,
					 solid->color.blue);
	}
    }

    return CAIRO_STATUS_SUCCESS;
}


static cairo_status_t
_emit_gradient_color_stops (cairo_gradient_pattern_t *gradient,
			    cairo_output_stream_t *output)
{
    unsigned int n;

    for (n = 0; n < gradient->n_stops; n++) {
	_cairo_output_stream_printf (output,
				     "\n  %f %f %f %f %f add-color-stop",
				     gradient->stops[n].offset,
				     gradient->stops[n].color.red,
				     gradient->stops[n].color.green,
				     gradient->stops[n].color.blue,
				     gradient->stops[n].color.alpha);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_linear_pattern (cairo_script_surface_t *surface,
		      const cairo_pattern_t *pattern)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_linear_pattern_t *linear;

    linear = (cairo_linear_pattern_t *) pattern;

    _cairo_output_stream_printf (ctx->stream,
				 "%f %f %f %f linear",
				 linear->pd1.x, linear->pd1.y,
				 linear->pd2.x, linear->pd2.y);
    return _emit_gradient_color_stops (&linear->base, ctx->stream);
}

static cairo_status_t
_emit_radial_pattern (cairo_script_surface_t *surface,
		      const cairo_pattern_t *pattern)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_radial_pattern_t *radial;

    radial = (cairo_radial_pattern_t *) pattern;

    _cairo_output_stream_printf (ctx->stream,
				 "%f %f %f %f %f %f radial",
				 radial->cd1.center.x,
				 radial->cd1.center.y,
				 radial->cd1.radius,
				 radial->cd2.center.x,
				 radial->cd2.center.y,
				 radial->cd2.radius);
    return _emit_gradient_color_stops (&radial->base, ctx->stream);
}

static cairo_status_t
_emit_mesh_pattern (cairo_script_surface_t *surface,
		    const cairo_pattern_t *pattern)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_pattern_t *mesh;
    cairo_status_t status;
    unsigned int i, n;

    mesh = (cairo_pattern_t *) pattern;
    status = cairo_mesh_pattern_get_patch_count (mesh, &n);
    if (unlikely (status))
	return status;

    _cairo_output_stream_printf (ctx->stream, "mesh");
    for (i = 0; i < n; i++) {
	cairo_path_t *path;
	cairo_path_data_t *data;
	int j;

	_cairo_output_stream_printf (ctx->stream, "\n  begin-patch");

	path = cairo_mesh_pattern_get_path (mesh, i);
	if (unlikely (path->status))
	    return path->status;

	for (j = 0; j < path->num_data; j+=data[0].header.length) {
	    data = &path->data[j];
	    switch (data->header.type) {
	    case CAIRO_PATH_MOVE_TO:
		_cairo_output_stream_printf (ctx->stream,
					     "\n  %f %f m",
					     data[1].point.x, data[1].point.y);
		break;
	    case CAIRO_PATH_LINE_TO:
		_cairo_output_stream_printf (ctx->stream,
					     "\n  %f %f l",
					     data[1].point.x, data[1].point.y);
		break;
	    case CAIRO_PATH_CURVE_TO:
		_cairo_output_stream_printf (ctx->stream,
					     "\n  %f %f %f %f %f %f c",
					     data[1].point.x, data[1].point.y,
					     data[2].point.x, data[2].point.y,
					     data[3].point.x, data[3].point.y);
		break;
	    case CAIRO_PATH_CLOSE_PATH:
		break;
	    }
	}
	cairo_path_destroy (path);

	for (j = 0; j < 4; j++) {
	    double x, y;

	    status = cairo_mesh_pattern_get_control_point (mesh, i, j, &x, &y);
	    if (unlikely (status))
		return status;
	    _cairo_output_stream_printf (ctx->stream,
					 "\n  %d %f %f set-control-point",
					 j, x, y);
	}

	for (j = 0; j < 4; j++) {
	    double r, g, b, a;

	    status = cairo_mesh_pattern_get_corner_color_rgba (mesh, i, j, &r, &g, &b, &a);
	    if (unlikely (status))
		return status;

	    _cairo_output_stream_printf (ctx->stream,
					 "\n  %d %f %f %f %f set-corner-color",
					 j, r, g, b, a);
	}

	_cairo_output_stream_printf (ctx->stream, "\n  end-patch");
    }

    return CAIRO_STATUS_SUCCESS;
}

struct script_snapshot {
    cairo_surface_t base;
};

static cairo_status_t
script_snapshot_finish (void *abstract_surface)
{
    return CAIRO_STATUS_SUCCESS;
}

static const cairo_surface_backend_t script_snapshot_backend = {
    CAIRO_SURFACE_TYPE_SCRIPT,
    script_snapshot_finish,
};

static void
detach_snapshot (cairo_surface_t *abstract_surface)
{
    cairo_script_surface_t *surface = (cairo_script_surface_t *)abstract_surface;
    cairo_script_context_t *ctx = to_context (surface);

    _cairo_output_stream_printf (ctx->stream,
				 "/s%d undef\n",
				 surface->base.unique_id);
}

static void
attach_snapshot (cairo_script_context_t *ctx,
		 cairo_surface_t *source)
{
    struct script_snapshot *surface;

    if (! ctx->attach_snapshots)
	return;

    surface = _cairo_malloc (sizeof (*surface));
    if (unlikely (surface == NULL))
	return;

    _cairo_surface_init (&surface->base,
			 &script_snapshot_backend,
			 &ctx->base,
			 source->content,
			 source->is_vector);

    _cairo_output_stream_printf (ctx->stream,
				 "dup /s%d exch def ",
				 surface->base.unique_id);

    _cairo_surface_attach_snapshot (source, &surface->base, detach_snapshot);
    cairo_surface_destroy (&surface->base);
}

static cairo_status_t
_emit_recording_surface_pattern (cairo_script_surface_t *surface,
				 cairo_recording_surface_t *source)
{
    cairo_script_implicit_context_t old_cr;
    cairo_script_context_t *ctx = to_context (surface);
    cairo_script_surface_t *similar;
    cairo_surface_t *snapshot;
    cairo_rectangle_t r, *extents;
    cairo_status_t status;

    snapshot = _cairo_surface_has_snapshot (&source->base, &script_snapshot_backend);
    if (snapshot) {
	_cairo_output_stream_printf (ctx->stream, "s%d", snapshot->unique_id);
	return CAIRO_INT_STATUS_SUCCESS;
    }

    extents = NULL;
    if (_cairo_recording_surface_get_bounds (&source->base, &r))
	extents = &r;

    similar = _cairo_script_surface_create_internal (ctx,
						     source->base.content,
						     extents,
						     NULL);
    if (unlikely (similar->base.status))
	return similar->base.status;

    similar->base.is_clear = TRUE;

    _cairo_output_stream_printf (ctx->stream, "//%s ",
				 _content_to_string (source->base.content));
    if (extents) {
	_cairo_output_stream_printf (ctx->stream, "[%f %f %f %f]",
				     extents->x, extents->y,
				     extents->width, extents->height);
    } else
	_cairo_output_stream_puts (ctx->stream, "[]");
    _cairo_output_stream_puts (ctx->stream, " record\n");

    attach_snapshot (ctx, &source->base);

    _cairo_output_stream_puts (ctx->stream, "dup context\n");

    target_push (similar);
    similar->emitted = TRUE;


    old_cr = surface->cr;
    _cairo_script_implicit_context_init (&surface->cr);
    status = _cairo_recording_surface_replay (&source->base, &similar->base);
    surface->cr = old_cr;

    if (unlikely (status)) {
	cairo_surface_destroy (&similar->base);
	return status;
    }

    cairo_list_del (&similar->operand.link);
    assert (target_is_active (surface));

    _cairo_output_stream_puts (ctx->stream, "pop ");
    cairo_surface_destroy (&similar->base);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_script_surface_pattern (cairo_script_surface_t *surface,
			      cairo_script_surface_t *source)
{
    _get_target (source);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_write_image_surface (cairo_output_stream_t *output,
		      const cairo_image_surface_t *image)
{
    int row, width;
    ptrdiff_t stride;
    uint8_t row_stack[CAIRO_STACK_BUFFER_SIZE];
    uint8_t *rowdata;
    uint8_t *data;

    stride = image->stride;
    width = image->width;
    data = image->data;
#if WORDS_BIGENDIAN
    switch (image->format) {
    case CAIRO_FORMAT_A1:
	for (row = image->height; row--; ) {
	    _cairo_output_stream_write (output, data, (width+7)/8);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_A8:
	for (row = image->height; row--; ) {
	    _cairo_output_stream_write (output, data, width);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_RGB16_565:
	for (row = image->height; row--; ) {
	    _cairo_output_stream_write (output, data, 2*width);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_RGB24:
	for (row = image->height; row--; ) {
	    int col;
	    rowdata = data;
	    for (col = width; col--; ) {
		_cairo_output_stream_write (output, rowdata, 3);
		rowdata+=4;
	    }
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_ARGB32:
	for (row = image->height; row--; ) {
	    _cairo_output_stream_write (output, data, 4*width);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_INVALID:
    default:
	ASSERT_NOT_REACHED;
	break;
    }
#else
    if (stride > ARRAY_LENGTH (row_stack)) {
	rowdata = _cairo_malloc (stride);
	if (unlikely (rowdata == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    } else
	rowdata = row_stack;

    switch (image->format) {
    case CAIRO_FORMAT_A1:
	for (row = image->height; row--; ) {
	    int col;
	    for (col = 0; col < (width + 7)/8; col++)
		rowdata[col] = CAIRO_BITSWAP8 (data[col]);
	    _cairo_output_stream_write (output, rowdata, (width+7)/8);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_A8:
	for (row = image->height; row--; ) {
	    _cairo_output_stream_write (output, data, width);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_RGB16_565:
	for (row = image->height; row--; ) {
	    uint16_t *src = (uint16_t *) data;
	    uint16_t *dst = (uint16_t *) rowdata;
	    int col;
	    for (col = 0; col < width; col++)
		dst[col] = bswap_16 (src[col]);
	    _cairo_output_stream_write (output, rowdata, 2*width);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_RGB24:
	for (row = image->height; row--; ) {
	    uint8_t *src = data;
	    int col;
	    for (col = 0; col < width; col++) {
		rowdata[3*col+2] = *src++;
		rowdata[3*col+1] = *src++;
		rowdata[3*col+0] = *src++;
		src++;
	    }
	    _cairo_output_stream_write (output, rowdata, 3*width);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_RGB30:
    case CAIRO_FORMAT_ARGB32:
	for (row = image->height; row--; ) {
	    uint32_t *src = (uint32_t *) data;
	    uint32_t *dst = (uint32_t *) rowdata;
	    int col;
	    for (col = 0; col < width; col++)
		dst[col] = bswap_32 (src[col]);
	    _cairo_output_stream_write (output, rowdata, 4*width);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_RGB96F:
	for (row = image->height; row--; ) {
	    _cairo_output_stream_write (output, data, 12*width);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_RGBA128F:
	for (row = image->height; row--; ) {
	    _cairo_output_stream_write (output, data, 16*width);
	    data += stride;
	}
	break;
    case CAIRO_FORMAT_INVALID:
    default:
	ASSERT_NOT_REACHED;
	break;
    }
    if (rowdata != row_stack)
	free (rowdata);
#endif

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_emit_png_surface (cairo_script_surface_t *surface,
		   cairo_image_surface_t *image)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_output_stream_t *base85_stream;
    cairo_status_t status;
    const uint8_t *mime_data;
    unsigned long mime_data_length;

    cairo_surface_get_mime_data (&image->base, CAIRO_MIME_TYPE_PNG,
				 &mime_data, &mime_data_length);
    if (mime_data == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    _cairo_output_stream_printf (ctx->stream,
				 "<< "
				 "/width %d "
				 "/height %d "
				 "/format //%s "
				 "/mime-type (image/png) "
				 "/source <~",
				 image->width, image->height,
				 _format_to_string (image->format));

    base85_stream = _cairo_base85_stream_create (ctx->stream);
    _cairo_output_stream_write (base85_stream, mime_data, mime_data_length);
    status = _cairo_output_stream_destroy (base85_stream);
    if (unlikely (status))
	return status;

    _cairo_output_stream_puts (ctx->stream, "~> >> image ");
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_emit_image_surface (cairo_script_surface_t *surface,
		     cairo_image_surface_t *image)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_output_stream_t *base85_stream;
    cairo_output_stream_t *zlib_stream;
    cairo_int_status_t status, status2;
    cairo_surface_t *snapshot;
    const uint8_t *mime_data;
    unsigned long mime_data_length;

    snapshot = _cairo_surface_has_snapshot (&image->base,
					    &script_snapshot_backend);
    if (snapshot) {
	_cairo_output_stream_printf (ctx->stream, "s%u ", snapshot->unique_id);
	return CAIRO_INT_STATUS_SUCCESS;
    }

    status = _emit_png_surface (surface, image);
    if (_cairo_int_status_is_error (status)) {
	return status;
    } else if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	cairo_image_surface_t *clone;
	uint32_t len;

	if (image->format == CAIRO_FORMAT_INVALID) {
	    clone = _cairo_image_surface_coerce (image);
	} else {
	    clone = (cairo_image_surface_t *)
		cairo_surface_reference (&image->base);
	}

	_cairo_output_stream_printf (ctx->stream,
				     "<< "
				     "/width %d "
				     "/height %d "
				     "/format //%s "
				     "/source ",
				     clone->width, clone->height,
				     _format_to_string (clone->format));

	switch (clone->format) {
	case CAIRO_FORMAT_A1:
	    len = (clone->width + 7)/8;
	    break;
	case CAIRO_FORMAT_A8:
	    len = clone->width;
	    break;
	case CAIRO_FORMAT_RGB16_565:
	    len = clone->width * 2;
	    break;
	case CAIRO_FORMAT_RGB24:
	    len = clone->width * 3;
	    break;
	case CAIRO_FORMAT_RGB30:
	case CAIRO_FORMAT_ARGB32:
	    len = clone->width * 4;
	    break;
	case CAIRO_FORMAT_RGB96F:
	    len = clone->width * 12;
	    break;
	case CAIRO_FORMAT_RGBA128F:
	    len = clone->width * 16;
	    break;
	case CAIRO_FORMAT_INVALID:
	default:
	    ASSERT_NOT_REACHED;
	    len = 0;
	    break;
	}
	len *= clone->height;

	if (len > 24) {
	    _cairo_output_stream_puts (ctx->stream, "<|");

	    base85_stream = _cairo_base85_stream_create (ctx->stream);

	    len = to_be32 (len);
	    _cairo_output_stream_write (base85_stream, &len, sizeof (len));

	    zlib_stream = _cairo_deflate_stream_create (base85_stream);
	    status = _write_image_surface (zlib_stream, clone);

	    status2 = _cairo_output_stream_destroy (zlib_stream);
	    if (status == CAIRO_INT_STATUS_SUCCESS)
		status = status2;
	    status2 = _cairo_output_stream_destroy (base85_stream);
	    if (status == CAIRO_INT_STATUS_SUCCESS)
		status = status2;
	    if (unlikely (status))
		return status;
	} else {
	    _cairo_output_stream_puts (ctx->stream, "<~");

	    base85_stream = _cairo_base85_stream_create (ctx->stream);
	    status = _write_image_surface (base85_stream, clone);
	    status2 = _cairo_output_stream_destroy (base85_stream);
	    if (status == CAIRO_INT_STATUS_SUCCESS)
		status = status2;
	    if (unlikely (status))
		return status;
	}
	_cairo_output_stream_puts (ctx->stream, "~> >> image ");

	cairo_surface_destroy (&clone->base);
    }

    cairo_surface_get_mime_data (&image->base, CAIRO_MIME_TYPE_JPEG,
				 &mime_data, &mime_data_length);
    if (mime_data != NULL) {
	_cairo_output_stream_printf (ctx->stream,
				     "\n  (%s) <~",
				     CAIRO_MIME_TYPE_JPEG);

	base85_stream = _cairo_base85_stream_create (ctx->stream);
	_cairo_output_stream_write (base85_stream, mime_data, mime_data_length);
	status = _cairo_output_stream_destroy (base85_stream);
	if (unlikely (status))
	    return status;

	_cairo_output_stream_puts (ctx->stream, "~> set-mime-data\n");
    }

    cairo_surface_get_mime_data (&image->base, CAIRO_MIME_TYPE_JP2,
				 &mime_data, &mime_data_length);
    if (mime_data != NULL) {
	_cairo_output_stream_printf (ctx->stream,
				     "\n  (%s) <~",
				     CAIRO_MIME_TYPE_JP2);

	base85_stream = _cairo_base85_stream_create (ctx->stream);
	_cairo_output_stream_write (base85_stream, mime_data, mime_data_length);
	status = _cairo_output_stream_destroy (base85_stream);
	if (unlikely (status))
	    return status;

	_cairo_output_stream_puts (ctx->stream, "~> set-mime-data\n");
    }

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_emit_image_surface_pattern (cairo_script_surface_t *surface,
			     cairo_surface_t *source)
{
    cairo_image_surface_t *image;
    cairo_status_t status;
    void *extra;

    status = _cairo_surface_acquire_source_image (source, &image, &extra);
    if (likely (status == CAIRO_STATUS_SUCCESS)) {
	status = _emit_image_surface (surface, image);
	_cairo_surface_release_source_image (source, image, extra);
    }

    return status;
}

static cairo_int_status_t
_emit_subsurface_pattern (cairo_script_surface_t *surface,
			  cairo_surface_subsurface_t *sub)
{
    cairo_surface_t *source = sub->target;
    cairo_int_status_t status;

    switch ((int) source->backend->type) {
    case CAIRO_SURFACE_TYPE_RECORDING:
	status = _emit_recording_surface_pattern (surface, (cairo_recording_surface_t *) source);
	break;
    case CAIRO_SURFACE_TYPE_SCRIPT:
	status = _emit_script_surface_pattern (surface, (cairo_script_surface_t *) source);
	break;
    default:
	status = _emit_image_surface_pattern (surface, source);
	break;
    }
    if (unlikely (status))
	return status;

    _cairo_output_stream_printf (to_context (surface)->stream,
				 "%d %d %d %d subsurface ",
				 sub->extents.x,
				 sub->extents.y,
				 sub->extents.width,
				 sub->extents.height);
    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_emit_surface_pattern (cairo_script_surface_t *surface,
		       const cairo_pattern_t *pattern)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_surface_pattern_t *surface_pattern;
    cairo_surface_t *source, *snapshot, *free_me = NULL;
    cairo_surface_t *take_snapshot = NULL;
    cairo_int_status_t status;

    surface_pattern = (cairo_surface_pattern_t *) pattern;
    source = surface_pattern->surface;

    if (_cairo_surface_is_snapshot (source)) {
	snapshot = _cairo_surface_has_snapshot (source, &script_snapshot_backend);
	if (snapshot) {
	    _cairo_output_stream_printf (ctx->stream,
					 "s%d pattern ",
					 snapshot->unique_id);
	    return CAIRO_INT_STATUS_SUCCESS;
	}

	if (_cairo_surface_snapshot_is_reused (source))
	    take_snapshot = source;

	free_me = source = _cairo_surface_snapshot_get_target (source);
    }

    switch ((int) source->backend->type) {
    case CAIRO_SURFACE_TYPE_RECORDING:
	status = _emit_recording_surface_pattern (surface, (cairo_recording_surface_t *) source);
	break;
    case CAIRO_SURFACE_TYPE_SCRIPT:
	status = _emit_script_surface_pattern (surface, (cairo_script_surface_t *) source);
	break;
    case CAIRO_SURFACE_TYPE_SUBSURFACE:
	status = _emit_subsurface_pattern (surface, (cairo_surface_subsurface_t *) source);
	break;
    default:
	status = _emit_image_surface_pattern (surface, source);
	break;
    }
    cairo_surface_destroy (free_me);
    if (unlikely (status))
	return status;

    if (take_snapshot)
	attach_snapshot (ctx, take_snapshot);

    _cairo_output_stream_puts (ctx->stream, "pattern");
    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_emit_raster_pattern (cairo_script_surface_t *surface,
		      const cairo_pattern_t *pattern)
{
    cairo_surface_t *source;
    cairo_int_status_t status;

    source = _cairo_raster_source_pattern_acquire (pattern, &surface->base, NULL);
    if (unlikely (source == NULL)) {
	ASSERT_NOT_REACHED;
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }
    if (unlikely (source->status))
	return source->status;

    status = _emit_image_surface_pattern (surface, source);
    _cairo_raster_source_pattern_release (pattern, source);
    if (unlikely (status))
	return status;

    _cairo_output_stream_puts (to_context(surface)->stream, "pattern");
    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_emit_pattern (cairo_script_surface_t *surface,
	       const cairo_pattern_t *pattern)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_int_status_t status;
    cairo_bool_t is_default_extend;
    cairo_bool_t need_newline = TRUE;

    switch (pattern->type) {
    case CAIRO_PATTERN_TYPE_SOLID:
	/* solid colors do not need filter/extend/matrix */
	return _emit_solid_pattern (surface, pattern);

    case CAIRO_PATTERN_TYPE_LINEAR:
	status = _emit_linear_pattern (surface, pattern);
	is_default_extend = pattern->extend == CAIRO_EXTEND_GRADIENT_DEFAULT;
	break;
    case CAIRO_PATTERN_TYPE_RADIAL:
	status = _emit_radial_pattern (surface, pattern);
	is_default_extend = pattern->extend == CAIRO_EXTEND_GRADIENT_DEFAULT;
	break;
    case CAIRO_PATTERN_TYPE_MESH:
	status = _emit_mesh_pattern (surface, pattern);
	is_default_extend = TRUE;
	break;
    case CAIRO_PATTERN_TYPE_SURFACE:
	status = _emit_surface_pattern (surface, pattern);
	is_default_extend = pattern->extend == CAIRO_EXTEND_SURFACE_DEFAULT;
	break;
    case CAIRO_PATTERN_TYPE_RASTER_SOURCE:
	status = _emit_raster_pattern (surface, pattern);
	is_default_extend = pattern->extend == CAIRO_EXTEND_SURFACE_DEFAULT;
	break;

    default:
	ASSERT_NOT_REACHED;
	status = CAIRO_INT_STATUS_UNSUPPORTED;
    }
    if (unlikely (status))
	return status;

    if (! _cairo_matrix_is_identity (&pattern->matrix)) {
	if (need_newline) {
	    _cairo_output_stream_puts (ctx->stream, "\n ");
	    need_newline = FALSE;
	}

	_cairo_output_stream_printf (ctx->stream,
				     " [%f %f %f %f %f %f] set-matrix\n ",
				     pattern->matrix.xx, pattern->matrix.yx,
				     pattern->matrix.xy, pattern->matrix.yy,
				     pattern->matrix.x0, pattern->matrix.y0);
    }

    /* XXX need to discriminate the user explicitly setting the default */
    if (pattern->filter != CAIRO_FILTER_DEFAULT) {
	if (need_newline) {
	    _cairo_output_stream_puts (ctx->stream, "\n ");
	    need_newline = FALSE;
	}

	_cairo_output_stream_printf (ctx->stream,
				     " //%s set-filter\n ",
				     _filter_to_string (pattern->filter));
    }
    if (! is_default_extend ){
	if (need_newline) {
	    _cairo_output_stream_puts (ctx->stream, "\n ");
	    need_newline = FALSE;
	}

	_cairo_output_stream_printf (ctx->stream,
				     " //%s set-extend\n ",
				     _extend_to_string (pattern->extend));
    }

    if (need_newline)
	_cairo_output_stream_puts (ctx->stream, "\n ");

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_emit_identity (cairo_script_surface_t *surface,
		cairo_bool_t *matrix_updated)
{
    assert (target_is_active (surface));

    if (_cairo_matrix_is_identity (&surface->cr.current_ctm))
	return CAIRO_INT_STATUS_SUCCESS;

    _cairo_output_stream_puts (to_context (surface)->stream,
			       "identity set-matrix\n");

    *matrix_updated = TRUE;
    cairo_matrix_init_identity (&surface->cr.current_ctm);

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_emit_source (cairo_script_surface_t *surface,
	      cairo_operator_t op,
	      const cairo_pattern_t *source)
{
    cairo_bool_t matrix_updated = FALSE;
    cairo_int_status_t status;

    assert (target_is_active (surface));

    if (op == CAIRO_OPERATOR_CLEAR) {
	/* the source is ignored, so don't change it */
	return CAIRO_INT_STATUS_SUCCESS;
    }

    if (_cairo_pattern_equal (&surface->cr.current_source.base, source))
	return CAIRO_INT_STATUS_SUCCESS;

    _cairo_pattern_fini (&surface->cr.current_source.base);
    status = _cairo_pattern_init_copy (&surface->cr.current_source.base,
				       source);
    if (unlikely (status))
	return status;

    status = _emit_identity (surface, &matrix_updated);
    if (unlikely (status))
	return status;

    status = _emit_pattern (surface, source);
    if (unlikely (status))
	return status;

    assert (target_is_active (surface));
    _cairo_output_stream_puts (to_context (surface)->stream,
			       " set-source\n");
    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_status_t
_path_move_to (void *closure,
	       const cairo_point_t *point)
{
    _cairo_output_stream_printf (closure,
				 " %f %f m",
				 _cairo_fixed_to_double (point->x),
				 _cairo_fixed_to_double (point->y));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_path_line_to (void *closure,
	       const cairo_point_t *point)
{
    _cairo_output_stream_printf (closure,
				 " %f %f l",
				 _cairo_fixed_to_double (point->x),
				 _cairo_fixed_to_double (point->y));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_path_curve_to (void *closure,
		const cairo_point_t *p1,
		const cairo_point_t *p2,
		const cairo_point_t *p3)
{
    _cairo_output_stream_printf (closure,
				 " %f %f %f %f %f %f c",
				 _cairo_fixed_to_double (p1->x),
				 _cairo_fixed_to_double (p1->y),
				 _cairo_fixed_to_double (p2->x),
				 _cairo_fixed_to_double (p2->y),
				 _cairo_fixed_to_double (p3->x),
				 _cairo_fixed_to_double (p3->y));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_path_close (void *closure)
{
    _cairo_output_stream_printf (closure,
				 " h");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_path_boxes (cairo_script_surface_t *surface,
		  const cairo_path_fixed_t *path)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_path_fixed_iter_t iter;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;
    struct _cairo_boxes_chunk *chunk;
    cairo_boxes_t boxes;
    cairo_box_t box;
    int i;

    _cairo_boxes_init (&boxes);
    _cairo_path_fixed_iter_init (&iter, path);
    while (_cairo_path_fixed_iter_is_fill_box (&iter, &box)) {
	if (box.p1.y == box.p2.y || box.p1.x == box.p2.x)
	    continue;

	status = _cairo_boxes_add (&boxes, CAIRO_ANTIALIAS_DEFAULT, &box);
	if (unlikely (status)) {
	    _cairo_boxes_fini (&boxes);
	    return status;
	}
    }

    if (! _cairo_path_fixed_iter_at_end (&iter)) {
	_cairo_boxes_fini (&boxes);
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    for (chunk = &boxes.chunks; chunk; chunk = chunk->next) {
	for (i = 0; i < chunk->count; i++) {
	    const cairo_box_t *b = &chunk->base[i];
	    double x1 = _cairo_fixed_to_double (b->p1.x);
	    double y1 = _cairo_fixed_to_double (b->p1.y);
	    double x2 = _cairo_fixed_to_double (b->p2.x);
	    double y2 = _cairo_fixed_to_double (b->p2.y);

	    _cairo_output_stream_printf (ctx->stream,
					 "\n  %f %f %f %f rectangle",
					 x1, y1, x2 - x1, y2 - y1);
	}
    }

    _cairo_boxes_fini (&boxes);
    return status;
}

static cairo_status_t
_emit_path (cairo_script_surface_t *surface,
	    const cairo_path_fixed_t *path,
	    cairo_bool_t is_fill)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_box_t box;
    cairo_int_status_t status;

    assert (target_is_active (surface));
    assert (_cairo_matrix_is_identity (&surface->cr.current_ctm));

    if (_cairo_path_fixed_equal (&surface->cr.current_path, path))
	return CAIRO_STATUS_SUCCESS;

    _cairo_path_fixed_fini (&surface->cr.current_path);

    _cairo_output_stream_puts (ctx->stream, "n");

    if (path == NULL) {
	_cairo_path_fixed_init (&surface->cr.current_path);
	_cairo_output_stream_puts (ctx->stream, "\n");
	return CAIRO_STATUS_SUCCESS;
    }

    status = _cairo_path_fixed_init_copy (&surface->cr.current_path, path);
    if (unlikely (status))
	return status;

    status = CAIRO_INT_STATUS_UNSUPPORTED;
    if (_cairo_path_fixed_is_rectangle (path, &box)) {
	double x1 = _cairo_fixed_to_double (box.p1.x);
	double y1 = _cairo_fixed_to_double (box.p1.y);
	double x2 = _cairo_fixed_to_double (box.p2.x);
	double y2 = _cairo_fixed_to_double (box.p2.y);

	assert (x1 > -9999);

	_cairo_output_stream_printf (ctx->stream,
				     " %f %f %f %f rectangle",
				     x1, y1, x2 - x1, y2 - y1);
	status = CAIRO_INT_STATUS_SUCCESS;
    } else if (is_fill && _cairo_path_fixed_fill_is_rectilinear (path)) {
	status = _emit_path_boxes (surface, path);
    }

    if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	status = _cairo_path_fixed_interpret (path,
					      _path_move_to,
					      _path_line_to,
					      _path_curve_to,
					      _path_close,
					      ctx->stream);
    }

    _cairo_output_stream_puts (ctx->stream, "\n");

    return status;
}
static cairo_bool_t
_scaling_matrix_equal (const cairo_matrix_t *a,
		       const cairo_matrix_t *b)
{
    return fabs (a->xx - b->xx) < 1e-5 &&
	   fabs (a->xy - b->xy) < 1e-5 &&
	   fabs (a->yx - b->yx) < 1e-5 &&
	   fabs (a->yy - b->yy) < 1e-5;
}

static cairo_status_t
_emit_scaling_matrix (cairo_script_surface_t *surface,
		      const cairo_matrix_t *ctm,
		      cairo_bool_t *matrix_updated)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_bool_t was_identity;
    assert (target_is_active (surface));

    if (_scaling_matrix_equal (&surface->cr.current_ctm, ctm))
	return CAIRO_STATUS_SUCCESS;

    was_identity = _cairo_matrix_is_identity (&surface->cr.current_ctm);

    *matrix_updated = TRUE;
    surface->cr.current_ctm = *ctm;
    surface->cr.current_ctm.x0 = 0.;
    surface->cr.current_ctm.y0 = 0.;

    if (_cairo_matrix_is_identity (&surface->cr.current_ctm)) {
	_cairo_output_stream_puts (ctx->stream,
				   "identity set-matrix\n");
    } else if (was_identity && fabs (ctm->yx) < 1e-5 && fabs (ctm->xy) < 1e-5) {
	_cairo_output_stream_printf (ctx->stream,
				     "%f %f scale\n",
				     ctm->xx, ctm->yy);
    } else {
	_cairo_output_stream_printf (ctx->stream,
				     "[%f %f %f %f 0 0] set-matrix\n",
				     ctm->xx, ctm->yx,
				     ctm->xy, ctm->yy);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_font_matrix (cairo_script_surface_t *surface,
		   const cairo_matrix_t *font_matrix)
{
    cairo_script_context_t *ctx = to_context (surface);
    assert (target_is_active (surface));

    if (memcmp (&surface->cr.current_font_matrix,
		font_matrix,
		sizeof (cairo_matrix_t)) == 0)
    {
	return CAIRO_STATUS_SUCCESS;
    }

    surface->cr.current_font_matrix = *font_matrix;

    if (_cairo_matrix_is_identity (font_matrix)) {
	_cairo_output_stream_puts (ctx->stream,
				   "identity set-font-matrix\n");
    } else {
	_cairo_output_stream_printf (ctx->stream,
				     "[%f %f %f %f %f %f] set-font-matrix\n",
				     font_matrix->xx, font_matrix->yx,
				     font_matrix->xy, font_matrix->yy,
				     font_matrix->x0, font_matrix->y0);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_surface_t *
_cairo_script_surface_create_similar (void	       *abstract_surface,
				      cairo_content_t	content,
				      int		width,
				      int		height)
{
    cairo_script_surface_t *surface, *other = abstract_surface;
    cairo_surface_t *passthrough = NULL;
    cairo_script_context_t *ctx;
    cairo_rectangle_t extents;
    cairo_status_t status;

    ctx = to_context (other);

    status = cairo_device_acquire (&ctx->base);
    if (unlikely (status))
	return _cairo_surface_create_in_error (status);

    if (! other->emitted) {
	status = _emit_surface (other);
	if (unlikely (status)) {
	    cairo_device_release (&ctx->base);
	    return _cairo_surface_create_in_error (status);
	}

	target_push (other);
    }

    if (_cairo_surface_wrapper_is_active (&other->wrapper)) {
	passthrough =
	    _cairo_surface_wrapper_create_similar (&other->wrapper,
						   content, width, height);
	if (unlikely (passthrough->status)) {
	    cairo_device_release (&ctx->base);
	    return passthrough;
	}
    }

    extents.x = extents.y = 0;
    extents.width = width;
    extents.height = height;
    surface = _cairo_script_surface_create_internal (ctx, content,
						     &extents, passthrough);
    cairo_surface_destroy (passthrough);

    if (unlikely (surface->base.status)) {
	cairo_device_release (&ctx->base);
	return &surface->base;
    }

    _get_target (other);
    _cairo_output_stream_printf (ctx->stream,
				 "%u %u //%s similar dup /s%u exch def context\n",
				 width, height,
				 _content_to_string (content),
				 surface->base.unique_id);

    surface->emitted = TRUE;
    surface->defined = TRUE;
    surface->base.is_clear = TRUE;
    target_push (surface);

    cairo_device_release (&ctx->base);
    return &surface->base;
}

static cairo_status_t
_device_flush (void *abstract_device)
{
    cairo_script_context_t *ctx = abstract_device;

    return _cairo_output_stream_flush (ctx->stream);
}

static void
_device_finish (void *abstract_device)
{
    cairo_script_context_t *ctx = abstract_device;

    cairo_status_t status = _cairo_output_stream_close (ctx->stream);
    status = _cairo_device_set_error (&ctx->base, status);
    (void) status;
}

static void
_device_destroy (void *abstract_device)
{
    cairo_script_context_t *ctx = abstract_device;
    cairo_status_t status;

    while (! cairo_list_is_empty (&ctx->fonts)) {
	cairo_script_font_t *font;

	font = cairo_list_first_entry (&ctx->fonts, cairo_script_font_t, link);
	cairo_list_del (&font->base.link);
	cairo_list_del (&font->link);
	free (font);
    }

    _bitmap_fini (ctx->surface_id.next);
    _bitmap_fini (ctx->font_id.next);

    if (ctx->owns_stream)
	status = _cairo_output_stream_destroy (ctx->stream);

    free (ctx);
}

static cairo_surface_t *
_cairo_script_surface_source (void                    *abstract_surface,
			      cairo_rectangle_int_t	*extents)
{
    cairo_script_surface_t *surface = abstract_surface;

    if (extents) {
	extents->x = extents->y = 0;
	extents->width  = surface->width;
	extents->height = surface->height;
    }

    return &surface->base;
}

static cairo_status_t
_cairo_script_surface_acquire_source_image (void                    *abstract_surface,
					    cairo_image_surface_t  **image_out,
					    void                   **image_extra)
{
    cairo_script_surface_t *surface = abstract_surface;

    if (_cairo_surface_wrapper_is_active (&surface->wrapper)) {
	return _cairo_surface_wrapper_acquire_source_image (&surface->wrapper,
							    image_out,
							    image_extra);
    }

    return CAIRO_INT_STATUS_UNSUPPORTED;
}

static void
_cairo_script_surface_release_source_image (void                   *abstract_surface,
					   cairo_image_surface_t  *image,
					   void                   *image_extra)
{
    cairo_script_surface_t *surface = abstract_surface;

    assert (_cairo_surface_wrapper_is_active (&surface->wrapper));
    _cairo_surface_wrapper_release_source_image (&surface->wrapper,
						 image,
						 image_extra);
}

static cairo_status_t
_cairo_script_surface_finish (void *abstract_surface)
{
    cairo_script_surface_t *surface = abstract_surface;
    cairo_script_context_t *ctx = to_context (surface);
    cairo_status_t status = CAIRO_STATUS_SUCCESS, status2;

    _cairo_surface_wrapper_fini (&surface->wrapper);

    free (surface->cr.current_style.dash);
    surface->cr.current_style.dash = NULL;

    _cairo_pattern_fini (&surface->cr.current_source.base);
    _cairo_path_fixed_fini (&surface->cr.current_path);
    _cairo_surface_clipper_reset (&surface->clipper);

    status = cairo_device_acquire (&ctx->base);
    if (unlikely (status))
	return status;

    if (surface->emitted) {
	assert (! surface->active);

	if (! cairo_list_is_empty (&surface->operand.link)) {
	    if (! ctx->active) {
		if (target_is_active (surface)) {
		    _cairo_output_stream_printf (ctx->stream,
						 "pop\n");
		} else {
		    int depth = target_depth (surface);
		    if (depth == 1) {
			_cairo_output_stream_printf (ctx->stream,
						     "exch pop\n");
		    } else {
			_cairo_output_stream_printf (ctx->stream,
						     "%d -1 roll pop\n",
						     depth);
		    }
		}
		cairo_list_del (&surface->operand.link);
	    } else {
		struct deferred_finish *link = _cairo_malloc (sizeof (*link));
		if (link == NULL) {
		    status2 = _cairo_error (CAIRO_STATUS_NO_MEMORY);
		    if (status == CAIRO_STATUS_SUCCESS)
			status = status2;
		    cairo_list_del (&surface->operand.link);
		} else {
		    link->operand.type = DEFERRED;
		    cairo_list_swap (&link->operand.link,
				     &surface->operand.link);
		    cairo_list_add (&link->link, &ctx->deferred);
		}
	    }
	}

	if (surface->defined) {
	    _cairo_output_stream_printf (ctx->stream,
					 "/s%u undef\n",
					 surface->base.unique_id);
	}
    }

    if (status == CAIRO_STATUS_SUCCESS)
	status = _cairo_output_stream_flush (to_context (surface)->stream);

    cairo_device_release (&ctx->base);

    return status;
}

static cairo_int_status_t
_cairo_script_surface_copy_page (void *abstract_surface)
{
    cairo_script_surface_t *surface = abstract_surface;
    cairo_status_t status;

    status = cairo_device_acquire (surface->base.device);
    if (unlikely (status))
	return status;

    status = _emit_context (surface);
    if (unlikely (status))
	goto BAIL;

    _cairo_output_stream_puts (to_context (surface)->stream, "copy-page\n");

BAIL:
    cairo_device_release (surface->base.device);
    return status;
}

static cairo_int_status_t
_cairo_script_surface_show_page (void *abstract_surface)
{
    cairo_script_surface_t *surface = abstract_surface;
    cairo_status_t status;

    status = cairo_device_acquire (surface->base.device);
    if (unlikely (status))
	return status;

    status = _emit_context (surface);
    if (unlikely (status))
	goto BAIL;

    _cairo_output_stream_puts (to_context (surface)->stream, "show-page\n");

BAIL:
    cairo_device_release (surface->base.device);
    return status;
}

static cairo_status_t
_cairo_script_surface_clipper_intersect_clip_path (cairo_surface_clipper_t *clipper,
						   cairo_path_fixed_t	*path,
						   cairo_fill_rule_t	 fill_rule,
						   double		 tolerance,
						   cairo_antialias_t	 antialias)
{
    cairo_script_surface_t *surface = cairo_container_of (clipper,
							  cairo_script_surface_t,
							  clipper);
    cairo_script_context_t *ctx = to_context (surface);
    cairo_bool_t matrix_updated = FALSE;
    cairo_status_t status;
    cairo_box_t box;

    status = _emit_context (surface);
    if (unlikely (status))
	return status;

    if (path == NULL) {
	if (surface->cr.has_clip) {
	    _cairo_output_stream_puts (ctx->stream, "reset-clip\n");
	    surface->cr.has_clip = FALSE;
	}
	return CAIRO_STATUS_SUCCESS;
    }

    /* skip the trivial clip covering the surface extents */
    if (surface->width >= 0 && surface->height >= 0 &&
	_cairo_path_fixed_is_box (path, &box))
    {
	if (box.p1.x <= 0 && box.p1.y <= 0 &&
	    box.p2.x >= _cairo_fixed_from_double (surface->width) &&
	    box.p2.y >= _cairo_fixed_from_double (surface->height))
	{
	    return CAIRO_STATUS_SUCCESS;
	}
    }

    status = _emit_identity (surface, &matrix_updated);
    if (unlikely (status))
	return status;

    status = _emit_fill_rule (surface, fill_rule);
    if (unlikely (status))
	return status;

    if (path->has_curve_to) {
	status = _emit_tolerance (surface, tolerance, matrix_updated);
	if (unlikely (status))
	    return status;
    }

    if (! _cairo_path_fixed_fill_maybe_region (path)) {
	status = _emit_antialias (surface, antialias);
	if (unlikely (status))
	    return status;
    }

    status = _emit_path (surface, path, TRUE);
    if (unlikely (status))
	return status;

    _cairo_output_stream_puts (ctx->stream, "clip+\n");
    surface->cr.has_clip = TRUE;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
active (cairo_script_surface_t *surface)
{
    cairo_status_t status;

    status = cairo_device_acquire (surface->base.device);
    if (unlikely (status))
	return status;

    if (surface->active++ == 0)
	to_context (surface)->active++;

    return CAIRO_STATUS_SUCCESS;
}

static void
inactive (cairo_script_surface_t *surface)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_list_t sorted;

    assert (surface->active > 0);
    if (--surface->active)
	goto DONE;

    assert (ctx->active > 0);
    if (--ctx->active)
	goto DONE;

    cairo_list_init (&sorted);
    while (! cairo_list_is_empty (&ctx->deferred)) {
	struct deferred_finish *df;
	cairo_list_t *operand;
	int depth;

	df = cairo_list_first_entry (&ctx->deferred,
				     struct deferred_finish,
				     link);

	depth = 0;
	cairo_list_foreach (operand, &ctx->operands) {
	    if (operand == &df->operand.link)
		break;
	    depth++;
	}

	df->operand.type = depth;

	if (cairo_list_is_empty (&sorted)) {
	    cairo_list_move (&df->link, &sorted);
	} else {
	    struct deferred_finish *pos;

	    cairo_list_foreach_entry (pos, struct deferred_finish,
				      &sorted,
				      link)
	    {
		if (df->operand.type < pos->operand.type)
		    break;
	    }
	    cairo_list_move_tail (&df->link, &pos->link);
	}
    }

    while (! cairo_list_is_empty (&sorted)) {
	struct deferred_finish *df;
	cairo_list_t *operand;
	int depth;

	df = cairo_list_first_entry (&sorted,
				     struct deferred_finish,
				     link);

	depth = 0;
	cairo_list_foreach (operand, &ctx->operands) {
	    if (operand == &df->operand.link)
		break;
	    depth++;
	}

	if (depth == 0) {
	    _cairo_output_stream_printf (ctx->stream,
					 "pop\n");
	} else if (depth == 1) {
	    _cairo_output_stream_printf (ctx->stream,
					 "exch pop\n");
	} else {
	    _cairo_output_stream_printf (ctx->stream,
					 "%d -1 roll pop\n",
					 depth);
	}

	cairo_list_del (&df->operand.link);
	cairo_list_del (&df->link);
	free (df);
    }

DONE:
    cairo_device_release (surface->base.device);
}

static cairo_int_status_t
_cairo_script_surface_paint (void			*abstract_surface,
			     cairo_operator_t		 op,
			     const cairo_pattern_t	*source,
			     const cairo_clip_t		*clip)
{
    cairo_script_surface_t *surface = abstract_surface;
    cairo_status_t status;

    status = active (surface);
    if (unlikely (status))
	return status;

    status = _cairo_surface_clipper_set_clip (&surface->clipper, clip);
    if (unlikely (status))
	goto BAIL;

    status = _emit_context (surface);
    if (unlikely (status))
	goto BAIL;

    status = _emit_source (surface, op, source);
    if (unlikely (status))
	goto BAIL;

    status = _emit_operator (surface, op);
    if (unlikely (status))
	goto BAIL;

    _cairo_output_stream_puts (to_context (surface)->stream,
			       "paint\n");

    inactive (surface);

    if (_cairo_surface_wrapper_is_active (&surface->wrapper)) {
	return _cairo_surface_wrapper_paint (&surface->wrapper,
					     op, source, 0, clip);
    }

    return CAIRO_STATUS_SUCCESS;

BAIL:
    inactive (surface);
    return status;
}

static cairo_int_status_t
_cairo_script_surface_mask (void			*abstract_surface,
			    cairo_operator_t		 op,
			    const cairo_pattern_t	*source,
			    const cairo_pattern_t	*mask,
			    const cairo_clip_t		*clip)
{
    cairo_script_surface_t *surface = abstract_surface;
    cairo_status_t status;

    status = active (surface);
    if (unlikely (status))
	return status;

    status = _cairo_surface_clipper_set_clip (&surface->clipper, clip);
    if (unlikely (status))
	goto BAIL;

    status = _emit_context (surface);
    if (unlikely (status))
	goto BAIL;

    status = _emit_source (surface, op, source);
    if (unlikely (status))
	goto BAIL;

    status = _emit_operator (surface, op);
    if (unlikely (status))
	goto BAIL;

    if (_cairo_pattern_equal (source, mask)) {
	_cairo_output_stream_puts (to_context (surface)->stream, "/source get");
    } else {
	status = _emit_pattern (surface, mask);
	if (unlikely (status))
	    goto BAIL;
    }

    assert (surface->cr.current_operator == op);

    _cairo_output_stream_puts (to_context (surface)->stream,
			       " mask\n");

    inactive (surface);

    if (_cairo_surface_wrapper_is_active (&surface->wrapper)) {
	return _cairo_surface_wrapper_mask (&surface->wrapper,
					    op, source, 0, mask, 0, clip);
    }

    return CAIRO_STATUS_SUCCESS;

BAIL:
    inactive (surface);
    return status;
}

static cairo_int_status_t
_cairo_script_surface_stroke (void				*abstract_surface,
			      cairo_operator_t			 op,
			      const cairo_pattern_t		*source,
			      const cairo_path_fixed_t		*path,
			      const cairo_stroke_style_t	*style,
			      const cairo_matrix_t		*ctm,
			      const cairo_matrix_t		*ctm_inverse,
			      double				 tolerance,
			      cairo_antialias_t			 antialias,
			      const cairo_clip_t		*clip)
{
    cairo_script_surface_t *surface = abstract_surface;
    cairo_bool_t matrix_updated = FALSE;
    cairo_status_t status;

    status = active (surface);
    if (unlikely (status))
	return status;

    status = _cairo_surface_clipper_set_clip (&surface->clipper, clip);
    if (unlikely (status))
	goto BAIL;

    status = _emit_context (surface);
    if (unlikely (status))
	goto BAIL;

    status = _emit_identity (surface, &matrix_updated);
    if (unlikely (status))
	goto BAIL;

    status = _emit_path (surface, path, FALSE);
    if (unlikely (status))
	goto BAIL;

    status = _emit_source (surface, op, source);
    if (unlikely (status))
	goto BAIL;

    status = _emit_scaling_matrix (surface, ctm, &matrix_updated);
    if (unlikely (status))
	goto BAIL;

    status = _emit_operator (surface, op);
    if (unlikely (status))
	goto BAIL;

    if (_scaling_matrix_equal (&surface->cr.current_ctm,
			       &surface->cr.current_stroke_matrix))
    {
	matrix_updated = FALSE;
    }
    else
    {
	matrix_updated = TRUE;
	surface->cr.current_stroke_matrix = surface->cr.current_ctm;
    }

    status = _emit_stroke_style (surface, style, matrix_updated);
    if (unlikely (status))
	goto BAIL;

    status = _emit_tolerance (surface, tolerance, matrix_updated);
    if (unlikely (status))
	goto BAIL;

    status = _emit_antialias (surface, antialias);
    if (unlikely (status))
	goto BAIL;

    _cairo_output_stream_puts (to_context (surface)->stream, "stroke+\n");

    inactive (surface);

    if (_cairo_surface_wrapper_is_active (&surface->wrapper)) {
	return _cairo_surface_wrapper_stroke (&surface->wrapper,
					      op, source, 0, path,
					      style,
					      ctm, ctm_inverse,
					      tolerance, antialias,
					      clip);
    }

    return CAIRO_STATUS_SUCCESS;

BAIL:
    inactive (surface);
    return status;
}

static cairo_int_status_t
_cairo_script_surface_fill (void			*abstract_surface,
			    cairo_operator_t		 op,
			    const cairo_pattern_t	*source,
			    const cairo_path_fixed_t	*path,
			    cairo_fill_rule_t		 fill_rule,
			    double			 tolerance,
			    cairo_antialias_t		 antialias,
			    const cairo_clip_t		*clip)
{
    cairo_script_surface_t *surface = abstract_surface;
    cairo_bool_t matrix_updated = FALSE;
    cairo_status_t status;
    cairo_box_t box;

    status = active (surface);
    if (unlikely (status))
	return status;

    status = _cairo_surface_clipper_set_clip (&surface->clipper, clip);
    if (unlikely (status))
	goto BAIL;

    status = _emit_context (surface);
    if (unlikely (status))
	goto BAIL;

    status = _emit_identity (surface, &matrix_updated);
    if (unlikely (status))
	goto BAIL;

    status = _emit_source (surface, op, source);
    if (unlikely (status))
	goto BAIL;

    if (! _cairo_path_fixed_is_box (path, &box)) {
	status = _emit_fill_rule (surface, fill_rule);
	if (unlikely (status))
	    goto BAIL;
    }

    if (path->has_curve_to) {
	status = _emit_tolerance (surface, tolerance, matrix_updated);
	if (unlikely (status))
	    goto BAIL;
    }

    if (! _cairo_path_fixed_fill_maybe_region (path)) {
	status = _emit_antialias (surface, antialias);
	if (unlikely (status))
	    goto BAIL;
    }

    status = _emit_path (surface, path, TRUE);
    if (unlikely (status))
	goto BAIL;

    status = _emit_operator (surface, op);
    if (unlikely (status))
	goto BAIL;

    _cairo_output_stream_puts (to_context (surface)->stream, "fill+\n");

    inactive (surface);

    if (_cairo_surface_wrapper_is_active (&surface->wrapper)) {
	return _cairo_surface_wrapper_fill (&surface->wrapper,
					    op, source, 0, path,
					    fill_rule,
					    tolerance,
					    antialias,
					    clip);
    }

    return CAIRO_STATUS_SUCCESS;

BAIL:
    inactive (surface);
    return status;
}

static cairo_surface_t *
_cairo_script_surface_snapshot (void *abstract_surface)
{
    cairo_script_surface_t *surface = abstract_surface;

    if (_cairo_surface_wrapper_is_active (&surface->wrapper))
	return _cairo_surface_wrapper_snapshot (&surface->wrapper);

    return NULL;
}

static cairo_bool_t
_cairo_script_surface_has_show_text_glyphs (void *abstract_surface)
{
    return TRUE;
}

static const char *
_subpixel_order_to_string (cairo_subpixel_order_t subpixel_order)
{
    static const char *names[] = {
	"SUBPIXEL_ORDER_DEFAULT",	/* CAIRO_SUBPIXEL_ORDER_DEFAULT */
	"SUBPIXEL_ORDER_RGB",		/* CAIRO_SUBPIXEL_ORDER_RGB */
	"SUBPIXEL_ORDER_BGR",		/* CAIRO_SUBPIXEL_ORDER_BGR */
	"SUBPIXEL_ORDER_VRGB",		/* CAIRO_SUBPIXEL_ORDER_VRGB */
	"SUBPIXEL_ORDER_VBGR"		/* CAIRO_SUBPIXEL_ORDER_VBGR */
    };
    return names[subpixel_order];
}
static const char *
_hint_style_to_string (cairo_hint_style_t hint_style)
{
    static const char *names[] = {
	"HINT_STYLE_DEFAULT",	/* CAIRO_HINT_STYLE_DEFAULT */
	"HINT_STYLE_NONE",	/* CAIRO_HINT_STYLE_NONE */
	"HINT_STYLE_SLIGHT",	/* CAIRO_HINT_STYLE_SLIGHT */
	"HINT_STYLE_MEDIUM",	/* CAIRO_HINT_STYLE_MEDIUM */
	"HINT_STYLE_FULL"	/* CAIRO_HINT_STYLE_FULL */
    };
    return names[hint_style];
}
static const char *
_hint_metrics_to_string (cairo_hint_metrics_t hint_metrics)
{
    static const char *names[] = {
	 "HINT_METRICS_DEFAULT",	/* CAIRO_HINT_METRICS_DEFAULT */
	 "HINT_METRICS_OFF",		/* CAIRO_HINT_METRICS_OFF */
	 "HINT_METRICS_ON"		/* CAIRO_HINT_METRICS_ON */
    };
    return names[hint_metrics];
}

static cairo_status_t
_emit_font_options (cairo_script_surface_t *surface,
		    cairo_font_options_t *font_options)
{
    cairo_script_context_t *ctx = to_context (surface);

    if (cairo_font_options_equal (&surface->cr.current_font_options,
				  font_options))
    {
	return CAIRO_STATUS_SUCCESS;
    }

    _cairo_output_stream_printf (ctx->stream, "<<");

    if (font_options->antialias != surface->cr.current_font_options.antialias) {
	_cairo_output_stream_printf (ctx->stream,
				     " /antialias //%s",
				     _antialias_to_string (font_options->antialias));
    }

    if (font_options->subpixel_order !=
	surface->cr.current_font_options.subpixel_order)
    {
	_cairo_output_stream_printf (ctx->stream,
				     " /subpixel-order //%s",
				     _subpixel_order_to_string (font_options->subpixel_order));
    }

    if (font_options->hint_style !=
	surface->cr.current_font_options.hint_style)
    {
	_cairo_output_stream_printf (ctx->stream,
				     " /hint-style //%s",
				     _hint_style_to_string (font_options->hint_style));
    }

    if (font_options->hint_metrics !=
	surface->cr.current_font_options.hint_metrics)
    {
	_cairo_output_stream_printf (ctx->stream,
				     " /hint-metrics //%s",
				     _hint_metrics_to_string (font_options->hint_metrics));
    }

    _cairo_output_stream_printf (ctx->stream,
				 " >> set-font-options\n");

    surface->cr.current_font_options = *font_options;
    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_script_scaled_font_fini (cairo_scaled_font_private_t *abstract_private,
				cairo_scaled_font_t *scaled_font)
{
    cairo_script_font_t *priv = (cairo_script_font_t *)abstract_private;
    cairo_script_context_t *ctx = (cairo_script_context_t *)abstract_private->key;
    cairo_status_t status;

    status = cairo_device_acquire (&ctx->base);
    if (likely (status == CAIRO_STATUS_SUCCESS)) {
	_cairo_output_stream_printf (ctx->stream,
				     "/f%lu undef /sf%lu undef\n",
				     priv->id,
				     priv->id);

	_bitmap_release_id (&ctx->font_id, priv->id);
	cairo_device_release (&ctx->base);
    }

    cairo_list_del (&priv->link);
    cairo_list_del (&priv->base.link);
    free (priv);
}

static cairo_script_font_t *
_cairo_script_font_get (cairo_script_context_t *ctx, cairo_scaled_font_t *font)
{
    return (cairo_script_font_t *) _cairo_scaled_font_find_private (font, ctx);
}

static long unsigned
_cairo_script_font_id (cairo_script_context_t *ctx, cairo_scaled_font_t *font)
{
    return _cairo_script_font_get (ctx, font)->id;
}

static cairo_status_t
_emit_type42_font (cairo_script_surface_t *surface,
		   cairo_scaled_font_t *scaled_font)
{
    cairo_script_context_t *ctx = to_context (surface);
    const cairo_scaled_font_backend_t *backend;
    cairo_output_stream_t *base85_stream;
    cairo_output_stream_t *zlib_stream;
    cairo_status_t status, status2;
    unsigned long size;
    unsigned int load_flags;
    uint32_t len;
    uint8_t *buf;

    backend = scaled_font->backend;
    if (backend->load_truetype_table == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    size = 0;
    status = backend->load_truetype_table (scaled_font, 0, 0, NULL, &size);
    if (unlikely (status))
	return status;

    buf = _cairo_malloc (size);
    if (unlikely (buf == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    status = backend->load_truetype_table (scaled_font, 0, 0, buf, &size);
    if (unlikely (status)) {
	free (buf);
	return status;
    }

#if CAIRO_HAS_FT_FONT
    load_flags = _cairo_ft_scaled_font_get_load_flags (scaled_font);
#else
    load_flags = 0;
#endif
    _cairo_output_stream_printf (ctx->stream,
				 "<< "
				 "/type 42 "
				 "/index 0 "
				 "/flags %d "
				 "/source <|",
				 load_flags);

    base85_stream = _cairo_base85_stream_create (ctx->stream);
    len = to_be32 (size);
    _cairo_output_stream_write (base85_stream, &len, sizeof (len));

    zlib_stream = _cairo_deflate_stream_create (base85_stream);

    _cairo_output_stream_write (zlib_stream, buf, size);
    free (buf);

    status2 = _cairo_output_stream_destroy (zlib_stream);
    if (status == CAIRO_STATUS_SUCCESS)
	status = status2;

    status2 = _cairo_output_stream_destroy (base85_stream);
    if (status == CAIRO_STATUS_SUCCESS)
	status = status2;

    _cairo_output_stream_printf (ctx->stream,
				 "~> >> font dup /f%lu exch def set-font-face",
				 _cairo_script_font_id (ctx, scaled_font));

    return status;
}

static cairo_status_t
_emit_scaled_font_init (cairo_script_surface_t *surface,
			cairo_scaled_font_t *scaled_font,
			cairo_script_font_t **font_out)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_script_font_t *font_private;
    cairo_int_status_t status;

    font_private = _cairo_malloc (sizeof (cairo_script_font_t));
    if (unlikely (font_private == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    _cairo_scaled_font_attach_private (scaled_font, &font_private->base, ctx,
				       _cairo_script_scaled_font_fini);

    font_private->parent = scaled_font;
    font_private->subset_glyph_index = 0;
    font_private->has_sfnt = TRUE;

    cairo_list_add (&font_private->link, &ctx->fonts);

    status = _bitmap_next_id (&ctx->font_id,
			      &font_private->id);
    if (unlikely (status)) {
	free (font_private);
	return status;
    }

    status = _emit_context (surface);
    if (unlikely (status)) {
	free (font_private);
	return status;
    }

    status = _emit_type42_font (surface, scaled_font);
    if (status != CAIRO_INT_STATUS_UNSUPPORTED) {
	*font_out = font_private;
	return status;
    }

    font_private->has_sfnt = FALSE;
    _cairo_output_stream_printf (ctx->stream,
				 "dict\n"
				 "  /type 3 set\n"
				 "  /metrics [%f %f %f %f %f] set\n"
				 "  /glyphs array set\n"
				 "  font dup /f%lu exch def set-font-face",
				 scaled_font->fs_extents.ascent,
				 scaled_font->fs_extents.descent,
				 scaled_font->fs_extents.height,
				 scaled_font->fs_extents.max_x_advance,
				 scaled_font->fs_extents.max_y_advance,
				 font_private->id);

    *font_out = font_private;
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_scaled_font (cairo_script_surface_t *surface,
		   cairo_scaled_font_t *scaled_font)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_matrix_t matrix;
    cairo_font_options_t options;
    cairo_bool_t matrix_updated = FALSE;
    cairo_status_t status;
    cairo_script_font_t *font_private;

    cairo_scaled_font_get_ctm (scaled_font, &matrix);
    status = _emit_scaling_matrix (surface, &matrix, &matrix_updated);
    if (unlikely (status))
	return status;

    if (! matrix_updated && surface->cr.current_scaled_font == scaled_font)
	return CAIRO_STATUS_SUCCESS;

    surface->cr.current_scaled_font = scaled_font;

    font_private = _cairo_script_font_get (ctx, scaled_font);
    if (font_private == NULL) {
	cairo_scaled_font_get_font_matrix (scaled_font, &matrix);
	status = _emit_font_matrix (surface, &matrix);
	if (unlikely (status))
	    return status;

	_cairo_font_options_init_default (&options);
	cairo_scaled_font_get_font_options (scaled_font, &options);
	status = _emit_font_options (surface, &options);
	if (unlikely (status))
	    return status;

	status = _emit_scaled_font_init (surface, scaled_font, &font_private);
	if (unlikely (status))
	    return status;

	assert (target_is_active (surface));
	_cairo_output_stream_printf (ctx->stream,
				     " /scaled-font get /sf%lu exch def\n",
				     font_private->id);
    } else {
	_cairo_output_stream_printf (ctx->stream,
				     "sf%lu set-scaled-font\n",
				     font_private->id);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_scaled_glyph_vector (cairo_script_surface_t *surface,
			   cairo_scaled_font_t *scaled_font,
			   cairo_script_font_t *font_private,
			   cairo_scaled_glyph_t *scaled_glyph)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_script_implicit_context_t old_cr;
    cairo_status_t status;
    unsigned long index;

    index = ++font_private->subset_glyph_index;
    scaled_glyph->dev_private_key = ctx;
    scaled_glyph->dev_private = (void *)(uintptr_t)index;

    _cairo_output_stream_printf (ctx->stream,
				 "%lu <<\n"
				 "  /metrics [%f %f %f %f %f %f]\n"
				 "  /render {\n",
				 index,
				 scaled_glyph->fs_metrics.x_bearing,
				 scaled_glyph->fs_metrics.y_bearing,
				 scaled_glyph->fs_metrics.width,
				 scaled_glyph->fs_metrics.height,
				 scaled_glyph->fs_metrics.x_advance,
				 scaled_glyph->fs_metrics.y_advance);

    if (! _cairo_matrix_is_identity (&scaled_font->scale_inverse)) {
	_cairo_output_stream_printf (ctx->stream,
				     "[%f %f %f %f %f %f] transform\n",
				     scaled_font->scale_inverse.xx,
				     scaled_font->scale_inverse.yx,
				     scaled_font->scale_inverse.xy,
				     scaled_font->scale_inverse.yy,
				     scaled_font->scale_inverse.x0,
				     scaled_font->scale_inverse.y0);
    }

    old_cr = surface->cr;
    _cairo_script_implicit_context_init (&surface->cr);
    status = _cairo_recording_surface_replay (scaled_glyph->recording_surface,
					      &surface->base);
    surface->cr = old_cr;

    _cairo_output_stream_puts (ctx->stream, "} >> set\n");

    return status;
}

static cairo_status_t
_emit_scaled_glyph_bitmap (cairo_script_surface_t *surface,
			   cairo_scaled_font_t *scaled_font,
			   cairo_script_font_t *font_private,
			   cairo_scaled_glyph_t *scaled_glyph)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_status_t status;
    unsigned long index;

    index = ++font_private->subset_glyph_index;
    scaled_glyph->dev_private_key = ctx;
    scaled_glyph->dev_private = (void *)(uintptr_t)index;

    _cairo_output_stream_printf (ctx->stream,
				 "%lu <<\n"
				 "  /metrics [%f %f %f %f %f %f]\n"
				 "  /render {\n"
				 "%f %f translate\n",
				 index,
				 scaled_glyph->fs_metrics.x_bearing,
				 scaled_glyph->fs_metrics.y_bearing,
				 scaled_glyph->fs_metrics.width,
				 scaled_glyph->fs_metrics.height,
				 scaled_glyph->fs_metrics.x_advance,
				 scaled_glyph->fs_metrics.y_advance,
				 scaled_glyph->fs_metrics.x_bearing,
				 scaled_glyph->fs_metrics.y_bearing);

    status = _emit_image_surface (surface, scaled_glyph->surface);
    if (unlikely (status))
	return status;

    _cairo_output_stream_puts (ctx->stream, "pattern ");

    if (! _cairo_matrix_is_identity (&scaled_font->font_matrix)) {
	_cairo_output_stream_printf (ctx->stream,
				     "\n  [%f %f %f %f %f %f] set-matrix\n",
				     scaled_font->font_matrix.xx,
				     scaled_font->font_matrix.yx,
				     scaled_font->font_matrix.xy,
				     scaled_font->font_matrix.yy,
				     scaled_font->font_matrix.x0,
				     scaled_font->font_matrix.y0);
    }
    _cairo_output_stream_puts (ctx->stream,
				 "mask\n} >> set\n");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_scaled_glyph_prologue (cairo_script_surface_t *surface,
			     cairo_scaled_font_t *scaled_font)
{
    cairo_script_context_t *ctx = to_context (surface);

    _cairo_output_stream_printf (ctx->stream, "f%lu /glyphs get\n",
				 _cairo_script_font_id (ctx, scaled_font));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_emit_scaled_glyphs (cairo_script_surface_t *surface,
		     cairo_scaled_font_t *scaled_font,
		     cairo_glyph_t *glyphs,
		     unsigned int num_glyphs)
{
    cairo_script_context_t *ctx = to_context (surface);
    cairo_script_font_t *font_private;
    cairo_status_t status;
    unsigned int n;
    cairo_bool_t have_glyph_prologue = FALSE;

    if (num_glyphs == 0)
	return CAIRO_STATUS_SUCCESS;

    font_private = _cairo_script_font_get (ctx, scaled_font);
    if (font_private->has_sfnt)
	return CAIRO_STATUS_SUCCESS;

    _cairo_scaled_font_freeze_cache (scaled_font);
    for (n = 0; n < num_glyphs; n++) {
	cairo_scaled_glyph_t *scaled_glyph;

	status = _cairo_scaled_glyph_lookup (scaled_font,
					     glyphs[n].index,
					     CAIRO_SCALED_GLYPH_INFO_METRICS,
                                             NULL, /* foreground color */
					     &scaled_glyph);
	if (unlikely (status))
	    break;

	if (scaled_glyph->dev_private_key == ctx)
	    continue;

	status = _cairo_scaled_glyph_lookup (scaled_font,
					     glyphs[n].index,
					     CAIRO_SCALED_GLYPH_INFO_RECORDING_SURFACE,
                                             NULL, /* foreground color */
					     &scaled_glyph);
	if (_cairo_status_is_error (status))
	    break;

	if (status == CAIRO_STATUS_SUCCESS) {
	    if (! have_glyph_prologue) {
		status = _emit_scaled_glyph_prologue (surface, scaled_font);
		if (unlikely (status))
		    break;

		have_glyph_prologue = TRUE;
	    }

	    status = _emit_scaled_glyph_vector (surface,
						scaled_font, font_private,
						scaled_glyph);
	    if (unlikely (status))
		break;

	    continue;
	}

	status = _cairo_scaled_glyph_lookup (scaled_font,
					     glyphs[n].index,
					     CAIRO_SCALED_GLYPH_INFO_SURFACE,
                                             NULL, /* foreground color */
					     &scaled_glyph);
	if (_cairo_status_is_error (status))
	    break;

	if (status == CAIRO_STATUS_SUCCESS) {
	    if (! have_glyph_prologue) {
		status = _emit_scaled_glyph_prologue (surface, scaled_font);
		if (unlikely (status))
		    break;

		have_glyph_prologue = TRUE;
	    }

	    status = _emit_scaled_glyph_bitmap (surface,
						scaled_font,
						font_private,
						scaled_glyph);
	    if (unlikely (status))
		break;

	    continue;
	}
    }
    _cairo_scaled_font_thaw_cache (scaled_font);

    if (have_glyph_prologue) {
	_cairo_output_stream_puts (to_context (surface)->stream, "pop pop\n");
    }

    return status;
}

static void
to_octal (int value, char *buf, size_t size)
{
    do {
	buf[--size] = '0' + (value & 7);
	value >>= 3;
    } while (size);
}

static void
_emit_string_literal (cairo_script_surface_t *surface,
		      const char *utf8, int len)
{
    cairo_script_context_t *ctx = to_context (surface);
    char c;
    const char *end;

    _cairo_output_stream_puts (ctx->stream, "(");

    if (utf8 == NULL) {
	end = utf8;
    } else {
	if (len < 0)
	    len = strlen (utf8);
	end = utf8 + len;
    }

    while (utf8 < end) {
	switch ((c = *utf8++)) {
	case '\n':
	    c = 'n';
	    goto ESCAPED_CHAR;
	case '\r':
	    c = 'r';
	    goto ESCAPED_CHAR;
	case '\t':
	    c = 't';
	    goto ESCAPED_CHAR;
	case '\b':
	    c = 'b';
	    goto ESCAPED_CHAR;
	case '\f':
	    c = 'f';
	    goto ESCAPED_CHAR;
	case '\\':
	case '(':
	case ')':
ESCAPED_CHAR:
	    _cairo_output_stream_printf (ctx->stream, "\\%c", c);
	    break;
	default:
	    if (_cairo_isprint(c)) {
		_cairo_output_stream_printf (ctx->stream, "%c", c);
	    } else {
		char buf[4] = { '\\' };

		to_octal (c, buf+1, 3);
		_cairo_output_stream_write (ctx->stream, buf, 4);
	    }
	    break;
	}
    }
    _cairo_output_stream_puts (ctx->stream, ")");
}

static cairo_int_status_t
_cairo_script_surface_show_text_glyphs (void			    *abstract_surface,
					cairo_operator_t	     op,
					const cairo_pattern_t	    *source,
					const char		    *utf8,
					int			     utf8_len,
					cairo_glyph_t		    *glyphs,
					int			     num_glyphs,
					const cairo_text_cluster_t  *clusters,
					int			     num_clusters,
					cairo_text_cluster_flags_t   backward,
					cairo_scaled_font_t	    *scaled_font,
					const cairo_clip_t	    *clip)
{
    cairo_script_surface_t *surface = abstract_surface;
    cairo_script_context_t *ctx = to_context (surface);
    cairo_script_font_t *font_private;
    cairo_scaled_glyph_t *scaled_glyph;
    cairo_matrix_t matrix;
    cairo_status_t status;
    double x, y, ix, iy;
    int n;
    cairo_output_stream_t *base85_stream = NULL;

    status = active (surface);
    if (unlikely (status))
	return status;

    status = _cairo_surface_clipper_set_clip (&surface->clipper, clip);
    if (unlikely (status))
	goto BAIL;

    status = _emit_context (surface);
    if (unlikely (status))
	goto BAIL;

    status = _emit_source (surface, op, source);
    if (unlikely (status))
	goto BAIL;

    status = _emit_scaled_font (surface, scaled_font);
    if (unlikely (status))
	goto BAIL;

    status = _emit_operator (surface, op);
    if (unlikely (status))
	goto BAIL;

    status = _emit_scaled_glyphs (surface, scaled_font, glyphs, num_glyphs);
    if (unlikely (status))
	goto BAIL;

    /* (utf8) [cx cy [glyphs]] [clusters] backward show_text_glyphs */
    /* [cx cy [glyphs]] show_glyphs */

    if (utf8 != NULL && clusters != NULL) {
	_emit_string_literal (surface, utf8, utf8_len);
	_cairo_output_stream_puts (ctx->stream, " ");
    }

    matrix = surface->cr.current_ctm;
    status = cairo_matrix_invert (&matrix);
    assert (status == CAIRO_STATUS_SUCCESS);

    ix = x = glyphs[0].x;
    iy = y = glyphs[0].y;
    cairo_matrix_transform_point (&matrix, &ix, &iy);
    ix -= scaled_font->font_matrix.x0;
    iy -= scaled_font->font_matrix.y0;

    _cairo_scaled_font_freeze_cache (scaled_font);
    font_private = _cairo_script_font_get (ctx, scaled_font);

    _cairo_output_stream_printf (ctx->stream,
				 "[%f %f ",
				 ix, iy);

    for (n = 0; n < num_glyphs; n++) {
	if (font_private->has_sfnt) {
	    if (glyphs[n].index > 256)
		break;
	} else {
	    status = _cairo_scaled_glyph_lookup (scaled_font,
						 glyphs[n].index,
						 CAIRO_SCALED_GLYPH_INFO_METRICS,
						 NULL, /* foreground color */
						 &scaled_glyph);
	    if (unlikely (status)) {
		_cairo_scaled_font_thaw_cache (scaled_font);
		goto BAIL;
	    }

	    if ((uintptr_t)scaled_glyph->dev_private > 256)
		break;
	}
    }

    if (n == num_glyphs) {
	_cairo_output_stream_puts (ctx->stream, "<~");
	base85_stream = _cairo_base85_stream_create (ctx->stream);
    } else
	_cairo_output_stream_puts (ctx->stream, "[");

    for (n = 0; n < num_glyphs; n++) {
	double dx, dy;

	status = _cairo_scaled_glyph_lookup (scaled_font,
					     glyphs[n].index,
					     CAIRO_SCALED_GLYPH_INFO_METRICS,
                                             NULL, /* foreground color */
					     &scaled_glyph);
	if (unlikely (status)) {
	    _cairo_scaled_font_thaw_cache (scaled_font);
	    goto BAIL;
	}

	if (fabs (glyphs[n].x - x) > 1e-5 || fabs (glyphs[n].y - y) > 1e-5) {
	    if (fabs (glyphs[n].y - y) < 1e-5) {
		if (base85_stream != NULL) {
		    status = _cairo_output_stream_destroy (base85_stream);
		    if (unlikely (status)) {
			base85_stream = NULL;
			break;
		    }

		    _cairo_output_stream_printf (ctx->stream,
						 "~> %f <~", glyphs[n].x - x);
		    base85_stream = _cairo_base85_stream_create (ctx->stream);
		} else {
		    _cairo_output_stream_printf (ctx->stream,
						 " ] %f [ ", glyphs[n].x - x);
		}

		x = glyphs[n].x;
	    } else {
		ix = x = glyphs[n].x;
		iy = y = glyphs[n].y;
		cairo_matrix_transform_point (&matrix, &ix, &iy);
		ix -= scaled_font->font_matrix.x0;
		iy -= scaled_font->font_matrix.y0;
		if (base85_stream != NULL) {
		    status = _cairo_output_stream_destroy (base85_stream);
		    if (unlikely (status)) {
			base85_stream = NULL;
			break;
		    }

		    _cairo_output_stream_printf (ctx->stream,
						 "~> %f %f <~",
						 ix, iy);
		    base85_stream = _cairo_base85_stream_create (ctx->stream);
		} else {
		    _cairo_output_stream_printf (ctx->stream,
						 " ] %f %f [ ",
						 ix, iy);
		}
	    }
	}
	if (base85_stream != NULL) {
	    uint8_t c;

	    if (font_private->has_sfnt)
		c = glyphs[n].index;
	    else
		c = (uint8_t) (uintptr_t) scaled_glyph->dev_private;

	    _cairo_output_stream_write (base85_stream, &c, 1);
	} else {
	    if (font_private->has_sfnt)
		_cairo_output_stream_printf (ctx->stream, " %lu",
					     glyphs[n].index);
	    else
		_cairo_output_stream_printf (ctx->stream, " %lu",
					     (long unsigned) (uintptr_t)scaled_glyph->dev_private);
	}

        dx = scaled_glyph->metrics.x_advance;
        dy = scaled_glyph->metrics.y_advance;
	cairo_matrix_transform_distance (&scaled_font->ctm, &dx, &dy);
	x += dx;
	y += dy;
    }
    _cairo_scaled_font_thaw_cache (scaled_font);

    if (base85_stream != NULL) {
	cairo_status_t status2;

	status2 = _cairo_output_stream_destroy (base85_stream);
	if (status == CAIRO_STATUS_SUCCESS)
	    status = status2;

	_cairo_output_stream_printf (ctx->stream, "~>");
    } else {
	_cairo_output_stream_puts (ctx->stream, " ]");
    }
    if (unlikely (status))
	return status;

    if (utf8 != NULL && clusters != NULL) {
	for (n = 0; n < num_clusters; n++) {
	    if (clusters[n].num_bytes > UCHAR_MAX ||
		clusters[n].num_glyphs > UCHAR_MAX)
	    {
		break;
	    }
	}

	if (n < num_clusters) {
	    _cairo_output_stream_puts (ctx->stream, "] [ ");
	    for (n = 0; n < num_clusters; n++) {
		_cairo_output_stream_printf (ctx->stream,
					     "%d %d ",
					     clusters[n].num_bytes,
					     clusters[n].num_glyphs);
	    }
	    _cairo_output_stream_puts (ctx->stream, "]");
	}
	else
	{
	    _cairo_output_stream_puts (ctx->stream, "] <~");
	    base85_stream = _cairo_base85_stream_create (ctx->stream);
	    for (n = 0; n < num_clusters; n++) {
		uint8_t c[2];
		c[0] = clusters[n].num_bytes;
		c[1] = clusters[n].num_glyphs;
		_cairo_output_stream_write (base85_stream, c, 2);
	    }
	    status = _cairo_output_stream_destroy (base85_stream);
	    if (unlikely (status))
		goto BAIL;

	    _cairo_output_stream_puts (ctx->stream, "~>");
	}

	_cairo_output_stream_printf (ctx->stream,
				     " //%s show-text-glyphs\n",
				     _direction_to_string (backward));
    } else {
	_cairo_output_stream_puts (ctx->stream,
				   "] show-glyphs\n");
    }

    inactive (surface);

    if (_cairo_surface_wrapper_is_active (&surface->wrapper)){
	return _cairo_surface_wrapper_show_text_glyphs (&surface->wrapper,
							op, source, 0,
							utf8, utf8_len,
							glyphs, num_glyphs,
							clusters, num_clusters,
							backward,
							scaled_font,
							clip);
    }

    return CAIRO_STATUS_SUCCESS;

BAIL:
    inactive (surface);
    return status;
}

static cairo_bool_t
_cairo_script_surface_get_extents (void *abstract_surface,
				   cairo_rectangle_int_t *rectangle)
{
    cairo_script_surface_t *surface = abstract_surface;

    if (_cairo_surface_wrapper_is_active (&surface->wrapper)) {
	return _cairo_surface_wrapper_get_extents (&surface->wrapper,
						   rectangle);
    }

    if (surface->width < 0 || surface->height < 0)
	return FALSE;

    rectangle->x = 0;
    rectangle->y = 0;
    rectangle->width = surface->width;
    rectangle->height = surface->height;

    return TRUE;
}

static const cairo_surface_backend_t
_cairo_script_surface_backend = {
    CAIRO_SURFACE_TYPE_SCRIPT,
    _cairo_script_surface_finish,

    _cairo_default_context_create,

    _cairo_script_surface_create_similar,
    NULL, /* create similar image */
    NULL, /* map to image */
    NULL, /* unmap image */

    _cairo_script_surface_source,
    _cairo_script_surface_acquire_source_image,
    _cairo_script_surface_release_source_image,
    _cairo_script_surface_snapshot,

    _cairo_script_surface_copy_page,
    _cairo_script_surface_show_page,

    _cairo_script_surface_get_extents,
    NULL, /* get_font_options */

    NULL, /* flush */
    NULL, /* mark_dirty_rectangle */

    _cairo_script_surface_paint,
    _cairo_script_surface_mask,
    _cairo_script_surface_stroke,
    _cairo_script_surface_fill,
    NULL, /* fill/stroke */
    NULL, /* glyphs */
    _cairo_script_surface_has_show_text_glyphs,
    _cairo_script_surface_show_text_glyphs
};

static void
_cairo_script_implicit_context_init (cairo_script_implicit_context_t *cr)
{
    cr->current_operator = CAIRO_GSTATE_OPERATOR_DEFAULT;
    cr->current_fill_rule = CAIRO_GSTATE_FILL_RULE_DEFAULT;
    cr->current_tolerance = CAIRO_GSTATE_TOLERANCE_DEFAULT;
    cr->current_antialias = CAIRO_ANTIALIAS_DEFAULT;
    _cairo_stroke_style_init (&cr->current_style);
    _cairo_pattern_init_solid (&cr->current_source.solid,
			       CAIRO_COLOR_BLACK);
    _cairo_path_fixed_init (&cr->current_path);
    cairo_matrix_init_identity (&cr->current_ctm);
    cairo_matrix_init_identity (&cr->current_stroke_matrix);
    cairo_matrix_init_identity (&cr->current_font_matrix);
    _cairo_font_options_init_default (&cr->current_font_options);
    cr->current_scaled_font = NULL;
    cr->has_clip = FALSE;
}

static void
_cairo_script_implicit_context_reset (cairo_script_implicit_context_t *cr)
{
    free (cr->current_style.dash);
    cr->current_style.dash = NULL;

    _cairo_pattern_fini (&cr->current_source.base);
    _cairo_path_fixed_fini (&cr->current_path);

    _cairo_script_implicit_context_init (cr);
}

static cairo_script_surface_t *
_cairo_script_surface_create_internal (cairo_script_context_t *ctx,
				       cairo_content_t content,
				       cairo_rectangle_t *extents,
				       cairo_surface_t *passthrough)
{
    cairo_script_surface_t *surface;

    if (unlikely (ctx == NULL))
	return (cairo_script_surface_t *) _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NULL_POINTER));

    surface = _cairo_malloc (sizeof (cairo_script_surface_t));
    if (unlikely (surface == NULL))
	return (cairo_script_surface_t *) _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    _cairo_surface_init (&surface->base,
			 &_cairo_script_surface_backend,
			 &ctx->base,
			 content,
			 TRUE); /* is_vector */

    _cairo_surface_wrapper_init (&surface->wrapper, passthrough);

    _cairo_surface_clipper_init (&surface->clipper,
				 _cairo_script_surface_clipper_intersect_clip_path);

    surface->width = surface->height = -1;
    if (extents) {
	surface->width = extents->width;
	surface->height = extents->height;
	cairo_surface_set_device_offset (&surface->base,
					 -extents->x, -extents->y);
    }

    surface->emitted = FALSE;
    surface->defined = FALSE;
    surface->active = FALSE;
    surface->operand.type = SURFACE;
    cairo_list_init (&surface->operand.link);

    _cairo_script_implicit_context_init (&surface->cr);

    return surface;
}

static const cairo_device_backend_t _cairo_script_device_backend = {
    CAIRO_DEVICE_TYPE_SCRIPT,

    NULL, NULL, /* lock, unlock */

    _device_flush,  /* flush */
    _device_finish,  /* finish */
    _device_destroy
};

cairo_device_t *
_cairo_script_context_create_internal (cairo_output_stream_t *stream)
{
    cairo_script_context_t *ctx;

    ctx = _cairo_malloc (sizeof (cairo_script_context_t));
    if (unlikely (ctx == NULL))
	return _cairo_device_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    memset (ctx, 0, sizeof (cairo_script_context_t));

    _cairo_device_init (&ctx->base, &_cairo_script_device_backend);

    cairo_list_init (&ctx->operands);
    cairo_list_init (&ctx->deferred);
    ctx->stream = stream;
    ctx->mode = CAIRO_SCRIPT_MODE_ASCII;

    cairo_list_init (&ctx->fonts);
    cairo_list_init (&ctx->defines);

    ctx->attach_snapshots = TRUE;

    return &ctx->base;
}

void
_cairo_script_context_attach_snapshots (cairo_device_t *device,
					cairo_bool_t enable)
{
    cairo_script_context_t *ctx;

    ctx = (cairo_script_context_t *) device;
    ctx->attach_snapshots = enable;
}

static cairo_device_t *
_cairo_script_context_create (cairo_output_stream_t *stream)
{
    cairo_script_context_t *ctx;

    ctx = (cairo_script_context_t *)
	_cairo_script_context_create_internal (stream);
    if (unlikely (ctx->base.status))
	return &ctx->base;

    ctx->owns_stream = TRUE;
    _cairo_output_stream_puts (ctx->stream, "%!CairoScript\n");
    return &ctx->base;
}

/**
 * cairo_script_create:
 * @filename: the name (path) of the file to write the script to
 *
 * Creates a output device for emitting the script, used when
 * creating the individual surfaces.
 *
 * Return value: a pointer to the newly created device. The caller
 * owns the surface and should call cairo_device_destroy() when done
 * with it.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" device if an error such as out of memory
 * occurs. You can use cairo_device_status() to check for this.
 *
 * Since: 1.12
 **/
cairo_device_t *
cairo_script_create (const char *filename)
{
    cairo_output_stream_t *stream;
    cairo_status_t status;

    stream = _cairo_output_stream_create_for_filename (filename);
    if ((status = _cairo_output_stream_get_status (stream)))
	return _cairo_device_create_in_error (status);

    return _cairo_script_context_create (stream);
}

/**
 * cairo_script_create_for_stream:
 * @write_func: callback function passed the bytes written to the script
 * @closure: user data to be passed to the callback
 *
 * Creates a output device for emitting the script, used when
 * creating the individual surfaces.
 *
 * Return value: a pointer to the newly created device. The caller
 * owns the surface and should call cairo_device_destroy() when done
 * with it.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" device if an error such as out of memory
 * occurs. You can use cairo_device_status() to check for this.
 *
 * Since: 1.12
 **/
cairo_device_t *
cairo_script_create_for_stream (cairo_write_func_t	 write_func,
				void			*closure)
{
    cairo_output_stream_t *stream;
    cairo_status_t status;

    stream = _cairo_output_stream_create (write_func, NULL, closure);
    if ((status = _cairo_output_stream_get_status (stream)))
	return _cairo_device_create_in_error (status);

    return _cairo_script_context_create (stream);
}

/**
 * cairo_script_write_comment:
 * @script: the script (output device)
 * @comment: the string to emit
 * @len:the length of the string to write, or -1 to use strlen()
 *
 * Emit a string verbatim into the script.
 *
 * Since: 1.12
 **/
void
cairo_script_write_comment (cairo_device_t *script,
			    const char *comment,
			    int len)
{
    cairo_script_context_t *context = (cairo_script_context_t *) script;

    if (len < 0)
	len = strlen (comment);

    _cairo_output_stream_puts (context->stream, "% ");
    _cairo_output_stream_write (context->stream, comment, len);
    _cairo_output_stream_puts (context->stream, "\n");
}

/**
 * cairo_script_set_mode:
 * @script: The script (output device)
 * @mode: the new mode
 *
 * Change the output mode of the script
 *
 * Since: 1.12
 **/
void
cairo_script_set_mode (cairo_device_t *script,
		       cairo_script_mode_t mode)
{
    cairo_script_context_t *context = (cairo_script_context_t *) script;

    context->mode = mode;
}

/**
 * cairo_script_get_mode:
 * @script: The script (output device) to query
 *
 * Queries the script for its current output mode.
 *
 * Return value: the current output mode of the script
 *
 * Since: 1.12
 **/
cairo_script_mode_t
cairo_script_get_mode (cairo_device_t *script)
{
    cairo_script_context_t *context = (cairo_script_context_t *) script;

    return context->mode;
}

/**
 * cairo_script_surface_create:
 * @script: the script (output device)
 * @content: the content of the surface
 * @width: width in pixels
 * @height: height in pixels
 *
 * Create a new surface that will emit its rendering through @script
 *
 * Return value: a pointer to the newly created surface. The caller
 * owns the surface and should call cairo_surface_destroy() when done
 * with it.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" surface if an error such as out of memory
 * occurs. You can use cairo_surface_status() to check for this.
 *
 * Since: 1.12
 **/
cairo_surface_t *
cairo_script_surface_create (cairo_device_t *script,
			     cairo_content_t content,
			     double width,
			     double height)
{
    cairo_rectangle_t *extents, r;

    if (unlikely (script->backend->type != CAIRO_DEVICE_TYPE_SCRIPT))
	return _cairo_surface_create_in_error (CAIRO_STATUS_DEVICE_TYPE_MISMATCH);

    if (unlikely (script->status))
	return _cairo_surface_create_in_error (script->status);

    extents = NULL;
    if (width > 0 && height > 0) {
	r.x = r.y = 0;
	r.width  = width;
	r.height = height;
	extents = &r;
    }
    return &_cairo_script_surface_create_internal ((cairo_script_context_t *) script,
						   content, extents,
						   NULL)->base;
}
slim_hidden_def (cairo_script_surface_create);

/**
 * cairo_script_surface_create_for_target:
 * @script: the script (output device)
 * @target: a target surface to wrap
 *
 * Create a pxoy surface that will render to @target and record
 * the operations to @device.
 *
 * Return value: a pointer to the newly created surface. The caller
 * owns the surface and should call cairo_surface_destroy() when done
 * with it.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" surface if an error such as out of memory
 * occurs. You can use cairo_surface_status() to check for this.
 *
 * Since: 1.12
 **/
cairo_surface_t *
cairo_script_surface_create_for_target (cairo_device_t *script,
					cairo_surface_t *target)
{
    cairo_rectangle_int_t extents;
    cairo_rectangle_t rect, *r;

    if (unlikely (script->backend->type != CAIRO_DEVICE_TYPE_SCRIPT))
	return _cairo_surface_create_in_error (CAIRO_STATUS_DEVICE_TYPE_MISMATCH);

    if (unlikely (script->status))
	return _cairo_surface_create_in_error (script->status);

    if (unlikely (target->status))
	return _cairo_surface_create_in_error (target->status);

    r = NULL;
    if (_cairo_surface_get_extents (target, &extents)) {
	rect.x = rect.y = 0;
	rect.width = extents.width;
	rect.height = extents.height;
	r= &rect;
    }
    return &_cairo_script_surface_create_internal ((cairo_script_context_t *) script,
						   target->content, r,
						   target)->base;
}

/**
 * cairo_script_from_recording_surface:
 * @script: the script (output device)
 * @recording_surface: the recording surface to replay
 *
 * Converts the record operations in @recording_surface into a script.
 *
 * Return value: #CAIRO_STATUS_SUCCESS on successful completion or an error code.
 *
 * Since: 1.12
 **/
cairo_status_t
cairo_script_from_recording_surface (cairo_device_t *script,
				     cairo_surface_t *recording_surface)
{
    cairo_rectangle_t r, *extents;
    cairo_surface_t *surface;
    cairo_status_t status;

    if (unlikely (script->backend->type != CAIRO_DEVICE_TYPE_SCRIPT))
	return _cairo_error (CAIRO_STATUS_DEVICE_TYPE_MISMATCH);

    if (unlikely (script->status))
	return _cairo_error (script->status);

    if (unlikely (recording_surface->status))
	return recording_surface->status;

    if (unlikely (! _cairo_surface_is_recording (recording_surface)))
	return _cairo_error (CAIRO_STATUS_SURFACE_TYPE_MISMATCH);

    extents = NULL;
    if (_cairo_recording_surface_get_bounds (recording_surface, &r))
	extents = &r;

    surface = &_cairo_script_surface_create_internal ((cairo_script_context_t *) script,
						      recording_surface->content,
						      extents,
						      NULL)->base;
    if (unlikely (surface->status))
	return surface->status;

    status = _cairo_recording_surface_replay (recording_surface, surface);
    cairo_surface_destroy (surface);

    return status;
}
