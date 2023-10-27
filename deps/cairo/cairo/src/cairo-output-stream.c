/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo-output-stream.c: Output stream abstraction
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

#define _DEFAULT_SOURCE /* for snprintf() */
#include "cairoint.h"

#include "cairo-output-stream-private.h"

#include "cairo-array-private.h"
#include "cairo-error-private.h"
#include "cairo-compiler-private.h"

#include <stdio.h>
#include <errno.h>

/* Numbers printed with %f are printed with this number of significant
 * digits after the decimal.
 */
#define SIGNIFICANT_DIGITS_AFTER_DECIMAL 6

/* Numbers printed with %g are assumed to only have %CAIRO_FIXED_FRAC_BITS
 * bits of precision available after the decimal point.
 *
 * FIXED_POINT_DECIMAL_DIGITS specifies the minimum number of decimal
 * digits after the decimal point required to preserve the available
 * precision.
 *
 * The conversion is:
 *
 * <programlisting>
 * FIXED_POINT_DECIMAL_DIGITS = ceil( CAIRO_FIXED_FRAC_BITS * ln(2)/ln(10) )
 * </programlisting>
 *
 * We can replace ceil(x) with (int)(x+1) since x will never be an
 * integer for any likely value of %CAIRO_FIXED_FRAC_BITS.
 */
#define FIXED_POINT_DECIMAL_DIGITS ((int)(CAIRO_FIXED_FRAC_BITS*0.301029996 + 1))

void
_cairo_output_stream_init (cairo_output_stream_t            *stream,
			   cairo_output_stream_write_func_t  write_func,
			   cairo_output_stream_flush_func_t  flush_func,
			   cairo_output_stream_close_func_t  close_func)
{
    stream->write_func = write_func;
    stream->flush_func = flush_func;
    stream->close_func = close_func;
    stream->position = 0;
    stream->status = CAIRO_STATUS_SUCCESS;
    stream->closed = FALSE;
}

cairo_status_t
_cairo_output_stream_fini (cairo_output_stream_t *stream)
{
    return _cairo_output_stream_close (stream);
}

const cairo_output_stream_t _cairo_output_stream_nil = {
    NULL, /* write_func */
    NULL, /* flush_func */
    NULL, /* close_func */
    0,    /* position */
    CAIRO_STATUS_NO_MEMORY,
    FALSE /* closed */
};

static const cairo_output_stream_t _cairo_output_stream_nil_write_error = {
    NULL, /* write_func */
    NULL, /* flush_func */
    NULL, /* close_func */
    0,    /* position */
    CAIRO_STATUS_WRITE_ERROR,
    FALSE /* closed */
};

typedef struct _cairo_output_stream_with_closure {
    cairo_output_stream_t	 base;
    cairo_write_func_t		 write_func;
    cairo_close_func_t		 close_func;
    void			*closure;
} cairo_output_stream_with_closure_t;


static cairo_status_t
closure_write (cairo_output_stream_t *stream,
	       const unsigned char *data, unsigned int length)
{
    cairo_output_stream_with_closure_t *stream_with_closure =
	(cairo_output_stream_with_closure_t *) stream;

    if (stream_with_closure->write_func == NULL)
	return CAIRO_STATUS_SUCCESS;

    return stream_with_closure->write_func (stream_with_closure->closure,
					    data, length);
}

static cairo_status_t
closure_close (cairo_output_stream_t *stream)
{
    cairo_output_stream_with_closure_t *stream_with_closure =
	(cairo_output_stream_with_closure_t *) stream;

    if (stream_with_closure->close_func != NULL)
	return stream_with_closure->close_func (stream_with_closure->closure);
    else
	return CAIRO_STATUS_SUCCESS;
}

cairo_output_stream_t *
_cairo_output_stream_create (cairo_write_func_t		write_func,
			     cairo_close_func_t		close_func,
			     void			*closure)
{
    cairo_output_stream_with_closure_t *stream;

    stream = _cairo_malloc (sizeof (cairo_output_stream_with_closure_t));
    if (unlikely (stream == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    }

    _cairo_output_stream_init (&stream->base,
			       closure_write, NULL, closure_close);
    stream->write_func = write_func;
    stream->close_func = close_func;
    stream->closure = closure;

    return &stream->base;
}

cairo_output_stream_t *
_cairo_output_stream_create_in_error (cairo_status_t status)
{
    cairo_output_stream_t *stream;

    /* check for the common ones */
    if (status == CAIRO_STATUS_NO_MEMORY)
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    if (status == CAIRO_STATUS_WRITE_ERROR)
	return (cairo_output_stream_t *) &_cairo_output_stream_nil_write_error;

    stream = _cairo_malloc (sizeof (cairo_output_stream_t));
    if (unlikely (stream == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    }

    _cairo_output_stream_init (stream, NULL, NULL, NULL);
    stream->status = status;

    return stream;
}

cairo_status_t
_cairo_output_stream_flush (cairo_output_stream_t *stream)
{
    cairo_status_t status;

    if (stream->closed)
	return stream->status;

    if (stream == &_cairo_output_stream_nil ||
	stream == &_cairo_output_stream_nil_write_error)
    {
	return stream->status;
    }

    if (stream->flush_func) {
	status = stream->flush_func (stream);
	/* Don't overwrite a pre-existing status failure. */
	if (stream->status == CAIRO_STATUS_SUCCESS)
	    stream->status = status;
    }

    return stream->status;
}

cairo_status_t
_cairo_output_stream_close (cairo_output_stream_t *stream)
{
    cairo_status_t status;

    if (stream->closed)
	return stream->status;

    if (stream == &_cairo_output_stream_nil ||
	stream == &_cairo_output_stream_nil_write_error)
    {
	return stream->status;
    }

    if (stream->close_func) {
	status = stream->close_func (stream);
	/* Don't overwrite a pre-existing status failure. */
	if (stream->status == CAIRO_STATUS_SUCCESS)
	    stream->status = status;
    }

    stream->closed = TRUE;

    return stream->status;
}

cairo_status_t
_cairo_output_stream_destroy (cairo_output_stream_t *stream)
{
    cairo_status_t status;

    assert (stream != NULL);

    if (stream == &_cairo_output_stream_nil ||
	stream == &_cairo_output_stream_nil_write_error)
    {
	return stream->status;
    }

    status = _cairo_output_stream_fini (stream);
    free (stream);

    return status;
}

void
_cairo_output_stream_write (cairo_output_stream_t *stream,
			    const void *data, size_t length)
{
    if (length == 0 || stream->status)
	return;

    if (stream->closed) {
	stream->status = CAIRO_STATUS_WRITE_ERROR;
	return;
    }

    stream->status = stream->write_func (stream, data, length);
    stream->position += length;
}

void
_cairo_output_stream_write_hex_string (cairo_output_stream_t *stream,
				       const unsigned char *data,
				       size_t length)
{
    const char hex_chars[] = "0123456789abcdef";
    char buffer[2];
    unsigned int i, column;

    for (i = 0, column = 0; i < length; i++, column++) {
	if (column == 38) {
	    _cairo_output_stream_write (stream, "\n", 1);
	    column = 0;
	}
	buffer[0] = hex_chars[(data[i] >> 4) & 0x0f];
	buffer[1] = hex_chars[data[i] & 0x0f];
	_cairo_output_stream_write (stream, buffer, 2);
    }
}

/* Format a double in a locale independent way and trim trailing
 * zeros.  Based on code from Alex Larson <alexl@redhat.com>.
 * https://mail.gnome.org/archives/gtk-devel-list/2001-October/msg00087.html
 *
 * The code in the patch is copyright Red Hat, Inc under the LGPL, but
 * has been relicensed under the LGPL/MPL dual license for inclusion
 * into cairo (see COPYING). -- Kristian Høgsberg <krh@redhat.com>
 */
static void
_cairo_dtostr (char *buffer, size_t size, double d, cairo_bool_t limited_precision)
{
    const char *decimal_point;
    int decimal_point_len;
    char *p;
    int decimal_len;
    int num_zeros, decimal_digits;

    /* Omit the minus sign from negative zero. */
    if (d == 0.0)
	d = 0.0;

    decimal_point = _cairo_get_locale_decimal_point ();
    decimal_point_len = strlen (decimal_point);

    assert (decimal_point_len != 0);

    if (limited_precision) {
	snprintf (buffer, size, "%.*f", FIXED_POINT_DECIMAL_DIGITS, d);
    } else {
	/* Using "%f" to print numbers less than 0.1 will result in
	 * reduced precision due to the default 6 digits after the
	 * decimal point.
	 *
	 * For numbers is < 0.1, we print with maximum precision and count
	 * the number of zeros between the decimal point and the first
	 * significant digit. We then print the number again with the
	 * number of decimal places that gives us the required number of
	 * significant digits. This ensures the number is correctly
	 * rounded.
	 */
	if (fabs (d) >= 0.1) {
	    snprintf (buffer, size, "%f", d);
	} else {
	    snprintf (buffer, size, "%.18f", d);
	    p = buffer;

	    if (*p == '+' || *p == '-')
		p++;

	    while (_cairo_isdigit (*p))
		p++;

	    if (strncmp (p, decimal_point, decimal_point_len) == 0)
		p += decimal_point_len;

	    num_zeros = 0;
	    while (*p++ == '0')
		num_zeros++;

	    decimal_digits = num_zeros + SIGNIFICANT_DIGITS_AFTER_DECIMAL;

	    if (decimal_digits < 18)
		snprintf (buffer, size, "%.*f", decimal_digits, d);
	}
    }
    p = buffer;

    if (*p == '+' || *p == '-')
	p++;

    while (_cairo_isdigit (*p))
	p++;

    if (strncmp (p, decimal_point, decimal_point_len) == 0) {
	*p = '.';
	decimal_len = strlen (p + decimal_point_len);
	memmove (p + 1, p + decimal_point_len, decimal_len);
	p[1 + decimal_len] = 0;

	/* Remove trailing zeros and decimal point if possible. */
	for (p = p + decimal_len; *p == '0'; p--)
	    *p = 0;

	if (*p == '.') {
	    *p = 0;
	    p--;
	}
    }
}

enum {
    LENGTH_MODIFIER_LONG = 0x100,
    LENGTH_MODIFIER_LONG_LONG = 0x200
};

/* Here's a limited reimplementation of printf.  The reason for doing
 * this is primarily to special case handling of doubles.  We want
 * locale independent formatting of doubles and we want to trim
 * trailing zeros.  This is handled by dtostr() above, and the code
 * below handles everything else by calling snprintf() to do the
 * formatting.  This functionality is only for internal use and we
 * only implement the formats we actually use.
 */
void
_cairo_output_stream_vprintf (cairo_output_stream_t *stream,
			      const char *fmt, va_list ap)
{
#define SINGLE_FMT_BUFFER_SIZE 32
    char buffer[512], single_fmt[SINGLE_FMT_BUFFER_SIZE];
    int single_fmt_length;
    char *p;
    const char *f, *start;
    int length_modifier, width;
    cairo_bool_t var_width;

    f = fmt;
    p = buffer;
    while (*f != '\0') {
	if (p == buffer + sizeof (buffer)) {
	    _cairo_output_stream_write (stream, buffer, sizeof (buffer));
	    p = buffer;
	}

	if (*f != '%') {
	    *p++ = *f++;
	    continue;
	}

	start = f;
	f++;

	if (*f == '0')
	    f++;

        var_width = FALSE;
        if (*f == '*') {
            var_width = TRUE;
	    f++;
        }

	while (_cairo_isdigit (*f))
	    f++;

	length_modifier = 0;
	if (*f == 'l') {
	    length_modifier = LENGTH_MODIFIER_LONG;
	    f++;
	    if (*f == 'l') {
		length_modifier = LENGTH_MODIFIER_LONG_LONG;
		f++;
	    }
	}

	/* The only format strings exist in the cairo implementation
	 * itself. So there's an internal consistency problem if any
	 * of them is larger than our format buffer size. */
	single_fmt_length = f - start + 1;
	assert (single_fmt_length + 1 <= SINGLE_FMT_BUFFER_SIZE);

	/* Reuse the format string for this conversion. */
	memcpy (single_fmt, start, single_fmt_length);
	single_fmt[single_fmt_length] = '\0';

	/* Flush contents of buffer before snprintf()'ing into it. */
	_cairo_output_stream_write (stream, buffer, p - buffer);

	/* We group signed and unsigned together in this switch, the
	 * only thing that matters here is the size of the arguments,
	 * since we're just passing the data through to sprintf(). */
	switch (*f | length_modifier) {
	case '%':
	    buffer[0] = *f;
	    buffer[1] = 0;
	    break;
	case 'd':
	case 'u':
	case 'o':
	case 'x':
	case 'X':
            if (var_width) {
                width = va_arg (ap, int);
                snprintf (buffer, sizeof buffer,
                          single_fmt, width, va_arg (ap, int));
            } else {
                snprintf (buffer, sizeof buffer, single_fmt, va_arg (ap, int));
            }
	    break;
	case 'd' | LENGTH_MODIFIER_LONG:
	case 'u' | LENGTH_MODIFIER_LONG:
	case 'o' | LENGTH_MODIFIER_LONG:
	case 'x' | LENGTH_MODIFIER_LONG:
	case 'X' | LENGTH_MODIFIER_LONG:
            if (var_width) {
                width = va_arg (ap, int);
                snprintf (buffer, sizeof buffer,
                          single_fmt, width, va_arg (ap, long int));
            } else {
                snprintf (buffer, sizeof buffer,
                          single_fmt, va_arg (ap, long int));
            }
	    break;
	case 'd' | LENGTH_MODIFIER_LONG_LONG:
	case 'u' | LENGTH_MODIFIER_LONG_LONG:
	case 'o' | LENGTH_MODIFIER_LONG_LONG:
	case 'x' | LENGTH_MODIFIER_LONG_LONG:
	case 'X' | LENGTH_MODIFIER_LONG_LONG:
	    if (var_width) {
		width = va_arg (ap, int);
		snprintf (buffer, sizeof buffer,
			  single_fmt, width, va_arg (ap, long long int));
	    } else {
		snprintf (buffer, sizeof buffer,
			  single_fmt, va_arg (ap, long long int));
	    }
	    break;
	case 's': {
	    /* Write out strings as they may be larger than the buffer. */
	    const char *s = va_arg (ap, const char *);
	    int len = strlen(s);
	    _cairo_output_stream_write (stream, s, len);
	    buffer[0] = 0;
	    }
	    break;
	case 'f':
	    _cairo_dtostr (buffer, sizeof buffer, va_arg (ap, double), FALSE);
	    break;
	case 'g':
	    _cairo_dtostr (buffer, sizeof buffer, va_arg (ap, double), TRUE);
	    break;
	case 'c':
	    buffer[0] = va_arg (ap, int);
	    buffer[1] = 0;
	    break;
	default:
	    ASSERT_NOT_REACHED;
	}
	p = buffer + strlen (buffer);
	f++;
    }

    _cairo_output_stream_write (stream, buffer, p - buffer);
}

void
_cairo_output_stream_printf (cairo_output_stream_t *stream,
			     const char *fmt, ...)
{
    va_list ap;

    va_start (ap, fmt);

    _cairo_output_stream_vprintf (stream, fmt, ap);

    va_end (ap);
}

/* Matrix elements that are smaller than the value of the largest element * MATRIX_ROUNDING_TOLERANCE
 * are rounded down to zero. */
#define MATRIX_ROUNDING_TOLERANCE 1e-12

void
_cairo_output_stream_print_matrix (cairo_output_stream_t *stream,
				   const cairo_matrix_t  *matrix)
{
    cairo_matrix_t m;
    double s, e;

    m = *matrix;
    s = fabs (m.xx);
    if (fabs (m.xy) > s)
	s = fabs (m.xy);
    if (fabs (m.yx) > s)
	s = fabs (m.yx);
    if (fabs (m.yy) > s)
	s = fabs (m.yy);

    e = s * MATRIX_ROUNDING_TOLERANCE;
    if (fabs(m.xx) < e)
	m.xx = 0;
    if (fabs(m.xy) < e)
	m.xy = 0;
    if (fabs(m.yx) < e)
	m.yx = 0;
    if (fabs(m.yy) < e)
	m.yy = 0;
    if (fabs(m.x0) < e)
	m.x0 = 0;
    if (fabs(m.y0) < e)
	m.y0 = 0;

    _cairo_output_stream_printf (stream,
				 "%f %f %f %f %f %f",
				 m.xx, m.yx, m.xy, m.yy, m.x0, m.y0);
}

long long
_cairo_output_stream_get_position (cairo_output_stream_t *stream)
{
    return stream->position;
}

cairo_status_t
_cairo_output_stream_get_status (cairo_output_stream_t *stream)
{
    return stream->status;
}

/* Maybe this should be a configure time option, so embedded targets
 * don't have to pull in stdio. */


typedef struct _stdio_stream {
    cairo_output_stream_t	 base;
    FILE			*file;
} stdio_stream_t;

static cairo_status_t
stdio_write (cairo_output_stream_t *base,
	     const unsigned char *data, unsigned int length)
{
    stdio_stream_t *stream = (stdio_stream_t *) base;

    if (fwrite (data, 1, length, stream->file) != length)
	return _cairo_error (CAIRO_STATUS_WRITE_ERROR);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
stdio_flush (cairo_output_stream_t *base)
{
    stdio_stream_t *stream = (stdio_stream_t *) base;

    fflush (stream->file);

    if (ferror (stream->file))
	return _cairo_error (CAIRO_STATUS_WRITE_ERROR);
    else
	return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
stdio_close (cairo_output_stream_t *base)
{
    cairo_status_t status;
    stdio_stream_t *stream = (stdio_stream_t *) base;

    status = stdio_flush (base);

    fclose (stream->file);

    return status;
}

cairo_output_stream_t *
_cairo_output_stream_create_for_file (FILE *file)
{
    stdio_stream_t *stream;

    if (file == NULL) {
	_cairo_error_throw (CAIRO_STATUS_WRITE_ERROR);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil_write_error;
    }

    stream = _cairo_malloc (sizeof *stream);
    if (unlikely (stream == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    }

    _cairo_output_stream_init (&stream->base,
			       stdio_write, stdio_flush, stdio_flush);
    stream->file = file;

    return &stream->base;
}

cairo_output_stream_t *
_cairo_output_stream_create_for_filename (const char *filename)
{
    stdio_stream_t *stream;
    FILE *file;
    cairo_status_t status;

    if (filename == NULL)
	return _cairo_null_stream_create ();

    status = _cairo_fopen (filename, "wb", &file);

    if (status != CAIRO_STATUS_SUCCESS)
	return _cairo_output_stream_create_in_error (status);

    if (file == NULL) {
	switch (errno) {
	case ENOMEM:
	    _cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	    return (cairo_output_stream_t *) &_cairo_output_stream_nil;
	default:
	    _cairo_error_throw (CAIRO_STATUS_WRITE_ERROR);
	    return (cairo_output_stream_t *) &_cairo_output_stream_nil_write_error;
	}
    }

    stream = _cairo_malloc (sizeof *stream);
    if (unlikely (stream == NULL)) {
	fclose (file);
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    }

    _cairo_output_stream_init (&stream->base,
			       stdio_write, stdio_flush, stdio_close);
    stream->file = file;

    return &stream->base;
}


typedef struct _memory_stream {
    cairo_output_stream_t	base;
    cairo_array_t		array;
} memory_stream_t;

static cairo_status_t
memory_write (cairo_output_stream_t *base,
	      const unsigned char *data, unsigned int length)
{
    memory_stream_t *stream = (memory_stream_t *) base;

    return _cairo_array_append_multiple (&stream->array, data, length);
}

static cairo_status_t
memory_close (cairo_output_stream_t *base)
{
    memory_stream_t *stream = (memory_stream_t *) base;

    _cairo_array_fini (&stream->array);

    return CAIRO_STATUS_SUCCESS;
}

cairo_output_stream_t *
_cairo_memory_stream_create (void)
{
    memory_stream_t *stream;

    stream = _cairo_malloc (sizeof *stream);
    if (unlikely (stream == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    }

    _cairo_output_stream_init (&stream->base, memory_write, NULL, memory_close);
    _cairo_array_init (&stream->array, 1);

    return &stream->base;
}

cairo_status_t
_cairo_memory_stream_destroy (cairo_output_stream_t *abstract_stream,
			      unsigned char **data_out,
			      unsigned long *length_out)
{
    memory_stream_t *stream;
    cairo_status_t status;

    status = abstract_stream->status;
    if (unlikely (status))
	return _cairo_output_stream_destroy (abstract_stream);

    stream = (memory_stream_t *) abstract_stream;

    *length_out = _cairo_array_num_elements (&stream->array);
    *data_out = _cairo_malloc (*length_out);
    if (unlikely (*data_out == NULL)) {
	status = _cairo_output_stream_destroy (abstract_stream);
	assert (status == CAIRO_STATUS_SUCCESS);
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }
    memcpy (*data_out, _cairo_array_index (&stream->array, 0), *length_out);

    return _cairo_output_stream_destroy (abstract_stream);
}

void
_cairo_memory_stream_copy (cairo_output_stream_t *base,
			   cairo_output_stream_t *dest)
{
    memory_stream_t *stream = (memory_stream_t *) base;

    if (base->status) {
	dest->status = base->status;
	return;
    }

    _cairo_output_stream_write (dest,
				_cairo_array_index (&stream->array, 0),
				_cairo_array_num_elements (&stream->array));
}

int
_cairo_memory_stream_length (cairo_output_stream_t *base)
{
    memory_stream_t *stream = (memory_stream_t *) base;

    return _cairo_array_num_elements (&stream->array);
}

static cairo_status_t
null_write (cairo_output_stream_t *base,
	    const unsigned char *data, unsigned int length)
{
    return CAIRO_STATUS_SUCCESS;
}

cairo_output_stream_t *
_cairo_null_stream_create (void)
{
    cairo_output_stream_t *stream;

    stream = _cairo_malloc (sizeof *stream);
    if (unlikely (stream == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_output_stream_t *) &_cairo_output_stream_nil;
    }

    _cairo_output_stream_init (stream, null_write, NULL, NULL);

    return stream;
}
