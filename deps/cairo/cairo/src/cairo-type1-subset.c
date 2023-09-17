/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
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
 *	Kristian Høgsberg <krh@redhat.com>
 */

/*
 * Useful links:
 * http://partners.adobe.com/public/developer/en/font/T1_SPEC.PDF
 */


#define _DEFAULT_SOURCE /* for snprintf(), strdup() */
#include "cairoint.h"

#include "cairo-array-private.h"
#include "cairo-error-private.h"

#if CAIRO_HAS_FONT_SUBSET

#include "cairo-type1-private.h"
#include "cairo-scaled-font-subsets-private.h"
#include "cairo-output-stream-private.h"

#include <ctype.h>

#define TYPE1_STACKSIZE 24 /* Defined in Type 1 Font Format */


typedef struct {
    int subset_index;
    double width;
    const char *encrypted_charstring;
    int encrypted_charstring_length;
} glyph_data_t;

typedef struct _cairo_type1_font_subset {
    cairo_scaled_font_subset_t *scaled_font_subset;

    struct {
	unsigned int font_id;
	char *base_font;
	unsigned int num_glyphs; /* Num /CharStrings in font */
	double x_min, y_min, x_max, y_max;
	double ascent, descent;
	double units_per_em;

	const char    *data;
	unsigned long  header_size;
	unsigned long  data_size;
	unsigned long  trailer_size;
    } base;

    /* Num glyphs in subset. May be greater than
     * scaled_font_subset->num_glyphs due to glyphs required by the
     * SEAC operator. */
    int num_glyphs;

    /* The glyphs and glyph_names arrays are indexed by the order of
     * the Charstrings in the font. This is not necessarily the same
     * order as the glyph index. The index_to_glyph_name() font backend
     * function is used to map the glyph index to the glyph order in
     * the Charstrings. */

    cairo_array_t glyphs_array;
    glyph_data_t *glyphs; /* pointer to first element of above array */
    cairo_array_t glyph_names_array;
    char **glyph_names; /* pointer to first element of above array */

    int num_subrs; /* Num /Subrs routines in the font */
    cairo_bool_t subset_subrs;
    struct {
	const char *subr_string;
	int subr_length;
	const char *np;
	int np_length;
	cairo_bool_t used;
    } *subrs; /* array with num_subrs elements */

    /* Maps scaled_font_subset index to glyphs_array.
     * Array size = scaled_font_subset->num_glyphs. */
    int *scaled_subset_index_to_glyphs;

    /* Keeps track of the glyphs that will be emitted in the subset.
     * Allocated size = base.num_glyphs. Number of entries = num_glyphs.
     * Array values are glyph_array indexes.
     */
    int *type1_subset_index_to_glyphs;

    cairo_output_stream_t *output;
    cairo_array_t contents;

    const char *rd, *nd, *np;

    int lenIV;

    char *type1_data;
    unsigned int type1_length;
    char *type1_end;

    char *header_segment;
    unsigned int header_segment_size;
    char *eexec_segment;
    unsigned int eexec_segment_size;
    cairo_bool_t eexec_segment_is_ascii;

    char *cleartext;
    char *cleartext_end;

    unsigned int header_size;

    unsigned short eexec_key;
    cairo_bool_t hex_encode;
    int hex_column;

    struct {
	double stack[TYPE1_STACKSIZE];
	int sp;
    } build_stack;

    struct {
	int stack[TYPE1_STACKSIZE];
	int sp;
    } ps_stack;


} cairo_type1_font_subset_t;


static cairo_status_t
_cairo_type1_font_subset_init (cairo_type1_font_subset_t  *font,
			       cairo_scaled_font_subset_t *scaled_font_subset,
			       cairo_bool_t                hex_encode)
{
    memset (font, 0, sizeof (*font));
    font->scaled_font_subset = scaled_font_subset;

    _cairo_array_init (&font->glyphs_array, sizeof (glyph_data_t));
    _cairo_array_init (&font->glyph_names_array, sizeof (char *));
    font->scaled_subset_index_to_glyphs = calloc (scaled_font_subset->num_glyphs, sizeof font->scaled_subset_index_to_glyphs[0]);
    if (unlikely (font->scaled_subset_index_to_glyphs == NULL))
        return _cairo_error (CAIRO_STATUS_NO_MEMORY);


    font->type1_subset_index_to_glyphs = NULL;
    font->base.num_glyphs = 0;
    font->num_subrs = 0;
    font->subset_subrs = TRUE;
    font->subrs = NULL;

    font->hex_encode = hex_encode;
    font->num_glyphs = 0;

    _cairo_array_init (&font->contents, sizeof (char));

    return CAIRO_STATUS_SUCCESS;
}

static void
cairo_type1_font_subset_use_glyph (cairo_type1_font_subset_t *font, int glyph)
{
    if (font->glyphs[glyph].subset_index >= 0)
	return;

    font->glyphs[glyph].subset_index = font->num_glyphs;
    font->type1_subset_index_to_glyphs[font->num_glyphs] = glyph;
    font->num_glyphs++;
}

static cairo_bool_t
is_ps_delimiter(int c)
{
    static const char delimiters[] = "()[]{}<>/% \t\r\n";

    return strchr (delimiters, c) != NULL;
}

static const char *
find_token (const char *buffer, const char *end, const char *token)
{
    int i, length;
    /* FIXME: find substring really must be find_token */

    if (buffer == NULL)
	return NULL;

    length = strlen (token);
    for (i = 0; buffer + i < end - length + 1; i++)
	if (memcmp (buffer + i, token, length) == 0)
	    if ((i == 0 || token[0] == '/' || is_ps_delimiter(buffer[i - 1])) &&
		(buffer + i == end - length || is_ps_delimiter(buffer[i + length])))
		return buffer + i;

    return NULL;
}

static cairo_status_t
cairo_type1_font_subset_find_segments (cairo_type1_font_subset_t *font)
{
    unsigned char *p;
    const char *eexec_token;
    unsigned int size, i;

    p = (unsigned char *) font->type1_data;
    font->type1_end = font->type1_data + font->type1_length;
    if (font->type1_length >= 2 && p[0] == 0x80 && p[1] == 0x01) {
	if (font->type1_end < (char *)(p + 6))
	    return CAIRO_INT_STATUS_UNSUPPORTED;
	font->header_segment_size =
	    p[2] | (p[3] << 8) | (p[4] << 16) | ((unsigned int) p[5] << 24);
	font->header_segment = (char *) p + 6;

	p += 6 + font->header_segment_size;
	if (font->type1_end < (char *)(p + 6))
	    return CAIRO_INT_STATUS_UNSUPPORTED;
	font->eexec_segment_size =
	    p[2] | (p[3] << 8) | (p[4] << 16) | ((unsigned int) p[5] << 24);
	font->eexec_segment = (char *) p + 6;
	font->eexec_segment_is_ascii = (p[1] == 1);

        p += 6 + font->eexec_segment_size;
	while (font->type1_end >= (char *)(p + 6) && p[1] != 0x03) {
	    size = p[2] | (p[3] << 8) | (p[4] << 16) | ((unsigned int) p[5] << 24);
	    if (font->type1_end < (char *)(p + 6 + size))
		return CAIRO_INT_STATUS_UNSUPPORTED;
	    p += 6 + size;
        }
        font->type1_end = (char *) p;
    } else {
	eexec_token = find_token ((char *) p, font->type1_end, "eexec");
	if (eexec_token == NULL)
	    return CAIRO_INT_STATUS_UNSUPPORTED;

	font->header_segment_size = eexec_token - (char *) p + strlen ("eexec\n");
	font->header_segment = (char *) p;
	font->eexec_segment_size = font->type1_length - font->header_segment_size;
	font->eexec_segment = (char *) p + font->header_segment_size;
	font->eexec_segment_is_ascii = TRUE;
	for (i = 0; i < 4; i++) {
	    if (!_cairo_isxdigit (font->eexec_segment[i]))
		font->eexec_segment_is_ascii = FALSE;
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

/* Search for the definition of key and erase it by overwriting with spaces.
 * This function is looks for definitions of the form:
 *
 * /key1 1234 def
 * /key2 [12 34 56] def
 *
 * ie a key defined as an integer or array of integers.
 *
 */
static void
cairo_type1_font_erase_dict_key (cairo_type1_font_subset_t *font,
				 const char *key)
{
    const char *start, *p, *segment_end;

    segment_end = font->header_segment + font->header_segment_size;

    start = font->header_segment;
    do {
	start = find_token (start, segment_end, key);
	if (start) {
	    p = start + strlen(key);
	    /* skip integers or array of integers */
	    while (p < segment_end &&
		   (_cairo_isspace(*p) ||
		    _cairo_isdigit(*p) ||
		    *p == '[' ||
		    *p == ']'))
	    {
		p++;
	    }

	    if (p + 3 < segment_end && memcmp(p, "def", 3) == 0) {
		/* erase definition of the key */
		memset((char *) start, ' ', p + 3 - start);
	    }
	    start += strlen(key);
	}
    } while (start);
}

static cairo_status_t
cairo_type1_font_subset_get_matrix (cairo_type1_font_subset_t *font,
				    const char                *name,
				    double                    *a,
				    double                    *b,
				    double                    *c,
				    double                    *d)
{
    const char *start, *end, *segment_end;
    int ret, s_max, i, j;
    char *s;
    const char *decimal_point;
    int decimal_point_len;

    decimal_point = _cairo_get_locale_decimal_point ();
    decimal_point_len = strlen (decimal_point);

    assert (decimal_point_len != 0);

    segment_end = font->header_segment + font->header_segment_size;
    start = find_token (font->header_segment, segment_end, name);
    if (start == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    end = find_token (start, segment_end, "def");
    if (end == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    s_max = end - start + 5*decimal_point_len + 1;
    s = _cairo_malloc (s_max);
    if (unlikely (s == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    i = 0;
    j = 0;
    while (i < end - start && j < s_max - decimal_point_len) {
	if (start[i] == '.') {
	    strncpy(s + j, decimal_point, decimal_point_len + 1);
	    i++;
	    j += decimal_point_len;
	} else {
	    s[j++] = start[i++];
	}
    }
    s[j] = 0;

    start = strpbrk (s, "{[");
    if (!start) {
	free (s);
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    start++;
    ret = 0;
    if (*start)
	ret = sscanf(start, "%lf %lf %lf %lf", a, b, c, d);

    free (s);

    if (ret != 4)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_type1_font_subset_get_bbox (cairo_type1_font_subset_t *font)
{
    cairo_status_t status;
    double x_min, y_min, x_max, y_max;
    double xx, yx, xy, yy;

    status = cairo_type1_font_subset_get_matrix (font, "/FontBBox",
						 &x_min,
						 &y_min,
						 &x_max,
						 &y_max);
    if (unlikely (status))
	return status;

    status = cairo_type1_font_subset_get_matrix (font, "/FontMatrix",
						 &xx, &yx, &xy, &yy);
    if (unlikely (status))
	return status;

    if (yy == 0.0)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    /* Freetype uses 1/yy to get units per EM */
    font->base.units_per_em = 1.0/yy;

    /* If the FontMatrix is not a uniform scale the metrics we extract
     * from the font won't match what FreeType returns */
    if (xx != yy || yx != 0.0 || xy != 0.0)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    font->base.x_min = x_min / font->base.units_per_em;
    font->base.y_min = y_min / font->base.units_per_em;
    font->base.x_max = x_max / font->base.units_per_em;
    font->base.y_max = y_max / font->base.units_per_em;
    font->base.ascent = font->base.y_max;
    font->base.descent = font->base.y_min;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_type1_font_subset_get_fontname (cairo_type1_font_subset_t *font)
{
    const char *start, *end, *segment_end;
    char *s;
    int i;
    cairo_status_t status;

    segment_end = font->header_segment + font->header_segment_size;
    start = find_token (font->header_segment, segment_end, "/FontName");
    if (start == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    start += strlen ("/FontName");

    end = find_token (start, segment_end, "def");
    if (end == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    while (end > start && _cairo_isspace(end[-1]))
	end--;

    s = _cairo_malloc (end - start + 1);
    if (unlikely (s == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    strncpy (s, start, end - start);
    s[end - start] = 0;

    start = strchr (s, '/');
    if (!start++ || !start) {
	free (s);
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    /* If font name is prefixed with a subset tag, strip it off. */
    if (strlen(start) > 7 && start[6] == '+') {
	for (i = 0; i < 6; i++) {
	    if (start[i] < 'A' || start[i] > 'Z')
		break;
	}
	if (i == 6)
	    start += 7;
    }

    font->base.base_font = strdup (start);
    free (s);
    if (unlikely (font->base.base_font == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    status = _cairo_escape_ps_name (&font->base.base_font);

    return status;
}

static cairo_status_t
cairo_type1_font_subset_write_header (cairo_type1_font_subset_t *font,
					 const char *name)
{
    const char *start, *end, *segment_end;
    unsigned int i;
    int glyph;

    /* FIXME:
     * This function assumes that /FontName always appears
     * before /Encoding. This appears to always be the case with Type1
     * fonts.
     *
     * The more recently added code for removing the UniqueID and XUID
     * keys can not make any assumptions about the position of the
     * keys in the dictionary so it is implemented by overwriting the
     * key definition with spaces before we start copying the font to
     * the output.
     *
     * This code should be rewritten to not make any assumptions about
     * the order of dictionary keys. This will allow UniqueID to be
     * stripped out instead of leaving a bunch of spaces in the
     * output.
     */
    cairo_type1_font_erase_dict_key (font, "/UniqueID");
    cairo_type1_font_erase_dict_key (font, "/XUID");

    segment_end = font->header_segment + font->header_segment_size;

    /* Type 1 fonts created by Fontforge have some PostScript code at
     * the start of the font that skips the font if the printer has a
     * cached copy of the font with the same unique id. This breaks
     * our subsetted font so we disable it by searching for the
     * PostScript operator "known" when used to check for the
     * "/UniqueID" dictionary key. We append " pop false " after it to
     * pop the result of this check off the stack and replace it with
     * "false" to make the PostScript code think "/UniqueID" does not
     * exist.
     */
    end = font->header_segment;
    start = find_token (font->header_segment, segment_end, "/UniqueID");
    if (start) {
	start += 9;
	while (start < segment_end && _cairo_isspace (*start))
	    start++;
	if (start + 5 < segment_end && memcmp(start, "known", 5) == 0) {
	    _cairo_output_stream_write (font->output, font->header_segment,
					start + 5 - font->header_segment);
	    _cairo_output_stream_printf (font->output, " pop false ");
	    end = start + 5;
	}
    }

    start = find_token (end, segment_end, "/FontName");
    if (start == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    _cairo_output_stream_write (font->output, end,
				start - end);

    _cairo_output_stream_printf (font->output, "/FontName /%s def", name);

    end = find_token (start, segment_end, "def");
    if (end == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;
    end += 3;

    start = find_token (end, segment_end, "/Encoding");
    if (start == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;
    _cairo_output_stream_write (font->output, end, start - end);

    _cairo_output_stream_printf (font->output,
				 "/Encoding 256 array\n"
				 "0 1 255 {1 index exch /.notdef put} for\n");
    if (font->scaled_font_subset->is_latin) {
	for (i = 1; i < 256; i++) {
	    int subset_glyph = font->scaled_font_subset->latin_to_subset_glyph_index[i];

	    if (subset_glyph > 0) {
		_cairo_output_stream_printf (font->output,
					     "dup %d /%s put\n",
					     i,
					     _cairo_winansi_to_glyphname (i));
	    }
	}
    } else {
	for (i = 1; i < font->scaled_font_subset->num_glyphs; i++) {
	    glyph = font->scaled_subset_index_to_glyphs[i];
	    _cairo_output_stream_printf (font->output,
					 "dup %d /%s put\n",
					 i,
					 font->glyph_names[glyph]);
	}
    }
    _cairo_output_stream_printf (font->output, "readonly def");

    end = find_token (start, segment_end, "def");
    if (end == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;
    end += 3;

    /* There are some buggy fonts that contain more than one /Encoding */
    if (find_token (end, segment_end, "/Encoding"))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    _cairo_output_stream_write (font->output, end, segment_end - end);

    return font->output->status;
}

static int
hex_to_int (int ch)
{
    if (ch <= '9')
	return ch - '0';
    else if (ch <= 'F')
	return ch - 'A' + 10;
    else
	return ch - 'a' + 10;
}

static cairo_status_t
cairo_type1_font_subset_write_encrypted (cairo_type1_font_subset_t *font,
					 const char *data, unsigned int length)
{
    const unsigned char *in, *end;
    int c, p;
    static const char hex_digits[16] = "0123456789abcdef";
    char digits[3];

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

    return font->output->status;
}

static cairo_status_t
cairo_type1_font_subset_decrypt_eexec_segment (cairo_type1_font_subset_t *font)
{
    unsigned short r = CAIRO_TYPE1_PRIVATE_DICT_KEY;
    unsigned char *in, *end;
    char *out;
    int c, p;
    unsigned int i;

    in = (unsigned char *) font->eexec_segment;
    end = (unsigned char *) in + font->eexec_segment_size;

    font->cleartext = _cairo_malloc (font->eexec_segment_size + 1);
    if (unlikely (font->cleartext == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    out = font->cleartext;
    while (in < end) {
	if (font->eexec_segment_is_ascii) {
	    c = *in++;
	    if (_cairo_isspace (c))
		continue;
	    c = (hex_to_int (c) << 4) | hex_to_int (*in++);
	} else {
	    c = *in++;
	}
	p = c ^ (r >> 8);
	r = (c + r) * CAIRO_TYPE1_ENCRYPT_C1 + CAIRO_TYPE1_ENCRYPT_C2;

	*out++ = p;
    }
    font->cleartext_end = out;

    /* Overwrite random bytes with spaces.
     *
     * The first 4 bytes of the cleartext are the random bytes
     * required by the encryption algorithm. When encrypting the
     * cleartext, the first ciphertext byte must not be a white space
     * character and the first 4 bytes must not be an ASCII Hex
     * character. Some fonts do not check that their randomly chosen
     * bytes results in ciphertext that complies with this
     * restriction. This may cause problems for some PDF consumers. By
     * replacing the random bytes with spaces, the first four bytes of
     * ciphertext will always be 0xf9, 0x83, 0xef, 0x00 which complies
     * with this restriction. Using spaces also means we don't have to
     * skip over the random bytes when parsing the cleartext.
     */
    for (i = 0; i < 4 && i < font->eexec_segment_size; i++)
	font->cleartext[i] = ' ';

    /* Ensure strtol() can not scan past the end of the cleartext */
    font->cleartext[font->eexec_segment_size] = 0;

    return CAIRO_STATUS_SUCCESS;
}

static const char *
skip_token (const char *p, const char *end)
{
    while (p < end && _cairo_isspace(*p))
	p++;

    while (p < end && !_cairo_isspace(*p))
	p++;

    if (p == end)
	return NULL;

    return p;
}

static void
cairo_type1_font_subset_decrypt_charstring (const unsigned char *in, int size, unsigned char *out)
{
    unsigned short r = CAIRO_TYPE1_CHARSTRING_KEY;
    int c, p, i;

    for (i = 0; i < size; i++) {
        c = *in++;
	p = c ^ (r >> 8);
	r = (c + r) * CAIRO_TYPE1_ENCRYPT_C1 + CAIRO_TYPE1_ENCRYPT_C2;
	*out++ = p;
    }
}

static const unsigned char *
cairo_type1_font_subset_decode_integer (const unsigned char *p, int *integer)
{
    if (*p <= 246) {
        *integer = *p++ - 139;
    } else if (*p <= 250) {
        *integer = (p[0] - 247) * 256 + p[1] + 108;
        p += 2;
    } else if (*p <= 254) {
        *integer = -(p[0] - 251) * 256 - p[1] - 108;
        p += 2;
    } else {
        *integer = ((uint32_t)p[1] << 24) | (p[2] << 16) | (p[3] << 8) | p[4];
        p += 5;
    }

    return p;
}

static cairo_status_t
use_standard_encoding_glyph (cairo_type1_font_subset_t *font, int index)
{
    const char *glyph_name;
    unsigned int i;

    if (index < 0 || index > 255)
	return CAIRO_STATUS_SUCCESS;

    glyph_name = _cairo_ps_standard_encoding_to_glyphname (index);
    if (glyph_name == NULL)
	return CAIRO_STATUS_SUCCESS;

    for (i = 0; i < font->base.num_glyphs; i++) {
	if (font->glyph_names[i] &&  strcmp (font->glyph_names[i], glyph_name) == 0) {
	    cairo_type1_font_subset_use_glyph (font, i);

	    return CAIRO_STATUS_SUCCESS;
	}
    }

    return CAIRO_INT_STATUS_UNSUPPORTED;
}


#define TYPE1_CHARSTRING_COMMAND_HSTEM		 0x01
#define TYPE1_CHARSTRING_COMMAND_VSTEM		 0x03
#define TYPE1_CHARSTRING_COMMAND_VMOVETO	 0x04
#define TYPE1_CHARSTRING_COMMAND_RLINETO	 0x05
#define TYPE1_CHARSTRING_COMMAND_HLINETO	 0x06
#define TYPE1_CHARSTRING_COMMAND_VLINETO	 0x07
#define TYPE1_CHARSTRING_COMMAND_RRCURVETO	 0x08
#define TYPE1_CHARSTRING_COMMAND_CLOSEPATH	 0x09
#define TYPE1_CHARSTRING_COMMAND_CALLSUBR	 0x0a
#define TYPE1_CHARSTRING_COMMAND_RETURN		 0x0b
#define TYPE1_CHARSTRING_COMMAND_ESCAPE		 0x0c
#define TYPE1_CHARSTRING_COMMAND_HSBW		 0x0d
#define TYPE1_CHARSTRING_COMMAND_ENDCHAR	 0x0e
#define TYPE1_CHARSTRING_COMMAND_RMOVETO	 0x15
#define TYPE1_CHARSTRING_COMMAND_HMOVETO	 0x16
#define TYPE1_CHARSTRING_COMMAND_VHCURVETO	 0x1e
#define TYPE1_CHARSTRING_COMMAND_HVCURVETO	 0x1f
#define TYPE1_CHARSTRING_COMMAND_DOTSECTION	 0x0c00
#define TYPE1_CHARSTRING_COMMAND_VSTEM3		 0x0c01
#define TYPE1_CHARSTRING_COMMAND_HSTEM3		 0x0c02
#define TYPE1_CHARSTRING_COMMAND_SEAC		 0x0c06
#define TYPE1_CHARSTRING_COMMAND_SBW		 0x0c07
#define TYPE1_CHARSTRING_COMMAND_DIV		 0x0c0c
#define TYPE1_CHARSTRING_COMMAND_CALLOTHERSUBR   0x0c10
#define TYPE1_CHARSTRING_COMMAND_POP	         0x0c11
#define TYPE1_CHARSTRING_COMMAND_SETCURRENTPOINT 0x0c21

/* Parse the charstring, including recursing into subroutines. Find
 * the glyph width, subroutines called, and glyphs required by the
 * SEAC operator. */
static cairo_status_t
cairo_type1_font_subset_parse_charstring (cairo_type1_font_subset_t *font,
					  int                        glyph,
					  const char                *encrypted_charstring,
					  int                        encrypted_charstring_length)
{
    cairo_status_t status;
    unsigned char *charstring;
    const unsigned char *end;
    const unsigned char *p;
    int command;

    charstring = _cairo_malloc (encrypted_charstring_length);
    if (unlikely (charstring == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    cairo_type1_font_subset_decrypt_charstring ((const unsigned char *)
						encrypted_charstring,
						encrypted_charstring_length,
						charstring);
    end = charstring + encrypted_charstring_length;
    p = charstring + font->lenIV;
    status = CAIRO_STATUS_SUCCESS;
    while (p < end) {
        if (*p < 32) {
	    command = *p++;
	    switch (command) {
	    case TYPE1_CHARSTRING_COMMAND_HSTEM:
	    case TYPE1_CHARSTRING_COMMAND_VSTEM:
	    case TYPE1_CHARSTRING_COMMAND_VMOVETO:
	    case TYPE1_CHARSTRING_COMMAND_RLINETO:
	    case TYPE1_CHARSTRING_COMMAND_HLINETO:
	    case TYPE1_CHARSTRING_COMMAND_VLINETO:
	    case TYPE1_CHARSTRING_COMMAND_RRCURVETO:
	    case TYPE1_CHARSTRING_COMMAND_CLOSEPATH:
	    case TYPE1_CHARSTRING_COMMAND_RMOVETO:
	    case TYPE1_CHARSTRING_COMMAND_HMOVETO:
	    case TYPE1_CHARSTRING_COMMAND_VHCURVETO:
	    case TYPE1_CHARSTRING_COMMAND_HVCURVETO:
	    case TYPE1_CHARSTRING_COMMAND_RETURN:
	    case TYPE1_CHARSTRING_COMMAND_ENDCHAR:
	    default:
		/* stack clearing operator */
		font->build_stack.sp = 0;
		break;

	    case TYPE1_CHARSTRING_COMMAND_CALLSUBR:
		if (font->subset_subrs && font->build_stack.sp > 0) {
		    double int_val;
		    if (modf(font->build_stack.stack[--font->build_stack.sp], &int_val) == 0.0) {
			int subr_num = int_val;
			if (subr_num >= 0 && subr_num < font->num_subrs) {
			    font->subrs[subr_num].used = TRUE;
			    status = cairo_type1_font_subset_parse_charstring (
				font,
				glyph,
				font->subrs[subr_num].subr_string,
				font->subrs[subr_num].subr_length);
			    break;
			}
		    }
		}
		font->subset_subrs = FALSE;
		break;

	    case TYPE1_CHARSTRING_COMMAND_HSBW:
		if (font->build_stack.sp < 2) {
		    status = CAIRO_INT_STATUS_UNSUPPORTED;
		    goto cleanup;
		}

		font->glyphs[glyph].width = font->build_stack.stack[1]/font->base.units_per_em;
		font->build_stack.sp = 0;
		break;

	    case TYPE1_CHARSTRING_COMMAND_ESCAPE:
		command = command << 8 | *p++;
		switch (command) {
		case TYPE1_CHARSTRING_COMMAND_DOTSECTION:
		case TYPE1_CHARSTRING_COMMAND_VSTEM3:
		case TYPE1_CHARSTRING_COMMAND_HSTEM3:
		case TYPE1_CHARSTRING_COMMAND_SETCURRENTPOINT:
		default:
		    /* stack clearing operator */
		    font->build_stack.sp = 0;
		    break;

		case TYPE1_CHARSTRING_COMMAND_SEAC:
		    /* The seac command takes five integer arguments.  The
		     * last two are glyph indices into the PS standard
		     * encoding give the names of the glyphs that this
		     * glyph is composed from.  All we need to do is to
		     * make sure those glyphs are present in the subset
		     * under their standard names. */
		    if (font->build_stack.sp < 5) {
			status = CAIRO_INT_STATUS_UNSUPPORTED;
			goto cleanup;
		    }

		    status = use_standard_encoding_glyph (font, font->build_stack.stack[3]);
		    if (unlikely (status))
			goto cleanup;

		    status = use_standard_encoding_glyph (font, font->build_stack.stack[4]);
		    if (unlikely (status))
			goto cleanup;

		    font->build_stack.sp = 0;
		    break;

		case TYPE1_CHARSTRING_COMMAND_SBW:
		    if (font->build_stack.sp < 4) {
			status = CAIRO_INT_STATUS_UNSUPPORTED;
			goto cleanup;
		    }

		    font->glyphs[glyph].width = font->build_stack.stack[2]/font->base.units_per_em;
		    font->build_stack.sp = 0;
		    break;

		case TYPE1_CHARSTRING_COMMAND_DIV:
		    if (font->build_stack.sp < 2) {
			status = CAIRO_INT_STATUS_UNSUPPORTED;
			goto cleanup;
		    } else {
			double num1 = font->build_stack.stack[font->build_stack.sp - 2];
			double num2 = font->build_stack.stack[font->build_stack.sp - 1];
			font->build_stack.sp--;
			if (num2 == 0.0) {
			    status = CAIRO_INT_STATUS_UNSUPPORTED;
			    goto cleanup;
			}
			font->build_stack.stack[font->build_stack.sp - 1] = num1/num2;
		    }
		    break;

		case TYPE1_CHARSTRING_COMMAND_CALLOTHERSUBR:
		    if (font->build_stack.sp < 1) {
			status = CAIRO_INT_STATUS_UNSUPPORTED;
			goto cleanup;
		    }

		    font->build_stack.sp--;
		    font->ps_stack.sp = 0;
		    while (font->build_stack.sp)
			font->ps_stack.stack[font->ps_stack.sp++] = font->build_stack.stack[--font->build_stack.sp];

                    break;

		case TYPE1_CHARSTRING_COMMAND_POP:
		    if (font->ps_stack.sp < 1) {
			status = CAIRO_INT_STATUS_UNSUPPORTED;
			goto cleanup;
		    }

		    /* T1 spec states that if the interpreter does not
		     * support executing the callothersub, the results
		     * must be taken from the callothersub arguments. */
		    font->build_stack.stack[font->build_stack.sp++] = font->ps_stack.stack[--font->ps_stack.sp];
		    break;
		}
		break;
	    }
	} else {
            /* integer argument */
	    if (font->build_stack.sp < TYPE1_STACKSIZE) {
		int val;
		p = cairo_type1_font_subset_decode_integer (p, &val);
		font->build_stack.stack[font->build_stack.sp++] = val;
	    } else {
		status = CAIRO_INT_STATUS_UNSUPPORTED;
		goto cleanup;
	    }
	}
    }

cleanup:
    free (charstring);

    return status;
}

static cairo_status_t
cairo_type1_font_subset_build_subr_list (cairo_type1_font_subset_t *font,
					 int subr_number,
					 const char *encrypted_charstring, int encrypted_charstring_length,
					 const char *np, int np_length)
{

    font->subrs[subr_number].subr_string = encrypted_charstring;
    font->subrs[subr_number].subr_length = encrypted_charstring_length;
    font->subrs[subr_number].np = np;
    font->subrs[subr_number].np_length = np_length;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
write_used_subrs (cairo_type1_font_subset_t *font,
		  int subr_number,
		  const char *subr_string, int subr_string_length,
		  const char *np, int np_length)
{
    cairo_status_t status;
    char buffer[256];
    int length;

    if (!font->subrs[subr_number].used)
	return CAIRO_STATUS_SUCCESS;

    length = snprintf (buffer, sizeof buffer,
		       "dup %d %d %s ",
		       subr_number, subr_string_length, font->rd);
    status = cairo_type1_font_subset_write_encrypted (font, buffer, length);
    if (unlikely (status))
	return status;

    status = cairo_type1_font_subset_write_encrypted (font,
					              subr_string,
						      subr_string_length);
    if (unlikely (status))
	return status;

    if (np) {
	status = cairo_type1_font_subset_write_encrypted (font, np, np_length);
    } else {
	length = snprintf (buffer, sizeof buffer, "%s\n", font->np);
	status = cairo_type1_font_subset_write_encrypted (font, buffer, length);
    }
    if (unlikely (status))
	return status;

    return CAIRO_STATUS_SUCCESS;
}

typedef cairo_status_t (*subr_func_t) (cairo_type1_font_subset_t *font,
				       int subr_number,
				       const char *subr_string, int subr_string_length,
				       const char *np, int np_length);

static cairo_status_t
cairo_type1_font_for_each_subr (cairo_type1_font_subset_t  *font,
				const char                 *array_start,
				const char                 *cleartext_end,
				subr_func_t                 func,
				const char                **array_end)
{
    const char *p, *subr_string;
    char *end;
    int subr_num, subr_length;
    const char *np;
    int np_length;
    cairo_status_t status;

    /* We're looking at "dup" at the start of the first subroutine. The subroutines
     * definitions are on the form:
     *
     *   dup 5 23 RD <23 binary bytes> NP
     *
     * or alternatively using -| and |- instead of RD and ND.
     * The first number is the subroutine number.
     */

    p = array_start;
    while (p + 3 < cleartext_end && strncmp (p, "dup", 3) == 0) {
	p = skip_token (p, cleartext_end);

	/* get subr number */
	subr_num = strtol (p, &end, 10);
	if (p == end)
	    return CAIRO_INT_STATUS_UNSUPPORTED;

	if (subr_num < 0 || subr_num >= font->num_subrs)
	    return CAIRO_INT_STATUS_UNSUPPORTED;

	/* get subr length */
	p = end;
	subr_length = strtol (p, &end, 10);
	if (p == end)
	    return CAIRO_INT_STATUS_UNSUPPORTED;

	/* Skip past -| or RD to binary data.  There is exactly one space
	 * between the -| or RD token and the encrypted data, thus '+ 1'. */
	subr_string = skip_token (end, cleartext_end) + 1;

	np = NULL;
	np_length = 0;

	/* Skip binary data and | or NP token. */
	p = skip_token (subr_string + subr_length, cleartext_end);
	while (p < cleartext_end && _cairo_isspace(*p))
	    p++;

	/* Some fonts have "noaccess put" instead of "NP" */
	if (p + 3 < cleartext_end && strncmp (p, "put", 3) == 0) {
	    p = skip_token (p, cleartext_end);
	    while (p < cleartext_end && _cairo_isspace(*p))
		p++;

	    np = subr_string + subr_length;
	    np_length = p - np;
	}

	status = func (font, subr_num,
		       subr_string, subr_length, np, np_length);
	if (unlikely (status))
	    return status;

    }

    *array_end = (char *) p;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_type1_font_subset_build_glyph_list (cairo_type1_font_subset_t *font,
					  int glyph_number,
					  const char *name, int name_length,
					  const char *encrypted_charstring, int encrypted_charstring_length)
{
    char *s;
    glyph_data_t glyph;
    cairo_status_t status;

    s = _cairo_malloc (name_length + 1);
    if (unlikely (s == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    strncpy (s, name, name_length);
    s[name_length] = 0;

    status = _cairo_array_append (&font->glyph_names_array, &s);
    if (unlikely (status))
	return status;

    glyph.subset_index = -1;
    glyph.width = 0;
    glyph.encrypted_charstring = encrypted_charstring;
    glyph.encrypted_charstring_length = encrypted_charstring_length;
    status = _cairo_array_append (&font->glyphs_array, &glyph);

    return status;
}

static cairo_status_t
write_used_glyphs (cairo_type1_font_subset_t *font,
		   int glyph_number,
		   const char *name, int name_length,
		   const char *charstring, int charstring_length)
{
    cairo_status_t status;
    char buffer[256];
    int length;
    unsigned int subset_id;
    int ch;
    const char *wa_name;

    if (font->glyphs[glyph_number].subset_index < 0)
	return CAIRO_STATUS_SUCCESS;

    if (font->scaled_font_subset->is_latin) {
	/* When using the WinAnsi encoding in PDF, the /Encoding array
	 * is ignored and instead glyphs are keyed by glyph names. To
	 * ensure correct rendering we replace the glyph name in the
	 * font with the standard name.
         **/
	subset_id = font->glyphs[glyph_number].subset_index;
	/* Any additional glyph included for use by the seac operator
	 * will either have subset_id >= font->scaled_font_subset->num_glyphs
	 * or will not map to a winansi name (wa_name = NULL).  In this
	 * case the original name is used.
	 */
	if (subset_id > 0 && subset_id < font->scaled_font_subset->num_glyphs) {
	    ch = font->scaled_font_subset->to_latin_char[subset_id];
	    wa_name = _cairo_winansi_to_glyphname (ch);
	    if (wa_name) {
		name = wa_name;
		name_length = strlen(name);
	    }
	}
    }

    length = snprintf (buffer, sizeof buffer,
		       "/%.*s %d %s ",
		       name_length, name, charstring_length, font->rd);
    status = cairo_type1_font_subset_write_encrypted (font, buffer, length);
    if (unlikely (status))
	return status;

    status = cairo_type1_font_subset_write_encrypted (font,
					              charstring,
						      charstring_length);
    if (unlikely (status))
	return status;

    length = snprintf (buffer, sizeof buffer, "%s\n", font->nd);
    status = cairo_type1_font_subset_write_encrypted (font, buffer, length);
    if (unlikely (status))
	return status;

    return CAIRO_STATUS_SUCCESS;
}

typedef cairo_status_t (*glyph_func_t) (cairo_type1_font_subset_t *font,
					int glyph_number,
			                const char *name, int name_length,
			                const char *charstring, int charstring_length);

static cairo_status_t
cairo_type1_font_subset_for_each_glyph (cairo_type1_font_subset_t *font,
					const char *dict_start,
					const char *dict_end,
					glyph_func_t func,
					const char **dict_out)
{
    int charstring_length, name_length;
    const char *p, *charstring, *name;
    char *end;
    cairo_status_t status;
    int glyph_count;

    /* We're looking at '/' in the name of the first glyph.  The glyph
     * definitions are on the form:
     *
     *   /name 23 RD <23 binary bytes> ND
     *
     * or alternatively using -| and |- instead of RD and ND.
     *
     * We parse the glyph name and see if it is in the subset.  If it
     * is, we call the specified callback with the glyph name and
     * glyph data, otherwise we just skip it.  We need to parse
     * through a glyph definition; we can't just find the next '/',
     * since the binary data could contain a '/'.
     */

    p = dict_start;
    glyph_count = 0;
    while (*p == '/') {
	name = p + 1;
	p = skip_token (p, dict_end);
	name_length = p - name;

	charstring_length = strtol (p, &end, 10);
	if (p == end)
	    return CAIRO_INT_STATUS_UNSUPPORTED;

	/* Skip past -| or RD to binary data.  There is exactly one space
	 * between the -| or RD token and the encrypted data, thus '+ 1'. */
	charstring = skip_token (end, dict_end) + 1;

	/* Skip binary data and |- or ND token. */
	p = skip_token (charstring + charstring_length, dict_end);
	while (p < dict_end && _cairo_isspace(*p))
	    p++;

	/* In case any of the skip_token() calls above reached EOF, p will
	 * be equal to dict_end. */
	if (p == dict_end)
	    return CAIRO_INT_STATUS_UNSUPPORTED;

	status = func (font, glyph_count++,
		       name, name_length,
		       charstring, charstring_length);
	if (unlikely (status))
	    return status;
    }

    *dict_out = p;

    return CAIRO_STATUS_SUCCESS;
}


static cairo_status_t
cairo_type1_font_subset_write_private_dict (cairo_type1_font_subset_t *font,
					    const char                *name)
{
    cairo_status_t status;
    const char *p, *subrs, *charstrings, *array_start, *array_end, *dict_start, *dict_end;
    const char *lenIV_start, *lenIV_end, *closefile_token;
    char buffer[32], *lenIV_str, *subr_count_end, *glyph_count_end;
    int ret, lenIV, length;
    const cairo_scaled_font_backend_t *backend;
    unsigned int i;
    int glyph, j;

    /* The private dict holds hint information, common subroutines and
     * the actual glyph definitions (charstrings).
     *
     * What we do here is scan directly to the /Subrs token, which
     * marks the beginning of the subroutines. We read in all the
     * subroutines, then move on to the /CharString token, which marks
     * the beginning of the glyph definitions, and read in the charstrings.
     *
     * The charstrings are parsed to extract glyph widths, work out
     * which subroutines are called, and to see if any extra glyphs
     * need to be included due to the use of the seac glyph combining
     * operator.
     *
     * Finally, the private dict is copied to the subset font minus the
     * subroutines and charstrings not required.
     */

    /* Determine lenIV, the number of random characters at the start of
       each encrypted charstring. The default is 4, but this can be
       overridden in the private dict. */
    font->lenIV = 4;
    if ((lenIV_start = find_token (font->cleartext, font->cleartext_end, "/lenIV")) != NULL) {
        lenIV_start += 6;
        lenIV_end = find_token (lenIV_start, font->cleartext_end, "def");
        if (lenIV_end == NULL)
	    return CAIRO_INT_STATUS_UNSUPPORTED;

        lenIV_str = _cairo_malloc (lenIV_end - lenIV_start + 1);
        if (unlikely (lenIV_str == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

        strncpy (lenIV_str, lenIV_start, lenIV_end - lenIV_start);
        lenIV_str[lenIV_end - lenIV_start] = 0;

        ret = sscanf(lenIV_str, "%d", &lenIV);
        free(lenIV_str);

        if (unlikely (ret <= 0))
	    return CAIRO_INT_STATUS_UNSUPPORTED;

        /* Apparently some fonts signal unencrypted charstrings with a negative lenIV,
           though this is not part of the Type 1 Font Format specification.  See, e.g.
           http://lists.gnu.org/archive/html/freetype-devel/2000-06/msg00064.html. */
        if (unlikely (lenIV < 0))
	    return CAIRO_INT_STATUS_UNSUPPORTED;

        font->lenIV = lenIV;
    }

    /* Find start of Subrs */
    subrs = find_token (font->cleartext, font->cleartext_end, "/Subrs");
    if (subrs == NULL) {
	font->subset_subrs = FALSE;
	p = font->cleartext;
	array_start = NULL;
	goto skip_subrs;
    }

    /* Scan past /Subrs and get the array size. */
    p = subrs + strlen ("/Subrs");
    font->num_subrs = strtol (p, &subr_count_end, 10);
    if (subr_count_end == p)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    if (font->num_subrs <= 0)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    font->subrs = calloc (font->num_subrs, sizeof (font->subrs[0]));
    if (unlikely (font->subrs == NULL))
        return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    /* look for "dup" which marks the beginning of the first subr */
    array_start = find_token (subr_count_end, font->cleartext_end, "dup");
    if (array_start == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    /* Read in the subroutines */
    status = cairo_type1_font_for_each_subr (font,
					     array_start,
					     font->cleartext_end,
					     cairo_type1_font_subset_build_subr_list,
					     &array_end);
    if (unlikely(status))
	return status;

    p = array_end;
skip_subrs:

    /* Find start of CharStrings */
    charstrings = find_token (p, font->cleartext_end, "/CharStrings");
    if (charstrings == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    /* Scan past /CharStrings and the integer following it. */
    p = charstrings + strlen ("/CharStrings");
    strtol (p, &glyph_count_end, 10);
    if (p == glyph_count_end)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    /* Look for a '/' which marks the beginning of the first glyph
     * definition. */
    for (p = glyph_count_end; p < font->cleartext_end; p++)
	if (*p == '/')
	    break;
    if (p == font->cleartext_end)
	return CAIRO_INT_STATUS_UNSUPPORTED;
    dict_start = p;

    /* Now that we have the private dictionary broken down in
     * sections, do the first pass through the glyph definitions to
     * build a list of glyph names and charstrings. */
    status = cairo_type1_font_subset_for_each_glyph (font,
						     dict_start,
						     font->cleartext_end,
						     cairo_type1_font_subset_build_glyph_list,
						     &dict_end);
    if (unlikely(status))
	return status;

    font->glyphs = _cairo_array_index (&font->glyphs_array, 0);
    font->glyph_names = _cairo_array_index (&font->glyph_names_array, 0);
    font->base.num_glyphs = _cairo_array_num_elements (&font->glyphs_array);
    font->type1_subset_index_to_glyphs = calloc (font->base.num_glyphs, sizeof font->type1_subset_index_to_glyphs[0]);
    if (unlikely (font->type1_subset_index_to_glyphs == NULL))
        return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    backend = font->scaled_font_subset->scaled_font->backend;
    if (!backend->index_to_glyph_name)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    /* Find the glyph number corresponding to each glyph in the subset
     * and mark it as in use */

    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++) {
	unsigned long index;

	status = backend->index_to_glyph_name (font->scaled_font_subset->scaled_font,
					       font->glyph_names,
					       font->base.num_glyphs,
					       font->scaled_font_subset->glyphs[i],
					       &index);
	if (unlikely(status))
	    return status;

	cairo_type1_font_subset_use_glyph (font, index);
	font->scaled_subset_index_to_glyphs[i] = index;
    }

    /* Go through the charstring of each glyph in use, get the glyph
     * width and figure out which extra glyphs may be required by the
     * seac operator (which may cause font->num_glyphs to increase
     * while this loop is executing). Also subset the Subrs. */
    for (j = 0; j < font->num_glyphs; j++) {
	glyph = font->type1_subset_index_to_glyphs[j];
	font->build_stack.sp = 0;
	font->ps_stack.sp = 0;
	status = cairo_type1_font_subset_parse_charstring (font,
							   glyph,
							   font->glyphs[glyph].encrypted_charstring,
							   font->glyphs[glyph].encrypted_charstring_length);
	if (unlikely (status))
	    return status;
    }

    /* Always include the first five subroutines in case the Flex/hint mechanism is
     * being used. */
    for (j = 0; j < MIN (font->num_subrs, 5); j++) {
	font->subrs[j].used = TRUE;
    }

    closefile_token = find_token (dict_end, font->cleartext_end, "closefile");
    if (closefile_token == NULL)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    /* We're ready to start outputting. First write the header,
     * i.e. the public part of the font dict.*/
    status = cairo_type1_font_subset_write_header (font, name);
    if (unlikely (status))
	return status;

    font->base.header_size = _cairo_output_stream_get_position (font->output);

    /* Start outputting the private dict */
    if (font->subset_subrs) {
	/* First output everything up to the start of the Subrs array. */
	status = cairo_type1_font_subset_write_encrypted (font, font->cleartext,
							  array_start - font->cleartext);
	if (unlikely (status))
	    return status;

	/* Write out the subr definitions for each of the glyphs in
	 * the subset. */
	status = cairo_type1_font_for_each_subr (font,
						 array_start,
						 font->cleartext_end,
						 write_used_subrs,
						 &p);
	if (unlikely (status))
	    return status;
    } else {
	p = font->cleartext;
    }

    /* If subr subsetting, output everything from end of subrs to
     * start of /CharStrings token.  If not subr subsetting, output
     * everything start of private dict to start of /CharStrings
     * token. */
    status = cairo_type1_font_subset_write_encrypted (font, p, charstrings - p);
    if (unlikely (status))
	return status;

    /* Write out new charstring count */
    length = snprintf (buffer, sizeof buffer,
		       "/CharStrings %d", font->num_glyphs);
    status = cairo_type1_font_subset_write_encrypted (font, buffer, length);
    if (unlikely (status))
	return status;

    /* Write out text between the charstring count and the first
     * charstring definition */
    status = cairo_type1_font_subset_write_encrypted (font, glyph_count_end,
	                                          dict_start - glyph_count_end);
    if (unlikely (status))
	return status;

    /* Write out the charstring definitions for each of the glyphs in
     * the subset. */
    status = cairo_type1_font_subset_for_each_glyph (font,
						     dict_start,
						     font->cleartext_end,
						     write_used_glyphs,
						     &p);
    if (unlikely (status))
	return status;

    /* Output what's left between the end of the glyph definitions and
     * the end of the private dict to the output. */
    status = cairo_type1_font_subset_write_encrypted (font, p,
	                        closefile_token - p + strlen ("closefile") + 1);
    if (unlikely (status))
	return status;

    if (font->hex_encode)
	_cairo_output_stream_write (font->output, "\n", 1);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_type1_font_subset_write_trailer(cairo_type1_font_subset_t *font)
{
    const char *cleartomark_token;
    int i;
    static const char zeros[65] =
	"0000000000000000000000000000000000000000000000000000000000000000\n";


    for (i = 0; i < 8; i++)
	_cairo_output_stream_write (font->output, zeros, sizeof zeros);

    cleartomark_token = find_token (font->type1_data, font->type1_end, "cleartomark");
    if (cleartomark_token) {
	/* Some fonts have conditional save/restore around the entire
	 * font dict, so we need to retain whatever postscript code
	 * that may come after 'cleartomark'. */

	_cairo_output_stream_write (font->output, cleartomark_token,
				    font->type1_end - cleartomark_token);
	if (*(font->type1_end - 1) != '\n')
	    _cairo_output_stream_printf (font->output, "\n");

    } else if (!font->eexec_segment_is_ascii) {
	/* Fonts embedded in PDF may omit the fixed-content portion
	 * that includes the 'cleartomark' operator. Type 1 in PDF is
	 * always binary. */

	_cairo_output_stream_printf (font->output, "cleartomark\n");
    } else {
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    /* some fonts do not have a newline at the end of the last line */
    _cairo_output_stream_printf (font->output, "\n");

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
type1_font_write (void *closure, const unsigned char *data, unsigned int length)
{
    cairo_type1_font_subset_t *font = closure;

    return _cairo_array_append_multiple (&font->contents, data, length);
}

static cairo_status_t
cairo_type1_font_subset_write (cairo_type1_font_subset_t *font,
			       const char *name)
{
    cairo_status_t status;

    status = cairo_type1_font_subset_find_segments (font);
    if (unlikely (status))
	return status;

    status = cairo_type1_font_subset_decrypt_eexec_segment (font);
    if (unlikely (status))
	return status;

    /* Determine which glyph definition delimiters to use. */
    if (find_token (font->cleartext, font->cleartext_end, "/-|") != NULL) {
	font->rd = "-|";
	font->nd = "|-";
	font->np = "|";
    } else if (find_token (font->cleartext, font->cleartext_end, "/RD") != NULL) {
	font->rd = "RD";
	font->nd = "ND";
	font->np = "NP";
    } else {
	/* Don't know *what* kind of font this is... */
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    font->eexec_key = CAIRO_TYPE1_PRIVATE_DICT_KEY;
    font->hex_column = 0;

    status = cairo_type1_font_subset_get_bbox (font);
    if (unlikely (status))
	return status;

    status = cairo_type1_font_subset_get_fontname (font);
    if (unlikely (status))
	return status;

    status = cairo_type1_font_subset_write_private_dict (font, name);
    if (unlikely (status))
	return status;

    font->base.data_size = _cairo_output_stream_get_position (font->output) -
	font->base.header_size;

    status = cairo_type1_font_subset_write_trailer (font);
    if (unlikely (status))
	return status;

    font->base.trailer_size =
	_cairo_output_stream_get_position (font->output) -
	font->base.header_size - font->base.data_size;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
check_fontdata_is_type1 (const unsigned char *data, long length)
{
    /* Test for  Type 1 Binary (PFB) */
    if (length > 2 && data[0] == 0x80 && data[1] == 0x01)
	return TRUE;

    /* Test for Type 1 1 ASCII (PFA) */
    if (length > 2 && data[0] == '%' && data[1] == '!')
	return TRUE;

    return FALSE;
}

static cairo_status_t
cairo_type1_font_subset_generate (void       *abstract_font,
				  const char *name)

{
    cairo_type1_font_subset_t *font = abstract_font;
    cairo_scaled_font_t *scaled_font;
    cairo_status_t status;
    unsigned long data_length;

    scaled_font = font->scaled_font_subset->scaled_font;
    if (!scaled_font->backend->load_type1_data)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    status = scaled_font->backend->load_type1_data (scaled_font, 0, NULL, &data_length);
    if (status)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    font->type1_length = data_length;
    font->type1_data = _cairo_malloc (font->type1_length);
    if (unlikely (font->type1_data == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    status = scaled_font->backend->load_type1_data (scaled_font, 0,
						    (unsigned char *) font->type1_data,
						    &data_length);
    if (unlikely (status))
        return status;

    if (!check_fontdata_is_type1 ((unsigned char *)font->type1_data, data_length))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    status = _cairo_array_grow_by (&font->contents, 4096);
    if (unlikely (status))
	return status;

    font->output = _cairo_output_stream_create (type1_font_write, NULL, font);
    if (unlikely ((status = font->output->status)))
	return status;

    status = cairo_type1_font_subset_write (font, name);
    if (unlikely (status))
	return status;

    font->base.data = _cairo_array_index (&font->contents, 0);

    return status;
}

static cairo_status_t
_cairo_type1_font_subset_fini (cairo_type1_font_subset_t *font)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;
    unsigned int i;

    /* If the subset generation failed, some of the pointers below may
     * be NULL depending on at which point the error occurred. */

    _cairo_array_fini (&font->contents);

    free (font->type1_data);
    for (i = 0; i < _cairo_array_num_elements (&font->glyph_names_array); i++) {
	char **s;

	s = _cairo_array_index (&font->glyph_names_array, i);
	free (*s);
    }
    _cairo_array_fini (&font->glyph_names_array);
    _cairo_array_fini (&font->glyphs_array);

    free (font->subrs);

    if (font->output != NULL)
	status = _cairo_output_stream_destroy (font->output);

    free (font->base.base_font);

    free (font->scaled_subset_index_to_glyphs);

    free (font->type1_subset_index_to_glyphs);

    free (font->cleartext);

    return status;
}

cairo_status_t
_cairo_type1_subset_init (cairo_type1_subset_t		*type1_subset,
			  const char			*name,
			  cairo_scaled_font_subset_t	*scaled_font_subset,
                          cairo_bool_t                   hex_encode)
{
    cairo_type1_font_subset_t font;
    cairo_status_t status;
    cairo_bool_t is_synthetic;
    unsigned long length;
    unsigned int i;
    char buf[30];
    int glyph;

    /* We need to use a fallback font if this font differs from the type1 outlines. */
    if (scaled_font_subset->scaled_font->backend->is_synthetic) {
	status = scaled_font_subset->scaled_font->backend->is_synthetic (scaled_font_subset->scaled_font, &is_synthetic);
	if (unlikely (status))
	    return status;

	if (is_synthetic)
	    return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    status = _cairo_type1_font_subset_init (&font, scaled_font_subset, hex_encode);
    if (unlikely (status))
	return status;

    status = cairo_type1_font_subset_generate (&font, name);
    if (unlikely (status))
	goto fail1;

    if (font.base.base_font) {
	type1_subset->base_font = strdup (font.base.base_font);
    } else {
        snprintf(buf, sizeof (buf), "CairoFont-%u-%u",
                 scaled_font_subset->font_id, scaled_font_subset->subset_id);
	type1_subset->base_font = strdup (buf);
    }
    if (unlikely (type1_subset->base_font == NULL))
	goto fail1;

    type1_subset->widths = calloc (sizeof (double), scaled_font_subset->num_glyphs);
    if (unlikely (type1_subset->widths == NULL))
	goto fail2;

    for (i = 0; i < font.scaled_font_subset->num_glyphs; i++) {
	glyph = font.scaled_subset_index_to_glyphs[i];
	type1_subset->widths[i] = font.glyphs[glyph].width;
    }

    type1_subset->x_min = font.base.x_min;
    type1_subset->y_min = font.base.y_min;
    type1_subset->x_max = font.base.x_max;
    type1_subset->y_max = font.base.y_max;
    type1_subset->ascent = font.base.ascent;
    type1_subset->descent = font.base.descent;

    length = font.base.header_size +
	     font.base.data_size +
	     font.base.trailer_size;
    type1_subset->data = _cairo_malloc (length);
    if (unlikely (type1_subset->data == NULL))
	goto fail3;

    memcpy (type1_subset->data,
	    _cairo_array_index (&font.contents, 0), length);

    type1_subset->header_length = font.base.header_size;
    type1_subset->data_length = font.base.data_size;
    type1_subset->trailer_length = font.base.trailer_size;

    return _cairo_type1_font_subset_fini (&font);

 fail3:
    free (type1_subset->widths);
 fail2:
    free (type1_subset->base_font);
 fail1:
    _cairo_type1_font_subset_fini (&font);

    return status;
}

void
_cairo_type1_subset_fini (cairo_type1_subset_t *subset)
{
    free (subset->base_font);
    free (subset->widths);
    free (subset->data);
}

cairo_bool_t
_cairo_type1_scaled_font_is_type1 (cairo_scaled_font_t *scaled_font)
{
    cairo_status_t status;
    unsigned long length;
    unsigned char buf[64];

    if (!scaled_font->backend->load_type1_data)
	return FALSE;

    status = scaled_font->backend->load_type1_data (scaled_font, 0, NULL, &length);
    if (status)
	return FALSE;

    /* We only need a few bytes to test for Type 1 */
    if (length > sizeof (buf))
	length = sizeof (buf);

    status = scaled_font->backend->load_type1_data (scaled_font, 0, buf, &length);
    if (status)
	return FALSE;

    return check_fontdata_is_type1 (buf, length);
}

#endif /* CAIRO_HAS_FONT_SUBSET */
