/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2006 Red Hat, Inc
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
 * Contributor(s):
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#define _DEFAULT_SOURCE /* for snprintf(), strdup() */
#include "cairoint.h"

#include "cairo-array-private.h"
#include "cairo-error-private.h"

#if CAIRO_HAS_FONT_SUBSET

#include "cairo-type1-private.h"
#include "cairo-scaled-font-subsets-private.h"
#include "cairo-path-fixed-private.h"
#include "cairo-output-stream-private.h"

typedef enum {
    CAIRO_CHARSTRING_TYPE1,
    CAIRO_CHARSTRING_TYPE2
} cairo_charstring_type_t;

typedef struct _cairo_type1_font {
    int *widths;

    cairo_scaled_font_subset_t *scaled_font_subset;
    cairo_scaled_font_t        *type1_scaled_font;

    cairo_array_t contents;

    double x_min, y_min, x_max, y_max;

    const char    *data;
    unsigned long  header_size;
    unsigned long  data_size;
    unsigned long  trailer_size;
    int            bbox_position;
    int            bbox_max_chars;

    cairo_output_stream_t *output;

    unsigned short eexec_key;
    cairo_bool_t hex_encode;
    int hex_column;
} cairo_type1_font_t;

static cairo_status_t
cairo_type1_font_create (cairo_scaled_font_subset_t  *scaled_font_subset,
                         cairo_type1_font_t         **subset_return,
                         cairo_bool_t                 hex_encode)
{
    cairo_type1_font_t *font;
    cairo_font_face_t *font_face;
    cairo_matrix_t font_matrix;
    cairo_matrix_t ctm;
    cairo_font_options_t font_options;
    cairo_status_t status;

    font = calloc (1, sizeof (cairo_type1_font_t));
    if (unlikely (font == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    font->widths = calloc (scaled_font_subset->num_glyphs, sizeof (int));
    if (unlikely (font->widths == NULL)) {
	free (font);
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    font->scaled_font_subset = scaled_font_subset;
    font->hex_encode = hex_encode;

    font_face = cairo_scaled_font_get_font_face (scaled_font_subset->scaled_font);

    cairo_matrix_init_scale (&font_matrix, 1000, -1000);
    cairo_matrix_init_identity (&ctm);

    _cairo_font_options_init_default (&font_options);
    cairo_font_options_set_hint_style (&font_options, CAIRO_HINT_STYLE_NONE);
    cairo_font_options_set_hint_metrics (&font_options, CAIRO_HINT_METRICS_OFF);

    font->type1_scaled_font = cairo_scaled_font_create (font_face,
							&font_matrix,
							&ctm,
							&font_options);
    status = font->type1_scaled_font->status;
    if (unlikely (status))
        goto fail;

    _cairo_array_init (&font->contents, sizeof (unsigned char));
    font->output = NULL;

    *subset_return = font;

    return CAIRO_STATUS_SUCCESS;

fail:
    free (font->widths);
    free (font);

    return status;
}

/* Charstring commands. If the high byte is 0 the command is encoded
 * with a single byte. */
#define CHARSTRING_sbw        0x0c07
#define CHARSTRING_rmoveto    0x0015
#define CHARSTRING_rlineto    0x0005
#define CHARSTRING_rcurveto   0x0008
#define CHARSTRING_closepath  0x0009
#define CHARSTRING_endchar    0x000e

/* Before calling this function, the caller must allocate sufficient
 * space in data (see _cairo_array_grow_by). The maximum number of
 * bytes that will be used is 2.
 */
static void
charstring_encode_command (cairo_array_t *data, int command)
{
    cairo_status_t status;
    unsigned int orig_size;
    unsigned char buf[5];
    unsigned char *p = buf;

    if (command & 0xff00)
        *p++ = command >> 8;
    *p++ = command & 0x00ff;

    /* Ensure the array doesn't grow, which allows this function to
     * have no possibility of failure. */
    orig_size = _cairo_array_size (data);
    status = _cairo_array_append_multiple (data, buf, p - buf);

    assert (status == CAIRO_STATUS_SUCCESS);
    assert (_cairo_array_size (data) == orig_size);
}

/* Before calling this function, the caller must allocate sufficient
 * space in data (see _cairo_array_grow_by). The maximum number of
 * bytes that will be used is 5.
 */
static void
charstring_encode_integer (cairo_array_t *data,
                           int i,
                           cairo_charstring_type_t type)
{
    cairo_status_t status;
    unsigned int orig_size;
    unsigned char buf[10];
    unsigned char *p = buf;

    if (i >= -107 && i <= 107) {
        *p++ = i + 139;
    } else if (i >= 108 && i <= 1131) {
        i -= 108;
        *p++ = (i >> 8)+ 247;
        *p++ = i & 0xff;
    } else if (i >= -1131 && i <= -108) {
        i = -i - 108;
        *p++ = (i >> 8)+ 251;
        *p++ = i & 0xff;
    } else {
        if (type == CAIRO_CHARSTRING_TYPE1) {
            *p++ = 0xff;
            *p++ = i >> 24;
            *p++ = (i >> 16) & 0xff;
            *p++ = (i >> 8)  & 0xff;
            *p++ = i & 0xff;
        } else {
            *p++ = 0xff;
            *p++ = (i >> 8)  & 0xff;
            *p++ = i & 0xff;
            *p++ = 0;
            *p++ = 0;
        }
    }

    /* Ensure the array doesn't grow, which allows this function to
     * have no possibility of failure. */
    orig_size = _cairo_array_size (data);
    status = _cairo_array_append_multiple (data, buf, p - buf);

    assert (status == CAIRO_STATUS_SUCCESS);
    assert (_cairo_array_size (data) == orig_size);
}

typedef struct _ps_path_info {
    cairo_array_t *data;
    int current_x, current_y;
    cairo_charstring_type_t type;
} t1_path_info_t;

static cairo_status_t
_charstring_move_to (void		    *closure,
                     const cairo_point_t    *point)
{
    t1_path_info_t *path_info = (t1_path_info_t *) closure;
    int dx, dy;
    cairo_status_t status;

    status = _cairo_array_grow_by (path_info->data, 12);
    if (unlikely (status))
        return status;

    dx = _cairo_fixed_integer_part (point->x) - path_info->current_x;
    dy = _cairo_fixed_integer_part (point->y) - path_info->current_y;
    charstring_encode_integer (path_info->data, dx, path_info->type);
    charstring_encode_integer (path_info->data, dy, path_info->type);
    path_info->current_x += dx;
    path_info->current_y += dy;

    charstring_encode_command (path_info->data, CHARSTRING_rmoveto);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_charstring_line_to (void		    *closure,
                     const cairo_point_t    *point)
{
    t1_path_info_t *path_info = (t1_path_info_t *) closure;
    int dx, dy;
    cairo_status_t status;

    status = _cairo_array_grow_by (path_info->data, 12);
    if (unlikely (status))
        return status;

    dx = _cairo_fixed_integer_part (point->x) - path_info->current_x;
    dy = _cairo_fixed_integer_part (point->y) - path_info->current_y;
    charstring_encode_integer (path_info->data, dx, path_info->type);
    charstring_encode_integer (path_info->data, dy, path_info->type);
    path_info->current_x += dx;
    path_info->current_y += dy;

    charstring_encode_command (path_info->data, CHARSTRING_rlineto);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_charstring_curve_to (void		    *closure,
                      const cairo_point_t   *point1,
                      const cairo_point_t   *point2,
                      const cairo_point_t   *point3)
{
    t1_path_info_t *path_info = (t1_path_info_t *) closure;
    int dx1, dy1, dx2, dy2, dx3, dy3;
    cairo_status_t status;

    status = _cairo_array_grow_by (path_info->data, 32);
    if (unlikely (status))
        return status;

    dx1 = _cairo_fixed_integer_part (point1->x) - path_info->current_x;
    dy1 = _cairo_fixed_integer_part (point1->y) - path_info->current_y;
    dx2 = _cairo_fixed_integer_part (point2->x) - path_info->current_x - dx1;
    dy2 = _cairo_fixed_integer_part (point2->y) - path_info->current_y - dy1;
    dx3 = _cairo_fixed_integer_part (point3->x) - path_info->current_x - dx1 - dx2;
    dy3 = _cairo_fixed_integer_part (point3->y) - path_info->current_y - dy1 - dy2;
    charstring_encode_integer (path_info->data, dx1, path_info->type);
    charstring_encode_integer (path_info->data, dy1, path_info->type);
    charstring_encode_integer (path_info->data, dx2, path_info->type);
    charstring_encode_integer (path_info->data, dy2, path_info->type);
    charstring_encode_integer (path_info->data, dx3, path_info->type);
    charstring_encode_integer (path_info->data, dy3, path_info->type);
    path_info->current_x += dx1 + dx2 + dx3;
    path_info->current_y += dy1 + dy2 + dy3;
    charstring_encode_command (path_info->data, CHARSTRING_rcurveto);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_charstring_close_path (void *closure)
{
    cairo_status_t status;
    t1_path_info_t *path_info = (t1_path_info_t *) closure;

    if (path_info->type == CAIRO_CHARSTRING_TYPE2)
        return CAIRO_STATUS_SUCCESS;

    status = _cairo_array_grow_by (path_info->data, 2);
    if (unlikely (status))
	return status;

    charstring_encode_command (path_info->data, CHARSTRING_closepath);

    return CAIRO_STATUS_SUCCESS;
}

static void
charstring_encrypt (cairo_array_t *data)
{
    unsigned char *d, *end;
    uint16_t c, p, r;

    r = CAIRO_TYPE1_CHARSTRING_KEY;
    d = (unsigned char *) _cairo_array_index (data, 0);
    end = d + _cairo_array_num_elements (data);
    while (d < end) {
	p = *d;
	c = p ^ (r >> 8);
	r = (c + r) * CAIRO_TYPE1_ENCRYPT_C1 + CAIRO_TYPE1_ENCRYPT_C2;
        *d++ = c;
    }
}

static cairo_int_status_t
cairo_type1_font_create_charstring (cairo_type1_font_t      *font,
                                    int                      subset_index,
                                    int                      glyph_index,
                                    cairo_charstring_type_t  type,
                                    cairo_array_t           *data)
{
    cairo_int_status_t status;
    cairo_scaled_glyph_t *scaled_glyph;
    t1_path_info_t path_info;
    cairo_text_extents_t *metrics;
    cairo_bool_t emit_path = TRUE;

    /* This call may return CAIRO_INT_STATUS_UNSUPPORTED for bitmap fonts. */
    status = _cairo_scaled_glyph_lookup (font->type1_scaled_font,
					 glyph_index,
					 CAIRO_SCALED_GLYPH_INFO_METRICS|
					 CAIRO_SCALED_GLYPH_INFO_PATH,
					 NULL, /* foreground color */
					 &scaled_glyph);

    /* It is ok for the .notdef glyph to not have a path available. We
     * just need the metrics to emit an empty glyph.  */
    if (glyph_index == 0 && status == CAIRO_INT_STATUS_UNSUPPORTED) {
	emit_path = FALSE;
	status = _cairo_scaled_glyph_lookup (font->type1_scaled_font,
					     glyph_index,
					     CAIRO_SCALED_GLYPH_INFO_METRICS,
                                             NULL, /* foreground color */
					     &scaled_glyph);
    }
    if (unlikely (status))
        return status;

    metrics = &scaled_glyph->metrics;
    if (subset_index == 0) {
        font->x_min = metrics->x_bearing;
        font->y_min = metrics->y_bearing;
        font->x_max = metrics->x_bearing + metrics->width;
        font->y_max = metrics->y_bearing + metrics->height;
    } else {
        if (metrics->x_bearing < font->x_min)
            font->x_min = metrics->x_bearing;
        if (metrics->y_bearing < font->y_min)
            font->y_min = metrics->y_bearing;
        if (metrics->x_bearing + metrics->width > font->x_max)
            font->x_max = metrics->x_bearing + metrics->width;
        if (metrics->y_bearing + metrics->height > font->y_max)
            font->y_max = metrics->y_bearing + metrics->height;
    }
    font->widths[subset_index] = metrics->x_advance;

    status = _cairo_array_grow_by (data, 30);
    if (unlikely (status))
        return status;

    if (type == CAIRO_CHARSTRING_TYPE1) {
        charstring_encode_integer (data, (int) scaled_glyph->metrics.x_bearing, type);
        charstring_encode_integer (data, (int) scaled_glyph->metrics.y_bearing, type);
        charstring_encode_integer (data, (int) scaled_glyph->metrics.x_advance, type);
        charstring_encode_integer (data, (int) scaled_glyph->metrics.y_advance, type);
        charstring_encode_command (data, CHARSTRING_sbw);

        path_info.current_x = (int) scaled_glyph->metrics.x_bearing;
        path_info.current_y = (int) scaled_glyph->metrics.y_bearing;
    } else {
        charstring_encode_integer (data, (int) scaled_glyph->metrics.x_advance, type);

        path_info.current_x = 0;
        path_info.current_y = 0;
    }
    path_info.data = data;
    path_info.type = type;
    if (emit_path) {
	status = _cairo_path_fixed_interpret (scaled_glyph->path,
					      _charstring_move_to,
					      _charstring_line_to,
					      _charstring_curve_to,
					      _charstring_close_path,
					      &path_info);
	if (unlikely (status))
	    return status;
    }

    status = _cairo_array_grow_by (data, 1);
    if (unlikely (status))
        return status;
    charstring_encode_command (path_info.data, CHARSTRING_endchar);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
cairo_type1_font_write_charstrings (cairo_type1_font_t    *font,
                                    cairo_output_stream_t *encrypted_output)
{
    cairo_status_t status;
    unsigned char zeros[] = { 0, 0, 0, 0 };
    cairo_array_t data;
    unsigned int i;
    int length;

    _cairo_array_init (&data, sizeof (unsigned char));
    status = _cairo_array_grow_by (&data, 1024);
    if (unlikely (status))
        goto fail;

    _cairo_output_stream_printf (encrypted_output,
                                 "2 index /CharStrings %d dict dup begin\n",
                                 font->scaled_font_subset->num_glyphs + 1);

    _cairo_scaled_font_freeze_cache (font->type1_scaled_font);
    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++) {
        _cairo_array_truncate (&data, 0);
        /* four "random" bytes required by encryption algorithm */
        status = _cairo_array_append_multiple (&data, zeros, 4);
        if (unlikely (status))
	    break;

        status = cairo_type1_font_create_charstring (font, i,
						     font->scaled_font_subset->glyphs[i],
                                                     CAIRO_CHARSTRING_TYPE1,
						     &data);
        if (unlikely (status))
	    break;

        charstring_encrypt (&data);
        length = _cairo_array_num_elements (&data);
	if (font->scaled_font_subset->glyph_names != NULL) {
	    _cairo_output_stream_printf (encrypted_output, "/%s %d RD ",
					 font->scaled_font_subset->glyph_names[i],
					 length);
	} else if (i == 0) {
	    _cairo_output_stream_printf (encrypted_output, "/.notdef %d RD ", length);
	} else {
	    _cairo_output_stream_printf (encrypted_output, "/g%d %d RD ", i, length);
	}
        _cairo_output_stream_write (encrypted_output,
                                    _cairo_array_index (&data, 0),
                                    length);
        _cairo_output_stream_printf (encrypted_output, " ND\n");
    }
    _cairo_scaled_font_thaw_cache (font->type1_scaled_font);

fail:
    _cairo_array_fini (&data);
    return status;
}

static void
cairo_type1_font_write_header (cairo_type1_font_t *font,
                               const char         *name)
{
    unsigned int i;
    const char spaces[50] = "                                                  ";

    _cairo_output_stream_printf (font->output,
                                 "%%!FontType1-1.1 %s 1.0\n"
                                 "11 dict begin\n"
                                 "/FontName /%s def\n"
                                 "/PaintType 0 def\n"
                                 "/FontType 1 def\n"
                                  "/FontMatrix [0.001 0 0 0.001 0 0] readonly def\n",
                                 name,
                                 name);

    /* We don't know the bbox values until after the charstrings have
     * been generated.  Reserve some space and fill in the bbox
     * later. */

    /* Worst case for four signed ints with spaces between each number */
    font->bbox_max_chars = 50;

    _cairo_output_stream_printf (font->output, "/FontBBox {");
    font->bbox_position = _cairo_output_stream_get_position (font->output);
    _cairo_output_stream_write (font->output, spaces, font->bbox_max_chars);

    _cairo_output_stream_printf (font->output,
                                 "} readonly def\n"
                                 "/Encoding 256 array\n"
				 "0 1 255 {1 index exch /.notdef put} for\n");
    if (font->scaled_font_subset->is_latin) {
	for (i = 1; i < 256; i++) {
	    int subset_glyph = font->scaled_font_subset->latin_to_subset_glyph_index[i];

	    if (subset_glyph > 0) {
		if (font->scaled_font_subset->glyph_names != NULL) {
		    _cairo_output_stream_printf (font->output, "dup %d /%s put\n",
						 i, font->scaled_font_subset->glyph_names[subset_glyph]);
		} else {
		    _cairo_output_stream_printf (font->output, "dup %d /g%d put\n", i, subset_glyph);
		}
	    }
	}
    } else {
	for (i = 1; i < font->scaled_font_subset->num_glyphs; i++) {
	    if (font->scaled_font_subset->glyph_names != NULL) {
		_cairo_output_stream_printf (font->output, "dup %d /%s put\n",
					     i, font->scaled_font_subset->glyph_names[i]);
	    } else {
		_cairo_output_stream_printf (font->output, "dup %d /g%d put\n", i, i);
	    }
	}
    }
    _cairo_output_stream_printf (font->output,
                                 "readonly def\n"
                                 "currentdict end\n"
                                 "currentfile eexec\n");
}

static cairo_status_t
cairo_type1_write_stream_encrypted (void                *closure,
                                    const unsigned char *data,
                                    unsigned int         length)
{
    const unsigned char *in, *end;
    uint16_t c, p;
    static const char hex_digits[16] = "0123456789abcdef";
    char digits[3];
    cairo_type1_font_t *font = closure;

    in = (const unsigned char *) data;
    end = (const unsigned char *) data + length;
    while (in < end) {
	p = *in++;
	c = p ^ (font->eexec_key >> 8);
	font->eexec_key = (c + font->eexec_key) * CAIRO_TYPE1_ENCRYPT_C1 + CAIRO_TYPE1_ENCRYPT_C2;

	if (font->hex_encode) {
	    digits[0] = hex_digits[c >> 4];
	    digits[1] = hex_digits[c & 0x0f];
	    digits[2] = '\n';
	    font->hex_column += 2;

	    if (font->hex_column == 78) {
		_cairo_output_stream_write (font->output, digits, 3);
		font->hex_column = 0;
	    } else {
		_cairo_output_stream_write (font->output, digits, 2);
	    }
	} else {
	    digits[0] = c;
	    _cairo_output_stream_write (font->output, digits, 1);
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
cairo_type1_font_write_private_dict (cairo_type1_font_t *font,
                                     const char         *name)
{
    cairo_int_status_t status;
    cairo_status_t status2;
    cairo_output_stream_t *encrypted_output;

    font->eexec_key = CAIRO_TYPE1_PRIVATE_DICT_KEY;
    font->hex_column = 0;
    encrypted_output = _cairo_output_stream_create (
        cairo_type1_write_stream_encrypted,
        NULL,
        font);
    if (_cairo_output_stream_get_status (encrypted_output))
	return  _cairo_output_stream_destroy (encrypted_output);

    /* Note: the first four spaces at the start of this private dict
     * are the four "random" bytes of plaintext required by the
     * encryption algorithm */
    _cairo_output_stream_printf (encrypted_output,
                                 "    dup /Private 9 dict dup begin\n"
                                 "/RD {string currentfile exch readstring pop}"
                                 " bind executeonly def\n"
                                 "/ND {noaccess def} executeonly def\n"
                                 "/NP {noaccess put} executeonly def\n"
                                 "/BlueValues [] def\n"
                                 "/MinFeature {16 16} def\n"
                                 "/lenIV 4 def\n"
                                 "/password 5839 def\n");

    status = cairo_type1_font_write_charstrings (font, encrypted_output);
    if (unlikely (status))
	goto fail;

    _cairo_output_stream_printf (encrypted_output,
                                 "end\n"
                                 "end\n"
                                 "readonly put\n"
                                 "noaccess put\n"
                                 "dup /FontName get exch definefont pop\n"
                                 "mark currentfile closefile\n");

  fail:
    status2 = _cairo_output_stream_destroy (encrypted_output);
    if (status == CAIRO_INT_STATUS_SUCCESS)
	status = status2;

    return status;
}

static void
cairo_type1_font_write_trailer(cairo_type1_font_t *font)
{
    int i;
    static const char zeros[65] =
	"0000000000000000000000000000000000000000000000000000000000000000\n";

    for (i = 0; i < 8; i++)
	_cairo_output_stream_write (font->output, zeros, sizeof zeros);

    _cairo_output_stream_printf (font->output, "cleartomark\n");
}

static cairo_status_t
cairo_type1_write_stream (void *closure,
                         const unsigned char *data,
                         unsigned int length)
{
    cairo_type1_font_t *font = closure;

    return _cairo_array_append_multiple (&font->contents, data, length);
}

static cairo_int_status_t
cairo_type1_font_write (cairo_type1_font_t *font,
                        const char *name)
{
    cairo_int_status_t status;

    cairo_type1_font_write_header (font, name);
    font->header_size = _cairo_output_stream_get_position (font->output);

    status = cairo_type1_font_write_private_dict (font, name);
    if (unlikely (status))
	return status;

    font->data_size = _cairo_output_stream_get_position (font->output) -
	font->header_size;

    cairo_type1_font_write_trailer (font);
    font->trailer_size =
	_cairo_output_stream_get_position (font->output) -
	font->header_size - font->data_size;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
cairo_type1_font_generate (cairo_type1_font_t *font, const char *name)
{
    cairo_int_status_t status;

    status = _cairo_array_grow_by (&font->contents, 4096);
    if (unlikely (status))
	return status;

    font->output = _cairo_output_stream_create (cairo_type1_write_stream, NULL, font);
    if (_cairo_output_stream_get_status (font->output))
	return _cairo_output_stream_destroy (font->output);

    status = cairo_type1_font_write (font, name);
    if (unlikely (status))
	return status;

    font->data = _cairo_array_index (&font->contents, 0);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_type1_font_destroy (cairo_type1_font_t *font)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    free (font->widths);
    cairo_scaled_font_destroy (font->type1_scaled_font);
    _cairo_array_fini (&font->contents);
    if (font->output)
	status = _cairo_output_stream_destroy (font->output);
    free (font);

    return status;
}

static cairo_status_t
_cairo_type1_fallback_init_internal (cairo_type1_subset_t	*type1_subset,
                                     const char			*name,
                                     cairo_scaled_font_subset_t	*scaled_font_subset,
                                     cairo_bool_t                hex_encode)
{
    cairo_type1_font_t *font;
    cairo_status_t status;
    unsigned long length;
    unsigned int i, len;

    status = cairo_type1_font_create (scaled_font_subset, &font, hex_encode);
    if (unlikely (status))
	return status;

    status = cairo_type1_font_generate (font, name);
    if (unlikely (status))
	goto fail1;

    type1_subset->base_font = strdup (name);
    if (unlikely (type1_subset->base_font == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail1;
    }

    type1_subset->widths = calloc (sizeof (double), font->scaled_font_subset->num_glyphs);
    if (unlikely (type1_subset->widths == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail2;
    }
    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++)
	type1_subset->widths[i] = (double)font->widths[i]/1000;

    type1_subset->x_min   = (double)font->x_min/1000;
    type1_subset->y_min   = (double)font->y_min/1000;
    type1_subset->x_max   = (double)font->x_max/1000;
    type1_subset->y_max   = (double)font->y_max/1000;
    type1_subset->ascent  = (double)font->y_max/1000;
    type1_subset->descent = (double)font->y_min/1000;

    length = font->header_size + font->data_size +
	font->trailer_size;
    type1_subset->data = _cairo_malloc (length);
    if (unlikely (type1_subset->data == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail3;
    }
    memcpy (type1_subset->data,
	    _cairo_array_index (&font->contents, 0), length);

    len = snprintf(type1_subset->data + font->bbox_position,
                   font->bbox_max_chars,
                   "%d %d %d %d",
                   (int)font->x_min,
                   (int)font->y_min,
                   (int)font->x_max,
                   (int)font->y_max);
    type1_subset->data[font->bbox_position + len] = ' ';

    type1_subset->header_length = font->header_size;
    type1_subset->data_length = font->data_size;
    type1_subset->trailer_length = font->trailer_size;

    return cairo_type1_font_destroy (font);

 fail3:
    free (type1_subset->widths);
 fail2:
    free (type1_subset->base_font);
 fail1:
    /* status is already set, ignore further errors */
    cairo_type1_font_destroy (font);

    return status;
}

cairo_status_t
_cairo_type1_fallback_init_binary (cairo_type1_subset_t	      *type1_subset,
                                   const char		      *name,
                                   cairo_scaled_font_subset_t *scaled_font_subset)
{
    return _cairo_type1_fallback_init_internal (type1_subset,
                                                name,
                                                scaled_font_subset, FALSE);
}

cairo_status_t
_cairo_type1_fallback_init_hex (cairo_type1_subset_t	   *type1_subset,
                                const char		   *name,
                                cairo_scaled_font_subset_t *scaled_font_subset)
{
    return _cairo_type1_fallback_init_internal (type1_subset,
                                                name,
                                                scaled_font_subset, TRUE);
}

void
_cairo_type1_fallback_fini (cairo_type1_subset_t *subset)
{
    free (subset->base_font);
    free (subset->widths);
    free (subset->data);
}

cairo_status_t
_cairo_type2_charstrings_init (cairo_type2_charstrings_t *type2_subset,
                               cairo_scaled_font_subset_t *scaled_font_subset)
{
    cairo_type1_font_t *font;
    cairo_status_t status;
    unsigned int i;
    cairo_array_t charstring;

    status = cairo_type1_font_create (scaled_font_subset, &font, FALSE);
    if (unlikely (status))
	return status;

    _cairo_array_init (&type2_subset->charstrings, sizeof (cairo_array_t));

    type2_subset->widths = calloc (sizeof (int), font->scaled_font_subset->num_glyphs);
    if (unlikely (type2_subset->widths == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail1;
    }

    _cairo_scaled_font_freeze_cache (font->type1_scaled_font);
    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++) {
        _cairo_array_init (&charstring, sizeof (unsigned char));
        status = _cairo_array_grow_by (&charstring, 32);
        if (unlikely (status))
            goto fail2;

	status = cairo_type1_font_create_charstring (font, i,
						     font->scaled_font_subset->glyphs[i],
						     CAIRO_CHARSTRING_TYPE2,
						     &charstring);
        if (unlikely (status))
            goto fail2;

        status = _cairo_array_append (&type2_subset->charstrings, &charstring);
        if (unlikely (status))
            goto fail2;
    }
    _cairo_scaled_font_thaw_cache (font->type1_scaled_font);

    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++)
	type2_subset->widths[i] = font->widths[i];

    type2_subset->x_min   = (int) font->x_min;
    type2_subset->y_min   = (int) font->y_min;
    type2_subset->x_max   = (int) font->x_max;
    type2_subset->y_max   = (int) font->y_max;
    type2_subset->ascent  = (int) font->y_max;
    type2_subset->descent = (int) font->y_min;

    return cairo_type1_font_destroy (font);

fail2:
    _cairo_scaled_font_thaw_cache (font->type1_scaled_font);
    _cairo_array_fini (&charstring);
    _cairo_type2_charstrings_fini (type2_subset);
fail1:
    cairo_type1_font_destroy (font);
    return status;
}

void
_cairo_type2_charstrings_fini (cairo_type2_charstrings_t *type2_subset)
{
    unsigned int i, num_charstrings;
    cairo_array_t *charstring;

    num_charstrings = _cairo_array_num_elements (&type2_subset->charstrings);
    for (i = 0; i < num_charstrings; i++) {
        charstring = _cairo_array_index (&type2_subset->charstrings, i);
        _cairo_array_fini (charstring);
    }
    _cairo_array_fini (&type2_subset->charstrings);

    free (type2_subset->widths);
}

#endif /* CAIRO_HAS_FONT_SUBSET */
