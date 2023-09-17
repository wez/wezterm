/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2006 Adrian Johnson
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
 * The Initial Developer of the Original Code is Adrian Johnson.
 *
 * Author(s):
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#include "cairoint.h"

#if CAIRO_HAS_DEFLATE_STREAM

#include "cairo-error-private.h"
#include "cairo-output-stream-private.h"
#include <zlib.h>

#define BUFFER_SIZE 16384

typedef struct _cairo_deflate_stream {
    cairo_output_stream_t  base;
    cairo_output_stream_t *output;
    z_stream               zlib_stream;
    unsigned char          input_buf[BUFFER_SIZE];
    unsigned char          output_buf[BUFFER_SIZE];
} cairo_deflate_stream_t;

static void
cairo_deflate_stream_deflate (cairo_deflate_stream_t *stream, cairo_bool_t flush)
{
    int ret;
    cairo_bool_t finished;

    do {
        ret = deflate (&stream->zlib_stream, flush ? Z_FINISH : Z_NO_FLUSH);
        if (flush || stream->zlib_stream.avail_out == 0)
        {
            _cairo_output_stream_write (stream->output,
                                        stream->output_buf,
                                        BUFFER_SIZE - stream->zlib_stream.avail_out);
            stream->zlib_stream.next_out = stream->output_buf;
            stream->zlib_stream.avail_out = BUFFER_SIZE;
        }

        finished = TRUE;
        if (stream->zlib_stream.avail_in != 0)
            finished = FALSE;
        if (flush && ret != Z_STREAM_END)
            finished = FALSE;

    } while (!finished);

    stream->zlib_stream.next_in = stream->input_buf;
}

static cairo_status_t
_cairo_deflate_stream_write (cairo_output_stream_t *base,
                             const unsigned char   *data,
                             unsigned int	    length)
{
    cairo_deflate_stream_t *stream = (cairo_deflate_stream_t *) base;
    unsigned int count;
    const unsigned char *p = data;

    while (length) {
        count = length;
        if (count > BUFFER_SIZE - stream->zlib_stream.avail_in)
            count = BUFFER_SIZE - stream->zlib_stream.avail_in;
        memcpy (stream->input_buf + stream->zlib_stream.avail_in, p, count);
        p += count;
        stream->zlib_stream.avail_in += count;
        length -= count;

        if (stream->zlib_stream.avail_in == BUFFER_SIZE)
            cairo_deflate_stream_deflate (stream, FALSE);
    }

    return _cairo_output_stream_get_status (stream->output);
}

static cairo_status_t
_cairo_deflate_stream_close (cairo_output_stream_t *base)
{
    cairo_deflate_stream_t *stream = (cairo_deflate_stream_t *) base;

    cairo_deflate_stream_deflate (stream, TRUE);
    deflateEnd (&stream->zlib_stream);

    return _cairo_output_stream_get_status (stream->output);
}

cairo_output_stream_t *
_cairo_deflate_stream_create (cairo_output_stream_t *output)
{
    cairo_deflate_stream_t *stream;

    if (output->status)
	return _cairo_output_stream_create_in_error (output->status);

    stream = _cairo_malloc (sizeof (cairo_deflate_stream_t));
    if (unlikely (stream == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    }

    _cairo_output_stream_init (&stream->base,
			       _cairo_deflate_stream_write,
			       NULL,
			       _cairo_deflate_stream_close);
    stream->output = output;

    stream->zlib_stream.zalloc = Z_NULL;
    stream->zlib_stream.zfree  = Z_NULL;
    stream->zlib_stream.opaque  = Z_NULL;

    if (deflateInit (&stream->zlib_stream, Z_DEFAULT_COMPRESSION) != Z_OK) {
	free (stream);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    }

    stream->zlib_stream.next_in = stream->input_buf;
    stream->zlib_stream.avail_in = 0;
    stream->zlib_stream.next_out = stream->output_buf;
    stream->zlib_stream.avail_out = BUFFER_SIZE;

    return &stream->base;
}

#endif /* CAIRO_HAS_DEFLATE_STREAM */
