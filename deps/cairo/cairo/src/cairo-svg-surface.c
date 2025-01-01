/* vim: set sw=4 sts=4: -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2004 Red Hat, Inc
 * Copyright © 2005-2007 Emmanuel Pacaud <emmanuel.pacaud@free.fr>
 * Copyright © 2006 Red Hat, Inc
 * Copyright © 2020-2021 Anton Danilkin <afdw@yandex.ru>
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
 *	Kristian Høgsberg <krh@redhat.com>
 *	Emmanuel Pacaud <emmanuel.pacaud@free.fr>
 *	Carl Worth <cworth@cworth.org>
 *	Anton Danilkin <afdw@yandex.ru>
 */

#include "cairoint.h"

#include "cairo-svg.h"

#include "cairo-array-private.h"
#include "cairo-default-context-private.h"
#include "cairo-error-private.h"
#include "cairo-image-info-private.h"
#include "cairo-image-surface-private.h"
#include "cairo-recording-surface-inline.h"
#include "cairo-output-stream-private.h"
#include "cairo-paginated-private.h"
#include "cairo-scaled-font-subsets-private.h"
#include "cairo-surface-clipper-private.h"
#include "cairo-surface-snapshot-inline.h"
#include "cairo-svg-surface-private.h"

/**
 * SECTION:cairo-svg
 * @Title: SVG Surfaces
 * @Short_Description: Rendering SVG documents
 * @See_Also: #cairo_surface_t
 *
 * The SVG surface is used to render cairo graphics to
 * SVG files and is a multi-page vector surface backend.
 **/

typedef struct _cairo_svg_source_surface {
    cairo_hash_entry_t base;
    unsigned int id;
    unsigned char *unique_id;
    unsigned long unique_id_length;
    cairo_bool_t transitive_paint_used;
} cairo_svg_source_surface_t;

/*
 * _cairo_svg_paint_element and _cairo_svg_paint are used to implement paints in transformed recording patterns.
 */

typedef struct _cairo_svg_paint_element {
    unsigned int source_id;
    cairo_matrix_t matrix;
} cairo_svg_paint_element_t;

typedef struct _cairo_svg_paint {
    cairo_hash_entry_t base;
    unsigned int source_id;
    cairo_array_t paint_elements;
    cairo_box_double_t box;
} cairo_svg_paint_t;

static void
_cairo_svg_source_surface_init_key (cairo_svg_source_surface_t *source_surface)
{
    if (source_surface->unique_id && source_surface->unique_id_length > 0) {
	source_surface->base.hash = _cairo_hash_bytes (_CAIRO_HASH_INIT_VALUE,
						       source_surface->unique_id,
						       source_surface->unique_id_length);
    } else {
	source_surface->base.hash = source_surface->id;
    }
}

static cairo_bool_t
_cairo_svg_source_surface_equal (const void *key_a, const void *key_b)
{
    const cairo_svg_source_surface_t *a = key_a;
    const cairo_svg_source_surface_t *b = key_b;

    if (a->unique_id && b->unique_id && a->unique_id_length == b->unique_id_length) {
	return memcmp (a->unique_id, b->unique_id, a->unique_id_length) == 0;
    }

    return a->id == b->id;
}

static void
_cairo_svg_source_surface_pluck (void *entry, void *closure)
{
    cairo_svg_source_surface_t *source_surface = entry;
    cairo_hash_table_t *patterns = closure;

    _cairo_hash_table_remove (patterns, &source_surface->base);
    free (source_surface->unique_id);
    free (source_surface);
}

static void
_cairo_svg_paint_init_key (cairo_svg_paint_t *paint)
{
    paint->base.hash = paint->source_id;
}

static cairo_bool_t
_cairo_svg_paint_equal (const void *key_a, const void *key_b)
{
    const cairo_svg_paint_t *a = key_a;
    const cairo_svg_paint_t *b = key_b;

    return a->source_id == b->source_id;
}

static void
_cairo_svg_paint_pluck (void *entry, void *closure)
{
    cairo_svg_paint_t *paint = entry;
    cairo_hash_table_t *patterns = closure;

    _cairo_hash_table_remove (patterns, &paint->base);
    _cairo_array_fini (&paint->paint_elements);
    free (paint);
}

static void
_cairo_svg_paint_box_add_padding (cairo_box_double_t *box)
{
    double width = box->p2.x - box->p1.x;
    double height = box->p2.y - box->p1.y;

    box->p1.x -= width / 10.0;
    box->p1.y -= height / 10.0;
    box->p2.x += width / 10.0;
    box->p2.y += height / 10.0;
}

enum cairo_svg_stream_element_type {
    CAIRO_SVG_STREAM_ELEMENT_TYPE_TEXT,
    CAIRO_SVG_STREAM_ELEMENT_TYPE_PAINT_DEPENDENT,
};

enum cairo_svg_stream_paint_dependent_element_type {
    CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE,
    CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE_AT_ORIGIN,
    CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_TRANSLATION,
    CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_INVERSE_TRANSLATION,
};

typedef struct _cairo_svg_stream_element {
    enum cairo_svg_stream_element_type type;
    union {
        struct {
	    cairo_output_stream_t *output_stream;
	} text;
        struct {
	    unsigned int source_id;
	    enum cairo_svg_stream_paint_dependent_element_type type;
        } paint_dependent;
    };
} cairo_svg_stream_element_t;

typedef struct _cairo_svg_stream {
    cairo_status_t status;
    cairo_array_t elements;
} cairo_svg_stream_t;

static cairo_svg_stream_t
_cairo_svg_stream_create ()
{
    cairo_svg_stream_t svg_stream;
    svg_stream.status = CAIRO_STATUS_SUCCESS;
    _cairo_array_init (&svg_stream.elements, sizeof (cairo_svg_stream_element_t));
    return svg_stream;
}

static void
_cairo_svg_stream_write (cairo_svg_stream_t *svg_stream,
			 const void *data,
			 size_t length)
{
    cairo_status_t status;

    cairo_svg_stream_element_t *last_element = NULL;
    if (svg_stream->elements.num_elements > 0) {
	last_element = _cairo_array_index (&svg_stream->elements,
					   svg_stream->elements.num_elements - 1);
    }

    if (last_element == NULL || last_element->type != CAIRO_SVG_STREAM_ELEMENT_TYPE_TEXT) {
	cairo_svg_stream_element_t element;
	element.type = CAIRO_SVG_STREAM_ELEMENT_TYPE_TEXT;
	element.text.output_stream = _cairo_memory_stream_create();
	status = _cairo_array_append (&svg_stream->elements, &element);
	if (unlikely (status)) {
	    if (svg_stream->status == CAIRO_STATUS_SUCCESS) {
		svg_stream->status = status;
	    }
	    return;
	}
	last_element = _cairo_array_index (&svg_stream->elements,
					   svg_stream->elements.num_elements - 1);
    }

    _cairo_output_stream_write (last_element->text.output_stream, data, length);
}

static void CAIRO_PRINTF_FORMAT (2, 0)
_cairo_svg_stream_printf (cairo_svg_stream_t *svg_stream,
			  const char *fmt,
			  ...)
{
    cairo_status_t status;

    cairo_svg_stream_element_t *last_element = NULL;
    if (svg_stream->elements.num_elements > 0) {
	last_element = _cairo_array_index (&svg_stream->elements,
					   svg_stream->elements.num_elements - 1);
    }

    if (last_element == NULL || last_element->type != CAIRO_SVG_STREAM_ELEMENT_TYPE_TEXT) {
        cairo_svg_stream_element_t element;
	element.type = CAIRO_SVG_STREAM_ELEMENT_TYPE_TEXT;
	element.text.output_stream = _cairo_memory_stream_create();
	status = _cairo_array_append (&svg_stream->elements, &element);
	if (unlikely (status)) {
	    if (svg_stream->status == CAIRO_STATUS_SUCCESS) {
		svg_stream->status = status;
	    }
	    return;
	}
	last_element = _cairo_array_index (&svg_stream->elements,
					   svg_stream->elements.num_elements - 1);
    }

    va_list ap;
    va_start (ap, fmt);
    _cairo_output_stream_vprintf (last_element->text.output_stream, fmt, ap);
    va_end (ap);
}

static void
_cairo_svg_stream_append_paint_dependent (cairo_svg_stream_t *svg_stream,
					  unsigned int source_id,
					  enum cairo_svg_stream_paint_dependent_element_type type)
{
    cairo_status_t status;

    cairo_svg_stream_element_t element;
    element.type = CAIRO_SVG_STREAM_ELEMENT_TYPE_PAINT_DEPENDENT;
    element.paint_dependent.source_id = source_id;
    element.paint_dependent.type = type;
    status = _cairo_array_append (&svg_stream->elements, &element);
    if (svg_stream->status == CAIRO_STATUS_SUCCESS) {
	svg_stream->status = status;
    }
}

static void
_cairo_svg_stream_copy (cairo_svg_stream_t *from,
			cairo_svg_stream_t *to)
{
    cairo_status_t status;

    if (unlikely (from->status)) {
	if (to->status == CAIRO_STATUS_SUCCESS) {
	    to->status = from->status;
	}
	return;
    }

    for (unsigned int i = 0; i < from->elements.num_elements; i++) {
	cairo_svg_stream_element_t *element = _cairo_array_index (&from->elements, i);
	cairo_svg_stream_element_t element_copy = *element;
	if (element->type == CAIRO_SVG_STREAM_ELEMENT_TYPE_TEXT) {
	    element_copy.text.output_stream = _cairo_memory_stream_create ();
	    _cairo_memory_stream_copy (element->text.output_stream, element_copy.text.output_stream);
	    if (to->status == CAIRO_STATUS_SUCCESS) {
		to->status = element->text.output_stream->status;
	    }
	}
	status = _cairo_array_append (&to->elements, &element_copy);
	if (unlikely (status)) {
	    if (to->status == CAIRO_STATUS_SUCCESS) {
		to->status = status;
	    }
	    return;
	}
    }
}

static void
_cairo_svg_stream_copy_to_output_stream (cairo_svg_stream_t *from,
					 cairo_output_stream_t *to,
					 cairo_hash_table_t *paints)
{
    if (unlikely (from->status)) {
	if (to->status == CAIRO_STATUS_SUCCESS) {
	    to->status = from->status;
	}
	return;
    }

    for (unsigned int i = 0; i < from->elements.num_elements; i++) {
	cairo_svg_stream_element_t *element = _cairo_array_index (&from->elements, i);
	if (element->type == CAIRO_SVG_STREAM_ELEMENT_TYPE_TEXT) {
	    _cairo_memory_stream_copy (element->text.output_stream, to);
	}
	if (element->type == CAIRO_SVG_STREAM_ELEMENT_TYPE_PAINT_DEPENDENT) {
	    cairo_svg_paint_t paint_key;
	    paint_key.source_id = element->paint_dependent.source_id;
	    _cairo_svg_paint_init_key (&paint_key);

	    cairo_svg_paint_t *found_paint_entry = _cairo_hash_table_lookup (paints,
									     &paint_key.base);
	    assert (found_paint_entry);

	    switch (element->paint_dependent.type) {
	    case CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE:
		_cairo_output_stream_printf (to,
					     " x=\"%f\" y=\"%f\" width=\"%f\" height=\"%f\"",
					     found_paint_entry->box.p1.x,
					     found_paint_entry->box.p1.y,
					     found_paint_entry->box.p2.x - found_paint_entry->box.p1.x,
					     found_paint_entry->box.p2.y - found_paint_entry->box.p1.y);
		break;
	    case CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE_AT_ORIGIN:
		_cairo_output_stream_printf (to,
					     " x=\"0\" y=\"0\" width=\"%f\" height=\"%f\"",
					     found_paint_entry->box.p2.x - found_paint_entry->box.p1.x,
					     found_paint_entry->box.p2.y - found_paint_entry->box.p1.y);
		break;
	    case CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_TRANSLATION:
		_cairo_output_stream_printf (to,
					     " transform=\"translate(%f, %f)\"",
					     found_paint_entry->box.p1.x,
					     found_paint_entry->box.p1.y);
		break;
	    case CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_INVERSE_TRANSLATION:
		_cairo_output_stream_printf (to,
					     " transform=\"translate(%f, %f)\"",
					     -found_paint_entry->box.p1.x,
					     -found_paint_entry->box.p1.y);
		break;
	    }
	}
    }
}

static cairo_status_t
_cairo_svg_stream_destroy (cairo_svg_stream_t *svg_stream)
{
    cairo_status_t status = svg_stream->status;
    for (unsigned int i = 0; i < svg_stream->elements.num_elements; i++) {
	cairo_svg_stream_element_t *element = _cairo_array_index (&svg_stream->elements, i);
	if (element->type == CAIRO_SVG_STREAM_ELEMENT_TYPE_TEXT) {
	    cairo_status_t element_status = _cairo_output_stream_destroy (element->text.output_stream);
	    if (status == CAIRO_STATUS_SUCCESS) {
		status = element_status;
	    }
	}
    }
    _cairo_array_fini (&svg_stream->elements);
    return status;
}

/**
 * CAIRO_HAS_SVG_SURFACE:
 *
 * Defined if the SVG surface backend is available.
 * This macro can be used to conditionally compile backend-specific code.
 *
 * Since: 1.2
 **/

static const unsigned int invalid_pattern_id = -1;

static const cairo_svg_version_t _cairo_svg_versions[] =
{
    CAIRO_SVG_VERSION_1_1,
    CAIRO_SVG_VERSION_1_2
};

#define CAIRO_SVG_VERSION_LAST ARRAY_LENGTH (_cairo_svg_versions)

static const char *_cairo_svg_supported_mime_types[] =
{
    CAIRO_MIME_TYPE_JPEG,
    CAIRO_MIME_TYPE_PNG,
    CAIRO_MIME_TYPE_UNIQUE_ID,
    CAIRO_MIME_TYPE_URI,
    NULL
};

static void
_cairo_svg_surface_emit_path (cairo_svg_stream_t *output,
			      const cairo_path_fixed_t *path,
			      const cairo_matrix_t *ctm_inverse);

static const char * _cairo_svg_version_strings[CAIRO_SVG_VERSION_LAST] =
{
    "SVG 1.1",
    "SVG 1.2"
};

static const char * _cairo_svg_unit_strings[] =
{
    "",
    "em",
    "ex",
    "px",
    "in",
    "cm",
    "mm",
    "pt",
    "pc",
    "%"
};

enum cairo_svg_filter {
    CAIRO_SVG_FILTER_REMOVE_COLOR,
    CAIRO_SVG_FILTER_REMOVE_COLOR_AND_INVERT_ALPHA,
    CAIRO_SVG_FILTER_COLOR_TO_ALPHA,
    CAIRO_SVG_FILTER_LAST_STATIC_FILTER,
    CAIRO_SVG_FILTER_OVER,
    CAIRO_SVG_FILTER_IN,
    CAIRO_SVG_FILTER_OUT,
    CAIRO_SVG_FILTER_ATOP,
    CAIRO_SVG_FILTER_XOR,
    CAIRO_SVG_FILTER_ADD,
    CAIRO_SVG_FILTER_MULTIPLY,
    CAIRO_SVG_FILTER_SCREEN,
    CAIRO_SVG_FILTER_OVERLAY,
    CAIRO_SVG_FILTER_DARKEN,
    CAIRO_SVG_FILTER_LIGHTEN,
    CAIRO_SVG_FILTER_COLOR_DODGE,
    CAIRO_SVG_FILTER_COLOR_BURN,
    CAIRO_SVG_FILTER_HARD_LIGHT,
    CAIRO_SVG_FILTER_SOFT_LIGHT,
    CAIRO_SVG_FILTER_DIFFERENCE,
    CAIRO_SVG_FILTER_EXCLUSION,
    CAIRO_SVG_FILTER_HUE,
    CAIRO_SVG_FILTER_SATURATION,
    CAIRO_SVG_FILTER_COLOR,
    CAIRO_SVG_FILTER_LUMINOSITY,
};

typedef struct _cairo_svg_page {
    cairo_svg_stream_t xml_node;
} cairo_svg_page_t;

typedef struct _cairo_svg_document {
    cairo_output_stream_t *output_stream;
    unsigned long refcount;
    cairo_surface_t *owner;
    cairo_bool_t finished;

    double width;
    double height;
    cairo_svg_unit_t unit;

    cairo_svg_stream_t xml_node_defs;
    cairo_svg_stream_t xml_node_glyphs;
    cairo_svg_stream_t xml_node_filters;

    unsigned int linear_pattern_id;
    unsigned int radial_pattern_id;
    unsigned int pattern_id;
    unsigned int clip_id;
    unsigned int mask_id;
    unsigned int compositing_group_id;
    unsigned int filter_id;

    cairo_bool_t filters_emitted[CAIRO_SVG_FILTER_LAST_STATIC_FILTER];

    cairo_svg_version_t svg_version;

    cairo_scaled_font_subsets_t *font_subsets;

    cairo_hash_table_t *paints;
} cairo_svg_document_t;

// Must be compatible with the struct _cairo_svg_surface_start.
typedef struct _cairo_svg_surface {
    cairo_surface_t base;

    cairo_bool_t force_fallbacks;

    unsigned int source_id;
    unsigned int depth;

    double width;
    double height;
    cairo_bool_t surface_bounded;

    cairo_svg_document_t *document;

    cairo_svg_stream_t xml_node;
    cairo_array_t page_set;

    cairo_hash_table_t *source_surfaces;

    cairo_surface_clipper_t clipper;
    cairo_svg_stream_t *current_clipper_stream;
    unsigned int clip_level;

    cairo_bool_t transitive_paint_used;

    cairo_paginated_mode_t paginated_mode;
} cairo_svg_surface_t;

static cairo_status_t
_cairo_svg_document_create (cairo_output_stream_t *stream,
			    double width,
			    double height,
			    cairo_svg_version_t version,
			    cairo_svg_document_t **document_out);

static cairo_status_t
_cairo_svg_document_destroy (cairo_svg_document_t *document);

static cairo_status_t
_cairo_svg_document_finish (cairo_svg_document_t *document);

static cairo_svg_document_t *
_cairo_svg_document_reference (cairo_svg_document_t *document);

static cairo_surface_t *
_cairo_svg_surface_create_for_document (cairo_svg_document_t *document,
					cairo_content_t content,
					double width,
					double height,
					cairo_bool_t bounded);

static cairo_surface_t *
_cairo_svg_surface_create_for_stream_internal (cairo_output_stream_t *stream,
					       double width,
					       double height,
					       cairo_svg_version_t version);

static cairo_status_t
_cairo_svg_surface_emit_composite_pattern (cairo_svg_stream_t *output,
					   cairo_svg_surface_t *surface,
					   cairo_surface_pattern_t *pattern,
					   unsigned int pattern_id,
					   const cairo_matrix_t *parent_matrix);

static cairo_status_t
_cairo_svg_surface_emit_paint (cairo_svg_stream_t *output,
			       cairo_svg_surface_t *surface,
			       const cairo_pattern_t *source,
			       cairo_bool_t at_origin);

static const cairo_surface_backend_t cairo_svg_surface_backend;
static const cairo_paginated_surface_backend_t cairo_svg_surface_paginated_backend;

/**
 * cairo_svg_surface_create_for_stream:
 * @write_func: a #cairo_write_func_t to accept the output data, may be %NULL
 *              to indicate a no-op @write_func. With a no-op @write_func,
 *              the surface may be queried or used as a source without
 *              generating any temporary files.
 * @closure: the closure argument for @write_func
 * @width_in_points: width of the surface, in points (1 point == 1/72.0 inch)
 * @height_in_points: height of the surface, in points (1 point == 1/72.0 inch)
 *
 * Creates a SVG surface of the specified size in points to be written
 * incrementally to the stream represented by @write_func and @closure.
 *
 * Return value: a pointer to the newly created surface. The caller
 * owns the surface and should call cairo_surface_destroy() when done
 * with it.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" surface if an error such as out of memory
 * occurs. You can use cairo_surface_status() to check for this.
 *
 * Since: 1.2
 **/
cairo_surface_t *
cairo_svg_surface_create_for_stream (cairo_write_func_t		 write_func,
				     void			*closure,
				     double			 width,
				     double			 height)
{
    cairo_output_stream_t *stream;

    stream = _cairo_output_stream_create (write_func, NULL, closure);
    if (_cairo_output_stream_get_status (stream))
	return _cairo_surface_create_in_error (_cairo_output_stream_destroy (stream));

    return _cairo_svg_surface_create_for_stream_internal (stream, width, height, CAIRO_SVG_VERSION_1_1);
}

/**
 * cairo_svg_surface_create:
 * @filename: a filename for the SVG output (must be writable), %NULL may be
 *            used to specify no output. This will generate a SVG surface that
 *            may be queried and used as a source, without generating a
 *            temporary file.
 * @width_in_points: width of the surface, in points (1 point == 1/72.0 inch)
 * @height_in_points: height of the surface, in points (1 point == 1/72.0 inch)
 *
 * Creates a SVG surface of the specified size in points to be written
 * to @filename.
 *
 * The SVG surface backend recognizes the following MIME types for the
 * data attached to a surface (see cairo_surface_set_mime_data()) when
 * it is used as a source pattern for drawing on this surface:
 * %CAIRO_MIME_TYPE_JPEG, %CAIRO_MIME_TYPE_PNG,
 * %CAIRO_MIME_TYPE_URI. If any of them is specified, the SVG backend
 * emits a href with the content of MIME data instead of a surface
 * snapshot (PNG, Base64-encoded) in the corresponding image tag.
 *
 * The unofficial MIME type %CAIRO_MIME_TYPE_URI is examined
 * first. If present, the URI is emitted as is: assuring the
 * correctness of URI is left to the client code.
 *
 * If %CAIRO_MIME_TYPE_URI is not present, but %CAIRO_MIME_TYPE_JPEG
 * or %CAIRO_MIME_TYPE_PNG is specified, the corresponding data is
 * Base64-encoded and emitted.
 *
 * If %CAIRO_MIME_TYPE_UNIQUE_ID is present, all surfaces with the same
 * unique identifier will only be embedded once.
 *
 * Return value: a pointer to the newly created surface. The caller
 * owns the surface and should call cairo_surface_destroy() when done
 * with it.
 *
 * This function always returns a valid pointer, but it will return a
 * pointer to a "nil" surface if an error such as out of memory
 * occurs. You can use cairo_surface_status() to check for this.
 *
 * Since: 1.2
 **/
cairo_surface_t *
cairo_svg_surface_create (const char	*filename,
			  double	 width,
			  double	 height)
{
    cairo_output_stream_t *stream;

    stream = _cairo_output_stream_create_for_filename (filename);
    if (_cairo_output_stream_get_status (stream))
	return _cairo_surface_create_in_error (_cairo_output_stream_destroy (stream));

    return _cairo_svg_surface_create_for_stream_internal (stream, width, height, CAIRO_SVG_VERSION_1_1);
}

static cairo_bool_t
_cairo_surface_is_svg (cairo_surface_t *surface)
{
    return surface->backend == &cairo_svg_surface_backend;
}

/* If the abstract_surface is a paginated surface, and that paginated
 * surface's target is a svg_surface, then set svg_surface to that
 * target. Otherwise return FALSE.
 */
static cairo_bool_t
_extract_svg_surface (cairo_surface_t *surface,
		      cairo_svg_surface_t **svg_surface)
{
    cairo_surface_t *target;

    if (surface->status)
	return FALSE;
    if (surface->finished) {
	(void) _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return FALSE;
    }

    if (!_cairo_surface_is_paginated (surface)) {
	(void) _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_TYPE_MISMATCH));
	return FALSE;
    }

    target = _cairo_paginated_surface_get_target (surface);
    if (target->status) {
	(void) _cairo_surface_set_error (surface, target->status);
	return FALSE;
    }
    if (target->finished) {
	(void) _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return FALSE;
    }

    if (!_cairo_surface_is_svg (target)) {
	(void) _cairo_surface_set_error (surface, _cairo_error (CAIRO_STATUS_SURFACE_TYPE_MISMATCH));
	return FALSE;
    }

    *svg_surface = (cairo_svg_surface_t *) target;
    return TRUE;
}

/**
 * cairo_svg_surface_restrict_to_version:
 * @surface: a SVG #cairo_surface_t
 * @version: SVG version
 *
 * Restricts the generated SVG file to @version. See cairo_svg_get_versions()
 * for a list of available version values that can be used here.
 *
 * This function should only be called before any drawing operations
 * have been performed on the given surface. The simplest way to do
 * this is to call this function immediately after creating the
 * surface.
 *
 * Since: 1.2
 **/
void
cairo_svg_surface_restrict_to_version (cairo_surface_t		*abstract_surface,
				       cairo_svg_version_t	 version)
{
    cairo_svg_surface_t *surface;

    if (! _extract_svg_surface (abstract_surface, &surface))
	return;

    if (version < CAIRO_SVG_VERSION_LAST)
	surface->document->svg_version = version;
}

/**
 * cairo_svg_get_versions:
 * @versions: supported version list
 * @num_versions: list length
 *
 * Used to retrieve the list of supported versions. See
 * cairo_svg_surface_restrict_to_version().
 *
 * Since: 1.2
 **/
void
cairo_svg_get_versions (cairo_svg_version_t const	**versions,
                        int				 *num_versions)
{
    if (versions != NULL)
	*versions = _cairo_svg_versions;

    if (num_versions != NULL)
	*num_versions = CAIRO_SVG_VERSION_LAST;
}

/**
 * cairo_svg_version_to_string:
 * @version: a version id
 *
 * Get the string representation of the given @version id. This function
 * will return %NULL if @version isn't valid. See cairo_svg_get_versions()
 * for a way to get the list of valid version ids.
 *
 * Return value: the string associated to given version.
 *
 * Since: 1.2
 **/
const char *
cairo_svg_version_to_string (cairo_svg_version_t version)
{
    if (version >= CAIRO_SVG_VERSION_LAST)
	return NULL;

    return _cairo_svg_version_strings[version];
}

/**
 * cairo_svg_surface_set_document_unit:
 * @surface: a SVG #cairo_surface_t
 * @unit: SVG unit
 *
 * Use the specified unit for the width and height of the generated SVG file.
 * See #cairo_svg_unit_t for a list of available unit values that can be used
 * here.
 *
 * This function can be called at any time before generating the SVG file.
 *
 * However to minimize the risk of ambiguities it's recommended to call it
 * before any drawing operations have been performed on the given surface, to
 * make it clearer what the unit used in the drawing operations is.
 *
 * The simplest way to do this is to call this function immediately after
 * creating the SVG surface.
 *
 * Note if this function is never called, the default unit for SVG documents
 * generated by cairo will be user unit.
 *
 * Since: 1.16
 **/
void
cairo_svg_surface_set_document_unit (cairo_surface_t	*abstract_surface,
				     cairo_svg_unit_t	 unit)
{
    cairo_svg_surface_t *surface;

    if (! _extract_svg_surface (abstract_surface, &surface))
	return;

    if (unit <= CAIRO_SVG_UNIT_PERCENT)
	surface->document->unit = unit;
}

/**
 * cairo_svg_surface_get_document_unit:
 * @surface: a SVG #cairo_surface_t
 *
 * Get the unit of the SVG surface.
 *
 * If the surface passed as an argument is not a SVG surface, the function
 * sets the error status to CAIRO_STATUS_SURFACE_TYPE_MISMATCH and returns
 * CAIRO_SVG_UNIT_USER.
 *
 * Return value: the SVG unit of the SVG surface.
 *
 * Since: 1.16
 **/
cairo_svg_unit_t
cairo_svg_surface_get_document_unit (cairo_surface_t	*abstract_surface)
{
    cairo_svg_surface_t *surface;

    if (! _extract_svg_surface (abstract_surface, &surface)) {
	_cairo_error_throw (CAIRO_STATUS_SURFACE_TYPE_MISMATCH);
	return CAIRO_SVG_UNIT_USER;
    }

    return surface->document->unit;
}

static void
_cairo_svg_paint_compute (cairo_svg_document_t *document, cairo_svg_paint_t *paint) {
    for (unsigned int i = 0; i < paint->paint_elements.num_elements; i++) {
	cairo_svg_paint_element_t *paint_element = _cairo_array_index (&paint->paint_elements, i);

	cairo_svg_paint_t paint_key;
	paint_key.source_id = paint_element->source_id;
	_cairo_svg_paint_init_key (&paint_key);

	cairo_svg_paint_t *found_paint_entry = _cairo_hash_table_lookup (document->paints,
									 &paint_key.base);
	assert (found_paint_entry);

	_cairo_svg_paint_compute (document, found_paint_entry);

	cairo_box_double_t box = found_paint_entry->box;
	_cairo_matrix_transform_bounding_box (&paint_element->matrix,
					      &box.p1.x, &box.p1.y,
					      &box.p2.x, &box.p2.y,
					      NULL);
	_cairo_svg_paint_box_add_padding (&box);

	if (i == 0) {
	    paint->box = box;
	} else {
	    paint->box.p1.x = MIN (paint->box.p1.x, box.p1.x);
	    paint->box.p1.y = MIN (paint->box.p1.y, box.p1.y);
	    paint->box.p2.x = MAX (paint->box.p2.x, box.p2.x);
	    paint->box.p2.y = MAX (paint->box.p2.y, box.p2.y);
	}
    }
    _cairo_array_truncate (&paint->paint_elements, 0);
}

static void
_cairo_svg_paint_compute_func (void *entry, void *closure)
{
    cairo_svg_paint_t *paint = entry;
    cairo_svg_document_t *document = closure;

    _cairo_svg_paint_compute (document, paint);
}

static cairo_status_t
_cairo_svg_surface_add_source_surface (cairo_svg_surface_t *surface,
				       cairo_surface_t *source_surface,
				       cairo_bool_t *is_new,
				       cairo_svg_source_surface_t **result_source_surface)
{
    cairo_status_t status;

    cairo_svg_source_surface_t source_surface_key;
    source_surface_key.id = source_surface->unique_id;
    cairo_surface_get_mime_data (source_surface,
				 CAIRO_MIME_TYPE_UNIQUE_ID,
				 (const unsigned char **) &source_surface_key.unique_id,
				 &source_surface_key.unique_id_length);
    _cairo_svg_source_surface_init_key (&source_surface_key);

    cairo_svg_source_surface_t *found_source_surface_entry = _cairo_hash_table_lookup (surface->source_surfaces,
										       &source_surface_key.base);
    if (found_source_surface_entry) {
	*is_new = FALSE;
	*result_source_surface = found_source_surface_entry;
	return CAIRO_STATUS_SUCCESS;
    }

    unsigned char *unique_id = NULL;
    unsigned long unique_id_length = 0;
    if (source_surface_key.unique_id && source_surface_key.unique_id_length > 0) {
	unique_id = _cairo_malloc (source_surface_key.unique_id_length);
	if (unique_id == NULL) {
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	}

	unique_id_length = source_surface_key.unique_id_length;
	memcpy (unique_id, source_surface_key.unique_id, unique_id_length);
    } else {
	unique_id = NULL;
	unique_id_length = 0;
    }

    cairo_svg_source_surface_t *source_surface_entry = malloc (sizeof (cairo_svg_source_surface_t));
    if (source_surface_entry == NULL) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail;
    }
    source_surface_entry->id = source_surface_key.id;
    source_surface_entry->unique_id_length = unique_id_length;
    source_surface_entry->unique_id = unique_id;
    _cairo_svg_source_surface_init_key (source_surface_entry);
    status = _cairo_hash_table_insert (surface->source_surfaces, &source_surface_entry->base);
    if (unlikely (status)) {
	goto fail;
    }

    *is_new = TRUE;
    *result_source_surface = source_surface_entry;
    return CAIRO_STATUS_SUCCESS;

    fail:
    free (unique_id);
    free (source_surface_entry);
    return status;
}

static cairo_bool_t
_cairo_svg_surface_cliprect_covers_surface (cairo_svg_surface_t *surface,
					    cairo_path_fixed_t *path)
{
    cairo_box_t box;

    return surface->surface_bounded &&
	   _cairo_path_fixed_is_box (path, &box) &&
	   box.p1.x <= 0 &&
	   box.p1.y <= 0 &&
	   _cairo_fixed_to_double (box.p2.x) >= surface->width &&
	   _cairo_fixed_to_double (box.p2.y) >= surface->height;
}

static cairo_status_t
_cairo_svg_surface_clipper_intersect_clip_path (cairo_surface_clipper_t *clipper,
						cairo_path_fixed_t *path,
						cairo_fill_rule_t fill_rule,
						double tolerance,
						cairo_antialias_t antialias)
{
    cairo_svg_surface_t *surface = cairo_container_of (clipper,
						       cairo_svg_surface_t,
						       clipper);
    cairo_svg_document_t *document = surface->document;

    if (path == NULL) {
	for (unsigned int i = 0; i < surface->clip_level; i++) {
	    _cairo_svg_stream_printf (surface->current_clipper_stream, "</g>\n");
	}
	surface->clip_level = 0;
	return CAIRO_STATUS_SUCCESS;
    }

    /* skip trivial whole-page clips */
    if (_cairo_svg_surface_cliprect_covers_surface (surface, path)) {
	return CAIRO_STATUS_SUCCESS;
    }

    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<clipPath id=\"clip-%d\">\n",
			      document->clip_id);

    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<path clip-rule=\"%s\"",
			      fill_rule == CAIRO_FILL_RULE_EVEN_ODD ? "evenodd" : "nonzero");
    _cairo_svg_surface_emit_path (&document->xml_node_defs, path, NULL);
    _cairo_svg_stream_printf (&document->xml_node_defs, "/>\n");

    _cairo_svg_stream_printf (&document->xml_node_defs, "</clipPath>\n");

    _cairo_svg_stream_printf (surface->current_clipper_stream,
			      "<g clip-path=\"url(#clip-%d)\">\n",
			      document->clip_id);

    document->clip_id++;
    surface->clip_level++;

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_svg_surface_reset_clip (cairo_svg_surface_t *surface)
{
    _cairo_surface_clipper_reset (&surface->clipper);
    if (surface->current_clipper_stream != NULL) {
	for (unsigned int i = 0; i < surface->clip_level; i++) {
	    _cairo_svg_stream_printf (surface->current_clipper_stream, "</g>\n");
	}
    }
    surface->clip_level = 0;
}

static cairo_status_t
_cairo_svg_surface_set_clip (cairo_svg_surface_t *surface,
			     cairo_svg_stream_t *clipper_stream,
			     const cairo_clip_t *clip)
{
    if (surface->current_clipper_stream != clipper_stream) {
	_cairo_svg_surface_reset_clip (surface);
	surface->current_clipper_stream = clipper_stream;
    }
    return _cairo_surface_clipper_set_clip (&surface->clipper, clip);
}

static cairo_surface_t *
_cairo_svg_surface_create_for_document (cairo_svg_document_t *document,
					cairo_content_t content,
					double width,
					double height,
					cairo_bool_t bounded)
{
    cairo_svg_surface_t *surface;
    cairo_surface_t *paginated;
    cairo_status_t status;

    surface = _cairo_malloc (sizeof (cairo_svg_surface_t));
    if (unlikely (surface == NULL)) {
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));
    }

    _cairo_surface_init (&surface->base,
			 &cairo_svg_surface_backend,
			 NULL, /* device */
			 content,
			 TRUE); /* is_vector */

    surface->source_id = surface->base.unique_id;
    surface->depth = 0;

    surface->width = width;
    surface->height = height;
    surface->surface_bounded = bounded;

    surface->document = _cairo_svg_document_reference (document);

    surface->xml_node = _cairo_svg_stream_create ();
    _cairo_array_init (&surface->page_set, sizeof (cairo_svg_page_t));

    surface->source_surfaces = _cairo_hash_table_create (_cairo_svg_source_surface_equal);
    if (unlikely (surface->source_surfaces == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto CLEANUP;
    }

    _cairo_surface_clipper_init (&surface->clipper, _cairo_svg_surface_clipper_intersect_clip_path);
    surface->current_clipper_stream = NULL;
    surface->clip_level = 0;
    surface->transitive_paint_used = FALSE;

    surface->paginated_mode = CAIRO_PAGINATED_MODE_ANALYZE;

    surface->force_fallbacks = FALSE;


    paginated = _cairo_paginated_surface_create (&surface->base,
						 surface->base.content,
						 &cairo_svg_surface_paginated_backend);
    status = paginated->status;
    if (status == CAIRO_STATUS_SUCCESS) {
	/* paginated keeps the only reference to surface now, drop ours */
	cairo_surface_destroy (&surface->base);
	return paginated;
    }

    /* ignore status as we are on the error path */
    CLEANUP:
    (void) _cairo_svg_stream_destroy (&surface->xml_node);
    (void) _cairo_svg_document_destroy (document);

    free (surface);

    return _cairo_surface_create_in_error (status);
}

static cairo_surface_t *
_cairo_svg_surface_create_for_stream_internal (cairo_output_stream_t	*stream,
					       double			 width,
					       double			 height,
					       cairo_svg_version_t	 version)
{
    cairo_svg_document_t *document;
    cairo_surface_t *surface;
    cairo_status_t status;

    status = _cairo_svg_document_create (stream,
	                                 width, height, version,
					 &document);
    if (unlikely (status)) {
	surface =  _cairo_surface_create_in_error (status);
	/* consume the output stream on behalf of caller */
	status = _cairo_output_stream_destroy (stream);
	return surface;
    }

    surface = _cairo_svg_surface_create_for_document (document, CAIRO_CONTENT_COLOR_ALPHA,
						      width, height, TRUE);
    if (surface->status) {
	return surface;
    }

    document->owner = surface;
    status = _cairo_svg_document_destroy (document);
    /* the ref count should be 2 at this point */
    assert (status == CAIRO_STATUS_SUCCESS);

    return surface;
}

static cairo_svg_page_t *
_cairo_svg_surface_store_page (cairo_svg_surface_t *surface)
{
    _cairo_svg_surface_reset_clip (surface);
    cairo_svg_page_t page;
    page.xml_node = surface->xml_node;
    if (_cairo_array_append (&surface->page_set, &page)) {
	return NULL;
    }
    surface->xml_node = _cairo_svg_stream_create ();
    return _cairo_array_index (&surface->page_set,
			       surface->page_set.num_elements - 1);
}

static cairo_int_status_t
_cairo_svg_surface_copy_page (void *abstract_surface)
{
    cairo_svg_surface_t *surface = abstract_surface;

    cairo_svg_page_t *page = _cairo_svg_surface_store_page (surface);
    if (unlikely (page == NULL)) {
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    _cairo_svg_stream_copy (&page->xml_node, &surface->xml_node);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_svg_surface_show_page (void *abstract_surface)
{
    cairo_svg_surface_t *surface = abstract_surface;

    cairo_svg_page_t *page = _cairo_svg_surface_store_page (surface);
    if (unlikely (page == NULL)) {
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_svg_surface_emit_transform (cairo_svg_stream_t *output,
				   char const *attribute_name,
				   const cairo_matrix_t *object_matrix,
				   const cairo_matrix_t *parent_matrix)
{
    cairo_matrix_t matrix = *object_matrix;

    if (parent_matrix != NULL) {
	cairo_matrix_multiply (&matrix, &matrix, parent_matrix);
    }

    if (!_cairo_matrix_is_identity (&matrix)) {
	_cairo_svg_stream_printf (output,
				  " %s=\"matrix(%f, %f, %f, %f, %f, %f)\"",
				  attribute_name,
				  matrix.xx, matrix.yx,
				  matrix.xy, matrix.yy,
				  matrix.x0, matrix.y0);
    }
}

typedef struct {
    cairo_svg_stream_t *output;
    const cairo_matrix_t *ctm_inverse;
} svg_path_info_t;

static cairo_status_t
_cairo_svg_path_move_to (void *closure,
			 const cairo_point_t *point)
{
    svg_path_info_t *info = closure;
    double x = _cairo_fixed_to_double (point->x);
    double y = _cairo_fixed_to_double (point->y);

    if (info->ctm_inverse)
	cairo_matrix_transform_point (info->ctm_inverse, &x, &y);

    _cairo_svg_stream_printf (info->output, "M %f %f ", x, y);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_svg_path_line_to (void *closure,
			 const cairo_point_t *point)
{
    svg_path_info_t *info = closure;
    double x = _cairo_fixed_to_double (point->x);
    double y = _cairo_fixed_to_double (point->y);

    if (info->ctm_inverse)
	cairo_matrix_transform_point (info->ctm_inverse, &x, &y);

    _cairo_svg_stream_printf (info->output, "L %f %f ", x, y);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_svg_path_curve_to (void          *closure,
			  const cairo_point_t *b,
			  const cairo_point_t *c,
			  const cairo_point_t *d)
{
    svg_path_info_t *info = closure;
    double bx = _cairo_fixed_to_double (b->x);
    double by = _cairo_fixed_to_double (b->y);
    double cx = _cairo_fixed_to_double (c->x);
    double cy = _cairo_fixed_to_double (c->y);
    double dx = _cairo_fixed_to_double (d->x);
    double dy = _cairo_fixed_to_double (d->y);

    if (info->ctm_inverse) {
	cairo_matrix_transform_point (info->ctm_inverse, &bx, &by);
	cairo_matrix_transform_point (info->ctm_inverse, &cx, &cy);
	cairo_matrix_transform_point (info->ctm_inverse, &dx, &dy);
    }

    _cairo_svg_stream_printf (info->output,
			      "C %f %f %f %f %f %f ",
			      bx, by, cx, cy, dx, dy);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_svg_path_close_path (void *closure)
{
    svg_path_info_t *info = closure;

    _cairo_svg_stream_printf (info->output, "Z ");

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_svg_surface_emit_path (cairo_svg_stream_t *output,
			      const cairo_path_fixed_t *path,
			      const cairo_matrix_t *ctm_inverse)
{
    cairo_status_t status;
    svg_path_info_t info;

    _cairo_svg_stream_printf (output, " d=\"");

    info.output = output;
    info.ctm_inverse = ctm_inverse;
    status = _cairo_path_fixed_interpret (path,
					  _cairo_svg_path_move_to,
					  _cairo_svg_path_line_to,
					  _cairo_svg_path_curve_to,
					  _cairo_svg_path_close_path,
					  &info);
    assert (status == CAIRO_STATUS_SUCCESS);

    _cairo_svg_stream_printf (output, "\"");
}

static cairo_int_status_t
_cairo_svg_document_emit_outline_glyph_data (cairo_svg_document_t *document,
					     cairo_scaled_font_t *scaled_font,
					     unsigned long glyph_index)
{
    cairo_scaled_glyph_t *scaled_glyph;
    cairo_int_status_t status;

    status = _cairo_scaled_glyph_lookup (scaled_font,
					 glyph_index,
					 CAIRO_SCALED_GLYPH_INFO_METRICS | CAIRO_SCALED_GLYPH_INFO_PATH,
					 NULL, /* foreground color */
					 &scaled_glyph);
    if (unlikely (status)) {
	return status;
    }

    if (_cairo_path_fixed_size (scaled_glyph->path) != 0) {
	_cairo_svg_stream_printf (&document->xml_node_glyphs,
				  "<path");

	_cairo_svg_surface_emit_path (&document->xml_node_glyphs,
				      scaled_glyph->path,
				      NULL);

	_cairo_svg_stream_printf (&document->xml_node_glyphs,
				  "/>\n");
    }

    return status;
}

static cairo_int_status_t
_cairo_svg_document_emit_bitmap_glyph_data (cairo_svg_document_t *document,
					    cairo_scaled_font_t *scaled_font,
					    unsigned long glyph_index)
{
    cairo_status_t status;

    cairo_scaled_glyph_t *scaled_glyph;
    status = _cairo_scaled_glyph_lookup (scaled_font,
					 glyph_index,
					 CAIRO_SCALED_GLYPH_INFO_METRICS | CAIRO_SCALED_GLYPH_INFO_SURFACE,
					 NULL, /* foreground color */
					 &scaled_glyph);
    if (unlikely (status)) {
	return status;
    }

    cairo_bool_t use_recording_surface = (scaled_glyph->has_info & CAIRO_SCALED_GLYPH_INFO_RECORDING_SURFACE) != 0;
    cairo_matrix_t glyph_matrix = scaled_glyph->surface->base.device_transform_inverse;
    cairo_image_surface_t *glyph_image_surface = scaled_glyph->surface;

    // Attempt to recognize a common pattern for a bitmap font and extract the original glyph image from it
    cairo_surface_t *extracted_surface;
    cairo_image_surface_t *extracted_image = NULL;
    void *extracted_image_extra;
    if (use_recording_surface) {
	cairo_recording_surface_t *recording_surface = (cairo_recording_surface_t *) scaled_glyph->recording_surface;
	if (recording_surface->commands.num_elements == 1) {
	    cairo_command_t *command = *((cairo_command_t **) _cairo_array_index (&recording_surface->commands, 0));
	    if (command->header.type == CAIRO_COMMAND_MASK &&
		command->header.op == CAIRO_OPERATOR_OVER &&
		command->header.clip == NULL &&
		command->mask.source.base.type == CAIRO_PATTERN_TYPE_SOLID &&
		_cairo_color_equal (&command->mask.source.solid.color, _cairo_stock_color (CAIRO_STOCK_BLACK)) &&
		command->mask.mask.base.extend == CAIRO_EXTEND_NONE &&
		command->mask.mask.base.type == CAIRO_PATTERN_TYPE_SURFACE &&
		command->mask.mask.surface.surface->type == CAIRO_SURFACE_TYPE_IMAGE) {
		extracted_surface = command->mask.mask.surface.surface;
		if (_cairo_surface_acquire_source_image (extracted_surface,
							 &extracted_image,
							 &extracted_image_extra) == CAIRO_STATUS_SUCCESS) {
		    if (extracted_image->format == CAIRO_FORMAT_A1 || extracted_image->format == CAIRO_FORMAT_A8) {
			use_recording_surface = FALSE;
			glyph_image_surface = extracted_image;
			glyph_matrix = command->mask.mask.base.matrix;
			status = cairo_matrix_invert (&glyph_matrix);
			assert (status == CAIRO_STATUS_SUCCESS);
		    }
		}
	    }
	}
    }

    cairo_surface_t *paginated_surface = _cairo_svg_surface_create_for_document (document,
										 CAIRO_CONTENT_COLOR_ALPHA,
										 0,
										 0,
										 FALSE);
    cairo_svg_surface_t *svg_surface = (cairo_svg_surface_t *) _cairo_paginated_surface_get_target (paginated_surface);
    status = paginated_surface->status;
    if (unlikely (status)) {
	goto cleanup;
    }

    unsigned int source_id = svg_surface->base.unique_id;

    cairo_surface_set_fallback_resolution (paginated_surface,
					   document->owner->x_fallback_resolution,
					   document->owner->y_fallback_resolution);

    cairo_svg_stream_t temporary_stream = _cairo_svg_stream_create ();

    unsigned int mask_id = document->mask_id++;

    _cairo_svg_stream_printf (&temporary_stream,
			      "<mask id=\"mask-%d\">\n",
			      mask_id);

    cairo_pattern_t *pattern = cairo_pattern_create_for_surface (use_recording_surface ? scaled_glyph->recording_surface
										       : &glyph_image_surface->base);
    _cairo_svg_surface_emit_composite_pattern (&temporary_stream,
					       svg_surface,
					       (cairo_surface_pattern_t *) pattern,
					       invalid_pattern_id,
					       NULL);
    cairo_pattern_destroy (pattern);

    _cairo_svg_stream_printf (&temporary_stream, "</mask>\n");

    _cairo_svg_stream_copy (&temporary_stream, &document->xml_node_defs);

    status = _cairo_svg_stream_destroy (&temporary_stream);
    if (unlikely (status)) {
	goto cleanup;
    }

    svg_surface->transitive_paint_used = TRUE;

    _cairo_svg_stream_printf (&document->xml_node_glyphs, "<rect");
    _cairo_svg_stream_append_paint_dependent (&document->xml_node_glyphs,
					      source_id,
					      CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE);
    _cairo_svg_stream_printf (&document->xml_node_glyphs,
			      " mask=\"url(#mask-%d)\"",
			      mask_id);
    if (!use_recording_surface) {
	_cairo_svg_surface_emit_transform (&document->xml_node_glyphs,
					   "transform",
					   &glyph_matrix,
					   NULL);
    }
    _cairo_svg_stream_printf (&document->xml_node_glyphs, "/>\n");

    cairo_svg_paint_t *paint_entry = malloc (sizeof (cairo_svg_paint_t));
    if (paint_entry == NULL) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto cleanup;
    }
    paint_entry->source_id = source_id;
    paint_entry->box.p1.x = 0;
    paint_entry->box.p1.y = 0;
    paint_entry->box.p2.x = glyph_image_surface->width;
    paint_entry->box.p2.y = glyph_image_surface->height;
    if (use_recording_surface) {
	_cairo_matrix_transform_bounding_box (&glyph_matrix,
					      &paint_entry->box.p1.x, &paint_entry->box.p1.y,
					      &paint_entry->box.p2.x, &paint_entry->box.p2.y,
					      NULL);
    }
    _cairo_svg_paint_box_add_padding (&paint_entry->box);
    _cairo_array_init (&paint_entry->paint_elements, sizeof (cairo_svg_paint_element_t));
    _cairo_svg_paint_init_key (paint_entry);
    status = _cairo_hash_table_insert (document->paints, &paint_entry->base);
    if (unlikely (status)) {
	goto cleanup;
    }

    cleanup:
    if (status == CAIRO_STATUS_SUCCESS) {
	status = cairo_surface_status (paginated_surface);
    }
    cairo_surface_destroy (paginated_surface);

    if (extracted_image != NULL) {
	_cairo_surface_release_source_image (extracted_surface, extracted_image, extracted_image_extra);
    }

    return status;
}

static cairo_int_status_t
_cairo_svg_document_emit_glyph (cairo_svg_document_t	*document,
				cairo_scaled_font_t	*scaled_font,
				unsigned long		 scaled_font_glyph_index,
				unsigned int		 font_id,
				unsigned int		 subset_glyph_index)
{
    cairo_int_status_t	     status;

    _cairo_svg_stream_printf (&document->xml_node_glyphs,
			      "<g id=\"glyph-%d-%d\">\n",
			      font_id,
			      subset_glyph_index);

    status = _cairo_svg_document_emit_outline_glyph_data (document,
							  scaled_font,
							  scaled_font_glyph_index);
    if (status == CAIRO_INT_STATUS_UNSUPPORTED)
	status = _cairo_svg_document_emit_bitmap_glyph_data (document,
							     scaled_font,
							     scaled_font_glyph_index);
    if (unlikely (status))
	return status;

    _cairo_svg_stream_printf (&document->xml_node_glyphs, "</g>\n");

    return CAIRO_INT_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_svg_document_emit_font_subset (cairo_scaled_font_subset_t	*font_subset,
				      void				*closure)
{
    cairo_svg_document_t *document = closure;
    cairo_int_status_t status = CAIRO_INT_STATUS_SUCCESS;
    unsigned int i;

    _cairo_scaled_font_freeze_cache (font_subset->scaled_font);
    for (i = 0; i < font_subset->num_glyphs; i++) {
	status = _cairo_svg_document_emit_glyph (document,
					         font_subset->scaled_font,
					         font_subset->glyphs[i],
					         font_subset->font_id, i);
	if (unlikely (status))
	    break;
    }
    _cairo_scaled_font_thaw_cache (font_subset->scaled_font);

    return status;
}

static cairo_status_t
_cairo_svg_document_emit_font_subsets (cairo_svg_document_t *document)
{
    cairo_status_t status;

    status = _cairo_scaled_font_subsets_foreach_scaled (document->font_subsets,
                                                        _cairo_svg_document_emit_font_subset,
                                                        document);
    if (unlikely (status))
	goto FAIL;

    status = _cairo_scaled_font_subsets_foreach_user (document->font_subsets,
						      _cairo_svg_document_emit_font_subset,
						      document);

  FAIL:
    _cairo_scaled_font_subsets_destroy (document->font_subsets);
    document->font_subsets = NULL;

    return status;
}

static cairo_bool_t
_cairo_svg_surface_are_operation_and_pattern_supported (cairo_svg_surface_t *surface,
							cairo_operator_t op,
							const cairo_pattern_t *pattern)
{
    if (surface->force_fallbacks) {
	return FALSE;
    }

    if (op == CAIRO_OPERATOR_SATURATE) {
        return FALSE;
    }

    /* SVG 1.1 does not support these operators. We already have code for them for SVG 2
     * that can be enabled when SVG 2 becomes widespread.  */
    if (op == CAIRO_OPERATOR_OVERLAY ||
	op == CAIRO_OPERATOR_COLOR_DODGE ||
	op == CAIRO_OPERATOR_COLOR_BURN ||
	op == CAIRO_OPERATOR_HARD_LIGHT ||
	op == CAIRO_OPERATOR_SOFT_LIGHT ||
	op == CAIRO_OPERATOR_DIFFERENCE ||
	op == CAIRO_OPERATOR_EXCLUSION ||
	op == CAIRO_OPERATOR_HSL_HUE ||
	op == CAIRO_OPERATOR_HSL_SATURATION ||
	op == CAIRO_OPERATOR_HSL_COLOR ||
	op == CAIRO_OPERATOR_HSL_LUMINOSITY) {
	return FALSE;
    }

    if (pattern->type == CAIRO_PATTERN_TYPE_SURFACE) {
        /* Do not cause stack overflow because of too deep or infinite recording surfaces. */
	if (((cairo_surface_pattern_t *) pattern)->surface->type == CAIRO_SURFACE_TYPE_RECORDING &&
	    surface->depth > 1000) {
	    return FALSE;
	}
	/* SVG doesn't support extends reflect and pad for surface pattern. */
        if (pattern->extend != CAIRO_EXTEND_NONE && pattern->extend != CAIRO_EXTEND_REPEAT) {
	    return FALSE;
	}
    }

    /* SVG 1.1 does not support the focal point (fx, fy) that is outside of the circle defined by (cx, cy) and r. */
    if (pattern->type == CAIRO_PATTERN_TYPE_RADIAL) {
	cairo_radial_pattern_t *radial_pattern = (cairo_radial_pattern_t *) pattern;
	double max_radius;
	if (radial_pattern->cd1.radius > radial_pattern->cd2.radius) {
	    max_radius = radial_pattern->cd1.radius;
	} else {
	    max_radius = radial_pattern->cd2.radius;
	}
	cairo_point_double_t c1 = radial_pattern->cd1.center;
	cairo_point_double_t c2 = radial_pattern->cd2.center;
	if ((c1.x - c2.x) * (c1.x - c2.x) + (c1.y - c2.y) * (c1.y - c2.y) >= max_radius * max_radius) {
	    return FALSE;
	}
    }

    if (pattern->type == CAIRO_PATTERN_TYPE_MESH) {
	return FALSE;
    }

    if (pattern->type == CAIRO_PATTERN_TYPE_RASTER_SOURCE) {
	return FALSE;
    }

    return TRUE;
}

static cairo_status_t
_cairo_svg_surface_finish (void *abstract_surface)
{
    cairo_status_t status, final_status;
    cairo_svg_surface_t *surface = abstract_surface;

    if (_cairo_paginated_surface_get_target (surface->document->owner) == &surface->base) {
	final_status = _cairo_svg_document_finish (surface->document);
    } else {
	final_status = CAIRO_STATUS_SUCCESS;
    }

    status = _cairo_svg_stream_destroy (&surface->xml_node);
    if (final_status == CAIRO_STATUS_SUCCESS) {
	final_status = status;
    }

    for (unsigned int i = 0; i < surface->page_set.num_elements; i++) {
	cairo_svg_page_t *page = _cairo_array_index (&surface->page_set, i);
	status = _cairo_svg_stream_destroy (&page->xml_node);
	if (final_status == CAIRO_STATUS_SUCCESS) {
	    final_status = status;
	}
    }
    _cairo_array_fini (&surface->page_set);

    _cairo_surface_clipper_reset (&surface->clipper);

    _cairo_hash_table_foreach (surface->source_surfaces, _cairo_svg_source_surface_pluck, surface->source_surfaces);
    _cairo_hash_table_destroy (surface->source_surfaces);

    status = _cairo_svg_document_destroy (surface->document);
    if (final_status == CAIRO_STATUS_SUCCESS) {
	final_status = status;
    }

    return final_status;
}

static const char *
_cairo_svg_surface_emit_static_filter (cairo_svg_document_t *document, enum cairo_svg_filter filter)
{
    if (!document->filters_emitted[filter]) {
	document->filters_emitted[filter] = TRUE;
	if (filter == CAIRO_SVG_FILTER_REMOVE_COLOR) {
	    // (r, g, b, a) -> (1, 1, 1, a)
	    _cairo_svg_stream_printf (&document->xml_node_filters,
				      "<filter id=\"filter-remove-color\" "
				      "x=\"0%%\" y=\"0%%\" width=\"100%%\" height=\"100%%\">\n"
				      "<feColorMatrix color-interpolation-filters=\"sRGB\" "
	                              "values=\"0 0 0 0 1 "
				      /*    */ "0 0 0 0 1 "
				      /*    */ "0 0 0 0 1 "
				      /*    */ "0 0 0 1 0\" />\n"
				      "</filter>\n");
	} else if (filter == CAIRO_SVG_FILTER_REMOVE_COLOR_AND_INVERT_ALPHA) {
	    // (r, g, b, a) -> (1, 1, 1, 1 - a)
	    _cairo_svg_stream_printf (&document->xml_node_filters,
				      "<filter id=\"filter-remove-color-and-invert-alpha\" "
				      "x=\"0%%\" y=\"0%%\" width=\"100%%\" height=\"100%%\">\n"
				      "<feColorMatrix color-interpolation-filters=\"sRGB\" "
				      "values=\"0 0 0 0 1 "
				      /*    */ "0 0 0 0 1 "
				      /*    */ "0 0 0 0 1 "
				      /*    */ "0 0 0 -1 1\"/>\n"
				      "</filter>\n");
	} else if (filter ==  CAIRO_SVG_FILTER_COLOR_TO_ALPHA) {
	    // (r, g, b, a) -> (1, 1, 1, 0.2126 * r + 0.7152 * g + 0.0722 * b)
	    _cairo_svg_stream_printf (&document->xml_node_filters,
				      "<filter id=\"filter-color-to-alpha\" "
				      "x=\"0%%\" y=\"0%%\" width=\"100%%\" height=\"100%%\">\n"
				      "<feColorMatrix color-interpolation-filters=\"sRGB\" "
				      "values=\"0 0 0 0 1 "
				      /*    */ "0 0 0 0 1 "
				      /*    */ "0 0 0 0 1 "
				      /*    */ "0.2126 0.7152 0.0722 0 0\"/>\n"
				      "</filter>\n");
	}
    }

    if (filter == CAIRO_SVG_FILTER_REMOVE_COLOR) {
	return "remove-color";
    } else if (filter == CAIRO_SVG_FILTER_REMOVE_COLOR_AND_INVERT_ALPHA) {
	return "remove-color-and-invert-alpha";
    } else if (filter ==  CAIRO_SVG_FILTER_COLOR_TO_ALPHA) {
	return "color-to-alpha";
    } else {
	ASSERT_NOT_REACHED;
    }
    return FALSE; /* squelch warning */
}

#define _CAIRO_SVG_SURFACE_OUTPUT_FE_COMPOSITE_FILTER(operation) \
    _cairo_svg_stream_printf (&surface->document->xml_node_filters, \
                              "<filter id=\"filter-%d\" " \
                              "x=\"0%%\" y=\"0%%\" width=\"100%%\" height=\"100%%\">\n" \
                              "<feImage xlink:href=\"#compositing-group-%d\" result=\"source\"", \
                              filter_id, \
                              source_compositing_group_id); \
    _cairo_svg_stream_append_paint_dependent (&surface->document->xml_node_filters, \
                                              surface->source_id, \
                                              CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE_AT_ORIGIN); \
    _cairo_svg_stream_printf (&surface->document->xml_node_filters, \
                              "/>\n" \
                              "<feImage xlink:href=\"#compositing-group-%d\" result=\"destination\"", \
                              destination_compositing_group_id); \
    _cairo_svg_stream_append_paint_dependent (&surface->document->xml_node_filters, \
                                              surface->source_id, \
                                              CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE_AT_ORIGIN); \
    _cairo_svg_stream_printf (&surface->document->xml_node_filters, \
                              "/>\n" \
                              "<feComposite in=\"source\" in2=\"destination\" " \
                              "operator=\"" operation "\" " \
                              "color-interpolation-filters=\"sRGB\"/>\n" \
                              "</filter>\n", \
                              filter_id, \
                              source_compositing_group_id, \
                              destination_compositing_group_id);

#define _CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER(mode) \
    _cairo_svg_stream_printf (&surface->document->xml_node_filters, \
                              "<filter id=\"filter-%d\" " \
                              "x=\"0%%\" y=\"0%%\" width=\"100%%\" height=\"100%%\">\n" \
                              "<feImage xlink:href=\"#compositing-group-%d\" result=\"source\"", \
                              filter_id, \
                              source_compositing_group_id); \
    _cairo_svg_stream_append_paint_dependent (&surface->document->xml_node_filters, \
                                              surface->source_id, \
                                              CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE_AT_ORIGIN); \
    _cairo_svg_stream_printf (&surface->document->xml_node_filters, \
                              "/>\n" \
                              "<feImage xlink:href=\"#compositing-group-%d\" result=\"destination\"", \
                              destination_compositing_group_id); \
    _cairo_svg_stream_append_paint_dependent (&surface->document->xml_node_filters, \
                                              surface->source_id, \
                                              CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE_AT_ORIGIN); \
    _cairo_svg_stream_printf (&surface->document->xml_node_filters, \
                              "/>\n" \
                              "<feBlend in=\"source\" in2=\"destination\" " \
                              "mode=\"" mode "\" " \
                              "color-interpolation-filters=\"sRGB\"/>\n" \
                              "</filter>\n", \
                              filter_id, \
                              source_compositing_group_id, \
                              destination_compositing_group_id);

static unsigned int
_cairo_svg_surface_emit_parametric_filter (cairo_svg_surface_t *surface,
					   enum cairo_svg_filter filter,
					   unsigned int source_compositing_group_id,
					   unsigned int destination_compositing_group_id)
{
    unsigned int filter_id = surface->document->filter_id++;
    switch (filter) {
    case CAIRO_SVG_FILTER_OVER:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_COMPOSITE_FILTER ("over")
	break;
    case CAIRO_SVG_FILTER_IN:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_COMPOSITE_FILTER ("in")
	break;
    case CAIRO_SVG_FILTER_OUT:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_COMPOSITE_FILTER ("out")
	break;
    case CAIRO_SVG_FILTER_ATOP:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_COMPOSITE_FILTER ("atop")
	break;
    case CAIRO_SVG_FILTER_XOR:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_COMPOSITE_FILTER ("xor")
	break;
    case CAIRO_SVG_FILTER_ADD:
	// This can also be done with <feComposite operator="lighter"/>, but it is not in SVG 1.1
	_cairo_svg_stream_printf (&surface->document->xml_node_filters,
				  "<filter id=\"filter-%d\" "
				  "x=\"0%%\" y=\"0%%\" width=\"100%%\" height=\"100%%\">\n"
				  "<feImage xlink:href=\"#compositing-group-%d\" result=\"source\"",
				  filter_id,
				  source_compositing_group_id);
	_cairo_svg_stream_append_paint_dependent (&surface->document->xml_node_filters,
						  surface->source_id,
						  CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE_AT_ORIGIN);
	_cairo_svg_stream_printf (&surface->document->xml_node_filters,
				  "/>\n"
				  "<feImage xlink:href=\"#compositing-group-%d\" result=\"destination\"",
				  destination_compositing_group_id);
	_cairo_svg_stream_append_paint_dependent (&surface->document->xml_node_filters,
						  surface->source_id,
						  CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE_AT_ORIGIN);
	_cairo_svg_stream_printf (&surface->document->xml_node_filters,
				  "/>\n"
				  "<feComposite in=\"source\" in2=\"destination\" "
				  "operator=\"arithmetic\" k1=\"0\" k2=\"1\" k3=\"1\" k4=\"0\" "
				  "color-interpolation-filters=\"sRGB\"/>\n"
				  "</filter>\n");
	break;
    case CAIRO_SVG_FILTER_MULTIPLY:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("multiply")
	break;
    case CAIRO_SVG_FILTER_SCREEN:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("screen")
	break;
    case CAIRO_SVG_FILTER_OVERLAY:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("overlay")
	break;
    case CAIRO_SVG_FILTER_DARKEN:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("darken")
	break;
    case CAIRO_SVG_FILTER_LIGHTEN:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("lighten")
	break;
    case CAIRO_SVG_FILTER_COLOR_DODGE:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("color-dodge")
	break;
    case CAIRO_SVG_FILTER_COLOR_BURN:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("color-burn")
	break;
    case CAIRO_SVG_FILTER_HARD_LIGHT:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("hard-light")
	break;
    case CAIRO_SVG_FILTER_SOFT_LIGHT:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("soft-light")
	break;
    case CAIRO_SVG_FILTER_DIFFERENCE:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("difference")
	break;
    case CAIRO_SVG_FILTER_EXCLUSION:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("exclusion")
	break;
    case CAIRO_SVG_FILTER_HUE:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("hue")
	break;
    case CAIRO_SVG_FILTER_SATURATION:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("saturation")
	break;
    case CAIRO_SVG_FILTER_COLOR:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("color")
	break;
    case CAIRO_SVG_FILTER_LUMINOSITY:
	_CAIRO_SVG_SURFACE_OUTPUT_FE_BLEND_FILTER ("luminosity")
	break;
    case CAIRO_SVG_FILTER_REMOVE_COLOR:
    case CAIRO_SVG_FILTER_REMOVE_COLOR_AND_INVERT_ALPHA:
    case CAIRO_SVG_FILTER_COLOR_TO_ALPHA:
    case CAIRO_SVG_FILTER_LAST_STATIC_FILTER:
    default:
	ASSERT_NOT_REACHED;
    }
    return filter_id;
}

typedef struct {
    cairo_svg_stream_t *output;
    unsigned int in_mem;
    unsigned int trailing;
    unsigned char src[3];
} base64_write_closure_t;

static char const base64_table[64] =
"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

static cairo_status_t
base64_write_func (void *closure,
		   const unsigned char *data,
		   unsigned int length)
{
    base64_write_closure_t *info = (base64_write_closure_t *) closure;
    unsigned int i;
    unsigned char *src;

    src = info->src;

    if (info->in_mem + length < 3) {
	for (i = 0; i < length; i++) {
	    src[i + info->in_mem] = *data++;
	}
	info->in_mem += length;
	return CAIRO_STATUS_SUCCESS;
    }

    do {
	unsigned char dst[4];

	for (i = info->in_mem; i < 3; i++) {
	    src[i] = *data++;
	    length--;
	}
	info->in_mem = 0;

	dst[0] = base64_table[src[0] >> 2];
	dst[1] = base64_table[(src[0] & 0x03) << 4 | src[1] >> 4];
	dst[2] = base64_table[(src[1] & 0x0f) << 2 | src[2] >> 6];
	dst[3] = base64_table[src[2] & 0xfc >> 2];
	/* Special case for the last missing bits */
	switch (info->trailing) {
	    case 2:
		dst[2] = '=';
		/* fall through */
	    case 1:
		dst[3] = '=';
	    default:
		break;
	}
	_cairo_svg_stream_write (info->output, dst, 4);
    } while (length >= 3);

    for (i = 0; i < length; i++) {
	src[i] = *data++;
    }
    info->in_mem = length;

    return info->output->status;
}

static cairo_int_status_t
_cairo_surface_base64_encode_jpeg (cairo_surface_t       *surface,
				   cairo_svg_stream_t *output)
{
    const unsigned char *mime_data;
    unsigned long mime_data_length;
    cairo_image_info_t image_info;
    base64_write_closure_t info;
    cairo_status_t status;

    cairo_surface_get_mime_data (surface, CAIRO_MIME_TYPE_JPEG,
				 &mime_data, &mime_data_length);
    if (mime_data == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    status = _cairo_image_info_get_jpeg_info (&image_info, mime_data, mime_data_length);
    if (unlikely (status))
	return status;

    if (image_info.num_components == 4)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    _cairo_svg_stream_printf (output, "data:image/jpeg;base64,");

    info.output = output;
    info.in_mem = 0;
    info.trailing = 0;

    status = base64_write_func (&info, mime_data, mime_data_length);
    if (unlikely (status))
	return status;

    if (info.in_mem > 0) {
	memset (info.src + info.in_mem, 0, 3 - info.in_mem);
	info.trailing = 3 - info.in_mem;
	info.in_mem = 3;
	status = base64_write_func (&info, NULL, 0);
    }

    return status;
}

static cairo_int_status_t
_cairo_surface_base64_encode_png (cairo_surface_t       *surface,
				  cairo_svg_stream_t *output)
{
    const unsigned char *mime_data;
    unsigned long mime_data_length;
    base64_write_closure_t info;
    cairo_status_t status;

    cairo_surface_get_mime_data (surface, CAIRO_MIME_TYPE_PNG,
				 &mime_data, &mime_data_length);
    if (unlikely (surface->status))
	return surface->status;
    if (mime_data == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    _cairo_svg_stream_printf (output, "data:image/png;base64,");

    info.output = output;
    info.in_mem = 0;
    info.trailing = 0;

    status = base64_write_func (&info, mime_data, mime_data_length);
    if (unlikely (status))
	return status;

    if (info.in_mem > 0) {
	memset (info.src + info.in_mem, 0, 3 - info.in_mem);
	info.trailing = 3 - info.in_mem;
	info.in_mem = 3;
	status = base64_write_func (&info, NULL, 0);
    }

    return status;
}

static cairo_int_status_t
_cairo_surface_base64_encode (cairo_surface_t       *surface,
			      cairo_svg_stream_t *output)
{
    cairo_int_status_t status;
    base64_write_closure_t info;

    status = _cairo_surface_base64_encode_jpeg (surface, output);
    if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	return status;

    status = _cairo_surface_base64_encode_png (surface, output);
    if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	return status;

    info.output = output;
    info.in_mem = 0;
    info.trailing = 0;

    _cairo_svg_stream_printf (info.output, "data:image/png;base64,");

    status = cairo_surface_write_to_png_stream (surface, base64_write_func,
						(void *) &info);

    if (unlikely (status))
	return status;

    if (info.in_mem > 0) {
	memset (info.src + info.in_mem, 0, 3 - info.in_mem);
	info.trailing = 3 - info.in_mem;
	info.in_mem = 3;
	status = base64_write_func (&info, NULL, 0);
    }

    return status;
}

/**
 * _cairo_svg_surface_emit_attr_value:
 *
 * Write the value to output the stream as a sequence of characters,
 * while escaping those which have special meaning in the XML
 * attribute's value context: &amp; and &quot;.
 **/
static void
_cairo_svg_surface_emit_attr_value (cairo_svg_stream_t *stream,
				    const unsigned char *value,
				    unsigned int length)
{
    const unsigned char *p;
    const unsigned char *q;
    unsigned int i;

    /* we'll accumulate non-special chars in [q, p) range */
    p = value;
    q = p;
    for (i = 0; i < length; i++, p++) {
	if (*p == '&' || *p == '"') {
	    /* flush what's left before special char */
	    if (p != q) {
		_cairo_svg_stream_write (stream, q, p - q);
		q = p + 1;
	    }

	    if (*p == '&')
		_cairo_svg_stream_printf (stream, "&amp;");
	    else // p == '"'
		_cairo_svg_stream_printf (stream, "&quot;");
	}
    }

    /* flush the trailing chars if any */
    if (p != q)
	_cairo_svg_stream_write (stream, q, p - q);
}

static cairo_status_t
_cairo_svg_surface_emit_surface (cairo_svg_document_t *document,
				 cairo_surface_t *surface,
				 unsigned int source_id)
{
    cairo_rectangle_int_t extents;
    cairo_bool_t is_bounded;
    cairo_status_t status;
    const unsigned char *uri;
    unsigned long uri_len;

    is_bounded = _cairo_surface_get_extents (surface, &extents);
    assert (is_bounded);

    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<image id=\"source-%d\" x=\"%d\" y=\"%d\" width=\"%d\" height=\"%d\"",
			      source_id,
			      extents.x, extents.y,
			      extents.width, extents.height);

    if (extents.width != 0 && extents.height != 0) {
	_cairo_svg_stream_printf (&document->xml_node_defs, " xlink:href=\"");
	cairo_surface_get_mime_data (surface, CAIRO_MIME_TYPE_URI,
				     &uri, &uri_len);
	if (uri != NULL) {
	    _cairo_svg_surface_emit_attr_value (&document->xml_node_defs,
						uri, uri_len);
	} else {
	    status = _cairo_surface_base64_encode (surface,
						   &document->xml_node_defs);
	    if (unlikely (status))
		return status;
	}
    _cairo_svg_stream_printf (&document->xml_node_defs, "\"");
    }

    _cairo_svg_stream_printf (&document->xml_node_defs, "/>\n");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_svg_surface_emit_composite_surface_pattern (cairo_svg_stream_t *output,
						   cairo_svg_surface_t *surface,
						   cairo_surface_pattern_t *pattern,
						   unsigned int pattern_id,
						   const cairo_matrix_t *parent_matrix)
{
    cairo_status_t status;

    cairo_matrix_t p2u = pattern->base.matrix;
    status = cairo_matrix_invert (&p2u);
    /* cairo_pattern_set_matrix ensures the matrix is invertible */
    assert (status == CAIRO_STATUS_SUCCESS);

    cairo_bool_t is_new;
    cairo_svg_source_surface_t *source_surface;
    status = _cairo_svg_surface_add_source_surface (surface,
						    pattern->surface,
						    &is_new,
						    &source_surface);
    if (unlikely (status)) {
	return status;
    }
    unsigned int source_id = source_surface->id;

    if (is_new) {
	status = _cairo_svg_surface_emit_surface (surface->document,
						  pattern->surface,
						  source_id);
	if (unlikely (status)) {
	    return status;
	}
    }

    if (pattern_id != invalid_pattern_id) {
	cairo_rectangle_int_t extents;
	cairo_bool_t is_bounded;

	is_bounded = _cairo_surface_get_extents (pattern->surface, &extents);
	assert (is_bounded);

	_cairo_svg_stream_printf (output,
				  "<pattern id=\"pattern-%d\" "
				  "patternUnits=\"userSpaceOnUse\" "
				  "x=\"%d\" y=\"%d\" "
				  "width=\"%d\" height=\"%d\" "
				  "viewBox=\"%d %d %d %d\"",
				  pattern_id,
				  extents.x, extents.y,
				  extents.width, extents.height,
				  extents.x, extents.y,
				  extents.width, extents.height);
	_cairo_svg_surface_emit_transform (output,
					   "patternTransform",
					   &p2u,
					   parent_matrix);
	_cairo_svg_stream_printf (output, ">\n");
    }

    _cairo_svg_stream_printf (output,
			      "<use xlink:href=\"#source-%d\"",
			      source_id);
    if (pattern->surface->content == CAIRO_CONTENT_ALPHA) {
	cairo_bool_t can_skip_filter = FALSE;
	if (pattern->surface->backend &&
	    pattern->surface->backend->type == CAIRO_SURFACE_TYPE_IMAGE &&
	    (((cairo_image_surface_t *) pattern->surface)->format == CAIRO_FORMAT_A1 ||
	     ((cairo_image_surface_t *) pattern->surface)->format == CAIRO_FORMAT_A8)) {
	    can_skip_filter = TRUE;
	}
	if (!can_skip_filter) {
	    _cairo_svg_stream_printf (output,
				      " filter=\"url(#filter-%s)\"",
				      _cairo_svg_surface_emit_static_filter (surface->document,
									     CAIRO_SVG_FILTER_COLOR_TO_ALPHA));
	}
    }
    if (pattern_id == invalid_pattern_id) {
	_cairo_svg_surface_emit_transform (output,
					   "transform",
					   &p2u,
					   parent_matrix);
    }
    _cairo_svg_stream_printf (output, "/>\n");

    if (pattern_id != invalid_pattern_id) {
	_cairo_svg_stream_printf (output, "</pattern>\n");
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_svg_surface_emit_recording_surface (cairo_svg_surface_t *surface,
					   cairo_recording_surface_t *source,
					   unsigned int source_id,
					   cairo_bool_t *transitive_paint_used)
{
    cairo_status_t status;
    cairo_svg_document_t *document = surface->document;

    cairo_surface_t *paginated_surface = _cairo_svg_surface_create_for_document (document,
										 source->base.content,
										 0,
										 0,
										 FALSE);
    cairo_svg_surface_t *svg_surface = (cairo_svg_surface_t *) _cairo_paginated_surface_get_target (paginated_surface);
    if (unlikely (paginated_surface->status)) {
	return paginated_surface->status;
    }

    svg_surface->source_id = source_id;
    svg_surface->depth = surface->depth + 1;

    cairo_rectangle_int_t extents;
    cairo_bool_t bounded = _cairo_surface_get_extents (&source->base, &extents);

    cairo_surface_set_fallback_resolution (paginated_surface,
					   document->owner->x_fallback_resolution,
					   document->owner->y_fallback_resolution);

    if (source->base.content == CAIRO_CONTENT_COLOR) {
	_cairo_svg_surface_emit_paint (&svg_surface->xml_node, svg_surface, &_cairo_pattern_black.base, FALSE);
    }
    status = _cairo_recording_surface_replay (&source->base, paginated_surface);
    if (unlikely (status)) {
	cairo_surface_destroy (paginated_surface);
	return status;
    }

    cairo_surface_show_page (paginated_surface);
    status = cairo_surface_status (paginated_surface);
    if (unlikely (status)) {
	cairo_surface_destroy (paginated_surface);
	return status;
    }

    unsigned int clip_id;
    if (bounded) {
	clip_id = document->clip_id++;

	_cairo_svg_stream_printf (&document->xml_node_defs,
				  "<clipPath id=\"clip-%d\">\n"
				  "<rect x=\"%d\" y=\"%d\" width=\"%d\" height=\"%d\"/>\n"
				  "</clipPath>\n",
				  clip_id,
				  extents.x,
				  extents.y,
				  extents.width,
				  extents.height);
    }

    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<g id=\"source-%d\"",
			      source_id);

    if (bounded) {
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  " clip-path=\"url(#clip-%d)\"",
				  clip_id);
    }

    if (source->base.content == CAIRO_CONTENT_ALPHA) {
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  " filter=\"url(#filter-%s)\"",
				  _cairo_svg_surface_emit_static_filter (document, CAIRO_SVG_FILTER_REMOVE_COLOR));
    }

    _cairo_svg_stream_printf (&document->xml_node_defs, ">\n");

    if (svg_surface->xml_node.elements.num_elements > 0) {
	cairo_svg_page_t *page = _cairo_svg_surface_store_page (svg_surface);
	if (unlikely (page == NULL)) {
	    cairo_surface_destroy (paginated_surface);
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	}
    }

    if (svg_surface->page_set.num_elements > 0) {
	cairo_svg_page_t *page = _cairo_array_index (&svg_surface->page_set, svg_surface->page_set.num_elements - 1);
	_cairo_svg_stream_copy (&page->xml_node, &document->xml_node_defs);
    }

    _cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");

    *transitive_paint_used = svg_surface->transitive_paint_used;

    status = cairo_surface_status (paginated_surface);
    cairo_surface_destroy (paginated_surface);

    return status;
}

static cairo_recording_surface_t *
_cairo_svg_surface_to_recording_surface (const cairo_surface_pattern_t *pattern)
{
    cairo_surface_t *surface = pattern->surface;
    if (_cairo_surface_is_paginated (surface))
	surface = _cairo_paginated_surface_get_recording (surface);
    if (_cairo_surface_is_snapshot (surface))
	surface = _cairo_surface_snapshot_get_target (surface);
    return (cairo_recording_surface_t *) surface;
}

static cairo_bool_t
_cairo_svg_surface_svg_pattern_should_be_used (const cairo_pattern_t *pattern)
{
    cairo_rectangle_int_t extents;
    return pattern->type == CAIRO_PATTERN_TYPE_SURFACE &&
	   pattern->extend == CAIRO_EXTEND_REPEAT &&
	   _cairo_surface_get_extents (((cairo_surface_pattern_t *) pattern)->surface, &extents);
}

static cairo_bool_t
_cairo_svg_surface_svg_clip_or_svg_mask_should_be_used (const cairo_pattern_t *pattern)
{
    return pattern->type == CAIRO_PATTERN_TYPE_SURFACE && !_cairo_svg_surface_svg_pattern_should_be_used (pattern);
}

static cairo_status_t
_cairo_svg_surface_emit_composite_recording_pattern (cairo_svg_stream_t *output,
						     cairo_svg_surface_t *surface,
						     cairo_surface_pattern_t *pattern,
						     unsigned int pattern_id,
						     const cairo_matrix_t *parent_matrix)
{
    cairo_status_t status;
    cairo_svg_document_t *document = surface->document;

    cairo_matrix_t p2u = pattern->base.matrix;
    status = cairo_matrix_invert (&p2u);
    /* cairo_pattern_set_matrix ensures the matrix is invertible */
    assert (status == CAIRO_STATUS_SUCCESS);

    cairo_bool_t is_new;
    cairo_svg_source_surface_t *source_surface;
    status = _cairo_svg_surface_add_source_surface (surface,
						    pattern->surface,
						    &is_new,
						    &source_surface);
    if (unlikely (status)) {
	return status;
    }
    unsigned int source_id = source_surface->id;

    cairo_recording_surface_t *recording_surface = _cairo_svg_surface_to_recording_surface (pattern);
    if (is_new) {
	status = _cairo_svg_surface_emit_recording_surface (surface,
							    recording_surface,
							    source_id,
							    &source_surface->transitive_paint_used);
	if (unlikely (status)) {
	    return status;
	}

	if (source_surface->transitive_paint_used) {
	    cairo_svg_paint_t *paint_entry = malloc (sizeof (cairo_svg_paint_t));
	    if (paint_entry == NULL) {
		return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    }
	    paint_entry->source_id = source_id;
	    _cairo_array_init (&paint_entry->paint_elements, sizeof (cairo_svg_paint_element_t));
	    _cairo_svg_paint_init_key (paint_entry);
	    status = _cairo_hash_table_insert (document->paints, &paint_entry->base);
	    if (unlikely (status)) {
		return status;
	    }
	}
    }

    if (source_surface->transitive_paint_used) {
	cairo_svg_paint_t paint_key;
	paint_key.source_id = source_id;
	_cairo_svg_paint_init_key (&paint_key);

	cairo_svg_paint_t *found_paint_entry = _cairo_hash_table_lookup (document->paints,
									 &paint_key.base);
	assert (found_paint_entry);

	cairo_svg_paint_element_t paint_element;
	paint_element.source_id = surface->source_id;
	paint_element.matrix = pattern->base.matrix;
	if (parent_matrix != NULL) {
	    cairo_matrix_t parent_matrix_inverse = *parent_matrix;
	    status = cairo_matrix_invert (&parent_matrix_inverse);
	    /* cairo_pattern_set_matrix ensures the matrix is invertible */
	    assert (status == CAIRO_STATUS_SUCCESS);
	    cairo_matrix_multiply (&paint_element.matrix, &parent_matrix_inverse, &paint_element.matrix);
	}
	status = _cairo_array_append (&found_paint_entry->paint_elements, &paint_element);
	if (unlikely (status)) {
	    return status;
	}

	surface->transitive_paint_used = TRUE;
    }

    if (pattern_id != invalid_pattern_id) {
	assert (!recording_surface->unbounded);
	_cairo_svg_stream_printf (output,
				  "<pattern id=\"pattern-%d\" "
				  "patternUnits=\"userSpaceOnUse\" "
				  "x=\"%f\" y=\"%f\" width=\"%f\" height=\"%f\" "
				  "viewBox=\"%f %f %f %f\"",
				  pattern_id,
				  recording_surface->extents_pixels.x,
				  recording_surface->extents_pixels.y,
				  recording_surface->extents_pixels.width,
				  recording_surface->extents_pixels.height,
				  recording_surface->extents_pixels.x,
				  recording_surface->extents_pixels.y,
				  recording_surface->extents_pixels.width,
				  recording_surface->extents_pixels.height);
	_cairo_svg_surface_emit_transform (output, "patternTransform", &p2u, parent_matrix);
	_cairo_svg_stream_printf (output, ">\n");
    }

    _cairo_svg_stream_printf (output,
			      "<use xlink:href=\"#source-%d\"",
			      source_id);

    if (pattern_id == invalid_pattern_id) {
	_cairo_svg_surface_emit_transform (output, "transform", &p2u, parent_matrix);
    }

    _cairo_svg_stream_printf (output, "/>\n");

    if (pattern_id != invalid_pattern_id) {
	_cairo_svg_stream_printf (output, "</pattern>\n");
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_svg_surface_emit_composite_pattern (cairo_svg_stream_t *output,
					   cairo_svg_surface_t *surface,
					   cairo_surface_pattern_t *pattern,
					   unsigned int pattern_id,
					   const cairo_matrix_t *parent_matrix)
{
    if (pattern_id != invalid_pattern_id) {
	assert (_cairo_svg_surface_svg_pattern_should_be_used (&pattern->base));
    }

    if (pattern->surface->type == CAIRO_SURFACE_TYPE_RECORDING) {
	return _cairo_svg_surface_emit_composite_recording_pattern (output,
								    surface,
								    pattern,
								    pattern_id,
								    parent_matrix);
    } else {
	return _cairo_svg_surface_emit_composite_surface_pattern (output,
								  surface,
								  pattern,
								  pattern_id,
								  parent_matrix);
    }
}

static cairo_status_t
_cairo_svg_surface_emit_solid_pattern (cairo_svg_surface_t *surface,
				       cairo_solid_pattern_t *pattern,
				       cairo_svg_stream_t *output,
				       cairo_bool_t is_stroke)
{
    _cairo_svg_stream_printf (output,
			      is_stroke ? " stroke=\"rgb(%f%%, %f%%, %f%%)\" stroke-opacity=\"%f\""
					: " fill=\"rgb(%f%%, %f%%, %f%%)\" fill-opacity=\"%f\"",
			      pattern->color.red * 100.0,
			      pattern->color.green * 100.0,
			      pattern->color.blue * 100.0,
			      pattern->color.alpha);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_svg_surface_emit_surface_pattern (cairo_svg_surface_t *surface,
					 cairo_surface_pattern_t *pattern,
					 cairo_svg_stream_t *output,
					 cairo_bool_t is_stroke,
					 const cairo_matrix_t *parent_matrix)
{
    cairo_svg_document_t *document = surface->document;
    cairo_status_t status;

    unsigned int pattern_id = document->pattern_id++;

    status = _cairo_svg_surface_emit_composite_pattern (&document->xml_node_defs,
							surface,
							pattern,
							pattern_id,
							parent_matrix);
    if (unlikely (status))
	return status;

    _cairo_svg_stream_printf (output,
			      is_stroke ? " stroke=\"url(#pattern-%d)\""
					: " fill=\"url(#pattern-%d)\"",
			      pattern_id);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_svg_surface_emit_pattern_stops (cairo_svg_stream_t *output,
				       const cairo_gradient_pattern_t *pattern,
				       double start_offset,
				       cairo_bool_t reverse_stops,
				       cairo_bool_t emulate_reflect)
{
    cairo_gradient_stop_t *stops;
    unsigned int n_stops;

    if (pattern->n_stops < 1) {
	return CAIRO_STATUS_SUCCESS;
    }

    if (pattern->n_stops == 1) {
	_cairo_svg_stream_printf (output,
				  "<stop offset=\"%f\" "
				  "stop-color=\"rgb(%f%%, %f%%, %f%%)\" "
				  "stop-opacity=\"%f\"/>\n",
				  pattern->stops[0].offset,
				  pattern->stops[0].color.red * 100.0,
				  pattern->stops[0].color.green * 100.0,
				  pattern->stops[0].color.blue * 100.0,
				  pattern->stops[0].color.alpha);
	return CAIRO_STATUS_SUCCESS;
    }

    if (emulate_reflect || reverse_stops) {
	n_stops = emulate_reflect ? pattern->n_stops * 2 - 2 : pattern->n_stops;
	stops = _cairo_malloc_ab (n_stops, sizeof (cairo_gradient_stop_t));
	if (unlikely (stops == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	for (unsigned int i = 0; i < pattern->n_stops; i++) {
	    if (reverse_stops) {
		stops[i] = pattern->stops[pattern->n_stops - i - 1];
		stops[i].offset = 1.0 - stops[i].offset;
	    } else {
		stops[i] = pattern->stops[i];
	    }
	    if (emulate_reflect) {
		stops[i].offset *= 0.5;
		if (i > 0 && i < pattern->n_stops - 1) {
		    if (reverse_stops) {
			stops[i + pattern->n_stops - 1] = pattern->stops[i];
			stops[i + pattern->n_stops - 1].offset = 0.5 + 0.5 * stops[i + pattern->n_stops - 1].offset;
		    } else {
			stops[i + pattern->n_stops - 1] = pattern->stops[pattern->n_stops - i - 1];
			stops[i + pattern->n_stops - 1].offset = 1.0 - 0.5 * stops[i + pattern->n_stops - 1].offset;
		    }
		}
	    }
	}
    } else {
	n_stops = pattern->n_stops;
	stops = pattern->stops;
    }

    if (start_offset >= 0.0) {
	for (unsigned int i = 0; i < n_stops; i++) {
	    _cairo_svg_stream_printf (output,
				      "<stop offset=\"%f\" "
				      "stop-color=\"rgb(%f%%, %f%%, %f%%)\" "
				      "stop-opacity=\"%f\"/>\n",
				      start_offset + (1.0 - start_offset) * stops[i].offset,
				      stops[i].color.red * 100.0,
				      stops[i].color.green * 100.0,
				      stops[i].color.blue * 100.0,
				      stops[i].color.alpha);
	}
    } else {
	cairo_bool_t found = FALSE;
	unsigned int offset_index;
	cairo_color_stop_t offset_color_start, offset_color_stop;

	for (unsigned int i = 0; i <= n_stops; i++) {
	    double x1 = i == n_stops ? stops[0].offset + 1 : stops[i].offset;
	    cairo_color_stop_t *color1 = i == n_stops ? &stops[0].color : &stops[i].color;
	    if (x1 >= -start_offset) {
		if (i > 0) {
		    double x0 = stops[i - 1].offset;
		    cairo_color_stop_t *color0 = &stops[i - 1].color;
		    if (x0 != x1) {
			offset_color_start.red = color0->red + (color1->red - color0->red)
							       * (-start_offset - x0) / (x1 - x0);
			offset_color_start.green = color0->green + (color1->green - color0->green)
								   * (-start_offset - x0) / (x1 - x0);
			offset_color_start.blue = color0->blue + (color1->blue - color0->blue)
								 * (-start_offset - x0) / (x1 - x0);
			offset_color_start.alpha = color0->alpha + (color1->alpha - color0->alpha)
								   * (-start_offset - x0) / (x1 - x0);
			offset_color_stop = offset_color_start;
		    } else {
			offset_color_stop = stops[i - 1].color;
			offset_color_start = stops[i].color;
		    }
		} else {
		    offset_color_stop = offset_color_start = stops[i].color;
		}
		offset_index = i;
		found = TRUE;
		break;
	    }
	}

	if (!found) {
	    offset_index = n_stops - 1;
	    offset_color_stop = offset_color_start = stops[offset_index].color;
	}

	_cairo_svg_stream_printf (output,
				  "<stop offset=\"0\" "
				  "stop-color=\"rgb(%f%%, %f%%, %f%%)\" "
				  "stop-opacity=\"%f\"/>\n",
				  offset_color_start.red * 100.0,
				  offset_color_start.green * 100.0,
				  offset_color_start.blue * 100.0,
				  offset_color_start.alpha);
	for (unsigned int i = offset_index; i < n_stops; i++) {
	    _cairo_svg_stream_printf (output,
				      "<stop offset=\"%f\" "
				      "stop-color=\"rgb(%f%%, %f%%, %f%%)\" "
				      "stop-opacity=\"%f\"/>\n",
				      stops[i].offset + start_offset,
				      stops[i].color.red * 100.0,
				      stops[i].color.green * 100.0,
				      stops[i].color.blue * 100.0,
				      stops[i].color.alpha);
	}
	for (unsigned int i = 0; i < offset_index; i++) {
	    _cairo_svg_stream_printf (output,
				      "<stop offset=\"%f\" "
				      "stop-color=\"rgb(%f%%, %f%%, %f%%)\" "
				      "stop-opacity=\"%f\"/>\n",
				      1.0 + stops[i].offset + start_offset,
				      stops[i].color.red * 100.0,
				      stops[i].color.green * 100.0,
				      stops[i].color.blue * 100.0,
				      stops[i].color.alpha);
	}

	_cairo_svg_stream_printf (output,
				  "<stop offset=\"1\" "
				  "stop-color=\"rgb(%f%%, %f%%, %f%%)\" "
				  "stop-opacity=\"%f\"/>\n",
				  offset_color_stop.red * 100.0,
				  offset_color_stop.green * 100.0,
				  offset_color_stop.blue * 100.0,
				  offset_color_stop.alpha);

    }

    if (reverse_stops || emulate_reflect) {
	free (stops);
    }

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_svg_surface_emit_pattern_extend (cairo_svg_stream_t *output,
					cairo_pattern_t *pattern)
{
    switch (pattern->extend) {
    case CAIRO_EXTEND_REPEAT:
	_cairo_svg_stream_printf (output, " spreadMethod=\"repeat\"");
	break;
    case CAIRO_EXTEND_REFLECT:
	_cairo_svg_stream_printf (output, " spreadMethod=\"reflect\"");
	break;
    case CAIRO_EXTEND_NONE:
    case CAIRO_EXTEND_PAD:
	break;
    }
}

static cairo_status_t
_cairo_svg_surface_emit_linear_pattern (cairo_svg_surface_t *surface,
					cairo_linear_pattern_t *pattern,
					cairo_svg_stream_t *output,
					cairo_bool_t is_stroke,
					const cairo_matrix_t *parent_matrix)
{
    cairo_status_t status;
    cairo_svg_document_t *document = surface->document;

    cairo_matrix_t p2u = pattern->base.base.matrix;
    status = cairo_matrix_invert (&p2u);
    /* cairo_pattern_set_matrix ensures the matrix is invertible */
    assert (status == CAIRO_STATUS_SUCCESS);

    unsigned int linear_pattern_id = document->linear_pattern_id++;

    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<linearGradient id=\"linear-pattern-%d\" "
			      "gradientUnits=\"userSpaceOnUse\" "
			      "x1=\"%f\" y1=\"%f\" x2=\"%f\" y2=\"%f\"",
			      linear_pattern_id,
			      pattern->pd1.x, pattern->pd1.y,
			      pattern->pd2.x, pattern->pd2.y);

    _cairo_svg_surface_emit_pattern_extend (&document->xml_node_defs, &pattern->base.base);
    _cairo_svg_surface_emit_transform (&document->xml_node_defs, "gradientTransform", &p2u, parent_matrix);
    _cairo_svg_stream_printf (&document->xml_node_defs, ">\n");

    status = _cairo_svg_surface_emit_pattern_stops (&document->xml_node_defs,
						    &pattern->base,
						    0.0,
						    FALSE,
						    FALSE);
    if (unlikely (status))
	return status;

    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "</linearGradient>\n");

    _cairo_svg_stream_printf (output,
			      is_stroke ? " stroke=\"url(#linear-pattern-%d)\""
					: " fill=\"url(#linear-pattern-%d)\"",
			      linear_pattern_id);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_svg_surface_emit_radial_pattern (cairo_svg_surface_t *surface,
					cairo_radial_pattern_t *pattern,
					cairo_svg_stream_t *output,
					cairo_bool_t is_stroke,
					const cairo_matrix_t *parent_matrix)
{
    cairo_status_t status;
    cairo_svg_document_t *document = surface->document;

    cairo_extend_t extend = pattern->base.base.extend;

    cairo_bool_t reverse_stops;
    cairo_circle_double_t *c0, *c1;
    if (pattern->cd1.radius < pattern->cd2.radius) {
	c0 = &pattern->cd1;
	c1 = &pattern->cd2;
	reverse_stops = FALSE;
    } else {
	c0 = &pattern->cd2;
	c1 = &pattern->cd1;
	reverse_stops = TRUE;
    }

    double x0 = c0->center.x;
    double y0 = c0->center.y;
    double r0 = c0->radius;
    double x1 = c1->center.x;
    double y1 = c1->center.y;
    double r1 = c1->radius;

    cairo_matrix_t p2u = pattern->base.base.matrix;
    status = cairo_matrix_invert (&p2u);
    /* cairo_pattern_set_matrix ensures the matrix is invertible */
    assert (status == CAIRO_STATUS_SUCCESS);

    unsigned int radial_pattern_id = document->radial_pattern_id++;

    double start_offset;
    cairo_bool_t emulate_reflect = FALSE;

    double fx = (r1 * x0 - r0 * x1) / (r1 - r0);
    double fy = (r1 * y0 - r0 * y1) / (r1 - r0);

    /* SVG doesn't support the inner circle and use instead a gradient focal.
     * That means we need to emulate the cairo behaviour by processing the
     * cairo gradient stops.
     * The CAIRO_EXTEND_NONE and CAIRO_EXTEND_PAD modes are quite easy to handle,
     * it's just a matter of stop position translation and calculation of
     * the corresponding SVG radial gradient focal.
     * The CAIRO_EXTEND_REFLECT and CAIRO_EXTEND_REPEAT modes require to compute a new
     * radial gradient, with an new outer circle, equal to r1 - r0 in the CAIRO_EXTEND_REPEAT
     * case, and 2 * r1 - r0 in the CAIRO_EXTEND_REFLECT case, and a new gradient stop
     * list that maps to the original cairo stop list.
     */
    if ((extend == CAIRO_EXTEND_REFLECT || extend == CAIRO_EXTEND_REPEAT) && r0 > 0.0) {
	double r_org = r1;

	if (extend == CAIRO_EXTEND_REFLECT) {
	    r1 = 2.0 * r1 - r0;
	    emulate_reflect = TRUE;
	}

	start_offset = fmod (r1, r1 - r0) / (r1 - r0) - 1.0;
	double r = r1 - r0;

	/* New position of outer circle. */
	double x = r * (x1 - fx) / r_org + fx;
	double y = r * (y1 - fy) / r_org + fy;

	x1 = x;
	y1 = y;
	r1 = r;
	r0 = 0.0;
    } else {
	start_offset = r0 / r1;
    }

    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<radialGradient id=\"radial-pattern-%d\" "
			      "gradientUnits=\"userSpaceOnUse\" "
			      "cx=\"%f\" cy=\"%f\" "
			      "fx=\"%f\" fy=\"%f\" r=\"%f\"",
			      radial_pattern_id,
			      x1, y1,
			      fx, fy, r1);

    if (emulate_reflect) {
	_cairo_svg_stream_printf (&document->xml_node_defs, " spreadMethod=\"repeat\"");
    } else {
	_cairo_svg_surface_emit_pattern_extend (&document->xml_node_defs, &pattern->base.base);
    }
    _cairo_svg_surface_emit_transform (&document->xml_node_defs, "gradientTransform", &p2u, parent_matrix);
    _cairo_svg_stream_printf (&document->xml_node_defs, ">\n");

    /* To support cairo's EXTEND_NONE, (for which SVG has no similar
     * notion), we add transparent color stops on either end of the
     * user-provided stops. */
    if (extend == CAIRO_EXTEND_NONE) {
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  "<stop offset=\"0\" "
				  "stop-color=\"rgb(0%%, 0%%, 0%%)\" "
				  "stop-opacity=\"0\"/>\n");
	if (r0 != 0.0) {
	    _cairo_svg_stream_printf (&document->xml_node_defs,
				      "<stop offset=\"%f\" "
				      "stop-color=\"rgb(0%%, 0%%, 0%%)\" "
				      "stop-opacity=\"0\"/>\n",
				      r0 / r1);
	}
    }
    status = _cairo_svg_surface_emit_pattern_stops (&document->xml_node_defs,
						    &pattern->base,
						    start_offset,
						    reverse_stops,
						    emulate_reflect);
    if (unlikely (status)) {
	return status;
    }

    if (pattern->base.base.extend == CAIRO_EXTEND_NONE) {
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  "<stop offset=\"1\" "
				  "stop-color=\"rgb(0%%, 0%%, 0%%)\" "
				  "stop-opacity=\"0\"/>\n");
    }

    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "</radialGradient>\n");

    _cairo_svg_stream_printf (output,
			      is_stroke ? " stroke=\"url(#radial-pattern-%d)\""
					: " fill=\"url(#radial-pattern-%d)\"",
			      radial_pattern_id);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_svg_surface_emit_pattern (cairo_svg_surface_t *surface,
				 const cairo_pattern_t *pattern,
				 cairo_svg_stream_t *output,
				 cairo_bool_t is_stroke,
				 const cairo_matrix_t *parent_matrix)
{
    switch (pattern->type) {
    case CAIRO_PATTERN_TYPE_SOLID:
	return _cairo_svg_surface_emit_solid_pattern (surface, (cairo_solid_pattern_t *) pattern,
						      output, is_stroke);

    case CAIRO_PATTERN_TYPE_SURFACE:
	return _cairo_svg_surface_emit_surface_pattern (surface, (cairo_surface_pattern_t *) pattern,
							output, is_stroke, parent_matrix);

    case CAIRO_PATTERN_TYPE_LINEAR:
	return _cairo_svg_surface_emit_linear_pattern (surface, (cairo_linear_pattern_t *) pattern,
						       output, is_stroke, parent_matrix);

    case CAIRO_PATTERN_TYPE_RADIAL:
	return _cairo_svg_surface_emit_radial_pattern (surface, (cairo_radial_pattern_t *) pattern,
						       output, is_stroke, parent_matrix);

    case CAIRO_PATTERN_TYPE_MESH:
    case CAIRO_PATTERN_TYPE_RASTER_SOURCE:
	ASSERT_NOT_REACHED;
    }
    return _cairo_error (CAIRO_STATUS_PATTERN_TYPE_MISMATCH);
}

static cairo_status_t
_cairo_svg_surface_emit_fill_style (cairo_svg_stream_t *output,
				    cairo_svg_surface_t *surface,
				    const cairo_pattern_t *source,
				    cairo_fill_rule_t fill_rule,
				    const cairo_matrix_t *parent_matrix)
{
    _cairo_svg_stream_printf (output,
			      " fill-rule=\"%s\"",
			      fill_rule == CAIRO_FILL_RULE_EVEN_ODD ? "evenodd" : "nonzero");
    return _cairo_svg_surface_emit_pattern (surface, source, output, FALSE, parent_matrix);
}

static cairo_status_t
_cairo_svg_surface_emit_stroke_style (cairo_svg_stream_t *output,
				      cairo_svg_surface_t *surface,
				      const cairo_pattern_t *source,
				      const cairo_stroke_style_t *stroke_style,
				      const cairo_matrix_t *parent_matrix)
{
    cairo_status_t status;
    const char *line_cap, *line_join;
    unsigned int i;

    switch (stroke_style->line_cap) {
    case CAIRO_LINE_CAP_BUTT:
	line_cap = "butt";
	break;
    case CAIRO_LINE_CAP_ROUND:
	line_cap = "round";
	break;
    case CAIRO_LINE_CAP_SQUARE:
	line_cap = "square";
	break;
    default:
	ASSERT_NOT_REACHED;
    }

    switch (stroke_style->line_join) {
    case CAIRO_LINE_JOIN_MITER:
	line_join = "miter";
	break;
    case CAIRO_LINE_JOIN_ROUND:
	line_join = "round";
	break;
    case CAIRO_LINE_JOIN_BEVEL:
	line_join = "bevel";
	break;
    default:
	ASSERT_NOT_REACHED;
    }

    if (stroke_style->is_hairline) {
		_cairo_svg_stream_printf (output,
					" stroke-width=\"1px\""
					" stroke-linecap=\"%s\""
					" stroke-linejoin=\"%s\""
					" style=\"vector-effect: non-scaling-stroke\"",
					line_cap,
					line_join);
	} else {
		_cairo_svg_stream_printf (output,
					" stroke-width=\"%f\""
					" stroke-linecap=\"%s\""
					" stroke-linejoin=\"%s\"",
					stroke_style->line_width,
					line_cap,
					line_join);
	}

    status = _cairo_svg_surface_emit_pattern (surface, source, output, TRUE, parent_matrix);
    if (unlikely (status)) {
	return status;
    }

    if (stroke_style->num_dashes > 0) {
	_cairo_svg_stream_printf (output, " stroke-dasharray=\"");
	for (i = 0; i < stroke_style->num_dashes; i++) {
	    _cairo_svg_stream_printf (output,
				      "%f",
				      stroke_style->dash[i]);
	    _cairo_svg_stream_printf (output, i + 1 < stroke_style->num_dashes ? " " : "\"");
	}
	if (stroke_style->dash_offset != 0.0) {
	    _cairo_svg_stream_printf (output,
				      " stroke-dashoffset=\"%f\"",
				      stroke_style->dash_offset);
	}
    }

    _cairo_svg_stream_printf (output,
			      " stroke-miterlimit=\"%f\"",
			      stroke_style->miter_limit);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
_cairo_svg_surface_get_extents (void		        *abstract_surface,
				cairo_rectangle_int_t   *rectangle)
{
    cairo_svg_surface_t *surface = abstract_surface;

    rectangle->x = 0;
    rectangle->y = 0;

    /* XXX: The conversion to integers here is pretty bogus, (not to
     * mention the arbitrary limitation of width to a short(!). We
     * may need to come up with a better interface for get_size.
     */
    rectangle->width  = ceil (surface->width);
    rectangle->height = ceil (surface->height);

    return surface->surface_bounded;
}

static cairo_status_t
_cairo_svg_surface_emit_paint (cairo_svg_stream_t *output,
			       cairo_svg_surface_t *surface,
			       const cairo_pattern_t *source,
			       cairo_bool_t at_origin)
{
    cairo_status_t status;

    if (_cairo_svg_surface_svg_clip_or_svg_mask_should_be_used (source)) {
	return _cairo_svg_surface_emit_composite_pattern (output,
							  surface,
							  (cairo_surface_pattern_t *) source,
							  invalid_pattern_id,
							  NULL);
    }

    surface->transitive_paint_used = TRUE;

    _cairo_svg_stream_printf (output, "<rect");
    if (at_origin) {
	_cairo_svg_stream_append_paint_dependent (output,
						  surface->source_id,
						  CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE_AT_ORIGIN);
    } else {
	_cairo_svg_stream_append_paint_dependent (output,
						  surface->source_id,
						  CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_RECTANGLE);
    }
    status = _cairo_svg_surface_emit_pattern (surface, source, output, FALSE, NULL);
    if (unlikely (status)) {
	return status;
    }
    _cairo_svg_stream_printf (output, "/>\n");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_svg_surface_do_operator (cairo_svg_stream_t *output,
				cairo_svg_surface_t *surface,
				cairo_operator_t op,
				const cairo_clip_t *clip,
				cairo_svg_stream_t *mask_stream,
				cairo_svg_stream_t *source_stream,
				cairo_svg_stream_t *destination_stream)
{
    cairo_status_t status;
    cairo_svg_document_t *document = surface->document;

    // For operators that do not always produce opaque output, we first need to emit a black paint
    // if the content does not have alpha
    if (surface->base.content == CAIRO_CONTENT_COLOR && (op == CAIRO_OPERATOR_CLEAR ||
							 op == CAIRO_OPERATOR_SOURCE ||
							 op == CAIRO_OPERATOR_IN ||
							 op == CAIRO_OPERATOR_OUT ||
							 op == CAIRO_OPERATOR_DEST_IN ||
							 op == CAIRO_OPERATOR_DEST_OUT ||
							 op == CAIRO_OPERATOR_DEST_ATOP ||
							 op == CAIRO_OPERATOR_XOR)) {
	_cairo_svg_surface_emit_paint (output, surface, &_cairo_pattern_black.base, FALSE);
    }

    if (op == CAIRO_OPERATOR_CLEAR) {
	/*
	 * The result is the same as one of the SOURCE operation application with the same arguments,
	 * but with an empty source.
	 */

	status = _cairo_svg_stream_destroy (source_stream);
	if (unlikely (status)) {
	    (void) _cairo_svg_stream_destroy (destination_stream);
	    (void) _cairo_svg_stream_destroy (mask_stream);
	    return status;
	}
	cairo_svg_stream_t empty_stream = _cairo_svg_stream_create ();
	return _cairo_svg_surface_do_operator (output,
					       surface,
					       CAIRO_OPERATOR_SOURCE,
					       clip,
					       mask_stream,
					       &empty_stream,
					       destination_stream);
    }

    if (op == CAIRO_OPERATOR_SOURCE) {
	/*
	 * Below we use the "Bounded" equation with SOURCE as the operation from the "Clipping and masking" section
	 * of https://cairographics.org/operators/:
	 * result = source LEPR_(clip IN mask) destination
	 *
	 * It is equivalent to:
	 * result = (source IN (clip IN mask)) ADD (destination IN (NOT (clip IN mask)))
	 *
	 * 1. We put the clip masked with the mask into the SVG group `lerp_compositing_group`.
	 * 2. `positive_lerp_mask` is an SVG mask with `lerp_compositing_group`.
	 * 3. `negative_lerp_mask` is an SVG mask with inverted `lerp_compositing_group`.
	 * 5. We put the source masked with `positive_lerp_mask` into the SVG group `lerped_source_compositing_group`.
	 * 6. We put the destination masked with `negative_lerp_mask` into
	 *    the SVG group `lerped_destination_compositing_group`.
	 * 7. The result is addition of `lerped_source_compositing_group` and `lerped_destination_compositing_group`.
	 */

	unsigned int lerp_compositing_group_id = document->compositing_group_id++;
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  "<g id=\"compositing-group-%d\"",
				  lerp_compositing_group_id);
	_cairo_svg_stream_append_paint_dependent (&document->xml_node_defs,
						  surface->source_id,
						  CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_INVERSE_TRANSLATION);
	_cairo_svg_stream_printf (&document->xml_node_defs, ">\n");
	_cairo_svg_surface_emit_paint (&document->xml_node_defs, surface, &_cairo_pattern_clear.base, FALSE);
	status = _cairo_svg_surface_set_clip (surface, &document->xml_node_defs, clip);
	if (unlikely (status)) {
	    (void) _cairo_svg_stream_destroy (destination_stream);
	    (void) _cairo_svg_stream_destroy (source_stream);
	    (void) _cairo_svg_stream_destroy (mask_stream);
	    return status;
	}
	_cairo_svg_stream_copy (mask_stream, &document->xml_node_defs);
	status = _cairo_svg_stream_destroy (mask_stream);
	if (unlikely (status)) {
	    (void) _cairo_svg_stream_destroy (destination_stream);
	    (void) _cairo_svg_stream_destroy (source_stream);
	    return status;
	}
	_cairo_svg_surface_reset_clip (surface);
	_cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");

	unsigned int positive_lerp_mask_id = document->mask_id++;
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  "<mask id=\"mask-%d\">\n",
				  positive_lerp_mask_id);
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  "<use xlink:href=\"#compositing-group-%d\"/>\n",
				  lerp_compositing_group_id);
	_cairo_svg_stream_printf (&document->xml_node_defs, "</mask>\n");

	unsigned int negative_lerp_mask_id = document->mask_id++;
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  "<mask id=\"mask-%d\">\n",
				  negative_lerp_mask_id);
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  "<use xlink:href=\"#compositing-group-%d\" filter=\"url(#filter-%s)\"/>\n",
				  lerp_compositing_group_id,
				  _cairo_svg_surface_emit_static_filter (document,
									 CAIRO_SVG_FILTER_REMOVE_COLOR_AND_INVERT_ALPHA));
	_cairo_svg_stream_printf (&document->xml_node_defs, "</mask>\n");

	unsigned int lerped_source_compositing_group_id = document->compositing_group_id++;
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  "<g id=\"compositing-group-%d\" mask=\"url(#mask-%d)\">\n",
				  lerped_source_compositing_group_id,
				  positive_lerp_mask_id);
	_cairo_svg_stream_printf (&document->xml_node_defs, "<g");
	_cairo_svg_stream_append_paint_dependent (&document->xml_node_defs,
						  surface->source_id,
						  CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_INVERSE_TRANSLATION);
	_cairo_svg_stream_printf (&document->xml_node_defs, ">\n");
	_cairo_svg_stream_copy (source_stream, &document->xml_node_defs);
	status = _cairo_svg_stream_destroy (source_stream);
	if (unlikely (status)) {
	    (void) _cairo_svg_stream_destroy (destination_stream);
	    return status;
	}
	_cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");
	_cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");

	unsigned int lerped_destination_compositing_group_id = document->compositing_group_id++;
	_cairo_svg_stream_printf (&document->xml_node_defs,
				  "<g id=\"compositing-group-%d\" mask=\"url(#mask-%d)\">\n",
				  lerped_destination_compositing_group_id,
				  negative_lerp_mask_id);
	_cairo_svg_stream_printf (&document->xml_node_defs, "<g");
	_cairo_svg_stream_append_paint_dependent (&document->xml_node_defs,
						  surface->source_id,
						  CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_INVERSE_TRANSLATION);
	_cairo_svg_stream_printf (&document->xml_node_defs, ">\n");
	_cairo_svg_stream_copy (destination_stream, &document->xml_node_defs);
	status = _cairo_svg_stream_destroy (destination_stream);
	if (unlikely (status)) {
	    return status;
	}
	_cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");
	_cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");

	_cairo_svg_stream_printf (&surface->xml_node,
				  "<g filter=\"url(#filter-%d)\"",
				  _cairo_svg_surface_emit_parametric_filter (surface,
									     CAIRO_SVG_FILTER_ADD,
									     lerped_source_compositing_group_id,
									     lerped_destination_compositing_group_id));
	_cairo_svg_stream_append_paint_dependent (&surface->xml_node,
						  surface->source_id,
						  CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_TRANSLATION);
	_cairo_svg_stream_printf (&surface->xml_node, ">\n");
	status = _cairo_svg_surface_emit_paint (&surface->xml_node, surface, &_cairo_pattern_black.base, TRUE);
	if (unlikely (status)) {
	    return status;
	}
	_cairo_svg_stream_printf (&surface->xml_node, "</g>\n");

	return CAIRO_STATUS_SUCCESS;
    }

    if (op == CAIRO_OPERATOR_DEST) {
	/*
	 * The result is the destination.
	 */

	_cairo_svg_stream_copy (destination_stream, &surface->xml_node);
	status = _cairo_svg_stream_destroy (destination_stream);
	if (unlikely (status)) {
	    (void) _cairo_svg_stream_destroy (source_stream);
	    (void) _cairo_svg_stream_destroy (mask_stream);
	    return status;
	}
	status = _cairo_svg_stream_destroy (source_stream);
	if (unlikely (status)) {
	    (void) _cairo_svg_stream_destroy (source_stream);
	    return status;
	}
	status = _cairo_svg_stream_destroy (source_stream);
	if (unlikely (status)) {
	    return status;
	}
	return CAIRO_STATUS_SUCCESS;
    }

    /*
     * Below we use the "XRender" equation from the "Clipping and masking" section
     * of https://cairographics.org/operators/:
     * result = ((source IN mask) OP destination) LERP_clip destination
     *
     * It is equivalent to:
     * result = (((source IN mask) OP destination) IN clip) ADD (destination IN (NOT clip))
     *
     * 1. We put the clip into the SVG group `lerp_compositing_group`.
     * 2. `positive_lerp_mask` is an SVG mask with `lerp_compositing_group`.
     * 3. `negative_lerp_mask` is an SVG mask with inverted `lerp_compositing_group`.
     * 4. We put the mask into the SVG mask `mask_mask`.
     * 5. We put the source masked with `mask_mask` into the SVG group `masked_source_compositing_group`.
     * 6. We put the destination into the SVG group `destination_compositing_group`.
     * 7. `lerped_operation_compositing_group` is an SVG group of operation applied to
     *    (`masked_source_compositing_group`, `destination_compositing_group`)
     *    masked with `positive_lerp_mask`.
     * 8. `lerped_destination_compositing_group` is an SVG group of `destination_compositing_group`
     *    masked with `negative_lerp_mask`.
     * 9. The result is addition of `lerped_operation_compositing_group` and `lerped_destination_compositing_group`.
     */

    unsigned int lerp_compositing_group_id = document->compositing_group_id++;
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<g id=\"compositing-group-%d\"",
			      lerp_compositing_group_id);
    _cairo_svg_stream_append_paint_dependent (&document->xml_node_defs,
					      surface->source_id,
					      CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_INVERSE_TRANSLATION);
    _cairo_svg_stream_printf (&document->xml_node_defs, ">\n");
    _cairo_svg_surface_emit_paint (&document->xml_node_defs, surface, &_cairo_pattern_clear.base, FALSE);
    status = _cairo_svg_surface_set_clip (surface, &document->xml_node_defs, clip);
    if (unlikely (status)) {
	(void) _cairo_svg_stream_destroy (destination_stream);
	(void) _cairo_svg_stream_destroy (source_stream);
	(void) _cairo_svg_stream_destroy (mask_stream);
	return status;
    }
    status = _cairo_svg_surface_emit_paint (&document->xml_node_defs, surface, &_cairo_pattern_white.base, FALSE);
    if (unlikely (status)) {
	(void) _cairo_svg_stream_destroy (destination_stream);
	(void) _cairo_svg_stream_destroy (source_stream);
	(void) _cairo_svg_stream_destroy (mask_stream);
	return status;
    }
    _cairo_svg_surface_reset_clip (surface);
    _cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");

    unsigned int positive_lerp_mask_id = document->mask_id++;
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<mask id=\"mask-%d\">\n",
			      positive_lerp_mask_id);
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<use xlink:href=\"#compositing-group-%d\"/>\n",
			      lerp_compositing_group_id);
    _cairo_svg_stream_printf (&document->xml_node_defs, "</mask>\n");

    unsigned int negative_lerp_mask_id = document->mask_id++;
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<mask id=\"mask-%d\">\n",
			      negative_lerp_mask_id);
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<use xlink:href=\"#compositing-group-%d\" filter=\"url(#filter-%s)\"/>\n",
			      lerp_compositing_group_id,
			      _cairo_svg_surface_emit_static_filter (document,
								     CAIRO_SVG_FILTER_REMOVE_COLOR_AND_INVERT_ALPHA));
    _cairo_svg_stream_printf (&document->xml_node_defs, "</mask>\n");

    unsigned int mask_mask_id = document->mask_id++;
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<mask id=\"mask-%d\">\n",
			      mask_mask_id);
    _cairo_svg_stream_printf (&document->xml_node_defs, "<g");
    _cairo_svg_stream_append_paint_dependent (&document->xml_node_defs,
					      surface->source_id,
					      CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_INVERSE_TRANSLATION);
    _cairo_svg_stream_printf (&document->xml_node_defs, ">\n");
    _cairo_svg_stream_copy (mask_stream, &document->xml_node_defs);
    status = _cairo_svg_stream_destroy (mask_stream);
    if (unlikely (status)) {
	(void) _cairo_svg_stream_destroy (source_stream);
	(void) _cairo_svg_stream_destroy (destination_stream);
	return status;
    }
    _cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");
    _cairo_svg_stream_printf (&document->xml_node_defs, "</mask>\n");

    unsigned int masked_source_compositing_group_id = document->compositing_group_id++;
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<g id=\"compositing-group-%d\" mask=\"url(#mask-%d)\">\n",
			      masked_source_compositing_group_id,
			      mask_mask_id);
    _cairo_svg_stream_printf (&document->xml_node_defs, "<g");
    _cairo_svg_stream_append_paint_dependent (&document->xml_node_defs,
					      surface->source_id,
					      CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_INVERSE_TRANSLATION);
    _cairo_svg_stream_printf (&document->xml_node_defs, ">\n");
    _cairo_svg_stream_copy (source_stream, &document->xml_node_defs);
    status = _cairo_svg_stream_destroy (source_stream);
    if (unlikely (status)) {
	(void) _cairo_svg_stream_destroy (destination_stream);
	return status;
    }
    _cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");
    _cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");

    unsigned int destination_compositing_group_id = document->compositing_group_id++;
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<g id=\"compositing-group-%d\"",
			      destination_compositing_group_id);
    _cairo_svg_stream_append_paint_dependent (&document->xml_node_defs,
					      surface->source_id,
					      CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_INVERSE_TRANSLATION);
    _cairo_svg_stream_printf (&document->xml_node_defs, ">\n");
    _cairo_svg_stream_copy (destination_stream, &document->xml_node_defs);
    status = _cairo_svg_stream_destroy (destination_stream);
    if (unlikely (status)) {
	return status;
    }
    _cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");

    unsigned int lerped_operation_compositing_group_id = document->compositing_group_id++;
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<g id=\"compositing-group-%d\"",
			      lerped_operation_compositing_group_id);
    unsigned int filter_id;
    switch (op) {
    case CAIRO_OPERATOR_CLEAR:
    case CAIRO_OPERATOR_SOURCE:
    case CAIRO_OPERATOR_OVER:
	ASSERT_NOT_REACHED;
    case CAIRO_OPERATOR_IN:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_IN,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_OUT:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_OUT,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_ATOP:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_ATOP,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_DEST:
	ASSERT_NOT_REACHED;
    case CAIRO_OPERATOR_DEST_OVER:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_OVER,
							       destination_compositing_group_id,
							       masked_source_compositing_group_id);
	break;
    case CAIRO_OPERATOR_DEST_IN:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_IN,
							       destination_compositing_group_id,
							       masked_source_compositing_group_id);
	break;
    case CAIRO_OPERATOR_DEST_OUT:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_OUT,
							       destination_compositing_group_id,
							       masked_source_compositing_group_id);
	break;
    case CAIRO_OPERATOR_DEST_ATOP:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_ATOP,
							       destination_compositing_group_id,
							       masked_source_compositing_group_id);
	break;
    case CAIRO_OPERATOR_XOR:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_XOR,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_ADD:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_ADD,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_SATURATE:
	ASSERT_NOT_REACHED;
    case CAIRO_OPERATOR_MULTIPLY:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_MULTIPLY,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_SCREEN:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_SCREEN,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_OVERLAY:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_OVERLAY,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_DARKEN:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_DARKEN,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_LIGHTEN:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_LIGHTEN,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_COLOR_DODGE:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_COLOR_DODGE,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_COLOR_BURN:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_COLOR_BURN,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_HARD_LIGHT:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_HARD_LIGHT,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_SOFT_LIGHT:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_SOFT_LIGHT,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_DIFFERENCE:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_DIFFERENCE,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_EXCLUSION:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_EXCLUSION,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_HSL_HUE:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_HUE,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_HSL_SATURATION:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_SATURATION,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_HSL_COLOR:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_COLOR,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    case CAIRO_OPERATOR_HSL_LUMINOSITY:
	filter_id = _cairo_svg_surface_emit_parametric_filter (surface,
							       CAIRO_SVG_FILTER_LUMINOSITY,
							       masked_source_compositing_group_id,
							       destination_compositing_group_id);
	break;
    default:
	ASSERT_NOT_REACHED;
    }
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      " filter=\"url(#filter-%d)\" mask=\"url(#mask-%d)\">\n",
			      filter_id,
			      positive_lerp_mask_id);
    status = _cairo_svg_surface_emit_paint (&document->xml_node_defs, surface, &_cairo_pattern_black.base, TRUE);
    if (unlikely (status)) {
	return status;
    }
    _cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");

    unsigned int lerped_destination_compositing_group_id = document->compositing_group_id++;
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<g id=\"compositing-group-%d\" mask=\"url(#mask-%d)\">\n",
			      lerped_destination_compositing_group_id,
			      negative_lerp_mask_id);
    _cairo_svg_stream_printf (&document->xml_node_defs,
			      "<use xlink:href=\"#compositing-group-%d\"/>\n",
			      destination_compositing_group_id);
    _cairo_svg_stream_printf (&document->xml_node_defs, "</g>\n");

    _cairo_svg_stream_printf (&surface->xml_node,
			      "<g filter=\"url(#filter-%d)\"",
			      _cairo_svg_surface_emit_parametric_filter (surface,
									 CAIRO_SVG_FILTER_ADD,
									 lerped_operation_compositing_group_id,
									 lerped_destination_compositing_group_id));
    _cairo_svg_stream_append_paint_dependent (&surface->xml_node,
					      surface->source_id,
					      CAIRO_SVG_STREAM_PAINT_DEPENDENT_ELEMENT_TYPE_TRANSLATION);
    _cairo_svg_stream_printf (&surface->xml_node, ">\n");
    status = _cairo_svg_surface_emit_paint (&surface->xml_node, surface, &_cairo_pattern_black.base, TRUE);
    if (unlikely (status)) {
	return status;
    }
    _cairo_svg_stream_printf (&surface->xml_node, "</g>\n");

    return CAIRO_STATUS_SUCCESS;
}

#define _CAIRO_SVG_SURFACE_CALL_OPERATOR_IMPL(OPERATOR_IMPL, SOURCE, ...) \
    if (op == CAIRO_OPERATOR_OVER) { \
        status = _cairo_svg_surface_set_clip (surface, &surface->xml_node, clip); \
        if (unlikely (status)) { \
            return status; \
        } \
        return OPERATOR_IMPL (&surface->xml_node, surface, SOURCE, ## __VA_ARGS__); \
    } else { \
        _cairo_svg_surface_reset_clip (surface); \
        cairo_svg_stream_t mask_stream = _cairo_svg_stream_create (); \
        status = OPERATOR_IMPL (&mask_stream, surface, &_cairo_pattern_white.base, ## __VA_ARGS__); \
        if (unlikely (status)) { \
            (void) _cairo_svg_stream_destroy (&mask_stream); \
            return status; \
        } \
        cairo_svg_stream_t source_stream = _cairo_svg_stream_create (); \
        status = _cairo_svg_surface_emit_paint (&source_stream, \
                                                surface, \
                                                SOURCE,                   \
                                                FALSE); \
        if (unlikely (status)) { \
            (void) _cairo_svg_stream_destroy (&source_stream); \
            (void) _cairo_svg_stream_destroy (&mask_stream); \
            return status; \
        } \
        cairo_svg_stream_t destination_stream = surface->xml_node; \
        surface->xml_node = _cairo_svg_stream_create (); \
        return _cairo_svg_surface_do_operator (&surface->xml_node, \
                                               surface, \
                                               op, \
                                               clip, \
                                               &mask_stream, \
                                               &source_stream, \
                                               &destination_stream); \
    }

static cairo_int_status_t
_cairo_svg_surface_paint_impl (cairo_svg_stream_t *output,
			       cairo_svg_surface_t *surface,
			       const cairo_pattern_t *source)
{
    return _cairo_svg_surface_emit_paint (output, surface, source, FALSE);
}

static cairo_int_status_t
_cairo_svg_surface_paint (void *abstract_surface,
			  cairo_operator_t op,
			  const cairo_pattern_t *source,
			  const cairo_clip_t *clip)
{
    cairo_status_t status;
    cairo_svg_surface_t *surface = abstract_surface;

    /* Emulation of clear and source operators, when no clipping region
     * is defined. We just delete existing content of surface root node,
     * and exit early if operator is clear.
     */
    if ((op == CAIRO_OPERATOR_CLEAR || op == CAIRO_OPERATOR_SOURCE) && clip == NULL) {
	switch (surface->paginated_mode) {
	case CAIRO_PAGINATED_MODE_ANALYZE:
	    return CAIRO_STATUS_SUCCESS;
	case CAIRO_PAGINATED_MODE_RENDER:
	    status = _cairo_svg_stream_destroy (&surface->xml_node);
	    if (unlikely (status)) {
		return status;
	    }

	    surface->xml_node = _cairo_svg_stream_create ();

	    if (op == CAIRO_OPERATOR_CLEAR) {
		return CAIRO_STATUS_SUCCESS;
	    }
	    break;
	case CAIRO_PAGINATED_MODE_FALLBACK:
	    ASSERT_NOT_REACHED;
	}
    } else if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE) {
	return _cairo_svg_surface_are_operation_and_pattern_supported (surface, op, source)
	       ? CAIRO_STATUS_SUCCESS
	       : CAIRO_INT_STATUS_UNSUPPORTED;
    }

    _CAIRO_SVG_SURFACE_CALL_OPERATOR_IMPL (_cairo_svg_surface_paint_impl,
					   source)
}

static cairo_int_status_t
_cairo_svg_surface_mask_impl (cairo_svg_stream_t *output,
			      cairo_svg_surface_t *surface,
			      const cairo_pattern_t *source,
			      const cairo_pattern_t *mask)
{
    cairo_status_t status;
    cairo_svg_document_t *document = surface->document;

    /* _cairo_svg_surface_emit_paint() will output a pattern definition to
     * document->xml_node_defs so we need to write the mask element to
     * a temporary stream and then copy that to xml_node_defs. */
    cairo_svg_stream_t temporary_stream = _cairo_svg_stream_create ();

    unsigned int mask_id = document->mask_id++;

    _cairo_svg_stream_printf (&temporary_stream,
				 "<mask id=\"mask-%d\">\n",
				 mask_id);
    _cairo_svg_stream_printf (&temporary_stream,
				 "<g filter=\"url(#filter-%s)\">\n",
				 _cairo_svg_surface_emit_static_filter (document, CAIRO_SVG_FILTER_REMOVE_COLOR));
    status = _cairo_svg_surface_emit_paint (&temporary_stream, surface, mask, FALSE);
    if (unlikely (status)) {
	(void) _cairo_svg_stream_destroy (&temporary_stream);
	return status;
    }
    _cairo_svg_stream_printf (&temporary_stream, "</g>\n");
    _cairo_svg_stream_printf (&temporary_stream, "</mask>\n");

    _cairo_svg_stream_copy (&temporary_stream, &document->xml_node_defs);

    status = _cairo_svg_stream_destroy (&temporary_stream);
    if (unlikely (status)) {
	return status;
    }

    _cairo_svg_stream_printf (output,
				 "<g mask=\"url(#mask-%d)\">\n",
				 mask_id);

    status = _cairo_svg_surface_emit_paint (output, surface, source, FALSE);
    if (unlikely (status)) {
	return status;
    }

    _cairo_svg_stream_printf (output, "</g>\n");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_svg_surface_mask (void *abstract_surface,
			 cairo_operator_t op,
			 const cairo_pattern_t *source,
			 const cairo_pattern_t *mask,
			 const cairo_clip_t *clip)
{
    cairo_status_t status;
    cairo_svg_surface_t *surface = abstract_surface;

    if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE) {
	return _cairo_svg_surface_are_operation_and_pattern_supported (surface, op, source) &&
	       _cairo_svg_surface_are_operation_and_pattern_supported (surface, op, mask)
	       ? CAIRO_STATUS_SUCCESS
	       : CAIRO_INT_STATUS_UNSUPPORTED;
    }

    _CAIRO_SVG_SURFACE_CALL_OPERATOR_IMPL (_cairo_svg_surface_mask_impl,
					   source,
					   mask)
}

static cairo_int_status_t
_cairo_svg_surface_stroke_impl (cairo_svg_stream_t *output,
				cairo_svg_surface_t *surface,
				const cairo_pattern_t *source,
				const cairo_path_fixed_t *path,
				const cairo_stroke_style_t *stroke_style,
				const cairo_matrix_t *ctm,
				const cairo_matrix_t *ctm_inverse,
				double tolerance,
				cairo_antialias_t antialias)
{
    cairo_status_t status;

    cairo_bool_t svg_clip_or_svg_mask_should_be_used = _cairo_svg_surface_svg_clip_or_svg_mask_should_be_used (source);
    unsigned int mask_id;
    cairo_svg_stream_t *output_stream = output;
    if (svg_clip_or_svg_mask_should_be_used) {
	mask_id = surface->document->mask_id++;

	output_stream = &surface->document->xml_node_defs;

	_cairo_svg_stream_printf (output_stream,
				  "<mask id=\"mask-%d\">\n",
				  mask_id);
    }

    _cairo_svg_stream_printf (output_stream, "<path fill=\"none\"");
    status = _cairo_svg_surface_emit_stroke_style (output_stream,
						   surface,
						   svg_clip_or_svg_mask_should_be_used ? &_cairo_pattern_white.base
										       : source,
						   stroke_style,
						   ctm_inverse);

    if (unlikely (status)) {
	return status;
    }

    _cairo_svg_surface_emit_path (output_stream, path, ctm_inverse);

    _cairo_svg_surface_emit_transform (output_stream, "transform", ctm, NULL);
    _cairo_svg_stream_printf (output_stream, "/>\n");

    if (svg_clip_or_svg_mask_should_be_used) {
	_cairo_svg_stream_printf (output_stream, "</mask>\n");

	_cairo_svg_stream_printf (output,
				  "<g mask=\"url(#mask-%d)\">\n",
				  mask_id);

	status = _cairo_svg_surface_emit_composite_pattern (output,
							    surface,
							    (cairo_surface_pattern_t *) source,
							    invalid_pattern_id,
							    NULL);
	if (unlikely (status)) {
	    return status;
	}

	_cairo_svg_stream_printf (output, "</g>\n");
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_svg_surface_stroke (void *abstract_dst,
			   cairo_operator_t op,
			   const cairo_pattern_t *source,
			   const cairo_path_fixed_t *path,
			   const cairo_stroke_style_t *stroke_style,
			   const cairo_matrix_t *ctm,
			   const cairo_matrix_t *ctm_inverse,
			   double tolerance,
			   cairo_antialias_t antialias,
			   const cairo_clip_t *clip)
{
    cairo_svg_surface_t *surface = abstract_dst;
    cairo_status_t status;

    if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE) {
	return _cairo_svg_surface_are_operation_and_pattern_supported (surface, op, source)
	       ? CAIRO_STATUS_SUCCESS
	       : CAIRO_INT_STATUS_UNSUPPORTED;
    }

    _CAIRO_SVG_SURFACE_CALL_OPERATOR_IMPL (_cairo_svg_surface_stroke_impl,
					   source,
					   path,
					   stroke_style,
					   ctm,
					   ctm_inverse,
					   tolerance,
					   antialias)
}

static cairo_int_status_t
_cairo_svg_surface_fill_impl (cairo_svg_stream_t *output,
			      cairo_svg_surface_t *surface,
			      const cairo_pattern_t *source,
			      const cairo_path_fixed_t *path,
			      cairo_fill_rule_t fill_rule,
			      double tolerance,
			      cairo_antialias_t antialias)
{
    cairo_status_t status;

    if (_cairo_svg_surface_svg_clip_or_svg_mask_should_be_used (source)) {
	_cairo_svg_stream_printf (&surface->document->xml_node_defs,
				  "<clipPath id=\"clip-%d\">\n",
				  surface->document->clip_id);

	_cairo_svg_stream_printf (&surface->document->xml_node_defs,
				  "<path clip-rule=\"%s\"",
				  fill_rule == CAIRO_FILL_RULE_EVEN_ODD ? "evenodd" : "nonzero");
	_cairo_svg_surface_emit_path (&surface->document->xml_node_defs, path, NULL);
	_cairo_svg_stream_printf (&surface->document->xml_node_defs, "/>\n");

	_cairo_svg_stream_printf (&surface->document->xml_node_defs, "</clipPath>\n");

	_cairo_svg_stream_printf (output,
				  "<g clip-path=\"url(#clip-%d)\">\n",
				  surface->document->clip_id++);

	status = _cairo_svg_surface_emit_composite_pattern (output,
							    surface,
							    (cairo_surface_pattern_t *) source,
							    invalid_pattern_id,
							    NULL);
	if (unlikely (status)) {
	    return status;
	}

	_cairo_svg_stream_printf (output, "</g>");
    } else {
	_cairo_svg_stream_printf (output, "<path");
	status = _cairo_svg_surface_emit_fill_style (output, surface, source, fill_rule, NULL);
	if (unlikely (status)) {
	    return status;
	}
	_cairo_svg_surface_emit_path (output, path, NULL);
	_cairo_svg_stream_printf (output, "/>\n");
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_svg_surface_fill (void *abstract_surface,
			 cairo_operator_t op,
			 const cairo_pattern_t *source,
			 const cairo_path_fixed_t *path,
			 cairo_fill_rule_t fill_rule,
			 double tolerance,
			 cairo_antialias_t antialias,
			 const cairo_clip_t *clip)
{
    cairo_svg_surface_t *surface = abstract_surface;
    cairo_status_t status;

    if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE) {
	return _cairo_svg_surface_are_operation_and_pattern_supported (surface, op, source)
	       ? CAIRO_STATUS_SUCCESS
	       : CAIRO_INT_STATUS_UNSUPPORTED;
    }

    _CAIRO_SVG_SURFACE_CALL_OPERATOR_IMPL (_cairo_svg_surface_fill_impl,
					   source,
					   path,
					   fill_rule,
					   tolerance,
					   antialias)
}

static cairo_int_status_t
_cairo_svg_surface_fill_stroke (void *abstract_surface,
				cairo_operator_t fill_op,
				const cairo_pattern_t *fill_source,
				cairo_fill_rule_t fill_rule,
				double fill_tolerance,
				cairo_antialias_t fill_antialias,
				const cairo_path_fixed_t *path,
				cairo_operator_t stroke_op,
				const cairo_pattern_t *stroke_source,
				const cairo_stroke_style_t *stroke_style,
				const cairo_matrix_t *stroke_ctm,
				const cairo_matrix_t *stroke_ctm_inverse,
				double stroke_tolerance,
				cairo_antialias_t stroke_antialias,
				const cairo_clip_t *clip)
{
    cairo_svg_surface_t *surface = abstract_surface;
    cairo_status_t status;

    if (_cairo_svg_surface_svg_clip_or_svg_mask_should_be_used (fill_source) ||
	_cairo_svg_surface_svg_clip_or_svg_mask_should_be_used (stroke_source) ||
	fill_op != CAIRO_OPERATOR_OVER ||
	stroke_op != CAIRO_OPERATOR_OVER) {
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE) {
	return _cairo_svg_surface_are_operation_and_pattern_supported (surface, fill_op, fill_source)
	       && _cairo_svg_surface_are_operation_and_pattern_supported (surface, stroke_op, stroke_source)
	       ? CAIRO_STATUS_SUCCESS
	       : CAIRO_INT_STATUS_UNSUPPORTED;
    }

    status = _cairo_svg_surface_set_clip (surface, &surface->xml_node, clip);
    if (unlikely (status)) {
	return status;
    }

    _cairo_svg_stream_printf (&surface->xml_node, "<path");
    status = _cairo_svg_surface_emit_fill_style (&surface->xml_node, surface,
						 fill_source, fill_rule, stroke_ctm_inverse);
    if (unlikely (status)) {
	return status;
    }

    status = _cairo_svg_surface_emit_stroke_style (&surface->xml_node, surface,
						   stroke_source, stroke_style, stroke_ctm_inverse);
    if (unlikely (status)) {
	return status;
    }

    _cairo_svg_surface_emit_path (&surface->xml_node, path, stroke_ctm_inverse);

    _cairo_svg_surface_emit_transform (&surface->xml_node, "transform", stroke_ctm, NULL);

    _cairo_svg_stream_printf (&surface->xml_node, "/>\n");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_svg_surface_show_glyphs_impl (cairo_svg_stream_t *output,
				     cairo_svg_surface_t *surface,
				     const cairo_pattern_t *source,
				     cairo_glyph_t *glyphs,
				     int num_glyphs,
				     cairo_scaled_font_t *scaled_font)
{
    cairo_status_t status;
    cairo_svg_document_t *document = surface->document;

    if (num_glyphs <= 0) {
	return CAIRO_STATUS_SUCCESS;
    }

    /* FIXME it's probably possible to apply a source of a gradient to
     * a group of symbols, but I don't know how yet. Gradients or patterns
     * are translated by x and y properties of use element. */
    if (source->type != CAIRO_PATTERN_TYPE_SOLID) {
	goto fallback;
    }

    _cairo_svg_stream_printf (output, "<g");

    status = _cairo_svg_surface_emit_pattern (surface, source, output, FALSE, NULL);
    if (unlikely (status)) {
	return status;
    }

    _cairo_svg_stream_printf (output, ">\n");

    for (int i = 0; i < num_glyphs; i++) {
	cairo_scaled_font_subsets_glyph_t subset_glyph;

	status = _cairo_scaled_font_subsets_map_glyph (document->font_subsets,
						       scaled_font,
						       glyphs[i].index,
						       NULL,
						       0,
						       &subset_glyph);
	if ((cairo_int_status_t) status == CAIRO_INT_STATUS_UNSUPPORTED) {
	    _cairo_svg_stream_printf (output, "</g>\n");

	    glyphs += i;
	    num_glyphs -= i;
	    goto fallback;
	}

	if (unlikely (status)) {
	    return status;
	}

	_cairo_svg_stream_printf (output,
				  "<use xlink:href=\"#glyph-%d-%d\" x=\"%f\" y=\"%f\"/>\n",
				  subset_glyph.font_id,
				  subset_glyph.subset_glyph_index,
				  glyphs[i].x, glyphs[i].y);
    }

    _cairo_svg_stream_printf (output, "</g>\n");

    return CAIRO_STATUS_SUCCESS;

    fallback:;
    cairo_path_fixed_t path;

    _cairo_path_fixed_init (&path);

    status = _cairo_scaled_font_glyph_path (scaled_font,
					    (cairo_glyph_t *) glyphs,
					    num_glyphs, &path);
    if (unlikely (status)) {
	_cairo_path_fixed_fini (&path);
	return status;
    }

    status = _cairo_svg_surface_fill_impl (output,
					   surface,
					   source,
					   &path,
					   CAIRO_FILL_RULE_WINDING,
					   0.0,
					   CAIRO_ANTIALIAS_DEFAULT);

    _cairo_path_fixed_fini (&path);

    return status;
}

static cairo_int_status_t
_cairo_svg_surface_show_glyphs (void *abstract_surface,
				cairo_operator_t op,
				const cairo_pattern_t *source,
				cairo_glyph_t *glyphs,
				int num_glyphs,
				cairo_scaled_font_t *scaled_font,
				const cairo_clip_t *clip)
{
    cairo_svg_surface_t *surface = abstract_surface;
    cairo_int_status_t status;

    if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE) {
	return _cairo_svg_surface_are_operation_and_pattern_supported (surface, op, source)
	       ? CAIRO_STATUS_SUCCESS
	       : CAIRO_INT_STATUS_UNSUPPORTED;
    }

    _CAIRO_SVG_SURFACE_CALL_OPERATOR_IMPL (_cairo_svg_surface_show_glyphs_impl,
					   source,
					   glyphs,
					   num_glyphs,
					   scaled_font)
}

static void
_cairo_svg_surface_get_font_options (void                  *abstract_surface,
				     cairo_font_options_t  *options)
{
    _cairo_font_options_init_default (options);

    cairo_font_options_set_hint_style (options, CAIRO_HINT_STYLE_NONE);
    cairo_font_options_set_hint_metrics (options, CAIRO_HINT_METRICS_OFF);
    cairo_font_options_set_antialias (options, CAIRO_ANTIALIAS_GRAY);
    _cairo_font_options_set_round_glyph_positions (options, CAIRO_ROUND_GLYPH_POS_OFF);
}


static const char **
_cairo_svg_surface_get_supported_mime_types (void	   *abstract_surface)
{
    return _cairo_svg_supported_mime_types;
}

static const cairo_surface_backend_t cairo_svg_surface_backend = {
	CAIRO_SURFACE_TYPE_SVG,
	_cairo_svg_surface_finish,

	_cairo_default_context_create,

	NULL, /* create_similar: handled by wrapper */
	NULL, /* create_similar_image */
	NULL, /* map to image */
	NULL, /* unmap image */

	_cairo_surface_default_source,
	NULL, /* acquire_source_image */
	NULL, /* release_source_image */
	NULL, /* snapshot */

	_cairo_svg_surface_copy_page,
	_cairo_svg_surface_show_page,

	_cairo_svg_surface_get_extents,
	_cairo_svg_surface_get_font_options,

	NULL, /* flush */
	NULL, /* mark dirty rectangle */

	_cairo_svg_surface_paint,
	_cairo_svg_surface_mask,
	_cairo_svg_surface_stroke,
	_cairo_svg_surface_fill,
	_cairo_svg_surface_fill_stroke,
	_cairo_svg_surface_show_glyphs,
	NULL, /* has_show_text_glyphs */
	NULL, /* show_text_glyphs */
	_cairo_svg_surface_get_supported_mime_types,
};

static cairo_status_t
_cairo_svg_document_create (cairo_output_stream_t *output_stream,
			    double width,
			    double height,
			    cairo_svg_version_t version,
			    cairo_svg_document_t **document_out)
{
    cairo_svg_document_t *document;

    if (output_stream->status) {
	return output_stream->status;
    }

    document = _cairo_malloc (sizeof (cairo_svg_document_t));
    if (unlikely (document == NULL)) {
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    document->output_stream = output_stream;
    document->refcount = 1;
    document->owner = NULL;
    document->finished = FALSE;

    document->width = width;
    document->height = height;
    document->unit = CAIRO_SVG_UNIT_USER;

    document->xml_node_defs = _cairo_svg_stream_create ();
    document->xml_node_glyphs = _cairo_svg_stream_create ();
    document->xml_node_filters = _cairo_svg_stream_create ();

    document->linear_pattern_id = 0;
    document->radial_pattern_id = 0;
    document->pattern_id = 0;
    document->clip_id = 0;
    document->mask_id = 0;
    document->compositing_group_id = 0;
    document->filter_id = 0;

    for (enum cairo_svg_filter filter = 0; filter < CAIRO_SVG_FILTER_LAST_STATIC_FILTER; filter++) {
	document->filters_emitted[filter] = FALSE;
    }

    document->svg_version = version;

    /* The use of defs for font glyphs imposes no per-subset limit. */
    document->font_subsets = _cairo_scaled_font_subsets_create_scaled ();
    if (unlikely (document->font_subsets == NULL)) {
	(void) _cairo_svg_stream_destroy(&document->xml_node_defs);
	(void) _cairo_svg_stream_destroy(&document->xml_node_glyphs);
	(void) _cairo_svg_stream_destroy(&document->xml_node_filters);
	free (document);
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    document->paints = _cairo_hash_table_create (_cairo_svg_paint_equal);
    if (unlikely (document->paints == NULL)) {
	(void) _cairo_svg_stream_destroy(&document->xml_node_defs);
	(void) _cairo_svg_stream_destroy(&document->xml_node_glyphs);
	(void) _cairo_svg_stream_destroy(&document->xml_node_filters);
	_cairo_scaled_font_subsets_destroy (document->font_subsets);
	free (document);
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    *document_out = document;
    return CAIRO_STATUS_SUCCESS;
}

static cairo_svg_document_t *
_cairo_svg_document_reference (cairo_svg_document_t *document)
{
    document->refcount++;

    return document;
}

static cairo_status_t
_cairo_svg_document_destroy (cairo_svg_document_t *document)
{
    cairo_status_t status;

    document->refcount--;
    if (document->refcount > 0)
      return CAIRO_STATUS_SUCCESS;

    status = _cairo_svg_document_finish (document);

    free (document);

    return status;
}

static cairo_status_t
_cairo_svg_document_finish (cairo_svg_document_t *document)
{
    if (document->finished) {
	return CAIRO_STATUS_SUCCESS;
    }
    document->finished = TRUE;

    cairo_status_t status, final_status = CAIRO_STATUS_SUCCESS;

    cairo_output_stream_t *output = document->output_stream;

    /*
     * Should we add DOCTYPE?
     *
     * Google says no.
     *
     * http://tech.groups.yahoo.com/group/svg-developers/message/48562:
     *   There's a bunch of issues, but just to pick a few:
     *   - they'll give false positives.
     *   - they'll give false negatives.
     *   - they're namespace-unaware.
     *   - they don't wildcard.
     *   So when they say OK they really haven't checked anything, when
     *   they say NOT OK they might be on crack, and like all
     *   namespace-unaware things they're a dead branch of the XML tree.
     *
     * http://jwatt.org/svg/authoring/:
     *   Unfortunately the SVG DTDs are a source of so many issues that the
     *   SVG WG has decided not to write one for the upcoming SVG 1.2
     *   standard. In fact SVG WG members are even telling people not to use
     *   a DOCTYPE declaration in SVG 1.0 and 1.1 documents.
     */

    _cairo_output_stream_printf (output,
				 "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
				 "<svg xmlns=\"http://www.w3.org/2000/svg\" "
				 "xmlns:xlink=\"http://www.w3.org/1999/xlink\" "
				 "width=\"%f%s\" height=\"%f%s\" "
				 "viewBox=\"0 0 %f %f\">\n",
				 document->width, _cairo_svg_unit_strings[document->unit],
				 document->height, _cairo_svg_unit_strings[document->unit],
				 document->width, document->height);

    status = _cairo_svg_document_emit_font_subsets (document);
    if (final_status == CAIRO_STATUS_SUCCESS) {
	final_status = status;
    }

    cairo_svg_surface_t *surface = NULL;
    if (document->owner != NULL) {
	surface = (cairo_svg_surface_t *) _cairo_paginated_surface_get_target (document->owner);

	if (surface->xml_node.elements.num_elements > 0) {
	    cairo_svg_page_t *page = _cairo_svg_surface_store_page (surface);
	    if (final_status == CAIRO_STATUS_SUCCESS && page == NULL) {
		final_status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    }
	}

	if (surface->transitive_paint_used) {
	    cairo_svg_paint_t *paint_entry = malloc (sizeof (cairo_svg_paint_t));
	    if (paint_entry == NULL) {
		return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    }
	    paint_entry->source_id = surface->source_id;
	    paint_entry->box.p1.x = 0;
	    paint_entry->box.p1.y = 0;
	    paint_entry->box.p2.x = document->width;
	    paint_entry->box.p2.y = document->height;
	    _cairo_svg_paint_box_add_padding (&paint_entry->box);
	    _cairo_array_init (&paint_entry->paint_elements, sizeof (cairo_svg_paint_element_t));
	    _cairo_svg_paint_init_key (paint_entry);
	    status = _cairo_hash_table_insert (document->paints, &paint_entry->base);
	    if (unlikely (status)) {
		return status;
	    }
	}
    }

    _cairo_hash_table_foreach (document->paints, _cairo_svg_paint_compute_func, document);

    if (document->xml_node_filters.elements.num_elements > 0 ||
	document->xml_node_glyphs.elements.num_elements > 0 ||
	document->xml_node_defs.elements.num_elements > 0) {
	_cairo_output_stream_printf (output, "<defs>\n");
	_cairo_svg_stream_copy_to_output_stream (&document->xml_node_filters, output, document->paints);
	if (document->xml_node_glyphs.elements.num_elements > 0) {
	    _cairo_output_stream_printf (output, "<g>\n");
	    _cairo_svg_stream_copy_to_output_stream (&document->xml_node_glyphs, output, document->paints);
	    _cairo_output_stream_printf (output, "</g>\n");
	}
	_cairo_svg_stream_copy_to_output_stream (&document->xml_node_defs, output, document->paints);
	_cairo_output_stream_printf (output, "</defs>\n");
    }

    if (document->owner != NULL) {
	if (surface->page_set.num_elements == 1) {
	    cairo_svg_page_t *page = _cairo_array_index (&surface->page_set, 0);
	    _cairo_svg_stream_copy_to_output_stream (&page->xml_node, output, document->paints);
	} else if (surface->page_set.num_elements > 1) {
	    _cairo_output_stream_printf (output, "<pageSet>\n");
	    for (unsigned int i = 0; i < surface->page_set.num_elements; i++) {
		cairo_svg_page_t *page = _cairo_array_index (&surface->page_set, i);
		_cairo_output_stream_printf (output, "<page>\n");
		_cairo_svg_stream_copy_to_output_stream (&page->xml_node, output, document->paints);
		_cairo_output_stream_printf (output, "</page>\n");
	    }
	    _cairo_output_stream_printf (output, "</pageSet>\n");
	}
    }

    _cairo_output_stream_printf (output, "</svg>\n");

    status = _cairo_svg_stream_destroy (&document->xml_node_defs);
    if (final_status == CAIRO_STATUS_SUCCESS) {
	final_status = status;
    }

    status = _cairo_svg_stream_destroy (&document->xml_node_glyphs);
    if (final_status == CAIRO_STATUS_SUCCESS) {
	final_status = status;
    }

    status = _cairo_svg_stream_destroy (&document->xml_node_filters);
    if (final_status == CAIRO_STATUS_SUCCESS) {
	final_status = status;
    }

    _cairo_hash_table_foreach (document->paints, _cairo_svg_paint_pluck, document->paints);
    _cairo_hash_table_destroy (document->paints);

    status = _cairo_output_stream_destroy (output);
    if (final_status == CAIRO_STATUS_SUCCESS) {
	final_status = status;
    }

    return final_status;
}

static cairo_int_status_t
_cairo_svg_surface_set_paginated_mode (void			*abstract_surface,
				       cairo_paginated_mode_t	 paginated_mode)
{
    cairo_svg_surface_t *surface = abstract_surface;

    surface->paginated_mode = paginated_mode;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
_cairo_svg_surface_supports_fine_grained_fallbacks (void *abstract_surface)
{
    return TRUE;
}

static const cairo_paginated_surface_backend_t cairo_svg_surface_paginated_backend = {
    NULL /*_cairo_svg_surface_start_page*/,
    _cairo_svg_surface_set_paginated_mode,
    NULL, /* _cairo_svg_surface_set_bounding_box */
    NULL, /* _cairo_svg_surface_set_fallback_images_required */
    _cairo_svg_surface_supports_fine_grained_fallbacks,
};
