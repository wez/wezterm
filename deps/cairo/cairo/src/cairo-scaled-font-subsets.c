/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2003 University of Southern California
 * Copyright © 2005 Red Hat, Inc
 * Copyright © 2006 Keith Packard
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
 * The Initial Developer of the Original Code is University of Southern
 * California.
 *
 * Contributor(s):
 *	Carl D. Worth <cworth@cworth.org>
 *	Kristian Høgsberg <krh@redhat.com>
 *	Keith Packard <keithp@keithp.com>
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#define _DEFAULT_SOURCE /* for snprintf(), strdup() */
#include "cairoint.h"
#include "cairo-error-private.h"

#if CAIRO_HAS_FONT_SUBSET

#include "cairo-scaled-font-subsets-private.h"
#include "cairo-user-font-private.h"

#define MAX_GLYPHS_PER_SIMPLE_FONT 256
#define MAX_GLYPHS_PER_COMPOSITE_FONT 65536

typedef enum {
    CAIRO_SUBSETS_SCALED,
    CAIRO_SUBSETS_SIMPLE,
    CAIRO_SUBSETS_COMPOSITE
} cairo_subsets_type_t;

typedef enum {
    CAIRO_SUBSETS_FOREACH_UNSCALED,
    CAIRO_SUBSETS_FOREACH_SCALED,
    CAIRO_SUBSETS_FOREACH_USER
} cairo_subsets_foreach_type_t;

typedef struct _cairo_sub_font {
    cairo_hash_entry_t base;

    cairo_bool_t is_scaled;
    cairo_bool_t is_composite;
    cairo_bool_t is_user;
    cairo_bool_t use_latin_subset;
    cairo_bool_t reserve_notdef;
    cairo_scaled_font_subsets_t *parent;
    cairo_scaled_font_t *scaled_font;
    unsigned int font_id;

    int current_subset;
    int num_glyphs_in_current_subset;
    int num_glyphs_in_latin_subset;
    int max_glyphs_per_subset;
    char latin_char_map[256];

    cairo_hash_table_t *sub_font_glyphs;
    struct _cairo_sub_font *next;
} cairo_sub_font_t;

struct _cairo_scaled_font_subsets {
    cairo_subsets_type_t type;
    cairo_bool_t use_latin_subset;

    int max_glyphs_per_unscaled_subset_used;
    cairo_hash_table_t *unscaled_sub_fonts;
    cairo_sub_font_t *unscaled_sub_fonts_list;
    cairo_sub_font_t *unscaled_sub_fonts_list_end;

    int max_glyphs_per_scaled_subset_used;
    cairo_hash_table_t *scaled_sub_fonts;
    cairo_sub_font_t *scaled_sub_fonts_list;
    cairo_sub_font_t *scaled_sub_fonts_list_end;

    int num_sub_fonts;
};

typedef struct _cairo_sub_font_glyph {
    cairo_hash_entry_t base;

    unsigned int subset_id;
    unsigned int subset_glyph_index;
    double       x_advance;
    double       y_advance;

    cairo_bool_t is_latin;
    int		 latin_character;
    cairo_bool_t is_mapped;
    uint32_t     unicode;
    char  	*utf8;
    int          utf8_len;
} cairo_sub_font_glyph_t;

typedef struct _cairo_sub_font_collection {
    unsigned long *glyphs; /* scaled_font_glyph_index */
    char       **utf8;
    unsigned int glyphs_size;
    int           *to_latin_char;
    unsigned long *latin_to_subset_glyph_index;
    unsigned int max_glyph;
    unsigned int num_glyphs;

    unsigned int subset_id;

    cairo_status_t status;
    cairo_scaled_font_subset_callback_func_t font_subset_callback;
    void *font_subset_callback_closure;
} cairo_sub_font_collection_t;

typedef struct _cairo_string_entry {
    cairo_hash_entry_t base;
    char *string;
} cairo_string_entry_t;

static cairo_status_t
_cairo_sub_font_map_glyph (cairo_sub_font_t	*sub_font,
			   unsigned long	 scaled_font_glyph_index,
			   const char *		 utf8,
			   int			 utf8_len,
                           cairo_scaled_font_subsets_glyph_t *subset_glyph);

static void
_cairo_sub_font_glyph_init_key (cairo_sub_font_glyph_t  *sub_font_glyph,
				unsigned long		 scaled_font_glyph_index)
{
    sub_font_glyph->base.hash = scaled_font_glyph_index;
}

static cairo_sub_font_glyph_t *
_cairo_sub_font_glyph_create (unsigned long	scaled_font_glyph_index,
			      unsigned int	subset_id,
			      unsigned int	subset_glyph_index,
                              double            x_advance,
                              double            y_advance,
			      int	        latin_character,
			      uint32_t          unicode,
			      char             *utf8,
			      int          	utf8_len)
{
    cairo_sub_font_glyph_t *sub_font_glyph;

    sub_font_glyph = _cairo_malloc (sizeof (cairo_sub_font_glyph_t));
    if (unlikely (sub_font_glyph == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return NULL;
    }

    _cairo_sub_font_glyph_init_key (sub_font_glyph, scaled_font_glyph_index);
    sub_font_glyph->subset_id = subset_id;
    sub_font_glyph->subset_glyph_index = subset_glyph_index;
    sub_font_glyph->x_advance = x_advance;
    sub_font_glyph->y_advance = y_advance;
    sub_font_glyph->is_latin = (latin_character >= 0);
    sub_font_glyph->latin_character = latin_character;
    sub_font_glyph->is_mapped = FALSE;
    sub_font_glyph->unicode = unicode;
    sub_font_glyph->utf8 = utf8;
    sub_font_glyph->utf8_len = utf8_len;

    return sub_font_glyph;
}

static void
_cairo_sub_font_glyph_destroy (cairo_sub_font_glyph_t *sub_font_glyph)
{
    free (sub_font_glyph->utf8);

    free (sub_font_glyph);
}

static void
_cairo_sub_font_glyph_pluck (void *entry, void *closure)
{
    cairo_sub_font_glyph_t *sub_font_glyph = entry;
    cairo_hash_table_t *sub_font_glyphs = closure;

    _cairo_hash_table_remove (sub_font_glyphs, &sub_font_glyph->base);
    _cairo_sub_font_glyph_destroy (sub_font_glyph);
}

static void
_cairo_sub_font_glyph_collect (void *entry, void *closure)
{
    cairo_sub_font_glyph_t *sub_font_glyph = entry;
    cairo_sub_font_collection_t *collection = closure;
    unsigned long scaled_font_glyph_index;
    unsigned int subset_glyph_index;

    if (sub_font_glyph->subset_id != collection->subset_id)
	return;

    scaled_font_glyph_index = sub_font_glyph->base.hash;
    subset_glyph_index = sub_font_glyph->subset_glyph_index;

    /* Ensure we don't exceed the allocated bounds. */
    assert (subset_glyph_index < collection->glyphs_size);

    collection->glyphs[subset_glyph_index] = scaled_font_glyph_index;
    collection->utf8[subset_glyph_index] = sub_font_glyph->utf8;
    collection->to_latin_char[subset_glyph_index] = sub_font_glyph->latin_character;
    if (sub_font_glyph->is_latin)
	collection->latin_to_subset_glyph_index[sub_font_glyph->latin_character] = subset_glyph_index;

    if (subset_glyph_index > collection->max_glyph)
	collection->max_glyph = subset_glyph_index;

    collection->num_glyphs++;
}

static cairo_bool_t
_cairo_sub_fonts_equal (const void *key_a, const void *key_b)
{
    const cairo_sub_font_t *sub_font_a = key_a;
    const cairo_sub_font_t *sub_font_b = key_b;
    cairo_scaled_font_t *a = sub_font_a->scaled_font;
    cairo_scaled_font_t *b = sub_font_b->scaled_font;

    if (sub_font_a->is_scaled)
        return a == b;
    else
	return a->font_face == b->font_face || a->original_font_face == b->original_font_face;
}

static void
_cairo_sub_font_init_key (cairo_sub_font_t	*sub_font,
			  cairo_scaled_font_t	*scaled_font)
{
    if (sub_font->is_scaled)
    {
        sub_font->base.hash = (uintptr_t) scaled_font;
        sub_font->scaled_font = scaled_font;
    }
    else
    {
        sub_font->base.hash = (uintptr_t) scaled_font->font_face;
        sub_font->scaled_font = scaled_font;
    }
}

static cairo_status_t
_cairo_sub_font_create (cairo_scaled_font_subsets_t	*parent,
			cairo_scaled_font_t		*scaled_font,
			unsigned int			 font_id,
			int				 max_glyphs_per_subset,
                        cairo_bool_t                     is_scaled,
			cairo_bool_t                     is_composite,
			cairo_sub_font_t               **sub_font_out)
{
    cairo_sub_font_t *sub_font;
    int i;

    sub_font = _cairo_malloc (sizeof (cairo_sub_font_t));
    if (unlikely (sub_font == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    sub_font->is_scaled = is_scaled;
    sub_font->is_composite = is_composite;
    sub_font->is_user = _cairo_font_face_is_user (scaled_font->font_face);
    sub_font->reserve_notdef = !sub_font->is_user;
    _cairo_sub_font_init_key (sub_font, scaled_font);

    sub_font->parent = parent;
    sub_font->scaled_font = scaled_font;
    sub_font->font_id = font_id;

    sub_font->use_latin_subset = parent->use_latin_subset;

    /* latin subsets of Type 3 and CID CFF fonts are not supported */
    if (sub_font->is_user || sub_font->is_scaled ||
	_cairo_cff_scaled_font_is_cid_cff (scaled_font) )
    {
	sub_font->use_latin_subset = FALSE;
    }

    if (sub_font->use_latin_subset)
	sub_font->current_subset = 1; /* reserve subset 0 for latin glyphs */
    else
	sub_font->current_subset = 0;

    sub_font->num_glyphs_in_current_subset = 0;
    sub_font->num_glyphs_in_latin_subset = 0;
    sub_font->max_glyphs_per_subset = max_glyphs_per_subset;
    for (i = 0; i < 256; i++)
	sub_font->latin_char_map[i] = FALSE;

    sub_font->sub_font_glyphs = _cairo_hash_table_create (NULL);
    if (unlikely (sub_font->sub_font_glyphs == NULL)) {
	free (sub_font);
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }
    sub_font->next = NULL;
    *sub_font_out = sub_font;
    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_sub_font_destroy (cairo_sub_font_t *sub_font)
{
    _cairo_hash_table_foreach (sub_font->sub_font_glyphs,
			       _cairo_sub_font_glyph_pluck,
			       sub_font->sub_font_glyphs);
    _cairo_hash_table_destroy (sub_font->sub_font_glyphs);
    cairo_scaled_font_destroy (sub_font->scaled_font);
    free (sub_font);
}

static void
_cairo_sub_font_pluck (void *entry, void *closure)
{
    cairo_sub_font_t *sub_font = entry;
    cairo_hash_table_t *sub_fonts = closure;

    _cairo_hash_table_remove (sub_fonts, &sub_font->base);
    _cairo_sub_font_destroy (sub_font);
}

/* Characters 0x80 to 0x9f in the winansi encoding.
 * All other characters in the range 0x00 to 0xff map 1:1 to unicode */
static unsigned int _winansi_0x80_to_0x9f[] = {
    0x20ac, 0x0000, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021,
    0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0x0000, 0x017d, 0x0000,
    0x0000, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014,
    0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0x0000, 0x017e, 0x0178
};

int
_cairo_unicode_to_winansi (unsigned long uni)
{
    int i;

    /* exclude the extra "hyphen" at 0xad to avoid duplicate glyphnames */
    if ((uni >= 0x20 && uni <= 0x7e) ||
	(uni >= 0xa1 && uni <= 0xff && uni != 0xad) ||
	uni == 0)
        return uni;

    for (i = 0; i < 32; i++)
	if (_winansi_0x80_to_0x9f[i] == uni)
	    return i + 0x80;

    return -1;
}

static cairo_status_t
_cairo_sub_font_glyph_lookup_unicode (cairo_scaled_font_t    *scaled_font,
				      unsigned long	      scaled_font_glyph_index,
				      uint32_t     	     *unicode_out,
				      char  		    **utf8_out,
				      int          	     *utf8_len_out)
{
    uint32_t unicode;
    char buf[8];
    int len;
    cairo_status_t status;

    /* Do a reverse lookup on the glyph index. unicode is -1 if the
     * index could not be mapped to a unicode character. */
    unicode = -1;
    status = _cairo_truetype_index_to_ucs4 (scaled_font,
					    scaled_font_glyph_index,
					    &unicode);
    if (_cairo_status_is_error (status))
	return status;

    if (unicode == (uint32_t)-1 && scaled_font->backend->index_to_ucs4) {
	status = scaled_font->backend->index_to_ucs4 (scaled_font,
						      scaled_font_glyph_index,
						      &unicode);
	if (unlikely (status))
	    return status;
    }

    *unicode_out = unicode;
    *utf8_out = NULL;
    *utf8_len_out = 0;
    if (unicode != (uint32_t) -1) {
	len = _cairo_ucs4_to_utf8 (unicode, buf);
	if (len > 0) {
            *utf8_out = _cairo_strndup (buf, len);
            if (unlikely (*utf8_out == NULL))
                return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	    *utf8_len_out = len;
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_sub_font_glyph_map_to_unicode (cairo_sub_font_glyph_t *sub_font_glyph,
				      const char	     *utf8,
				      int		      utf8_len,
				      cairo_bool_t	     *is_mapped)
{
    *is_mapped = FALSE;

    if (utf8_len < 0)
	return CAIRO_STATUS_SUCCESS;

    if (utf8 != NULL && utf8_len != 0 && utf8[utf8_len - 1] == '\0')
	utf8_len--;

    if (utf8 != NULL && utf8_len != 0) {
	if (sub_font_glyph->utf8 != NULL) {
	    if (utf8_len == sub_font_glyph->utf8_len &&
		memcmp (utf8, sub_font_glyph->utf8, utf8_len) == 0)
	    {
		/* Requested utf8 mapping matches the existing mapping */
		*is_mapped = TRUE;
	    }
	} else {
	    /* No existing mapping. Use the requested mapping */
            sub_font_glyph->utf8 = _cairo_strndup (utf8, utf8_len);
            if (unlikely (sub_font_glyph->utf8 == NULL))
                return _cairo_error (CAIRO_STATUS_NO_MEMORY);

	    sub_font_glyph->utf8_len = utf8_len;
	    *is_mapped = TRUE;
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_sub_font_lookup_glyph (cairo_sub_font_t	                *sub_font,
                              unsigned long	                 scaled_font_glyph_index,
			      const char			*utf8,
			      int				 utf8_len,
                              cairo_scaled_font_subsets_glyph_t *subset_glyph)
{
    cairo_sub_font_glyph_t key, *sub_font_glyph;
    cairo_int_status_t status;

    _cairo_sub_font_glyph_init_key (&key, scaled_font_glyph_index);
    sub_font_glyph = _cairo_hash_table_lookup (sub_font->sub_font_glyphs,
					      &key.base);
    if (sub_font_glyph != NULL) {
        subset_glyph->font_id = sub_font->font_id;
        subset_glyph->subset_id = sub_font_glyph->subset_id;
	if (sub_font_glyph->is_latin)
	    subset_glyph->subset_glyph_index = sub_font_glyph->latin_character;
	else
	    subset_glyph->subset_glyph_index = sub_font_glyph->subset_glyph_index;

        subset_glyph->is_scaled = sub_font->is_scaled;
        subset_glyph->is_composite = sub_font->is_composite;
	subset_glyph->is_latin = sub_font_glyph->is_latin;
        subset_glyph->x_advance = sub_font_glyph->x_advance;
        subset_glyph->y_advance = sub_font_glyph->y_advance;
	status = _cairo_sub_font_glyph_map_to_unicode (sub_font_glyph,
						       utf8, utf8_len,
						       &subset_glyph->utf8_is_mapped);
	subset_glyph->unicode = sub_font_glyph->unicode;

	return status;
    }

    return CAIRO_INT_STATUS_UNSUPPORTED;
}

static cairo_status_t
_cairo_sub_font_add_glyph (cairo_sub_font_t	   *sub_font,
			   unsigned long	    scaled_font_glyph_index,
			   cairo_bool_t		    is_latin,
			   int			    latin_character,
			   uint32_t 		    unicode,
			   char 		   *utf8,
			   int 			    utf8_len,
			   cairo_sub_font_glyph_t **sub_font_glyph_out)
{
    cairo_scaled_glyph_t *scaled_glyph;
    cairo_sub_font_glyph_t *sub_font_glyph;
    int *num_glyphs_in_subset_ptr;
    double x_advance;
    double y_advance;
    cairo_int_status_t status;

    _cairo_scaled_font_freeze_cache (sub_font->scaled_font);
    status = _cairo_scaled_glyph_lookup (sub_font->scaled_font,
					 scaled_font_glyph_index,
					 CAIRO_SCALED_GLYPH_INFO_METRICS,
					 NULL, /* foreground color */
					 &scaled_glyph);
    assert (status != CAIRO_INT_STATUS_UNSUPPORTED);
    if (unlikely (status)) {
	_cairo_scaled_font_thaw_cache (sub_font->scaled_font);
	return status;
    }

    x_advance = scaled_glyph->metrics.x_advance;
    y_advance = scaled_glyph->metrics.y_advance;
    _cairo_scaled_font_thaw_cache (sub_font->scaled_font);

    if (!is_latin && sub_font->num_glyphs_in_current_subset == sub_font->max_glyphs_per_subset)
    {
	sub_font->current_subset++;
	sub_font->num_glyphs_in_current_subset = 0;
    }

    if (is_latin)
	num_glyphs_in_subset_ptr = &sub_font->num_glyphs_in_latin_subset;
    else
	num_glyphs_in_subset_ptr = &sub_font->num_glyphs_in_current_subset;

    if ((*num_glyphs_in_subset_ptr == 0) && sub_font->reserve_notdef)
	(*num_glyphs_in_subset_ptr)++;

    sub_font_glyph = _cairo_sub_font_glyph_create (scaled_font_glyph_index,
						   is_latin ? 0 : sub_font->current_subset,
						   *num_glyphs_in_subset_ptr,
						   x_advance,
						   y_advance,
						   is_latin ? latin_character : -1,
						   unicode,
						   utf8,
						   utf8_len);

    if (unlikely (sub_font_glyph == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    status = _cairo_hash_table_insert (sub_font->sub_font_glyphs, &sub_font_glyph->base);
    if (unlikely (status)) {
	_cairo_sub_font_glyph_destroy (sub_font_glyph);
	return status;
    }

    (*num_glyphs_in_subset_ptr)++;
    if (sub_font->is_scaled) {
	if (*num_glyphs_in_subset_ptr > sub_font->parent->max_glyphs_per_scaled_subset_used)
	    sub_font->parent->max_glyphs_per_scaled_subset_used = *num_glyphs_in_subset_ptr;
    } else {
	if (*num_glyphs_in_subset_ptr > sub_font->parent->max_glyphs_per_unscaled_subset_used)
	    sub_font->parent->max_glyphs_per_unscaled_subset_used = *num_glyphs_in_subset_ptr;
    }

    *sub_font_glyph_out = sub_font_glyph;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_sub_font_map_glyph (cairo_sub_font_t	*sub_font,
			   unsigned long	 scaled_font_glyph_index,
			   const char		*text_utf8,
			   int			 text_utf8_len,
                           cairo_scaled_font_subsets_glyph_t *subset_glyph)
{
    cairo_sub_font_glyph_t key, *sub_font_glyph;
    cairo_status_t status;

    _cairo_sub_font_glyph_init_key (&key, scaled_font_glyph_index);
    sub_font_glyph = _cairo_hash_table_lookup (sub_font->sub_font_glyphs,
					       &key.base);
    if (sub_font_glyph == NULL) {
	uint32_t font_unicode;
	char *font_utf8;
	int font_utf8_len;
	cairo_bool_t is_latin;
	int latin_character;

	status = _cairo_sub_font_glyph_lookup_unicode (sub_font->scaled_font,
							   scaled_font_glyph_index,
							   &font_unicode,
							   &font_utf8,
							   &font_utf8_len);
	if (unlikely(status))
	    return status;

	/* If the supplied utf8 is a valid single character, use it
	 * instead of the font lookup */
	if (text_utf8 != NULL && text_utf8_len > 0) {
	    uint32_t  *ucs4;
	    int	ucs4_len;

	    status = _cairo_utf8_to_ucs4 (text_utf8, text_utf8_len,
					  &ucs4, &ucs4_len);
	    if (status == CAIRO_STATUS_SUCCESS) {
		if (ucs4_len == 1) {
		    font_unicode = ucs4[0];
		    free (font_utf8);
                    font_utf8 = _cairo_strndup (text_utf8, text_utf8_len);
                    if (font_utf8 == NULL) {
                        free (ucs4);
                        return _cairo_error (CAIRO_STATUS_NO_MEMORY);
		    }
		    font_utf8_len = text_utf8_len;
		}
		free (ucs4);
	    }
	}

	/* If glyph is in the winansi encoding and font is not a user
	 * font, put glyph in the latin subset. */
	is_latin = FALSE;
	latin_character = -1;
	if (sub_font->use_latin_subset &&
	    (! _cairo_font_face_is_user (sub_font->scaled_font->font_face)))
	{
	    latin_character = _cairo_unicode_to_winansi (font_unicode);
	    if (latin_character > 0)
	    {
		if (!sub_font->latin_char_map[latin_character]) {
		    sub_font->latin_char_map[latin_character] = TRUE;
		    is_latin = TRUE;
		}
	    }
	}

	status = _cairo_sub_font_add_glyph (sub_font,
					    scaled_font_glyph_index,
					    is_latin,
					    latin_character,
					    font_unicode,
					    font_utf8,
					    font_utf8_len,
					    &sub_font_glyph);
	if (unlikely(status))
	    return status;
    }

    subset_glyph->font_id = sub_font->font_id;
    subset_glyph->subset_id = sub_font_glyph->subset_id;
    if (sub_font_glyph->is_latin)
	subset_glyph->subset_glyph_index = sub_font_glyph->latin_character;
    else
	subset_glyph->subset_glyph_index = sub_font_glyph->subset_glyph_index;

    subset_glyph->is_scaled = sub_font->is_scaled;
    subset_glyph->is_composite = sub_font->is_composite;
    subset_glyph->is_latin = sub_font_glyph->is_latin;
    subset_glyph->x_advance = sub_font_glyph->x_advance;
    subset_glyph->y_advance = sub_font_glyph->y_advance;
    status = _cairo_sub_font_glyph_map_to_unicode (sub_font_glyph,
						   text_utf8, text_utf8_len,
						   &subset_glyph->utf8_is_mapped);
    subset_glyph->unicode = sub_font_glyph->unicode;

    return status;
}

static void
_cairo_sub_font_collect (void *entry, void *closure)
{
    cairo_sub_font_t *sub_font = entry;
    cairo_sub_font_collection_t *collection = closure;
    cairo_scaled_font_subset_t subset;
    int i;
    unsigned int j;

    if (collection->status)
	return;

    collection->status = sub_font->scaled_font->status;
    if (collection->status)
	return;

    for (i = 0; i <= sub_font->current_subset; i++) {
	collection->subset_id = i;
	collection->num_glyphs = 0;
	collection->max_glyph = 0;
	memset (collection->latin_to_subset_glyph_index, 0, 256*sizeof(unsigned long));

	if (sub_font->reserve_notdef) {
	    // add .notdef
	    collection->glyphs[0] = 0;
	    collection->utf8[0] = 0;
	    collection->to_latin_char[0] = 0;
	    collection->latin_to_subset_glyph_index[0] = 0;
	    collection->num_glyphs++;
	}

	_cairo_hash_table_foreach (sub_font->sub_font_glyphs,
				   _cairo_sub_font_glyph_collect, collection);
	if (collection->status)
	    break;

	if (collection->num_glyphs == 0)
	    continue;

	if (sub_font->reserve_notdef && collection->num_glyphs == 1)
	    continue;

        /* Ensure the resulting array has no uninitialized holes */
	assert (collection->num_glyphs == collection->max_glyph + 1);

	subset.scaled_font = sub_font->scaled_font;
	subset.is_composite = sub_font->is_composite;
	subset.is_scaled = sub_font->is_scaled;
	subset.font_id = sub_font->font_id;
	subset.subset_id = i;
	subset.glyphs = collection->glyphs;
	subset.utf8 = collection->utf8;
	subset.num_glyphs = collection->num_glyphs;
        subset.glyph_names = NULL;

	subset.is_latin = FALSE;
	if (sub_font->use_latin_subset && i == 0) {
	    subset.is_latin = TRUE;
	    subset.to_latin_char = collection->to_latin_char;
	    subset.latin_to_subset_glyph_index = collection->latin_to_subset_glyph_index;
	} else {
	    subset.to_latin_char = NULL;
	    subset.latin_to_subset_glyph_index = NULL;
	}

        collection->status = (collection->font_subset_callback) (&subset,
					    collection->font_subset_callback_closure);

	if (subset.glyph_names != NULL) {
            for (j = 0; j < collection->num_glyphs; j++)
		free (subset.glyph_names[j]);
	    free (subset.glyph_names);
	}

	if (collection->status)
	    break;
    }
}

static cairo_scaled_font_subsets_t *
_cairo_scaled_font_subsets_create_internal (cairo_subsets_type_t type)
{
    cairo_scaled_font_subsets_t *subsets;

    subsets = _cairo_malloc (sizeof (cairo_scaled_font_subsets_t));
    if (unlikely (subsets == NULL)) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return NULL;
    }

    subsets->type = type;
    subsets->use_latin_subset = FALSE;
    subsets->max_glyphs_per_unscaled_subset_used = 0;
    subsets->max_glyphs_per_scaled_subset_used = 0;
    subsets->num_sub_fonts = 0;

    subsets->unscaled_sub_fonts = _cairo_hash_table_create (_cairo_sub_fonts_equal);
    if (! subsets->unscaled_sub_fonts) {
	free (subsets);
	return NULL;
    }
    subsets->unscaled_sub_fonts_list = NULL;
    subsets->unscaled_sub_fonts_list_end = NULL;

    subsets->scaled_sub_fonts = _cairo_hash_table_create (_cairo_sub_fonts_equal);
    if (! subsets->scaled_sub_fonts) {
	_cairo_hash_table_destroy (subsets->unscaled_sub_fonts);
	free (subsets);
	return NULL;
    }
    subsets->scaled_sub_fonts_list = NULL;
    subsets->scaled_sub_fonts_list_end = NULL;

    return subsets;
}

cairo_scaled_font_subsets_t *
_cairo_scaled_font_subsets_create_scaled (void)
{
    return _cairo_scaled_font_subsets_create_internal (CAIRO_SUBSETS_SCALED);
}

cairo_scaled_font_subsets_t *
_cairo_scaled_font_subsets_create_simple (void)
{
    return _cairo_scaled_font_subsets_create_internal (CAIRO_SUBSETS_SIMPLE);
}

cairo_scaled_font_subsets_t *
_cairo_scaled_font_subsets_create_composite (void)
{
    return _cairo_scaled_font_subsets_create_internal (CAIRO_SUBSETS_COMPOSITE);
}

void
_cairo_scaled_font_subsets_destroy (cairo_scaled_font_subsets_t *subsets)
{
    _cairo_hash_table_foreach (subsets->scaled_sub_fonts, _cairo_sub_font_pluck, subsets->scaled_sub_fonts);
    _cairo_hash_table_destroy (subsets->scaled_sub_fonts);

    _cairo_hash_table_foreach (subsets->unscaled_sub_fonts, _cairo_sub_font_pluck, subsets->unscaled_sub_fonts);
    _cairo_hash_table_destroy (subsets->unscaled_sub_fonts);

    free (subsets);
}

void
_cairo_scaled_font_subsets_enable_latin_subset (cairo_scaled_font_subsets_t *font_subsets,
						cairo_bool_t                 use_latin)
{
    font_subsets->use_latin_subset = use_latin;
}

cairo_status_t
_cairo_scaled_font_subsets_map_glyph (cairo_scaled_font_subsets_t	*subsets,
				      cairo_scaled_font_t		*scaled_font,
				      unsigned long			 scaled_font_glyph_index,
				      const char *			 utf8,
				      int				 utf8_len,
                                      cairo_scaled_font_subsets_glyph_t *subset_glyph)
{
    cairo_sub_font_t key, *sub_font;
    cairo_scaled_glyph_t *scaled_glyph;
    cairo_font_face_t *font_face;
    cairo_matrix_t identity;
    cairo_font_options_t font_options;
    cairo_scaled_font_t	*unscaled_font;
    cairo_int_status_t status;
    int max_glyphs;
    cairo_bool_t type1_font;

    /* Lookup glyph in unscaled subsets */
    if (subsets->type != CAIRO_SUBSETS_SCALED) {
        key.is_scaled = FALSE;
        _cairo_sub_font_init_key (&key, scaled_font);
	sub_font = _cairo_hash_table_lookup (subsets->unscaled_sub_fonts,
					     &key.base);
        if (sub_font != NULL) {
            status = _cairo_sub_font_lookup_glyph (sub_font,
						   scaled_font_glyph_index,
						   utf8, utf8_len,
						   subset_glyph);
	    if (status != CAIRO_INT_STATUS_UNSUPPORTED)
                return status;
        }
    }

    /* Lookup glyph in scaled subsets */
    key.is_scaled = TRUE;
    _cairo_sub_font_init_key (&key, scaled_font);
    sub_font = _cairo_hash_table_lookup (subsets->scaled_sub_fonts,
					 &key.base);
    if (sub_font != NULL) {
	status = _cairo_sub_font_lookup_glyph (sub_font,
					       scaled_font_glyph_index,
					       utf8, utf8_len,
					       subset_glyph);
	if (status != CAIRO_INT_STATUS_UNSUPPORTED)
	    return status;
    }

    /* Glyph not found. Determine whether the glyph is outline or
     * bitmap and add to the appropriate subset.
     *
     * glyph_index 0 (the .notdef glyph) is a special case. Some fonts
     * will return CAIRO_INT_STATUS_UNSUPPORTED when doing a
     * _scaled_glyph_lookup(_GLYPH_INFO_PATH). Type1-fallback creates
     * empty glyphs in this case so we can put the glyph in a unscaled
     * subset. */
    if (scaled_font_glyph_index == 0 ||
	_cairo_font_face_is_user (scaled_font->font_face)) {
	status = CAIRO_STATUS_SUCCESS;
    } else {
	_cairo_scaled_font_freeze_cache (scaled_font);
	status = _cairo_scaled_glyph_lookup (scaled_font,
					     scaled_font_glyph_index,
					     CAIRO_SCALED_GLYPH_INFO_PATH,
                                             NULL, /* foreground color */
					     &scaled_glyph);
	_cairo_scaled_font_thaw_cache (scaled_font);
    }
    if (_cairo_int_status_is_error (status))
        return status;

    if (status == CAIRO_INT_STATUS_SUCCESS &&
	subsets->type != CAIRO_SUBSETS_SCALED &&
	! _cairo_font_face_is_user (scaled_font->font_face))
    {
        /* Path available. Add to unscaled subset. */
        key.is_scaled = FALSE;
        _cairo_sub_font_init_key (&key, scaled_font);
	sub_font = _cairo_hash_table_lookup (subsets->unscaled_sub_fonts,
					     &key.base);
        if (sub_font == NULL) {
            font_face = cairo_scaled_font_get_font_face (scaled_font);
            cairo_matrix_init_identity (&identity);
            _cairo_font_options_init_default (&font_options);
            cairo_font_options_set_hint_style (&font_options, CAIRO_HINT_STYLE_NONE);
            cairo_font_options_set_hint_metrics (&font_options, CAIRO_HINT_METRICS_OFF);
            unscaled_font = cairo_scaled_font_create (font_face,
                                                      &identity,
                                                      &identity,
                                                      &font_options);
	    if (unlikely (unscaled_font->status))
		return unscaled_font->status;

            subset_glyph->is_scaled = FALSE;
            type1_font = _cairo_type1_scaled_font_is_type1 (unscaled_font);
            if (subsets->type == CAIRO_SUBSETS_COMPOSITE && !type1_font) {
                max_glyphs = MAX_GLYPHS_PER_COMPOSITE_FONT;
                subset_glyph->is_composite = TRUE;
            } else {
                max_glyphs = MAX_GLYPHS_PER_SIMPLE_FONT;
                subset_glyph->is_composite = FALSE;
            }

            status = _cairo_sub_font_create (subsets,
					     unscaled_font,
					     subsets->num_sub_fonts,
					     max_glyphs,
					     subset_glyph->is_scaled,
					     subset_glyph->is_composite,
					     &sub_font);

            if (unlikely (status)) {
		cairo_scaled_font_destroy (unscaled_font);
                return status;
	    }

            status = _cairo_hash_table_insert (subsets->unscaled_sub_fonts,
                                               &sub_font->base);

            if (unlikely (status)) {
		_cairo_sub_font_destroy (sub_font);
                return status;
	    }
	    if (!subsets->unscaled_sub_fonts_list)
		subsets->unscaled_sub_fonts_list = sub_font;
	    else
		subsets->unscaled_sub_fonts_list_end->next = sub_font;
	    subsets->unscaled_sub_fonts_list_end = sub_font;
	    subsets->num_sub_fonts++;
        }
    } else {
        /* No path available. Add to scaled subset. */
        key.is_scaled = TRUE;
        _cairo_sub_font_init_key (&key, scaled_font);
	sub_font = _cairo_hash_table_lookup (subsets->scaled_sub_fonts,
					     &key.base);
        if (sub_font == NULL) {
            subset_glyph->is_scaled = TRUE;
            subset_glyph->is_composite = FALSE;
            if (subsets->type == CAIRO_SUBSETS_SCALED)
                max_glyphs = INT_MAX;
            else
                max_glyphs = MAX_GLYPHS_PER_SIMPLE_FONT;

            status = _cairo_sub_font_create (subsets,
					     cairo_scaled_font_reference (scaled_font),
					     subsets->num_sub_fonts,
					     max_glyphs,
					     subset_glyph->is_scaled,
					     subset_glyph->is_composite,
					     &sub_font);
            if (unlikely (status)) {
		cairo_scaled_font_destroy (scaled_font);
                return status;
	    }

            status = _cairo_hash_table_insert (subsets->scaled_sub_fonts,
                                               &sub_font->base);
            if (unlikely (status)) {
		_cairo_sub_font_destroy (sub_font);
                return status;
	    }
	    if (!subsets->scaled_sub_fonts_list)
		subsets->scaled_sub_fonts_list = sub_font;
	    else
		subsets->scaled_sub_fonts_list_end->next = sub_font;
	    subsets->scaled_sub_fonts_list_end = sub_font;
	    subsets->num_sub_fonts++;
        }
    }

    return _cairo_sub_font_map_glyph (sub_font,
				      scaled_font_glyph_index,
				      utf8, utf8_len,
				      subset_glyph);
}

static cairo_status_t
_cairo_scaled_font_subsets_foreach_internal (cairo_scaled_font_subsets_t              *font_subsets,
                                             cairo_scaled_font_subset_callback_func_t  font_subset_callback,
                                             void				      *closure,
					     cairo_subsets_foreach_type_t	       type)
{
    cairo_sub_font_collection_t collection;
    cairo_sub_font_t *sub_font;
    cairo_bool_t is_scaled, is_user;

    is_scaled = FALSE;
    is_user = FALSE;

    if (type == CAIRO_SUBSETS_FOREACH_USER)
	is_user = TRUE;

    if (type == CAIRO_SUBSETS_FOREACH_SCALED ||
	type == CAIRO_SUBSETS_FOREACH_USER)
    {
	is_scaled = TRUE;
    }

    if (is_scaled)
        collection.glyphs_size = font_subsets->max_glyphs_per_scaled_subset_used;
    else
        collection.glyphs_size = font_subsets->max_glyphs_per_unscaled_subset_used;

    if (! collection.glyphs_size)
	return CAIRO_STATUS_SUCCESS;

    collection.glyphs = _cairo_malloc_ab (collection.glyphs_size, sizeof(unsigned long));
    collection.utf8 = _cairo_malloc_ab (collection.glyphs_size, sizeof(char *));
    collection.to_latin_char = _cairo_malloc_ab (collection.glyphs_size, sizeof(int));
    collection.latin_to_subset_glyph_index = _cairo_malloc_ab (256, sizeof(unsigned long));
    if (unlikely (collection.glyphs == NULL ||
		  collection.utf8 == NULL ||
		  collection.to_latin_char == NULL ||
		  collection.latin_to_subset_glyph_index == NULL)) {
	free (collection.glyphs);
	free (collection.utf8);
	free (collection.to_latin_char);
	free (collection.latin_to_subset_glyph_index);

	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    collection.font_subset_callback = font_subset_callback;
    collection.font_subset_callback_closure = closure;
    collection.status = CAIRO_STATUS_SUCCESS;

    if (is_scaled)
	sub_font = font_subsets->scaled_sub_fonts_list;
    else
	sub_font = font_subsets->unscaled_sub_fonts_list;

    while (sub_font) {
	if (sub_font->is_user == is_user)
	    _cairo_sub_font_collect (sub_font, &collection);

	sub_font = sub_font->next;
    }
    free (collection.utf8);
    free (collection.glyphs);
    free (collection.to_latin_char);
    free (collection.latin_to_subset_glyph_index);

    return collection.status;
}

cairo_status_t
_cairo_scaled_font_subsets_foreach_scaled (cairo_scaled_font_subsets_t		    *font_subsets,
                                           cairo_scaled_font_subset_callback_func_t  font_subset_callback,
                                           void					    *closure)
{
    return _cairo_scaled_font_subsets_foreach_internal (font_subsets,
                                                        font_subset_callback,
                                                        closure,
							CAIRO_SUBSETS_FOREACH_SCALED);
}

cairo_status_t
_cairo_scaled_font_subsets_foreach_unscaled (cairo_scaled_font_subsets_t	    *font_subsets,
                                           cairo_scaled_font_subset_callback_func_t  font_subset_callback,
                                           void					    *closure)
{
    return _cairo_scaled_font_subsets_foreach_internal (font_subsets,
                                                        font_subset_callback,
                                                        closure,
							CAIRO_SUBSETS_FOREACH_UNSCALED);
}

cairo_status_t
_cairo_scaled_font_subsets_foreach_user (cairo_scaled_font_subsets_t		  *font_subsets,
					 cairo_scaled_font_subset_callback_func_t  font_subset_callback,
					 void					  *closure)
{
    return _cairo_scaled_font_subsets_foreach_internal (font_subsets,
                                                        font_subset_callback,
                                                        closure,
							CAIRO_SUBSETS_FOREACH_USER);
}

static cairo_bool_t
_cairo_string_equal (const void *key_a, const void *key_b)
{
    const cairo_string_entry_t *a = key_a;
    const cairo_string_entry_t *b = key_b;

    if (strcmp (a->string, b->string) == 0)
	return TRUE;
    else
	return FALSE;
}

#if DEBUG_SUBSETS

static void
dump_glyph (void *entry, void *closure)
{
    cairo_sub_font_glyph_t *glyph = entry;
    char buf[10];
    int i;

    printf("    font_glyph_index: %ld\n", glyph->base.hash);
    printf("      subset_id: %d\n", glyph->subset_id);
    printf("      subset_glyph_index: %d\n", glyph->subset_glyph_index);
    printf("      x_advance: %f\n", glyph->x_advance);
    printf("      y_advance: %f\n", glyph->y_advance);
    printf("      is_latin: %d\n", glyph->is_latin);
    printf("      latin_character: '%c' (0x%02x)\n", glyph->latin_character, glyph->latin_character);
    printf("      is_latin: %d\n", glyph->is_latin);
    printf("      is_mapped: %d\n", glyph->is_mapped);
    printf("      unicode: U+%04x\n", glyph->unicode);
    memset(buf, 0, sizeof(buf));
    memcpy(buf, glyph->utf8, glyph->utf8_len);
    printf("      utf8: '%s'\n", buf);
    printf("      utf8 (hex):");
    for (i = 0; i < glyph->utf8_len; i++)
	printf(" 0x%02x", glyph->utf8[i]);
    printf("\n\n");
}

static void
dump_subfont (cairo_sub_font_t *sub_font)
{
    while (sub_font) {
	printf("    font_id: %d\n", sub_font->font_id);
	printf("    current_subset: %d\n", sub_font->current_subset);
	printf("    is_scaled: %d\n", sub_font->is_scaled);
	printf("    is_composite: %d\n", sub_font->is_composite);
	printf("    is_user: %d\n", sub_font->is_user);
	printf("    use_latin_subset: %d\n", sub_font->use_latin_subset);
	printf("    reserve_notdef: %d\n", sub_font->reserve_notdef);
	printf("    num_glyphs_in_current_subset: %d\n", sub_font->num_glyphs_in_current_subset);
	printf("    num_glyphs_in_latin_subset: %d\n", sub_font->num_glyphs_in_latin_subset);
	printf("    max_glyphs_per_subset: %d\n\n", sub_font->max_glyphs_per_subset);

	_cairo_hash_table_foreach (sub_font->sub_font_glyphs, dump_glyph, NULL);

	printf("\n");
	sub_font = sub_font->next;
    }
}

void
dump_scaled_font_subsets (cairo_scaled_font_subsets_t *font_subsets)
{
    printf("font subsets\n");
    switch (font_subsets->type)
    {
	case CAIRO_SUBSETS_SCALED:
	    printf("  type: CAIRO_SUBSETS_SCALED\n");
	    break;
	case CAIRO_SUBSETS_SIMPLE:
	    printf("  type: CAIRO_SUBSETS_SIMPLE\n");
	    break;
	case CAIRO_SUBSETS_COMPOSITE:
	    printf("  type: CAIRO_SUBSETS_COMPOSITE\n");
	    break;
    }
    printf("  use_latin_subset: %d\n", font_subsets->use_latin_subset);
    printf("  max_glyphs_per_unscaled_subset_used: %d\n", font_subsets->max_glyphs_per_unscaled_subset_used);
    printf("  max_glyphs_per_scaled_subset_used: %d\n", font_subsets->max_glyphs_per_scaled_subset_used);
    printf("  num_sub_fonts: %d\n\n", font_subsets->num_sub_fonts);

    printf("  scaled subsets:\n");
    dump_subfont (font_subsets->scaled_sub_fonts_list);

    printf("\n  unscaled subsets:\n");
    dump_subfont (font_subsets->unscaled_sub_fonts_list);
}

#endif


static void
_cairo_string_init_key (cairo_string_entry_t *key, char *s)
{
    unsigned long sum = 0;
    unsigned int i;

    for (i = 0; i < strlen(s); i++)
        sum += s[i];
    key->base.hash = sum;
    key->string = s;
}

static cairo_status_t
create_string_entry (char *s, cairo_string_entry_t **entry)
{
    *entry = _cairo_malloc (sizeof (cairo_string_entry_t));
    if (unlikely (*entry == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    _cairo_string_init_key (*entry, s);

    return CAIRO_STATUS_SUCCESS;
}

static void
_pluck_entry (void *entry, void *closure)
{
    _cairo_hash_table_remove (closure, entry);
    free (entry);
}

cairo_int_status_t
_cairo_scaled_font_subset_create_glyph_names (cairo_scaled_font_subset_t *subset)
{
    unsigned int i;
    cairo_hash_table_t *names;
    cairo_string_entry_t key, *entry;
    char buf[30];
    char *utf8;
    uint16_t *utf16;
    int utf16_len;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    names = _cairo_hash_table_create (_cairo_string_equal);
    if (unlikely (names == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    subset->glyph_names = calloc (subset->num_glyphs, sizeof (char *));
    if (unlikely (subset->glyph_names == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto CLEANUP_HASH;
    }

    i = 0;
    if (! subset->is_scaled) {
	subset->glyph_names[0] = strdup (".notdef");
	if (unlikely (subset->glyph_names[0] == NULL)) {
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    goto CLEANUP_HASH;
	}

	status = create_string_entry (subset->glyph_names[0], &entry);
	if (unlikely (status))
	    goto CLEANUP_HASH;

	status = _cairo_hash_table_insert (names, &entry->base);
	if (unlikely (status)) {
	    free (entry);
	    goto CLEANUP_HASH;
	}
	i++;
    }

    for (; i < subset->num_glyphs; i++) {
	utf8 = subset->utf8[i];
	utf16 = NULL;
	utf16_len = 0;
	if (utf8 && *utf8) {
	    status = _cairo_utf8_to_utf16 (utf8, -1, &utf16, &utf16_len);
	    if (status == CAIRO_STATUS_INVALID_STRING) {
		utf16 = NULL;
		utf16_len = 0;
	    } else if (unlikely (status)) {
		goto CLEANUP_HASH;
	    }
	}

	if (utf16_len == 1) {
	    int ch = _cairo_unicode_to_winansi (utf16[0]);
	    if (ch > 0 && _cairo_winansi_to_glyphname (ch)) {
		strncpy (buf, _cairo_winansi_to_glyphname (ch), sizeof (buf));
		buf[sizeof (buf)-1] = '\0';
	    } else {
		snprintf (buf, sizeof (buf), "uni%04X", (int) utf16[0]);
	    }

	    _cairo_string_init_key (&key, buf);
	    entry = _cairo_hash_table_lookup (names, &key.base);
	    if (entry != NULL)
		snprintf (buf, sizeof (buf), "g%d", i);
	} else {
	    snprintf (buf, sizeof (buf), "g%d", i);
	}
	free (utf16);

	subset->glyph_names[i] = strdup (buf);
	if (unlikely (subset->glyph_names[i] == NULL)) {
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    goto CLEANUP_HASH;
	}

	status = create_string_entry (subset->glyph_names[i], &entry);
	if (unlikely (status))
	    goto CLEANUP_HASH;

	status = _cairo_hash_table_insert (names, &entry->base);
	if (unlikely (status)) {
	    free (entry);
	    goto CLEANUP_HASH;
	}
    }

CLEANUP_HASH:
    _cairo_hash_table_foreach (names, _pluck_entry, names);
    _cairo_hash_table_destroy (names);

    if (likely (status == CAIRO_STATUS_SUCCESS))
	return CAIRO_STATUS_SUCCESS;

    if (subset->glyph_names != NULL) {
	for (i = 0; i < subset->num_glyphs; i++) {
	    free (subset->glyph_names[i]);
	}

	free (subset->glyph_names);
	subset->glyph_names = NULL;
    }

    return status;
}

cairo_int_status_t
_cairo_escape_ps_name (char **ps_name)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    /* Ensure PS name is a valid PDF/PS name object. In PDF names are
     * treated as UTF8 and non ASCII bytes, ' ', and '#' are encoded
     * as '#' followed by 2 hex digits that encode the byte. By also
     * encoding the characters in the reserved string we ensure the
     * name is also PS compatible. */
    if (*ps_name) {
	static const char *reserved = "()<>[]{}/%#\\";
	char buf[128]; /* max name length is 127 bytes */
	char *src = *ps_name;
	char *dst = buf;

	while (*src && dst < buf + 127) {
	    unsigned char c = *src;
	    if (c < 0x21 || c > 0x7e || strchr (reserved, c)) {
		if (dst + 4 > buf + 127)
		    break;

		snprintf (dst, 4, "#%02X", c);
		src++;
		dst += 3;
	    } else {
		*dst++ = *src++;
	    }
	}
	*dst = 0;
	free (*ps_name);
	*ps_name = strdup (buf);
	if (*ps_name == NULL) {
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	}
    }

    return status;
}

#endif /* CAIRO_HAS_FONT_SUBSET */
