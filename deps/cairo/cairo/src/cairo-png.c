/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2003 University of Southern California
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
 *	Kristian Høgsberg <krh@redhat.com>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-error-private.h"
#include "cairo-image-surface-private.h"
#include "cairo-output-stream-private.h"

#include <stdio.h>
#include <errno.h>
#include <png.h>

/**
 * SECTION:cairo-png
 * @Title: PNG Support
 * @Short_Description: Reading and writing PNG images
 * @See_Also: #cairo_surface_t
 *
 * The PNG functions allow reading PNG images into image surfaces, and writing
 * any surface to a PNG file.
 *
 * It is a toy API. It only offers very simple support for reading and
 * writing PNG files, which is sufficient for testing and
 * demonstration purposes. Applications which need more control over
 * the generated PNG file should access the pixel data directly, using
 * cairo_image_surface_get_data() or a backend-specific access
 * function, and process it with another library, e.g. gdk-pixbuf or
 * libpng.
 **/

/**
 * CAIRO_HAS_PNG_FUNCTIONS:
 *
 * Defined if the PNG functions are available.
 * This macro can be used to conditionally compile code using the cairo
 * PNG functions.
 *
 * Since: 1.0
 **/

struct png_read_closure_t {
    cairo_read_func_t		 read_func;
    void			*closure;
    cairo_output_stream_t	*png_data;
};


/* Unpremultiplies data and converts native endian ARGB => RGBA bytes */
static void
unpremultiply_data (png_structp png, png_row_infop row_info, png_bytep data)
{
    unsigned int i;

    for (i = 0; i < row_info->rowbytes; i += 4) {
        uint8_t *b = &data[i];
        uint32_t pixel;
        uint8_t  alpha;

	memcpy (&pixel, b, sizeof (uint32_t));
	alpha = (pixel & 0xff000000) >> 24;
        if (alpha == 0) {
	    b[0] = b[1] = b[2] = b[3] = 0;
	} else {
            b[0] = (((pixel & 0xff0000) >> 16) * 255 + alpha / 2) / alpha;
            b[1] = (((pixel & 0x00ff00) >>  8) * 255 + alpha / 2) / alpha;
            b[2] = (((pixel & 0x0000ff) >>  0) * 255 + alpha / 2) / alpha;
	    b[3] = alpha;
	}
    }
}

static uint16_t f_to_u16(float val)
{
    if (val < 0)
	return 0;
    else if (val > 1)
	return 65535;
    else
	return (uint16_t)(val * 65535.f);
}

static void
unpremultiply_float (float *f, uint16_t *d16, unsigned width)
{
    unsigned int i;

    for (i = 0; i < width; i++) {
	float r, g, b, a;

	r = *f++;
	g = *f++;
	b = *f++;
	a = *f++;

	if (a > 0) {
	    *d16++ = f_to_u16(r / a);
	    *d16++ = f_to_u16(g / a);
	    *d16++ = f_to_u16(b / a);
	    *d16++ = f_to_u16(a);
	} else {
	    *d16++ = 0;
	    *d16++ = 0;
	    *d16++ = 0;
	    *d16++ = 0;
	}
    }
}

static void
premultiply_float (float *f, const uint16_t *d16, unsigned int width)
{
    unsigned int i = width;

    /* Convert d16 in place back to float */
    while (i--) {
	float a = d16[i * 4 + 3] / 65535.f;

	f[i * 4 + 3] = a;
	f[i * 4 + 2] = (float)d16[i * 4 + 2] / 65535.f * a;
	f[i * 4 + 1] = (float)d16[i * 4 + 1] / 65535.f * a;
	f[i * 4] = (float)d16[i * 4] / 65535.f * a;
    }
}

static void convert_u16_to_float (float *f, const uint16_t *d16, unsigned int width)
{
    /* Convert d16 in place back to float */
    unsigned int i = width;

    while (i--) {
	f[i * 3 + 2] = (float)d16[i * 4 + 2] / 65535.f;
	f[i * 3 + 1] = (float)d16[i * 4 + 1] / 65535.f;
	f[i * 3] = (float)d16[i * 4] / 65535.f;
    }
}

static void
convert_float_to_u16 (float *f, uint16_t *d16, unsigned int width)
{
    unsigned int i;

    for (i = 0; i < width; i++) {
	*d16++ = f_to_u16(*f++);
	*d16++ = f_to_u16(*f++);
	*d16++ = f_to_u16(*f++);
	*d16++ = 0;
    }
}

/* Converts native endian xRGB => RGBx bytes */
static void
convert_data_to_bytes (png_structp png, png_row_infop row_info, png_bytep data)
{
    unsigned int i;

    for (i = 0; i < row_info->rowbytes; i += 4) {
        uint8_t *b = &data[i];
        uint32_t pixel;

	memcpy (&pixel, b, sizeof (uint32_t));

	b[0] = (pixel & 0xff0000) >> 16;
	b[1] = (pixel & 0x00ff00) >>  8;
	b[2] = (pixel & 0x0000ff) >>  0;
	b[3] = 0;
    }
}

/* Use a couple of simple error callbacks that do not print anything to
 * stderr and rely on the user to check for errors via the #cairo_status_t
 * return.
 */
static void
png_simple_error_callback (png_structp png,
	                   png_const_charp error_msg)
{
    cairo_status_t *error = png_get_error_ptr (png);

    /* default to the most likely error */
    if (*error == CAIRO_STATUS_SUCCESS)
	*error = _cairo_error (CAIRO_STATUS_PNG_ERROR);

#ifdef PNG_SETJMP_SUPPORTED
    longjmp (png_jmpbuf (png), 1);
#endif

    /* if we get here, then we have to choice but to abort ... */
}

static void
png_simple_warning_callback (png_structp png,
	                     png_const_charp error_msg)
{
    /* png does not expect to abort and will try to tidy up and continue
     * loading the image after a warning. So we also want to return the
     * (incorrect?) surface.
     *
     * We use our own warning callback to squelch any attempts by libpng
     * to write to stderr as we may not be in control of that output.
     */
}


/* Starting with libpng-1.2.30, we must explicitly specify an output_flush_fn.
 * Otherwise, we will segfault if we are writing to a stream. */
static void
png_simple_output_flush_fn (png_structp png_ptr)
{
}

static cairo_status_t
write_png (cairo_surface_t	*surface,
	   png_rw_ptr		write_func,
	   void			*closure)
{
    int i;
    cairo_int_status_t status;
    cairo_image_surface_t *image;
    cairo_image_surface_t * volatile clone;
    void *image_extra;
    png_struct *png;
    png_info *info;
    png_byte **volatile rows = NULL;
    png_color_16 white;
    int png_color_type;
    int bpc;
    unsigned char *volatile u16_copy = NULL;

    status = _cairo_surface_acquire_source_image (surface,
						  &image,
						  &image_extra);

    if (status == CAIRO_INT_STATUS_UNSUPPORTED)
	return _cairo_error (CAIRO_STATUS_SURFACE_TYPE_MISMATCH);
    else if (unlikely (status))
        return status;

    /* PNG complains about "Image width or height is zero in IHDR" */
    if (image->width == 0 || image->height == 0) {
	status = _cairo_error (CAIRO_STATUS_WRITE_ERROR);
	goto BAIL1;
    }

    /* Don't coerce to a lower resolution format */
    if (image->format == CAIRO_FORMAT_RGB96F ||
        image->format == CAIRO_FORMAT_RGBA128F) {
	u16_copy = _cairo_malloc_ab (image->width * 8, image->height);
	if (!u16_copy) {
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    goto BAIL1;
	}
	clone = (cairo_image_surface_t *)cairo_surface_reference (&image->base);
    } else {
	  /* Handle the various fallback formats (e.g. low bit-depth XServers)
	  * by coercing them to a simpler format using pixman.
	  */
	  clone = _cairo_image_surface_coerce (image);
	  status = clone->base.status;
    }
    if (unlikely (status))
        goto BAIL1;

    rows = _cairo_malloc_ab (clone->height, sizeof (png_byte*));
    if (unlikely (rows == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto BAIL2;
    }

    if (!u16_copy) {
	for (i = 0; i < clone->height; i++)
	    rows[i] = (png_byte *)clone->data + i * clone->stride;
    } else {
	for (i = 0; i < clone->height; i++) {
	    float *float_line = (float *)&clone->data[i * clone->stride];
	    uint16_t *u16_line = (uint16_t *)&u16_copy[i * clone->width * 8];

	    if (image->format == CAIRO_FORMAT_RGBA128F)
		unpremultiply_float (float_line, u16_line, clone->width);
	    else
		convert_float_to_u16 (float_line, u16_line, clone->width);

	    rows[i] = (png_byte *)u16_line;
	}
    }

    png = png_create_write_struct (PNG_LIBPNG_VER_STRING, &status,
	                           png_simple_error_callback,
	                           png_simple_warning_callback);
    if (unlikely (png == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto BAIL3;
    }

    info = png_create_info_struct (png);
    if (unlikely (info == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto BAIL4;
    }

#ifdef PNG_SETJMP_SUPPORTED
    if (setjmp (png_jmpbuf (png)))
	goto BAIL4;
#endif

    png_set_write_fn (png, closure, write_func, png_simple_output_flush_fn);

    switch (clone->format) {
    case CAIRO_FORMAT_ARGB32:
	bpc = 8;
	if (_cairo_image_analyze_transparency (clone) == CAIRO_IMAGE_IS_OPAQUE)
	    png_color_type = PNG_COLOR_TYPE_RGB;
	else
	    png_color_type = PNG_COLOR_TYPE_RGB_ALPHA;
	break;
    case CAIRO_FORMAT_RGB30:
	bpc = 10;
	png_color_type = PNG_COLOR_TYPE_RGB;
	break;
    case CAIRO_FORMAT_RGB24:
	bpc = 8;
	png_color_type = PNG_COLOR_TYPE_RGB;
	break;
    case CAIRO_FORMAT_A8:
	bpc = 8;
	png_color_type = PNG_COLOR_TYPE_GRAY;
	break;
    case CAIRO_FORMAT_A1:
	bpc = 1;
	png_color_type = PNG_COLOR_TYPE_GRAY;
#ifndef WORDS_BIGENDIAN
	png_set_packswap (png);
#endif
	break;
    case CAIRO_FORMAT_RGB96F:
	bpc = 16;
	png_color_type = PNG_COLOR_TYPE_RGB;
	break;
    case CAIRO_FORMAT_RGBA128F:
	bpc = 16;
	png_color_type = PNG_COLOR_TYPE_RGB_ALPHA;
	break;
    case CAIRO_FORMAT_INVALID:
    case CAIRO_FORMAT_RGB16_565:
    default:
	status = _cairo_error (CAIRO_STATUS_INVALID_FORMAT);
	goto BAIL4;
    }

    png_set_IHDR (png, info,
		  clone->width,
		  clone->height, bpc,
		  png_color_type,
		  PNG_INTERLACE_NONE,
		  PNG_COMPRESSION_TYPE_DEFAULT,
		  PNG_FILTER_TYPE_DEFAULT);

    white.gray = (1 << bpc) - 1;
    white.red = white.blue = white.green = white.gray;
    png_set_bKGD (png, info, &white);

    if (0) { /* XXX extract meta-data from surface (i.e. creation date) */
	png_time pt;

	png_convert_from_time_t (&pt, time (NULL));
	png_set_tIME (png, info, &pt);
    }

    /* We have to call png_write_info() before setting up the write
     * transformation, since it stores data internally in 'png'
     * that is needed for the write transformation functions to work.
     */
    png_write_info (png, info);

#ifndef WORDS_BIGENDIAN
    /* libpng treats 16-bit data as big-endian by default. Swapping the
     * byte-order on little endian ensures the native-endian data can be
     * provided to png_write_image. This does not affect 8-bit data.
     */
    png_set_swap (png);
#endif

    if (png_color_type == PNG_COLOR_TYPE_RGB_ALPHA) {
	if (clone->format != CAIRO_FORMAT_RGBA128F)
	    png_set_write_user_transform_fn (png, unpremultiply_data);
    } else if (png_color_type == PNG_COLOR_TYPE_RGB) {
	if (clone->format != CAIRO_FORMAT_RGB96F)
	    png_set_write_user_transform_fn (png, convert_data_to_bytes);
	png_set_filler (png, 0, PNG_FILLER_AFTER);
    }

    png_write_image (png, rows);
    png_write_end (png, info);

BAIL4:
    png_destroy_write_struct (&png, &info);
BAIL3:
    free (rows);
BAIL2:
    cairo_surface_destroy (&clone->base);
    free (u16_copy);
BAIL1:
    _cairo_surface_release_source_image (surface, image, image_extra);

    return status;
}

static void
stdio_write_func (png_structp png, png_bytep data, png_size_t size)
{
    FILE *fp;

    fp = png_get_io_ptr (png);
    while (size) {
	size_t ret = fwrite (data, 1, size, fp);
	size -= ret;
	data += ret;
	if (size && ferror (fp)) {
	    cairo_status_t *error = png_get_error_ptr (png);
	    if (*error == CAIRO_STATUS_SUCCESS)
		*error = _cairo_error (CAIRO_STATUS_WRITE_ERROR);
	    png_error (png, NULL);
	}
    }
}

/**
 * cairo_surface_write_to_png:
 * @surface: a #cairo_surface_t with pixel contents
 * @filename: the name of a file to write to; on Windows this filename
 *   is encoded in UTF-8.
 *
 * Writes the contents of @surface to a new file @filename as a PNG
 * image.
 *
 * Return value: %CAIRO_STATUS_SUCCESS if the PNG file was written
 * successfully. Otherwise, %CAIRO_STATUS_NO_MEMORY if memory could not
 * be allocated for the operation or
 * %CAIRO_STATUS_SURFACE_TYPE_MISMATCH if the surface does not have
 * pixel contents, or %CAIRO_STATUS_WRITE_ERROR if an I/O error occurs
 * while attempting to write the file, or %CAIRO_STATUS_PNG_ERROR if libpng
 * returned an error.
 *
 * Since: 1.0
 **/
cairo_status_t
cairo_surface_write_to_png (cairo_surface_t	*surface,
			    const char		*filename)
{
    FILE *fp;
    cairo_status_t status;

    if (surface->status)
	return surface->status;

    if (surface->finished)
	return _cairo_error (CAIRO_STATUS_SURFACE_FINISHED);

    status = _cairo_fopen (filename, "wb", &fp);

    if (status != CAIRO_STATUS_SUCCESS)
	return _cairo_error (status);

    if (fp == NULL) {
	switch (errno) {
	case ENOMEM:
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	default:
	    return _cairo_error (CAIRO_STATUS_WRITE_ERROR);
	}
    }

    status = write_png (surface, stdio_write_func, fp);

    if (fclose (fp) && status == CAIRO_STATUS_SUCCESS)
	status = _cairo_error (CAIRO_STATUS_WRITE_ERROR);

    return status;
}

struct png_write_closure_t {
    cairo_write_func_t		 write_func;
    void			*closure;
};

static void
stream_write_func (png_structp png, png_bytep data, png_size_t size)
{
    cairo_status_t status;
    struct png_write_closure_t *png_closure;

    png_closure = png_get_io_ptr (png);
    status = png_closure->write_func (png_closure->closure, data, size);
    if (unlikely (status)) {
	cairo_status_t *error = png_get_error_ptr (png);
	if (*error == CAIRO_STATUS_SUCCESS)
	    *error = status;
	png_error (png, NULL);
    }
}

/**
 * cairo_surface_write_to_png_stream:
 * @surface: a #cairo_surface_t with pixel contents
 * @write_func: a #cairo_write_func_t
 * @closure: closure data for the write function
 *
 * Writes the image surface to the write function.
 *
 * Return value: %CAIRO_STATUS_SUCCESS if the PNG file was written
 * successfully.  Otherwise, %CAIRO_STATUS_NO_MEMORY is returned if
 * memory could not be allocated for the operation,
 * %CAIRO_STATUS_SURFACE_TYPE_MISMATCH if the surface does not have
 * pixel contents, or %CAIRO_STATUS_PNG_ERROR if libpng
 * returned an error.
 *
 * Since: 1.0
 **/
cairo_status_t
cairo_surface_write_to_png_stream (cairo_surface_t	*surface,
				   cairo_write_func_t	write_func,
				   void			*closure)
{
    struct png_write_closure_t png_closure;

    if (surface->status)
	return surface->status;

    if (surface->finished)
	return _cairo_error (CAIRO_STATUS_SURFACE_FINISHED);

    png_closure.write_func = write_func;
    png_closure.closure = closure;

    return write_png (surface, stream_write_func, &png_closure);
}
slim_hidden_def (cairo_surface_write_to_png_stream);

static inline int
multiply_alpha (int alpha, int color)
{
    int temp = (alpha * color) + 0x80;
    return ((temp + (temp >> 8)) >> 8);
}

/* Premultiplies data and converts RGBA bytes => native endian */
static void
premultiply_data (png_structp   png,
                  png_row_infop row_info,
                  png_bytep     data)
{
    unsigned int i;

    for (i = 0; i < row_info->rowbytes; i += 4) {
	uint8_t *base  = &data[i];
	uint8_t  alpha = base[3];
	uint32_t p;

	if (alpha == 0) {
	    p = 0;
	} else {
	    uint8_t  red   = base[0];
	    uint8_t  green = base[1];
	    uint8_t  blue  = base[2];

	    if (alpha != 0xff) {
		red   = multiply_alpha (alpha, red);
		green = multiply_alpha (alpha, green);
		blue  = multiply_alpha (alpha, blue);
	    }
	    p = ((uint32_t)alpha << 24) | (red << 16) | (green << 8) | (blue << 0);
	}
	memcpy (base, &p, sizeof (uint32_t));
    }
}

/* Converts RGBx bytes to native endian xRGB */
static void
convert_bytes_to_data (png_structp png, png_row_infop row_info, png_bytep data)
{
    unsigned int i;

    for (i = 0; i < row_info->rowbytes; i += 4) {
	uint8_t *base  = &data[i];
	uint8_t  red   = base[0];
	uint8_t  green = base[1];
	uint8_t  blue  = base[2];
	uint32_t pixel;

	pixel = (0xffu << 24) | (red << 16) | (green << 8) | (blue << 0);
	memcpy (base, &pixel, sizeof (uint32_t));
    }
}

static cairo_status_t
stdio_read_func (void *closure, unsigned char *data, unsigned int size)
{
    FILE *file = closure;

    while (size) {
	size_t ret;

	ret = fread (data, 1, size, file);
	size -= ret;
	data += ret;

	if (size && (feof (file) || ferror (file)))
	    return _cairo_error (CAIRO_STATUS_READ_ERROR);
    }

    return CAIRO_STATUS_SUCCESS;
}

static void
stream_read_func (png_structp png, png_bytep data, png_size_t size)
{
    cairo_status_t status;
    struct png_read_closure_t *png_closure;

    png_closure = png_get_io_ptr (png);
    status = png_closure->read_func (png_closure->closure, data, size);
    if (unlikely (status)) {
	cairo_status_t *error = png_get_error_ptr (png);
	if (*error == CAIRO_STATUS_SUCCESS)
	    *error = status;
	png_error (png, NULL);
    }

    _cairo_output_stream_write (png_closure->png_data, data, size);
}

static cairo_surface_t *
read_png (struct png_read_closure_t *png_closure)
{
    cairo_surface_t * volatile surface;
    png_struct *png = NULL;
    png_info *info;
    png_byte * volatile data = NULL;
    png_byte ** volatile row_pointers = NULL;
    png_uint_32 png_width, png_height;
    int depth, color_type, interlace, stride;
    unsigned int i;
    cairo_format_t format;
    cairo_status_t status;
    unsigned char *mime_data;
    unsigned long mime_data_length;

    png_closure->png_data = _cairo_memory_stream_create ();

    /* XXX: Perhaps we'll want some other error handlers? */
    png = png_create_read_struct (PNG_LIBPNG_VER_STRING,
                                  &status,
	                          png_simple_error_callback,
	                          png_simple_warning_callback);
    if (unlikely (png == NULL)) {
	surface = _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));
	goto BAIL;
    }

    info = png_create_info_struct (png);
    if (unlikely (info == NULL)) {
	surface = _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));
	goto BAIL;
    }

    png_set_read_fn (png, png_closure, stream_read_func);

    status = CAIRO_STATUS_SUCCESS;
#ifdef PNG_SETJMP_SUPPORTED
    if (setjmp (png_jmpbuf (png))) {
	surface = _cairo_surface_create_in_error (status);
	goto BAIL;
    }
#endif

    png_read_info (png, info);

#ifndef WORDS_BIGENDIAN
    /* libpng treats 16-bit data as big-endian by default. Swapping the
     * byte-order on little endian ensures the native-endian data can be
     * provided to png_read_image. This does not affect 8-bit data.
     */
    png_set_swap (png);
#endif

    png_get_IHDR (png, info,
                  &png_width, &png_height, &depth,
                  &color_type, &interlace, NULL, NULL);
    if (unlikely (status)) { /* catch any early warnings */
	surface = _cairo_surface_create_in_error (status);
	goto BAIL;
    }

    /* convert palette/gray image to rgb */
    if (color_type == PNG_COLOR_TYPE_PALETTE)
        png_set_palette_to_rgb (png);

    /* expand gray bit depth if needed */
    if (color_type == PNG_COLOR_TYPE_GRAY) {
#if PNG_LIBPNG_VER >= 10209
        png_set_expand_gray_1_2_4_to_8 (png);
#else
        png_set_gray_1_2_4_to_8 (png);
#endif
    }

    /* transform transparency to alpha */
    if (png_get_valid (png, info, PNG_INFO_tRNS))
        png_set_tRNS_to_alpha (png);

    if (depth < 8)
        png_set_packing (png);

    /* convert grayscale to RGB */
    if (color_type == PNG_COLOR_TYPE_GRAY ||
	color_type == PNG_COLOR_TYPE_GRAY_ALPHA)
    {
	png_set_gray_to_rgb (png);
    }

    if (interlace != PNG_INTERLACE_NONE)
        png_set_interlace_handling (png);

    png_set_filler (png, 0xff, PNG_FILLER_AFTER);

    /* recheck header after setting EXPAND options */
    png_read_update_info (png, info);
    png_get_IHDR (png, info,
                  &png_width, &png_height, &depth,
                  &color_type, &interlace, NULL, NULL);
    if ((depth != 8 && depth != 16) ||
	! (color_type == PNG_COLOR_TYPE_RGB ||
	   color_type == PNG_COLOR_TYPE_RGB_ALPHA))
    {
	surface = _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_READ_ERROR));
	goto BAIL;
    }

    switch (color_type) {
	default:
	    ASSERT_NOT_REACHED;
	    /* fall-through just in case ;-) */

	case PNG_COLOR_TYPE_RGB_ALPHA:
	    if (depth == 8) {
		format = CAIRO_FORMAT_ARGB32;
		png_set_read_user_transform_fn (png, premultiply_data);
	    } else {
		format = CAIRO_FORMAT_RGBA128F;
	    }
	    break;

	case PNG_COLOR_TYPE_RGB:
	    if (depth == 8) {
		format = CAIRO_FORMAT_RGB24;
		png_set_read_user_transform_fn (png, convert_bytes_to_data);
	    } else {
		format = CAIRO_FORMAT_RGB96F;
	    }
	    break;
    }

    stride = cairo_format_stride_for_width (format, png_width);
    if (stride < 0) {
	surface = _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_INVALID_STRIDE));
	goto BAIL;
    }

    data = _cairo_malloc_ab (png_height, stride);
    if (unlikely (data == NULL)) {
	surface = _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));
	goto BAIL;
    }

    row_pointers = _cairo_malloc_ab (png_height, sizeof (char *));
    if (unlikely (row_pointers == NULL)) {
	surface = _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));
	goto BAIL;
    }

    for (i = 0; i < png_height; i++)
        row_pointers[i] = &data[i * (ptrdiff_t)stride];

    png_read_image (png, row_pointers);
    png_read_end (png, info);

    if (unlikely (status)) { /* catch any late warnings - probably hit an error already */
	surface = _cairo_surface_create_in_error (status);
	goto BAIL;
    }

    if (format == CAIRO_FORMAT_RGBA128F) {
	i = png_height;

	while (i--) {
	    float *float_line = (float *)row_pointers[i];
	    uint16_t *u16_line = (uint16_t *)row_pointers[i];

	    premultiply_float (float_line, u16_line, png_width);
	}
    } else if (format == CAIRO_FORMAT_RGB96F) {
	i = png_height;

	while (i--) {
	    float *float_line = (float *)row_pointers[i];
	    uint16_t *u16_line = (uint16_t *)row_pointers[i];

	    convert_u16_to_float (float_line, u16_line, png_width);
	}
    }

    surface = cairo_image_surface_create_for_data (data, format,
						   png_width, png_height,
						   stride);
    if (surface->status)
	goto BAIL;

    _cairo_image_surface_assume_ownership_of_data ((cairo_image_surface_t*)surface);
    data = NULL;

    _cairo_debug_check_image_surface_is_defined (surface);

    status = _cairo_memory_stream_destroy (png_closure->png_data,
					   &mime_data,
					   &mime_data_length);
    png_closure->png_data = NULL;
    if (unlikely (status)) {
	cairo_surface_destroy (surface);
	surface = _cairo_surface_create_in_error (status);
	goto BAIL;
    }

    status = cairo_surface_set_mime_data (surface,
					  CAIRO_MIME_TYPE_PNG,
					  mime_data,
					  mime_data_length,
					  free,
					  mime_data);
    if (unlikely (status)) {
	free (mime_data);
	cairo_surface_destroy (surface);
	surface = _cairo_surface_create_in_error (status);
	goto BAIL;
    }

 BAIL:
    free (row_pointers);
    free (data);
    if (png != NULL)
	png_destroy_read_struct (&png, &info, NULL);
    if (png_closure->png_data != NULL) {
	cairo_status_t status_ignored;

	status_ignored = _cairo_output_stream_destroy (png_closure->png_data);
    }

    return surface;
}

/**
 * cairo_image_surface_create_from_png:
 * @filename: name of PNG file to load. On Windows this filename
 *   is encoded in UTF-8.
 *
 * Creates a new image surface and initializes the contents to the
 * given PNG file.
 *
 * Return value: a new #cairo_surface_t initialized with the contents
 * of the PNG file, or a "nil" surface if any error occurred. A nil
 * surface can be checked for with cairo_surface_status(surface) which
 * may return one of the following values:
 *
 *	%CAIRO_STATUS_NO_MEMORY
 *	%CAIRO_STATUS_FILE_NOT_FOUND
 *	%CAIRO_STATUS_READ_ERROR
 *	%CAIRO_STATUS_PNG_ERROR
 *
 * Alternatively, you can allow errors to propagate through the drawing
 * operations and check the status on the context upon completion
 * using cairo_status().
 *
 * Since: 1.0
 **/
cairo_surface_t *
cairo_image_surface_create_from_png (const char *filename)
{
    struct png_read_closure_t png_closure;
    cairo_surface_t *surface;
    cairo_status_t status;

    status = _cairo_fopen (filename, "rb", (FILE **) &png_closure.closure);

    if (status != CAIRO_STATUS_SUCCESS)
	return _cairo_surface_create_in_error (status);

    if (png_closure.closure == NULL) {
	switch (errno) {
	case ENOMEM:
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    break;
	case ENOENT:
	    status = _cairo_error (CAIRO_STATUS_FILE_NOT_FOUND);
	    break;
	default:
	    status = _cairo_error (CAIRO_STATUS_READ_ERROR);
	    break;
	}
	return _cairo_surface_create_in_error (status);
    }

    png_closure.read_func = stdio_read_func;

    surface = read_png (&png_closure);

    fclose (png_closure.closure);

    return surface;
}

/**
 * cairo_image_surface_create_from_png_stream:
 * @read_func: function called to read the data of the file
 * @closure: data to pass to @read_func.
 *
 * Creates a new image surface from PNG data read incrementally
 * via the @read_func function.
 *
 * Return value: a new #cairo_surface_t initialized with the contents
 * of the PNG file or a "nil" surface if the data read is not a valid PNG image
 * or memory could not be allocated for the operation.  A nil
 * surface can be checked for with cairo_surface_status(surface) which
 * may return one of the following values:
 *
 *	%CAIRO_STATUS_NO_MEMORY
 *	%CAIRO_STATUS_READ_ERROR
 *	%CAIRO_STATUS_PNG_ERROR
 *
 * Alternatively, you can allow errors to propagate through the drawing
 * operations and check the status on the context upon completion
 * using cairo_status().
 *
 * Since: 1.0
 **/
cairo_surface_t *
cairo_image_surface_create_from_png_stream (cairo_read_func_t	read_func,
					    void		*closure)
{
    struct png_read_closure_t png_closure;

    png_closure.read_func = read_func;
    png_closure.closure = closure;

    return read_png (&png_closure);
}
slim_hidden_def (cairo_image_surface_create_from_png_stream);
