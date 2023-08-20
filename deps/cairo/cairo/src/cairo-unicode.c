/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * The code in this file is derived from GLib's gutf8.c and
 *   ultimately from libunicode. It is relicensed under the
 *   dual LGPL/MPL with permission of the original authors.
 *
 * Copyright © 1999 Tom Tromey
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
 * The Initial Developer of the Original Code is Tom Tromey.
 *  and Red Hat, Inc.
 *
 * Contributor(s):
 *	Owen Taylor <otaylor@redhat.com>
 */

#include "cairoint.h"
#include "cairo-error-private.h"

#define UTF8_COMPUTE(Char, Mask, Len)					      \
  if (Char < 128)							      \
    {									      \
      Len = 1;								      \
      Mask = 0x7f;							      \
    }									      \
  else if ((Char & 0xe0) == 0xc0)					      \
    {									      \
      Len = 2;								      \
      Mask = 0x1f;							      \
    }									      \
  else if ((Char & 0xf0) == 0xe0)					      \
    {									      \
      Len = 3;								      \
      Mask = 0x0f;							      \
    }									      \
  else if ((Char & 0xf8) == 0xf0)					      \
    {									      \
      Len = 4;								      \
      Mask = 0x07;							      \
    }									      \
  else if ((Char & 0xfc) == 0xf8)					      \
    {									      \
      Len = 5;								      \
      Mask = 0x03;							      \
    }									      \
  else if ((Char & 0xfe) == 0xfc)					      \
    {									      \
      Len = 6;								      \
      Mask = 0x01;							      \
    }									      \
  else									      \
    Len = -1;

#define UTF8_LENGTH(Char)              \
  ((Char) < 0x80 ? 1 :                 \
   ((Char) < 0x800 ? 2 :               \
    ((Char) < 0x10000 ? 3 :            \
     ((Char) < 0x200000 ? 4 :          \
      ((Char) < 0x4000000 ? 5 : 6)))))

#define UTF8_GET(Result, Chars, Count, Mask, Len)			      \
  (Result) = (Chars)[0] & (Mask);					      \
  for ((Count) = 1; (Count) < (Len); ++(Count))				      \
    {									      \
      if (((Chars)[(Count)] & 0xc0) != 0x80)				      \
	{								      \
	  (Result) = -1;						      \
	  break;							      \
	}								      \
      (Result) <<= 6;							      \
      (Result) |= ((Chars)[(Count)] & 0x3f);				      \
    }

#define UNICODE_VALID(Char)                   \
    ((Char) < 0x110000 &&                     \
     (((Char) & 0xFFFFF800) != 0xD800))

static const char utf8_skip_data[256] = {
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
    3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,4,4,4,4,4,4,4,4,5,5,5,5,6,6,1,1
};

#define UTF8_NEXT_CHAR(p) ((p) + utf8_skip_data[*(unsigned char *)(p)])

/* Converts a sequence of bytes encoded as UTF-8 to a Unicode character.
 * If @p does not point to a valid UTF-8 encoded character, results are
 * undefined.
 **/
static uint32_t
_utf8_get_char (const unsigned char *p)
{
    int i, mask = 0, len;
    uint32_t result;
    unsigned char c = (unsigned char) *p;

    UTF8_COMPUTE (c, mask, len);
    if (len == -1)
	return (uint32_t)-1;
    UTF8_GET (result, p, i, mask, len);

    return result;
}

/* Like _utf8_get_char, but take a maximum length
 * and return (uint32_t)-2 on incomplete trailing character
 */
static uint32_t
_utf8_get_char_extended (const unsigned char *p,
			 long		      max_len)
{
    int i, len;
    uint32_t wc = (unsigned char) *p;

    if (wc < 0x80) {
	return wc;
    } else if (wc < 0xc0) {
	return (uint32_t)-1;
    } else if (wc < 0xe0) {
	len = 2;
	wc &= 0x1f;
    } else if (wc < 0xf0) {
	len = 3;
	wc &= 0x0f;
    } else if (wc < 0xf8) {
	len = 4;
	wc &= 0x07;
    } else if (wc < 0xfc) {
	len = 5;
	wc &= 0x03;
    } else if (wc < 0xfe) {
	len = 6;
	wc &= 0x01;
    } else {
	return (uint32_t)-1;
    }

    if (max_len >= 0 && len > max_len) {
	for (i = 1; i < max_len; i++) {
	    if ((((unsigned char *)p)[i] & 0xc0) != 0x80)
		return (uint32_t)-1;
	}
	return (uint32_t)-2;
    }

    for (i = 1; i < len; ++i) {
	uint32_t ch = ((unsigned char *)p)[i];

	if ((ch & 0xc0) != 0x80) {
	    if (ch)
		return (uint32_t)-1;
	    else
		return (uint32_t)-2;
	}

	wc <<= 6;
	wc |= (ch & 0x3f);
    }

    if (UTF8_LENGTH(wc) != len)
	return (uint32_t)-1;

    return wc;
}

/**
 * _cairo_utf8_get_char_validated:
 * @p: a UTF-8 string
 * @unicode: location to store one Unicode character
 *
 * Decodes the first character of a valid UTF-8 string, and returns
 * the number of bytes consumed.
 *
 * Note that the string should be valid.  Do not use this without
 * validating the string first.
 *
 * Returns: the number of bytes forming the character returned.
 **/
int
_cairo_utf8_get_char_validated (const char *p,
				uint32_t   *unicode)
{
    int i, mask = 0, len;
    uint32_t result;
    unsigned char c = (unsigned char) *p;

    UTF8_COMPUTE (c, mask, len);
    if (len == -1) {
	if (unicode)
	    *unicode = (uint32_t)-1;
	return 1;
    }
    UTF8_GET (result, p, i, mask, len);

    if (unicode)
	*unicode = result;
    return len;
}

/**
 * _cairo_utf8_to_ucs4:
 * @str: an UTF-8 string
 * @len: length of @str in bytes, or -1 if it is nul-terminated.
 *   If @len is supplied and the string has an embedded nul
 *   byte, only the portion before the nul byte is converted.
 * @result: location to store a pointer to a newly allocated UTF-32
 *   string (always native endian), or %NULL. Free with free(). A 0
 *   word will be written after the last character.
 * @items_written: location to store number of 32-bit words
 *   written. (Not including the trailing 0)
 *
 * Converts a UTF-8 string to UCS-4. UCS-4 is an encoding of Unicode
 * with 1 32-bit word per character. The string is validated to
 * consist entirely of valid Unicode characters.
 *
 * Return value: %CAIRO_STATUS_SUCCESS if the entire string was
 *   successfully converted. %CAIRO_STATUS_INVALID_STRING if an
 *   invalid sequence was found.
 **/
cairo_status_t
_cairo_utf8_to_ucs4 (const char *str,
		     int	 len,
		     uint32_t  **result,
		     int	*items_written)
{
    uint32_t *str32 = NULL;
    int n_chars, i;
    const unsigned char *in;
    const unsigned char * const ustr = (const unsigned char *) str;

    in = ustr;
    n_chars = 0;
    while ((len < 0 || ustr + len - in > 0) && *in)
    {
	uint32_t wc = _utf8_get_char_extended (in, ustr + len - in);
	if (wc & 0x80000000 || !UNICODE_VALID (wc))
	    return _cairo_error (CAIRO_STATUS_INVALID_STRING);

	n_chars++;
	if (n_chars == INT_MAX)
	    return _cairo_error (CAIRO_STATUS_INVALID_STRING);

	in = UTF8_NEXT_CHAR (in);
    }

    if (result) {
	str32 = _cairo_malloc_ab (n_chars + 1, sizeof (uint32_t));
	if (!str32)
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	in = ustr;
	for (i=0; i < n_chars; i++) {
	    str32[i] = _utf8_get_char (in);
	    in = UTF8_NEXT_CHAR (in);
	}
	str32[i] = 0;

	*result = str32;
    }

    if (items_written)
	*items_written = n_chars;

    return CAIRO_STATUS_SUCCESS;
}

/**
 * _cairo_ucs4_to_utf8:
 * @unicode: a UCS-4 character
 * @utf8: buffer to write utf8 string into. Must have at least 4 bytes
 * space available. Or %NULL.
 *
 * This space left intentionally blank.
 *
 * Return value: Number of bytes in the utf8 string or 0 if an invalid
 * unicode character
 **/
int
_cairo_ucs4_to_utf8 (uint32_t  unicode,
		     char     *utf8)
{
    int bytes;
    char *p;

    if (unicode < 0x80) {
	if (utf8)
	    *utf8 = unicode;
	return 1;
    } else if (unicode < 0x800) {
	bytes = 2;
    } else if (unicode < 0x10000) {
	bytes = 3;
    } else if (unicode < 0x200000) {
	bytes = 4;
    } else {
	return 0;
    }

    if (!utf8)
	return bytes;

    p = utf8 + bytes;
    while (p > utf8) {
	*--p = 0x80 | (unicode & 0x3f);
	unicode >>= 6;
    }
    *p |= 0xf0 << (4 - bytes);

    return bytes;
}

/**
 * _cairo_ucs4_to_utf16:
 * @unicode: a UCS-4 character
 * @utf16: buffer to write utf16 string into. Must have at least 2
 * elements. Or %NULL.
 *
 * This space left intentionally blank.
 *
 * Return value: Number of elements in the utf16 string or 0 if an
 * invalid unicode character
 **/
int
_cairo_ucs4_to_utf16 (uint32_t  unicode,
		      uint16_t *utf16)
{
    if (unicode < 0x10000) {
	if (utf16)
	    utf16[0] = unicode;
	return 1;
    } else if (unicode < 0x110000) {
	if (utf16) {
	    utf16[0] = (unicode - 0x10000) / 0x400 + 0xd800;
	    utf16[1] = (unicode - 0x10000) % 0x400 + 0xdc00;
	}
	return 2;
    } else {
	return 0;
    }
}

#if CAIRO_HAS_UTF8_TO_UTF16
/**
 * _cairo_utf8_to_utf16:
 * @str: an UTF-8 string
 * @len: length of @str in bytes, or -1 if it is nul-terminated.
 *   If @len is supplied and the string has an embedded nul
 *   byte, only the portion before the nul byte is converted.
 * @result: location to store a pointer to a newly allocated UTF-16
 *   string (always native endian). Free with free(). A 0
 *   word will be written after the last character.
 * @items_written: location to store number of 16-bit words
 *   written. (Not including the trailing 0)
 *
 * Converts a UTF-8 string to UTF-16. UTF-16 is an encoding of Unicode
 * where characters are represented either as a single 16-bit word, or
 * as a pair of 16-bit "surrogates". The string is validated to
 * consist entirely of valid Unicode characters.
 *
 * Return value: %CAIRO_STATUS_SUCCESS if the entire string was
 *   successfully converted. %CAIRO_STATUS_INVALID_STRING if an
 *   an invalid sequence was found.
 **/
cairo_status_t
_cairo_utf8_to_utf16 (const char *str,
		      int	  len,
		      uint16_t **result,
		      int	*items_written)
{
    uint16_t *str16 = NULL;
    int n16, i;
    const unsigned char *in;
    const unsigned char * const ustr = (const unsigned char *) str;

    in = ustr;
    n16 = 0;
    while ((len < 0 || ustr + len - in > 0) && *in) {
	uint32_t wc = _utf8_get_char_extended (in, ustr + len - in);
	if (wc & 0x80000000 || !UNICODE_VALID (wc))
	    return _cairo_error (CAIRO_STATUS_INVALID_STRING);

	if (wc < 0x10000)
	    n16 += 1;
	else
	    n16 += 2;

	if (n16 == INT_MAX - 1 || n16 == INT_MAX)
	    return _cairo_error (CAIRO_STATUS_INVALID_STRING);

	in = UTF8_NEXT_CHAR (in);
    }

    str16 = _cairo_malloc_ab (n16 + 1, sizeof (uint16_t));
    if (!str16)
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    in = ustr;
    for (i = 0; i < n16;) {
	uint32_t wc = _utf8_get_char (in);

	i += _cairo_ucs4_to_utf16 (wc, str16 + i);

	in = UTF8_NEXT_CHAR (in);
    }

    str16[i] = 0;

    *result = str16;
    if (items_written)
	*items_written = n16;

    return CAIRO_STATUS_SUCCESS;
}
#endif
