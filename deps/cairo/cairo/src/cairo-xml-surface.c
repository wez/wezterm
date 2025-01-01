/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2009 Chris Wilson
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

/* This surface is intended to produce a verbose, hierarchical, DAG XML file
 * representing a single surface. It is intended to be used by debuggers,
 * such as cairo-sphinx, or by application test-suites that want a log of
 * operations.
 */

#include "cairoint.h"

#include "cairo-xml.h"

#include "cairo-clip-private.h"
#include "cairo-device-private.h"
#include "cairo-default-context-private.h"
#include "cairo-image-surface-private.h"
#include "cairo-error-private.h"
#include "cairo-output-stream-private.h"
#include "cairo-recording-surface-inline.h"

#define static cairo_warn static

typedef struct _cairo_xml_surface cairo_xml_surface_t;

typedef struct _cairo_xml {
    cairo_device_t base;

    cairo_output_stream_t *stream;
    int indent;
} cairo_xml_t;

struct _cairo_xml_surface {
    cairo_surface_t base;

    double width, height;
};

slim_hidden_proto (cairo_xml_for_recording_surface);

static const cairo_surface_backend_t _cairo_xml_surface_backend;

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
	"DEFAULT",	/* CAIRO_ANTIALIAS_DEFAULT */
	"NONE",         /* CAIRO_ANTIALIAS_NONE */
	"GRAY",         /* CAIRO_ANTIALIAS_GRAY */
	"SUBPIXEL",	/* CAIRO_ANTIALIAS_SUBPIXEL */
	"FAST",         /* CAIRO_ANTIALIAS_FAST */
	"GOOD",         /* CAIRO_ANTIALIAS_GOOD */
	"BEST",         /* CAIRO_ANTIALIAS_BEST */
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

static const char *
_format_to_string (cairo_format_t format)
{
    switch (format) {
    case CAIRO_FORMAT_ARGB32:    return "ARGB32";
    case CAIRO_FORMAT_RGB30:     return "RGB30";
    case CAIRO_FORMAT_RGB24:     return "RGB24";
    case CAIRO_FORMAT_RGB16_565: return "RGB16_565";
    case CAIRO_FORMAT_RGB96F:    return "RGB96F";
    case CAIRO_FORMAT_RGBA128F:  return "RGBA128F";
    case CAIRO_FORMAT_A8:        return "A8";
    case CAIRO_FORMAT_A1:        return "A1";
    case CAIRO_FORMAT_INVALID:   return "INVALID";
    }
    ASSERT_NOT_REACHED;
    return "INVALID";
}

static cairo_status_t
_device_flush (void *abstract_device)
{
    cairo_xml_t *xml = abstract_device;
    cairo_status_t status;

    status = _cairo_output_stream_flush (xml->stream);

    return status;
}

static void
_device_destroy (void *abstract_device)
{
    cairo_xml_t *xml = abstract_device;
    cairo_status_t status;

    status = _cairo_output_stream_destroy (xml->stream);

    free (xml);
}

static const cairo_device_backend_t _cairo_xml_device_backend = {
    CAIRO_DEVICE_TYPE_XML,

    NULL, NULL, /* lock, unlock */

    _device_flush,
    NULL,  /* finish */
    _device_destroy
};

static cairo_device_t *
_cairo_xml_create_internal (cairo_output_stream_t *stream)
{
    cairo_xml_t *xml;

    xml = _cairo_malloc (sizeof (cairo_xml_t));
    if (unlikely (xml == NULL))
	return _cairo_device_create_in_error (CAIRO_STATUS_NO_MEMORY);

    memset (xml, 0, sizeof (cairo_xml_t));

    _cairo_device_init (&xml->base, &_cairo_xml_device_backend);

    xml->indent = 0;
    xml->stream = stream;

    return &xml->base;
}

static void
_cairo_xml_indent (cairo_xml_t *xml, int indent)
{
    xml->indent += indent;
    assert (xml->indent >= 0);
}

static void CAIRO_PRINTF_FORMAT (2, 3)
_cairo_xml_printf (cairo_xml_t *xml, const char *fmt, ...)
{
    va_list ap;
    char indent[80];
    int len;

    len = MIN (xml->indent, ARRAY_LENGTH (indent));
    memset (indent, ' ', len);
    _cairo_output_stream_write (xml->stream, indent, len);

    va_start (ap, fmt);
    _cairo_output_stream_vprintf (xml->stream, fmt, ap);
    va_end (ap);

    _cairo_output_stream_write (xml->stream, "\n", 1);
}

static void CAIRO_PRINTF_FORMAT (2, 3)
_cairo_xml_printf_start (cairo_xml_t *xml, const char *fmt, ...)
{
    char indent[80];
    int len;

    len = MIN (xml->indent, ARRAY_LENGTH (indent));
    memset (indent, ' ', len);
    _cairo_output_stream_write (xml->stream, indent, len);

    if (fmt != NULL) {
	va_list ap;

	va_start (ap, fmt);
	_cairo_output_stream_vprintf (xml->stream, fmt, ap);
	va_end (ap);
    }
}

static void CAIRO_PRINTF_FORMAT (2, 3)
_cairo_xml_printf_continue (cairo_xml_t *xml, const char *fmt, ...)
{
    va_list ap;

    va_start (ap, fmt);
    _cairo_output_stream_vprintf (xml->stream, fmt, ap);
    va_end (ap);
}

static void CAIRO_PRINTF_FORMAT (2, 3)
_cairo_xml_printf_end (cairo_xml_t *xml, const char *fmt, ...)
{
    if (fmt != NULL) {
	va_list ap;

	va_start (ap, fmt);
	_cairo_output_stream_vprintf (xml->stream, fmt, ap);
	va_end (ap);
    }

    _cairo_output_stream_write (xml->stream, "\n", 1);
}

static cairo_surface_t *
_cairo_xml_surface_create_similar (void			*abstract_surface,
				   cairo_content_t	 content,
				   int			 width,
				   int			 height)
{
    cairo_rectangle_t extents;

    extents.x = extents.y = 0;
    extents.width  = width;
    extents.height = height;

    return cairo_recording_surface_create (content, &extents);
}

static cairo_bool_t
_cairo_xml_surface_get_extents (void *abstract_surface,
				cairo_rectangle_int_t *rectangle)
{
    cairo_xml_surface_t *surface = abstract_surface;

    if (surface->width < 0 || surface->height < 0)
	return FALSE;

    rectangle->x = 0;
    rectangle->y = 0;
    rectangle->width  = surface->width;
    rectangle->height = surface->height;

    return TRUE;
}

static cairo_status_t
_cairo_xml_move_to (void *closure,
		    const cairo_point_t *p1)
{
    _cairo_xml_printf_continue (closure, " %f %f m",
				_cairo_fixed_to_double (p1->x),
				_cairo_fixed_to_double (p1->y));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_xml_line_to (void *closure,
		    const cairo_point_t *p1)
{
    _cairo_xml_printf_continue (closure, " %f %f l",
				_cairo_fixed_to_double (p1->x),
				_cairo_fixed_to_double (p1->y));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_xml_curve_to (void *closure,
		     const cairo_point_t *p1,
		     const cairo_point_t *p2,
		     const cairo_point_t *p3)
{
    _cairo_xml_printf_continue (closure, " %f %f %f %f %f %f c",
				_cairo_fixed_to_double (p1->x),
				_cairo_fixed_to_double (p1->y),
				_cairo_fixed_to_double (p2->x),
				_cairo_fixed_to_double (p2->y),
				_cairo_fixed_to_double (p3->x),
				_cairo_fixed_to_double (p3->y));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_xml_close_path (void *closure)
{
    _cairo_xml_printf_continue (closure, " h");

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_xml_emit_path (cairo_xml_t *xml,
		      const cairo_path_fixed_t *path)
{
    cairo_status_t status;

    _cairo_xml_printf_start (xml, "<path>");
    status = _cairo_path_fixed_interpret (path,
					_cairo_xml_move_to,
					_cairo_xml_line_to,
					_cairo_xml_curve_to,
					_cairo_xml_close_path,
					xml);
    assert (status == CAIRO_STATUS_SUCCESS);
    _cairo_xml_printf_end (xml, "</path>");
}

static void
_cairo_xml_emit_string (cairo_xml_t *xml,
			const char *node,
			const char *data)
{
    _cairo_xml_printf (xml, "<%s>%s</%s>", node, data, node);
}

static void
_cairo_xml_emit_double (cairo_xml_t *xml,
			const char *node,
			double data)
{
    _cairo_xml_printf (xml, "<%s>%f</%s>", node, data, node);
}

static cairo_xml_t *
to_xml (cairo_xml_surface_t *surface)
{
    return (cairo_xml_t *) surface->base.device;
}

static cairo_status_t
_cairo_xml_surface_emit_clip_boxes (cairo_xml_surface_t *surface,
				    const cairo_clip_t *clip)
{
    cairo_box_t *box;
    cairo_xml_t *xml;
    int n;

    if (clip->num_boxes == 0)
	return CAIRO_STATUS_SUCCESS;

    /* skip the trivial clip covering the surface extents */
    if (surface->width >= 0 && surface->height >= 0 && clip->num_boxes == 1) {
	box = &clip->boxes[0];
	if (box->p1.x <= 0 && box->p1.y <= 0 &&
	    box->p2.x - box->p1.x >= _cairo_fixed_from_double (surface->width) &&
	    box->p2.y - box->p1.y >= _cairo_fixed_from_double (surface->height))
	{
	    return CAIRO_STATUS_SUCCESS;
	}
    }

    xml = to_xml (surface);

    _cairo_xml_printf (xml, "<clip>");
    _cairo_xml_indent (xml, 2);

    _cairo_xml_printf (xml, "<path>");
    _cairo_xml_indent (xml, 2);
    for (n = 0; n < clip->num_boxes; n++) {
	box = &clip->boxes[n];

	_cairo_xml_printf_start (xml, "%f %f m",
				 _cairo_fixed_to_double (box->p1.x),
				 _cairo_fixed_to_double (box->p1.y));
	_cairo_xml_printf_continue (xml, " %f %f l",
				    _cairo_fixed_to_double (box->p2.x),
				    _cairo_fixed_to_double (box->p1.y));
	_cairo_xml_printf_continue (xml, " %f %f l",
				    _cairo_fixed_to_double (box->p2.x),
				    _cairo_fixed_to_double (box->p2.y));
	_cairo_xml_printf_continue (xml, " %f %f l",
				    _cairo_fixed_to_double (box->p1.x),
				    _cairo_fixed_to_double (box->p2.y));
	_cairo_xml_printf_end (xml, " h");
    }
    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</path>");
    _cairo_xml_emit_double (xml, "tolerance", 1.0);
    _cairo_xml_emit_string (xml, "antialias",
			    _antialias_to_string (CAIRO_ANTIALIAS_NONE));
    _cairo_xml_emit_string (xml, "fill-rule",
			    _fill_rule_to_string (CAIRO_FILL_RULE_WINDING));

    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</clip>");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_xml_surface_emit_clip_path (cairo_xml_surface_t *surface,
				   const cairo_clip_path_t *clip_path)
{
    cairo_box_t box;
    cairo_status_t status;
    cairo_xml_t *xml;

    if (clip_path == NULL)
	return CAIRO_STATUS_SUCCESS;

    status = _cairo_xml_surface_emit_clip_path (surface, clip_path->prev);
    if (unlikely (status))
	return status;

    /* skip the trivial clip covering the surface extents */
    if (surface->width >= 0 && surface->height >= 0 &&
	_cairo_path_fixed_is_box (&clip_path->path, &box))
    {
	if (box.p1.x <= 0 && box.p1.y <= 0 &&
	    box.p2.x - box.p1.x >= _cairo_fixed_from_double (surface->width) &&
	    box.p2.y - box.p1.y >= _cairo_fixed_from_double (surface->height))
	{
	    return CAIRO_STATUS_SUCCESS;
	}
    }

    xml = to_xml (surface);

    _cairo_xml_printf_start (xml, "<clip>");
    _cairo_xml_indent (xml, 2);

    _cairo_xml_emit_path (xml, &clip_path->path);
    _cairo_xml_emit_double (xml, "tolerance", clip_path->tolerance);
    _cairo_xml_emit_string (xml, "antialias",
			    _antialias_to_string (clip_path->antialias));
    _cairo_xml_emit_string (xml, "fill-rule",
			    _fill_rule_to_string (clip_path->fill_rule));

    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf_end (xml, "</clip>");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_xml_surface_emit_clip (cairo_xml_surface_t *surface,
			      const cairo_clip_t *clip)
{
    cairo_status_t status;

    if (clip == NULL)
	return CAIRO_STATUS_SUCCESS;

    status = _cairo_xml_surface_emit_clip_boxes (surface, clip);
    if (unlikely (status))
	return status;

    return _cairo_xml_surface_emit_clip_path (surface, clip->path);
}

static cairo_status_t
_cairo_xml_emit_solid (cairo_xml_t *xml,
		       const cairo_solid_pattern_t *solid)
{
    _cairo_xml_printf (xml, "<solid>%f %f %f %f</solid>",
		       solid->color.red,
		       solid->color.green,
		       solid->color.blue,
		       solid->color.alpha);
    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_xml_emit_matrix (cairo_xml_t *xml,
			const cairo_matrix_t *matrix)
{
    if (! _cairo_matrix_is_identity (matrix)) {
	_cairo_xml_printf (xml, "<matrix>%f %f %f %f %f %f</matrix>",
			   matrix->xx, matrix->yx,
			   matrix->xy, matrix->yy,
			   matrix->x0, matrix->y0);
    }
}

static void
_cairo_xml_emit_gradient (cairo_xml_t *xml,
			  const cairo_gradient_pattern_t *gradient)
{
    unsigned int i;

    for (i = 0; i < gradient->n_stops; i++) {
	_cairo_xml_printf (xml,
			   "<color-stop>%f %f %f %f %f</color-stop>",
			   gradient->stops[i].offset,
			   gradient->stops[i].color.red,
			   gradient->stops[i].color.green,
			   gradient->stops[i].color.blue,
			   gradient->stops[i].color.alpha);
    }
}

static cairo_status_t
_cairo_xml_emit_linear (cairo_xml_t *xml,
			const cairo_linear_pattern_t *linear)
{
    _cairo_xml_printf (xml,
		       "<linear x1='%f' y1='%f' x2='%f' y2='%f'>",
		       linear->pd1.x, linear->pd1.y,
		       linear->pd2.x, linear->pd2.y);
    _cairo_xml_indent (xml, 2);
    _cairo_xml_emit_gradient (xml, &linear->base);
    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</linear>");
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_xml_emit_radial (cairo_xml_t *xml,
			const cairo_radial_pattern_t *radial)
{
    _cairo_xml_printf (xml,
		       "<radial x1='%f' y1='%f' r1='%f' x2='%f' y2='%f' r2='%f'>",
		       radial->cd1.center.x, radial->cd1.center.y, radial->cd1.radius,
		       radial->cd2.center.x, radial->cd2.center.y, radial->cd2.radius);
    _cairo_xml_indent (xml, 2);
    _cairo_xml_emit_gradient (xml, &radial->base);
    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</radial>");
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_write_func (void *closure, const unsigned char *data, unsigned len)
{
    _cairo_output_stream_write (closure, data, len);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_xml_emit_image (cairo_xml_t *xml,
		       cairo_image_surface_t *image)
{
    cairo_output_stream_t *stream;
    cairo_status_t status;

    _cairo_xml_printf_start (xml,
			     "<image width='%d' height='%d' format='%s'>",
			     image->width, image->height,
			     _format_to_string (image->format));

    stream = _cairo_base64_stream_create (xml->stream);
    status = cairo_surface_write_to_png_stream (&image->base,
						_write_func, stream);
    assert (status == CAIRO_STATUS_SUCCESS);
    status = _cairo_output_stream_destroy (stream);
    if (unlikely (status))
	return status;

    _cairo_xml_printf_end (xml, "</image>");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_xml_emit_surface (cairo_xml_t *xml,
			 const cairo_surface_pattern_t *pattern)
{
    cairo_surface_t *source = pattern->surface;
    cairo_status_t status;

    if (_cairo_surface_is_recording (source)) {
	status = cairo_xml_for_recording_surface (&xml->base, source);
    } else {
	cairo_image_surface_t *image;
	void *image_extra;

	status = _cairo_surface_acquire_source_image (source,
						      &image, &image_extra);
	if (unlikely (status))
	    return status;

	status = _cairo_xml_emit_image (xml, image);

	_cairo_surface_release_source_image (source, image, image_extra);
    }

    return status;
}

static cairo_status_t
_cairo_xml_emit_pattern (cairo_xml_t *xml,
			 const char *source_or_mask,
			 const cairo_pattern_t *pattern)
{
    cairo_status_t status;

    _cairo_xml_printf (xml, "<%s-pattern>", source_or_mask);
    _cairo_xml_indent (xml, 2);

    switch (pattern->type) {
    case CAIRO_PATTERN_TYPE_SOLID:
	status = _cairo_xml_emit_solid (xml, (cairo_solid_pattern_t *) pattern);
	break;
    case CAIRO_PATTERN_TYPE_LINEAR:
	status = _cairo_xml_emit_linear (xml, (cairo_linear_pattern_t *) pattern);
	break;
    case CAIRO_PATTERN_TYPE_RADIAL:
	status = _cairo_xml_emit_radial (xml, (cairo_radial_pattern_t *) pattern);
	break;
    case CAIRO_PATTERN_TYPE_SURFACE:
	status = _cairo_xml_emit_surface (xml, (cairo_surface_pattern_t *) pattern);
	break;
    default:
	ASSERT_NOT_REACHED;
	status = CAIRO_INT_STATUS_UNSUPPORTED;
	break;
    }

    if (pattern->type != CAIRO_PATTERN_TYPE_SOLID) {
	_cairo_xml_emit_matrix (xml, &pattern->matrix);
	_cairo_xml_printf (xml,
			   "<extend>%s</extend>",
			   _extend_to_string (pattern->extend));
	_cairo_xml_printf (xml,
			   "<filter>%s</filter>",
			   _filter_to_string (pattern->filter));
    }

    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</%s-pattern>", source_or_mask);

    return status;
}

static cairo_int_status_t
_cairo_xml_surface_paint (void			*abstract_surface,
			  cairo_operator_t	 op,
			  const cairo_pattern_t	*source,
			  const cairo_clip_t	*clip)
{
    cairo_xml_surface_t *surface = abstract_surface;
    cairo_xml_t *xml = to_xml (surface);
    cairo_status_t status;

    _cairo_xml_printf (xml, "<paint>");
    _cairo_xml_indent (xml, 2);

    _cairo_xml_emit_string (xml, "operator", _operator_to_string (op));

    status = _cairo_xml_surface_emit_clip (surface, clip);
    if (unlikely (status))
	return status;

    status = _cairo_xml_emit_pattern (xml, "source", source);
    if (unlikely (status))
	return status;

    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</paint>");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_xml_surface_mask (void			*abstract_surface,
			 cairo_operator_t	 op,
			 const cairo_pattern_t	*source,
			 const cairo_pattern_t	*mask,
			 const cairo_clip_t	*clip)
{
    cairo_xml_surface_t *surface = abstract_surface;
    cairo_xml_t *xml = to_xml (surface);
    cairo_status_t status;

    _cairo_xml_printf (xml, "<mask>");
    _cairo_xml_indent (xml, 2);

    _cairo_xml_emit_string (xml, "operator", _operator_to_string (op));

    status = _cairo_xml_surface_emit_clip (surface, clip);
    if (unlikely (status))
	return status;

    status = _cairo_xml_emit_pattern (xml, "source", source);
    if (unlikely (status))
	return status;

    status = _cairo_xml_emit_pattern (xml, "mask", mask);
    if (unlikely (status))
	return status;

    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</mask>");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_xml_surface_stroke (void				*abstract_surface,
			   cairo_operator_t		 op,
			   const cairo_pattern_t	*source,
			   const cairo_path_fixed_t		*path,
			   const cairo_stroke_style_t		*style,
			   const cairo_matrix_t		*ctm,
			   const cairo_matrix_t		*ctm_inverse,
			   double			 tolerance,
			   cairo_antialias_t		 antialias,
			   const cairo_clip_t		*clip)
{
    cairo_xml_surface_t *surface = abstract_surface;
    cairo_xml_t *xml = to_xml (surface);
    cairo_status_t status;

    _cairo_xml_printf (xml, "<stroke>");
    _cairo_xml_indent (xml, 2);

    _cairo_xml_emit_string (xml, "operator", _operator_to_string (op));
    _cairo_xml_emit_double (xml, "line-width", style->line_width);
    _cairo_xml_emit_double (xml, "miter-limit", style->miter_limit);
    _cairo_xml_emit_string (xml, "line-cap", _line_cap_to_string (style->line_cap));
    _cairo_xml_emit_string (xml, "line-join", _line_join_to_string (style->line_join));

    status = _cairo_xml_surface_emit_clip (surface, clip);
    if (unlikely (status))
	return status;

    status = _cairo_xml_emit_pattern (xml, "source", source);
    if (unlikely (status))
	return status;

    if (style->num_dashes) {
	unsigned int i;

	_cairo_xml_printf_start (xml, "<dash offset='%f'>",
				 style->dash_offset);
	for (i = 0; i < style->num_dashes; i++)
	    _cairo_xml_printf_continue (xml, "%f ", style->dash[i]);

	_cairo_xml_printf_end (xml, "</dash>");
    }

    _cairo_xml_emit_path (xml, path);
    _cairo_xml_emit_double (xml, "tolerance", tolerance);
    _cairo_xml_emit_string (xml, "antialias", _antialias_to_string (antialias));

    _cairo_xml_emit_matrix (xml, ctm);

    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</stroke>");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_xml_surface_fill (void			*abstract_surface,
			 cairo_operator_t	 op,
			 const cairo_pattern_t	*source,
			 const cairo_path_fixed_t*path,
			 cairo_fill_rule_t	 fill_rule,
			 double			 tolerance,
			 cairo_antialias_t	 antialias,
			 const cairo_clip_t	*clip)
{
    cairo_xml_surface_t *surface = abstract_surface;
    cairo_xml_t *xml = to_xml (surface);
    cairo_status_t status;

    _cairo_xml_printf (xml, "<fill>");
    _cairo_xml_indent (xml, 2);

    _cairo_xml_emit_string (xml, "operator", _operator_to_string (op));

    status = _cairo_xml_surface_emit_clip (surface, clip);
    if (unlikely (status))
	return status;

    status = _cairo_xml_emit_pattern (xml, "source", source);
    if (unlikely (status))
	return status;

    _cairo_xml_emit_path (xml, path);
    _cairo_xml_emit_double (xml, "tolerance", tolerance);
    _cairo_xml_emit_string (xml, "antialias", _antialias_to_string (antialias));
    _cairo_xml_emit_string (xml, "fill-rule", _fill_rule_to_string (fill_rule));

    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</fill>");

    return CAIRO_STATUS_SUCCESS;
}

#if CAIRO_HAS_FT_FONT
#include "cairo-ft-private.h"
static cairo_status_t
_cairo_xml_emit_type42_font (cairo_xml_t *xml,
			     cairo_scaled_font_t *scaled_font)
{
    const cairo_scaled_font_backend_t *backend;
    cairo_output_stream_t *base64_stream;
    cairo_output_stream_t *zlib_stream;
    cairo_status_t status, status2;
    unsigned long size;
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

    _cairo_xml_printf_start (xml, "<font type='42' flags='%d' index='0'>",
		       _cairo_ft_scaled_font_get_load_flags (scaled_font));


    base64_stream = _cairo_base64_stream_create (xml->stream);
    len = size;
    _cairo_output_stream_write (base64_stream, &len, sizeof (len));

    zlib_stream = _cairo_deflate_stream_create (base64_stream);

    _cairo_output_stream_write (zlib_stream, buf, size);
    free (buf);

    status2 = _cairo_output_stream_destroy (zlib_stream);
    if (status == CAIRO_STATUS_SUCCESS)
	status = status2;

    status2 = _cairo_output_stream_destroy (base64_stream);
    if (status == CAIRO_STATUS_SUCCESS)
	status = status2;

    _cairo_xml_printf_end (xml, "</font>");

    return status;
}
#else
static cairo_status_t
_cairo_xml_emit_type42_font (cairo_xml_t *xml,
			     cairo_scaled_font_t *scaled_font)
{
    return CAIRO_INT_STATUS_UNSUPPORTED;
}
#endif

static cairo_status_t
_cairo_xml_emit_type3_font (cairo_xml_t *xml,
			    cairo_scaled_font_t *scaled_font,
			    cairo_glyph_t *glyphs,
			    int num_glyphs)
{
    _cairo_xml_printf_start (xml, "<font type='3'>");
    _cairo_xml_printf_end (xml, "</font>");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_xml_emit_scaled_font (cairo_xml_t *xml,
			     cairo_scaled_font_t *scaled_font,
			     cairo_glyph_t *glyphs,
			     int num_glyphs)
{
    cairo_int_status_t status;

    _cairo_xml_printf (xml, "<scaled-font>");
    _cairo_xml_indent (xml, 2);

    status = _cairo_xml_emit_type42_font (xml, scaled_font);
    if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
	status = _cairo_xml_emit_type3_font (xml, scaled_font,
					     glyphs, num_glyphs);
    }

    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</scaled-font>");

    return status;
}

static cairo_int_status_t
_cairo_xml_surface_glyphs (void			    *abstract_surface,
			   cairo_operator_t	     op,
			   const cairo_pattern_t    *source,
			   cairo_glyph_t	    *glyphs,
			   int			     num_glyphs,
			   cairo_scaled_font_t	    *scaled_font,
			   const cairo_clip_t       *clip)
{
    cairo_xml_surface_t *surface = abstract_surface;
    cairo_xml_t *xml = to_xml (surface);
    cairo_status_t status;
    int i;

    _cairo_xml_printf (xml, "<glyphs>");
    _cairo_xml_indent (xml, 2);

    _cairo_xml_emit_string (xml, "operator", _operator_to_string (op));

    status = _cairo_xml_surface_emit_clip (surface, clip);
    if (unlikely (status))
	return status;

    status = _cairo_xml_emit_pattern (xml, "source", source);
    if (unlikely (status))
	return status;

    status = _cairo_xml_emit_scaled_font (xml, scaled_font, glyphs, num_glyphs);
    if (unlikely (status))
	return status;

    for (i = 0; i < num_glyphs; i++) {
	_cairo_xml_printf (xml, "<glyph index='%lu'>%f %f</glyph>",
			   glyphs[i].index,
			   glyphs[i].x,
			   glyphs[i].y);
    }

    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</glyphs>");

    return CAIRO_STATUS_SUCCESS;
}

static const cairo_surface_backend_t
_cairo_xml_surface_backend = {
    CAIRO_SURFACE_TYPE_XML,
    NULL,

    _cairo_default_context_create,

    _cairo_xml_surface_create_similar,
    NULL, /* create_similar_image */
    NULL, /* map_to_image */
    NULL, /* unmap_image */

    _cairo_surface_default_source,
    NULL, /* acquire source image */
    NULL, /* release source image */
    NULL, /* snapshot */

    NULL, /* copy page */
    NULL, /* show page */

    _cairo_xml_surface_get_extents,
    NULL, /* get_font_options */

    NULL, /* flush */
    NULL, /* mark_dirty_rectangle */

    _cairo_xml_surface_paint,
    _cairo_xml_surface_mask,
    _cairo_xml_surface_stroke,
    _cairo_xml_surface_fill,
    NULL, /* fill_stroke */
    _cairo_xml_surface_glyphs,
};

static cairo_surface_t *
_cairo_xml_surface_create_internal (cairo_device_t *device,
				    cairo_content_t content,
				    double width,
				    double height)
{
    cairo_xml_surface_t *surface;

    surface = _cairo_malloc (sizeof (cairo_xml_surface_t));
    if (unlikely (surface == NULL))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    _cairo_surface_init (&surface->base,
			 &_cairo_xml_surface_backend,
			 device,
			 content,
			 TRUE); /* is_vector */

    surface->width = width;
    surface->height = height;

    return &surface->base;
}

cairo_device_t *
cairo_xml_create (const char *filename)
{
    cairo_output_stream_t *stream;
    cairo_status_t status;

    stream = _cairo_output_stream_create_for_filename (filename);
    if ((status = _cairo_output_stream_get_status (stream)))
	return _cairo_device_create_in_error (status);

    return _cairo_xml_create_internal (stream);
}

cairo_device_t *
cairo_xml_create_for_stream (cairo_write_func_t	 write_func,
			     void		*closure)
{
    cairo_output_stream_t *stream;
    cairo_status_t status;

    stream = _cairo_output_stream_create (write_func, NULL, closure);
    if ((status = _cairo_output_stream_get_status (stream)))
	return _cairo_device_create_in_error (status);

    return _cairo_xml_create_internal (stream);
}

cairo_surface_t *
cairo_xml_surface_create (cairo_device_t *device,
			  cairo_content_t content,
			  double width, double height)
{
    if (unlikely (device->backend->type != CAIRO_DEVICE_TYPE_XML))
	return _cairo_surface_create_in_error (CAIRO_STATUS_DEVICE_TYPE_MISMATCH);

    if (unlikely (device->status))
	return _cairo_surface_create_in_error (device->status);

    return _cairo_xml_surface_create_internal (device, content, width, height);
}

cairo_status_t
cairo_xml_for_recording_surface (cairo_device_t	 *device,
				 cairo_surface_t *recording_surface)
{
    cairo_box_t bbox;
    cairo_rectangle_int_t extents;
    cairo_surface_t *surface;
    cairo_xml_t *xml;
    cairo_status_t status;

    if (unlikely (device->status))
	return device->status;

    if (unlikely (recording_surface->status))
	return recording_surface->status;

    if (unlikely (device->backend->type != CAIRO_DEVICE_TYPE_XML))
	return _cairo_error (CAIRO_STATUS_DEVICE_TYPE_MISMATCH);

    if (unlikely (! _cairo_surface_is_recording (recording_surface)))
	return _cairo_error (CAIRO_STATUS_SURFACE_TYPE_MISMATCH);

    status = _cairo_recording_surface_get_bbox ((cairo_recording_surface_t *) recording_surface,
						&bbox, NULL);
    if (unlikely (status))
	return status;

    _cairo_box_round_to_rectangle (&bbox, &extents);
    surface = _cairo_xml_surface_create_internal (device,
						  recording_surface->content,
						  extents.width,
						  extents.height);
    if (unlikely (surface->status))
	return surface->status;

    xml = (cairo_xml_t *) device;

    _cairo_xml_printf (xml,
		       "<surface content='%s' width='%d' height='%d'>",
		       _content_to_string (recording_surface->content),
		       extents.width, extents.height);
    _cairo_xml_indent (xml, 2);

    cairo_surface_set_device_offset (surface, -extents.x, -extents.y);
    status = _cairo_recording_surface_replay (recording_surface, surface);
    cairo_surface_destroy (surface);

    _cairo_xml_indent (xml, -2);
    _cairo_xml_printf (xml, "</surface>");

    return status;
}
slim_hidden_def (cairo_xml_for_recording_surface);
