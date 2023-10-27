/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2005 Red Hat, Inc
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
 * The Initial Developer of the Original Code is Red Hat, Inc.
 *
 * Author(s):
 *	Kristian Høgsberg <krh@redhat.com>
 */

#include "cairoint.h"
#include "cairo-error-private.h"
#include "cairo-output-stream-private.h"

typedef struct _cairo_base85_stream {
    cairo_output_stream_t base;
    cairo_output_stream_t *output;
    unsigned char four_tuple[4];
    int pending;
} cairo_base85_stream_t;

static void
_expand_four_tuple_to_five (unsigned char four_tuple[4],
			    unsigned char five_tuple[5],
			    cairo_bool_t *all_zero)
{
    uint32_t value;
    int digit, i;

    value = (uint32_t)four_tuple[0] << 24 | four_tuple[1] << 16 | four_tuple[2] << 8 | four_tuple[3];
    if (all_zero)
	*all_zero = TRUE;
    for (i = 0; i < 5; i++) {
	digit = value % 85;
	if (digit != 0 && all_zero)
	    *all_zero = FALSE;
	five_tuple[4-i] = digit + 33;
	value = value / 85;
    }
}

static cairo_status_t
_cairo_base85_stream_write (cairo_output_stream_t *base,
			    const unsigned char	  *data,
			    unsigned int	   length)
{
    cairo_base85_stream_t *stream = (cairo_base85_stream_t *) base;
    const unsigned char *ptr = data;
    unsigned char five_tuple[5];
    cairo_bool_t is_zero;

    while (length) {
	stream->four_tuple[stream->pending++] = *ptr++;
	length--;
	if (stream->pending == 4) {
	    _expand_four_tuple_to_five (stream->four_tuple, five_tuple, &is_zero);
	    if (is_zero)
		_cairo_output_stream_write (stream->output, "z", 1);
	    else
		_cairo_output_stream_write (stream->output, five_tuple, 5);
	    stream->pending = 0;
	}
    }

    return _cairo_output_stream_get_status (stream->output);
}

static cairo_status_t
_cairo_base85_stream_close (cairo_output_stream_t *base)
{
    cairo_base85_stream_t *stream = (cairo_base85_stream_t *) base;
    unsigned char five_tuple[5];

    if (stream->pending) {
	memset (stream->four_tuple + stream->pending, 0, 4 - stream->pending);
	_expand_four_tuple_to_five (stream->four_tuple, five_tuple, NULL);
	_cairo_output_stream_write (stream->output, five_tuple, stream->pending + 1);
    }

    return _cairo_output_stream_get_status (stream->output);
}

cairo_output_stream_t *
_cairo_base85_stream_create (cairo_output_stream_t *output)
{
    cairo_base85_stream_t *stream;

    if (output->status)
	return _cairo_output_stream_create_in_error (output->status);

    stream = _cairo_malloc (sizeof (cairo_base85_stream_t));
    if (unlikely (stream == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    }

    _cairo_output_stream_init (&stream->base,
			       _cairo_base85_stream_write,
			       NULL,
			       _cairo_base85_stream_close);
    stream->output = output;
    stream->pending = 0;

    return &stream->base;
}
