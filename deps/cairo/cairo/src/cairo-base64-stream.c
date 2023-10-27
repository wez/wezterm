/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2005-2007 Emmanuel Pacaud <emmanuel.pacaud@free.fr>
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
 * The Initial Developer of the Original Code is Red Hat, Inc.
 *
 * Author(s):
 *	Kristian Høgsberg <krh@redhat.com>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"
#include "cairo-error-private.h"
#include "cairo-output-stream-private.h"

typedef struct _cairo_base64_stream {
    cairo_output_stream_t base;
    cairo_output_stream_t *output;
    unsigned int in_mem;
    unsigned int trailing;
    unsigned char src[3];
} cairo_base64_stream_t;

static char const base64_table[64] =
"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

static cairo_status_t
_cairo_base64_stream_write (cairo_output_stream_t *base,
			    const unsigned char	  *data,
			    unsigned int	   length)
{
    cairo_base64_stream_t * stream = (cairo_base64_stream_t *) base;
    unsigned char *src = stream->src;
    unsigned int i;

    if (stream->in_mem + length < 3) {
	for (i = 0; i < length; i++) {
	    src[i + stream->in_mem] = *data++;
	}
	stream->in_mem += length;
	return CAIRO_STATUS_SUCCESS;
    }

    do {
	unsigned char dst[4];

	for (i = stream->in_mem; i < 3; i++) {
	    src[i] = *data++;
	    length--;
	}
	stream->in_mem = 0;

	dst[0] = base64_table[src[0] >> 2];
	dst[1] = base64_table[(src[0] & 0x03) << 4 | src[1] >> 4];
	dst[2] = base64_table[(src[1] & 0x0f) << 2 | src[2] >> 6];
	dst[3] = base64_table[src[2] & 0xfc >> 2];
	/* Special case for the last missing bits */
	switch (stream->trailing) {
	    case 2:
		dst[2] = '=';
		/* fall through */
	    case 1:
		dst[3] = '=';
	    default:
		break;
	}
	_cairo_output_stream_write (stream->output, dst, 4);
    } while (length >= 3);

    for (i = 0; i < length; i++) {
	src[i] = *data++;
    }
    stream->in_mem = length;

    return _cairo_output_stream_get_status (stream->output);
}

static cairo_status_t
_cairo_base64_stream_close (cairo_output_stream_t *base)
{
    cairo_base64_stream_t *stream = (cairo_base64_stream_t *) base;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    if (stream->in_mem > 0) {
	memset (stream->src + stream->in_mem, 0, 3 - stream->in_mem);
	stream->trailing = 3 - stream->in_mem;
	stream->in_mem = 3;
	status = _cairo_base64_stream_write (base, NULL, 0);
    }

    return status;
}

cairo_output_stream_t *
_cairo_base64_stream_create (cairo_output_stream_t *output)
{
    cairo_base64_stream_t *stream;

    if (output->status)
	return _cairo_output_stream_create_in_error (output->status);

    stream = _cairo_malloc (sizeof (cairo_base64_stream_t));
    if (unlikely (stream == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    }

    _cairo_output_stream_init (&stream->base,
			       _cairo_base64_stream_write,
			       NULL,
			       _cairo_base64_stream_close);

    stream->output = output;
    stream->in_mem = 0;
    stream->trailing = 0;

    return &stream->base;
}
