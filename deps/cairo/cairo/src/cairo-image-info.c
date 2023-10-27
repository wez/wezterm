/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2008 Adrian Johnson
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
 * Contributor(s):
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#include "cairoint.h"

#include "cairo-error-private.h"
#include "cairo-image-info-private.h"

/* JPEG (image/jpeg)
 *
 * http://www.w3.org/Graphics/JPEG/itu-t81.pdf
 */

/* Markers with no parameters. All other markers are followed by a two
 * byte length of the parameters. */
#define TEM       0x01
#define RST_begin 0xd0
#define RST_end   0xd7
#define SOI       0xd8
#define EOI       0xd9

/* Start of frame markers. */
#define SOF0  0xc0
#define SOF1  0xc1
#define SOF2  0xc2
#define SOF3  0xc3
#define SOF5  0xc5
#define SOF6  0xc6
#define SOF7  0xc7
#define SOF9  0xc9
#define SOF10 0xca
#define SOF11 0xcb
#define SOF13 0xcd
#define SOF14 0xce
#define SOF15 0xcf

static const unsigned char *
_jpeg_skip_segment (const unsigned char *p)
{
    int len;

    p++;
    len = (p[0] << 8) | p[1];

    return p + len;
}

static void
_jpeg_extract_info (cairo_image_info_t *info, const unsigned char *p)
{
    info->width = (p[6] << 8) + p[7];
    info->height = (p[4] << 8) + p[5];
    info->num_components = p[8];
    info->bits_per_component = p[3];
}

cairo_int_status_t
_cairo_image_info_get_jpeg_info (cairo_image_info_t	*info,
				 const unsigned char	*data,
				 unsigned long		 length)
{
    const unsigned char *p = data;

    while (p + 1 < data + length) {
	if (*p != 0xff)
	    return CAIRO_INT_STATUS_UNSUPPORTED;
	p++;

	switch (*p) {
	    /* skip fill bytes */
	case 0xff:
	    p++;
	    break;

	case TEM:
	case SOI:
	case EOI:
	    p++;
	    break;

	case SOF0:
	case SOF1:
	case SOF2:
	case SOF3:
	case SOF5:
	case SOF6:
	case SOF7:
	case SOF9:
	case SOF10:
	case SOF11:
	case SOF13:
	case SOF14:
	case SOF15:
	    /* Start of frame found. Extract the image parameters. */
	    if (p + 8 > data + length)
		return CAIRO_INT_STATUS_UNSUPPORTED;

	    _jpeg_extract_info (info, p);
	    return CAIRO_STATUS_SUCCESS;

	default:
	    if (*p >= RST_begin && *p <= RST_end) {
		p++;
		break;
	    }

	    if (p + 3 > data + length)
		return CAIRO_INT_STATUS_UNSUPPORTED;

	    p = _jpeg_skip_segment (p);
	    break;
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

/* JPEG 2000 (image/jp2)
 *
 * http://www.jpeg.org/public/15444-1annexi.pdf
 */

#define JPX_FILETYPE 0x66747970
#define JPX_JP2_HEADER 0x6A703268
#define JPX_IMAGE_HEADER 0x69686472

static const unsigned char _jpx_signature[] = {
    0x00, 0x00, 0x00, 0x0c, 0x6a, 0x50, 0x20, 0x20, 0x0d, 0x0a, 0x87, 0x0a
};

static const unsigned char *
_jpx_next_box (const unsigned char *p, const unsigned char *end)
{
    if (p + 4 < end) {
	uint32_t length = get_unaligned_be32 (p);
	if (p + length < end)
	    return p + length;
    }

    return end;
}

static const unsigned char *
_jpx_get_box_contents (const unsigned char *p)
{
    return p + 8;
}

static cairo_bool_t
_jpx_match_box (const unsigned char *p, const unsigned char *end, uint32_t type)
{
    uint32_t length;

    if (p + 8 < end) {
	length = get_unaligned_be32 (p);
	if (get_unaligned_be32 (p + 4) == type &&  p + length < end)
	    return TRUE;
    }

    return FALSE;
}

static const unsigned char *
_jpx_find_box (const unsigned char *p, const unsigned char *end, uint32_t type)
{
    while (p < end) {
	if (_jpx_match_box (p, end, type))
	    return p;
	p = _jpx_next_box (p, end);
    }

    return NULL;
}

static cairo_int_status_t
_jpx_extract_info (const unsigned char *p, cairo_image_info_t *info, const unsigned char *end)
{
    if (p + 11 >= end) {
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    info->height = get_unaligned_be32 (p);
    info->width = get_unaligned_be32 (p + 4);
    info->num_components = (p[8] << 8) + p[9];
    info->bits_per_component = p[10];

    return CAIRO_STATUS_SUCCESS;
}

cairo_int_status_t
_cairo_image_info_get_jpx_info (cairo_image_info_t	*info,
				const unsigned char	*data,
				unsigned long		 length)
{
    const unsigned char *p = data;
    const unsigned char *end = data + length;

    /* First 12 bytes must be the JPEG 2000 signature box. */
    if (length < ARRAY_LENGTH(_jpx_signature) ||
	memcmp(p, _jpx_signature, ARRAY_LENGTH(_jpx_signature)) != 0)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    p += ARRAY_LENGTH(_jpx_signature);

    /* Next box must be a File Type Box */
    if (! _jpx_match_box (p, end, JPX_FILETYPE))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    p = _jpx_next_box (p, end);

    /* Locate the JP2 header box. */
    p = _jpx_find_box (p, end, JPX_JP2_HEADER);
    if (!p)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    /* Step into the JP2 header box. First box must be the Image
     * Header */
    p = _jpx_get_box_contents (p);
    if (! _jpx_match_box (p, end, JPX_IMAGE_HEADER))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    /* Get the image info */
    p = _jpx_get_box_contents (p);
    return _jpx_extract_info (p, info, end);
}

/* PNG (image/png)
 *
 * http://www.w3.org/TR/2003/REC-PNG-20031110/
 */

#define PNG_IHDR 0x49484452

static const unsigned char _png_magic[8] = { 137, 80, 78, 71, 13, 10, 26, 10 };

cairo_int_status_t
_cairo_image_info_get_png_info (cairo_image_info_t     *info,
                               const unsigned char     *data,
                               unsigned long            length)
{
    const unsigned char *p = data;
    const unsigned char *end = data + length;

    if (length < 8 || memcmp (data, _png_magic, 8) != 0)
       return CAIRO_INT_STATUS_UNSUPPORTED;

    p += 8;

    /* The first chunk must be IDHR. IDHR has 13 bytes of data plus
     * the 12 bytes of overhead for the chunk. */
    if (p + 13 + 12 > end)
       return CAIRO_INT_STATUS_UNSUPPORTED;

    p += 4;
    if (get_unaligned_be32 (p) != PNG_IHDR)
       return CAIRO_INT_STATUS_UNSUPPORTED;

    p += 4;
    info->width = get_unaligned_be32 (p);
    p += 4;
    info->height = get_unaligned_be32 (p);

    return CAIRO_STATUS_SUCCESS;
}

static const unsigned char *
_jbig2_find_data_end (const unsigned char *p,
		      const unsigned char *end,
		      int                  type)
{
    unsigned char end_seq[2];
    int mmr;

    /* Segments of type "Immediate generic region" may have an
     * unspecified data length.  The JBIG2 specification specifies the
     * method to find the end of the data for these segments. */
    if (type == 36 || type == 38 || type == 39) {
	if (p + 18 < end) {
	    mmr = p[17] & 0x01;
	    if (mmr) {
		/* MMR encoding ends with 0x00, 0x00 */
		end_seq[0] = 0x00;
		end_seq[1] = 0x00;
	    } else {
		/* Template encoding ends with 0xff, 0xac */
		end_seq[0] = 0xff;
		end_seq[1] = 0xac;
	    }
	    p += 18;
	    while (p < end) {
		if (p[0] == end_seq[0] && p[1] == end_seq[1]) {
		    /* Skip the 2 terminating bytes and the 4 byte row count that follows. */
		    p += 6;
		    if (p < end)
			return p;
		}
		p++;
	    }
	}
    }

    return NULL;
}

static const unsigned char *
_jbig2_get_next_segment (const unsigned char  *p,
			 const unsigned char  *end,
			 int                  *type,
			 const unsigned char **data,
			 unsigned long        *data_len)
{
    unsigned long seg_num;
    cairo_bool_t big_page_size;
    int num_segs;
    int ref_seg_bytes;
    int referred_size;

    if (p + 6 >= end)
	return NULL;

    seg_num = get_unaligned_be32 (p);
    *type = p[4] & 0x3f;
    big_page_size = (p[4] & 0x40) != 0;
    p += 5;

    num_segs = p[0] >> 5;
    if (num_segs == 7) {
	if (p + 4 >= end)
	    return NULL;
	num_segs = get_unaligned_be32 (p) & 0x1fffffff;
	ref_seg_bytes = 4 + ((num_segs + 1)/8);
    } else {
	ref_seg_bytes = 1;
    }
    p += ref_seg_bytes;

    if (seg_num <= 256)
	referred_size = 1;
    else if (seg_num <= 65536)
	referred_size = 2;
    else
	referred_size = 4;

    p += num_segs * referred_size;
    p += big_page_size ? 4 : 1;
    if (p + 4 >= end)
	return NULL;

    *data_len = get_unaligned_be32 (p);
    p += 4;
    *data = p;

    if (*data_len == 0xffffffff) {
	/* if data length is -1 we have to scan through the data to find the end */
	p = _jbig2_find_data_end (*data, end, *type);
	if (!p || p >= end)
	    return NULL;

	*data_len = p - *data;
    } else {
	p += *data_len;
    }

    if (p < end)
	return p;
    else
	return NULL;
}

static void
_jbig2_extract_info (cairo_image_info_t *info, const unsigned char *p)
{
    info->width = get_unaligned_be32 (p);
    info->height = get_unaligned_be32 (p + 4);
    info->num_components = 1;
    info->bits_per_component = 1;
}

cairo_int_status_t
_cairo_image_info_get_jbig2_info (cairo_image_info_t	*info,
				  const unsigned char	*data,
				  unsigned long		 length)
{
    const unsigned char *p = data;
    const unsigned char *end = data + length;
    int seg_type;
    const unsigned char *seg_data;
    unsigned long seg_data_len;

    while (p && p < end) {
	p = _jbig2_get_next_segment (p, end, &seg_type, &seg_data, &seg_data_len);
	if (p && seg_type == 48 && seg_data_len > 8) {
	    /* page information segment */
	    _jbig2_extract_info (info, seg_data);
	    return CAIRO_STATUS_SUCCESS;
	}
    }

    return CAIRO_INT_STATUS_UNSUPPORTED;
}
