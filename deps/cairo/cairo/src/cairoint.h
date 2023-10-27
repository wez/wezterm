/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2005 Red Hat, Inc.
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
 */

/*
 * These definitions are solely for use by the implementation of cairo
 * and constitute no kind of standard.  If you need any of these
 * functions, please drop me a note.  Either the library needs new
 * functionality, or there's a way to do what you need using the
 * existing published interfaces. cworth@cworth.org
 */

#ifndef _CAIROINT_H_
#define _CAIROINT_H_

#include "config.h"

#ifdef _MSC_VER
#define cairo_public __declspec(dllexport)
#endif

#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <stddef.h>

#ifdef _MSC_VER
#define _USE_MATH_DEFINES
#endif
#include <math.h>
#include <limits.h>
#include <stdio.h>

#include "cairo.h"
#include <pixman.h>

#include "cairo-compiler-private.h"
#include "cairo-error-private.h"

#if CAIRO_HAS_PDF_SURFACE    || \
    CAIRO_HAS_PS_SURFACE     || \
    CAIRO_HAS_SCRIPT_SURFACE || \
    CAIRO_HAS_XML_SURFACE
#define CAIRO_HAS_DEFLATE_STREAM 1
#endif

#if CAIRO_HAS_PS_SURFACE  || \
    CAIRO_HAS_PDF_SURFACE || \
    CAIRO_HAS_SVG_SURFACE || \
    CAIRO_HAS_WIN32_SURFACE
#define CAIRO_HAS_FONT_SUBSET 1
#endif

#if CAIRO_HAS_PS_SURFACE  || \
    CAIRO_HAS_PDF_SURFACE || \
    CAIRO_HAS_FONT_SUBSET
#define CAIRO_HAS_PDF_OPERATORS 1
#endif

CAIRO_BEGIN_DECLS

#if _WIN32 && !_WIN32_WCE /* Permissions on WinCE? No worries! */
cairo_private FILE *
_cairo_win32_tmpfile (void);
#define tmpfile() _cairo_win32_tmpfile()
#endif

#undef MIN
#define MIN(a, b) ((a) < (b) ? (a) : (b))

#undef MAX
#define MAX(a, b) ((a) > (b) ? (a) : (b))

#if _XOPEN_SOURCE >= 600 || defined (_ISOC99_SOURCE)
#define ISFINITE(x) isfinite (x)
#else
#define ISFINITE(x) ((x) * (x) >= 0.) /* check for NaNs */
#endif

#ifndef FALSE
#define FALSE 0
#endif

#ifndef TRUE
#define TRUE 1
#endif

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

#ifndef M_SQRT2
#define M_SQRT2 1.41421356237309504880
#endif

#ifndef M_SQRT1_2
#define M_SQRT1_2 0.707106781186547524400844362104849039
#endif

#undef  ARRAY_LENGTH
#define ARRAY_LENGTH(__array) ((int) (sizeof (__array) / sizeof (__array[0])))

#undef STRINGIFY
#undef STRINGIFY_ARG
#define STRINGIFY(macro_or_string)    STRINGIFY_ARG (macro_or_string)
#define STRINGIFY_ARG(contents)       #contents

#if defined (__GNUC__)
#define cairo_container_of(ptr, type, member) ({ \
    const __typeof__ (((type *) 0)->member) *mptr__ = (ptr); \
    (type *) ((char *) mptr__ - offsetof (type, member)); \
})
#else
#define cairo_container_of(ptr, type, member) \
    ((type *)((char *) (ptr) - (char *) &((type *)0)->member))
#endif


#define ASSERT_NOT_REACHED		\
do {					\
    assert (!"reached");		\
} while (0)
#define COMPILE_TIME_ASSERT1(condition, line)		\
    typedef int compile_time_assertion_at_line_##line##_failed [(condition)?1:-1]
#define COMPILE_TIME_ASSERT0(condition, line)	COMPILE_TIME_ASSERT1(condition, line)
#define COMPILE_TIME_ASSERT(condition)		COMPILE_TIME_ASSERT0(condition, __LINE__)

#define CAIRO_ALPHA_IS_CLEAR(alpha) ((alpha) <= ((double)0x00ff / (double)0xffff))
#define CAIRO_ALPHA_SHORT_IS_CLEAR(alpha) ((alpha) <= 0x00ff)

#define CAIRO_ALPHA_IS_OPAQUE(alpha) ((alpha) >= ((double)0xff00 / (double)0xffff))
#define CAIRO_ALPHA_SHORT_IS_OPAQUE(alpha) ((alpha) >= 0xff00)
#define CAIRO_ALPHA_IS_ZERO(alpha) ((alpha) <= 0.0)

#define CAIRO_COLOR_IS_CLEAR(color) CAIRO_ALPHA_SHORT_IS_CLEAR ((color)->alpha_short)
#define CAIRO_COLOR_IS_OPAQUE(color) CAIRO_ALPHA_SHORT_IS_OPAQUE ((color)->alpha_short)

/* Reverse the bits in a byte with 7 operations (no 64-bit):
 * Devised by Sean Anderson, July 13, 2001.
 * Source: http://graphics.stanford.edu/~seander/bithacks.html#ReverseByteWith32Bits
 */
#define CAIRO_BITSWAP8(c) ((((c) * 0x0802LU & 0x22110LU) | ((c) * 0x8020LU & 0x88440LU)) * 0x10101LU >> 16)

/* Return the number of 1 bits in mask.
 *
 * GCC 3.4 supports a "population count" builtin, which on many targets is
 * implemented with a single instruction. There is a fallback definition
 * in libgcc in case a target does not have one, which should be just as
 * good as the open-coded solution below, (which is "HACKMEM 169").
 */
static inline int cairo_const
_cairo_popcount (uint32_t mask)
{
#if __GNUC__ > 3 || (__GNUC__ == 3 && __GNUC_MINOR__ >= 4)
    return __builtin_popcount (mask);
#else
    register int y;

    y = (mask >> 1) &033333333333;
    y = mask - y - ((y >>1) & 033333333333);
    return (((y + (y >> 3)) & 030707070707) % 077);
#endif
}

static cairo_always_inline cairo_bool_t
_cairo_is_little_endian (void)
{
    static const int i = 1;
    return *((char *) &i) == 0x01;
}

#ifdef WORDS_BIGENDIAN
#define CAIRO_BITSWAP8_IF_LITTLE_ENDIAN(c) (c)
#else
#define CAIRO_BITSWAP8_IF_LITTLE_ENDIAN(c) CAIRO_BITSWAP8(c)
#endif

#ifdef WORDS_BIGENDIAN

#define cpu_to_be16(v) (v)
#define be16_to_cpu(v) (v)
#define cpu_to_be32(v) (v)
#define be32_to_cpu(v) (v)

#else

static inline uint16_t cairo_const
cpu_to_be16(uint16_t v)
{
    return (v << 8) | (v >> 8);
}

static inline uint16_t cairo_const
be16_to_cpu(uint16_t v)
{
    return cpu_to_be16 (v);
}

static inline uint32_t cairo_const
cpu_to_be32(uint32_t v)
{
    return (v >> 24) | ((v >> 8) & 0xff00) | ((v << 8) & 0xff0000) | (v << 24);
}

static inline uint32_t cairo_const
be32_to_cpu(uint32_t v)
{
    return cpu_to_be32 (v);
}

#endif

/* Unaligned big endian access
 */

static inline uint16_t get_unaligned_be16 (const unsigned char *p)
{
    return p[0] << 8 | p[1];
}

static inline uint32_t get_unaligned_be32 (const unsigned char *p)
{
    return (uint32_t)p[0] << 24 | p[1] << 16 | p[2] << 8 | p[3];
}

static inline void put_unaligned_be16 (uint16_t v, unsigned char *p)
{
    p[0] = (v >> 8) & 0xff;
    p[1] = v & 0xff;
}

static inline void put_unaligned_be32 (uint32_t v, unsigned char *p)
{
    p[0] = (v >> 24) & 0xff;
    p[1] = (v >> 16) & 0xff;
    p[2] = (v >> 8)  & 0xff;
    p[3] = v & 0xff;
}

#include "cairo-ctype-inline.h"
#include "cairo-types-private.h"
#include "cairo-cache-private.h"
#include "cairo-reference-count-private.h"
#include "cairo-spans-private.h"
#include "cairo-surface-private.h"

cairo_private void
_cairo_box_from_doubles (cairo_box_t *box,
			 double *x1, double *y1,
			 double *x2, double *y2);

cairo_private void
_cairo_box_to_doubles (const cairo_box_t *box,
		       double *x1, double *y1,
		       double *x2, double *y2);

cairo_private void
_cairo_box_from_rectangle (cairo_box_t                 *box,
			   const cairo_rectangle_int_t *rectangle);

cairo_private void
_cairo_box_round_to_rectangle (const cairo_box_t     *box,
			       cairo_rectangle_int_t *rectangle);

cairo_private void
_cairo_box_add_curve_to (cairo_box_t         *extents,
			 const cairo_point_t *a,
			 const cairo_point_t *b,
			 const cairo_point_t *c,
			 const cairo_point_t *d);

cairo_private void
_cairo_boxes_get_extents (const cairo_box_t *boxes,
			  int num_boxes,
			  cairo_box_t *extents);

cairo_private extern const cairo_rectangle_int_t _cairo_empty_rectangle;
cairo_private extern const cairo_rectangle_int_t _cairo_unbounded_rectangle;

static inline void
_cairo_unbounded_rectangle_init (cairo_rectangle_int_t *rect)
{
    *rect = _cairo_unbounded_rectangle;
}

cairo_private_no_warn cairo_bool_t
_cairo_rectangle_intersect (cairo_rectangle_int_t *dst,
			    const cairo_rectangle_int_t *src);

static inline cairo_bool_t
_cairo_rectangle_intersects (const cairo_rectangle_int_t *dst,
			     const cairo_rectangle_int_t *src)
{
    return !(src->x >= dst->x + dst->width  ||
	     src->x + src->width <= dst->x  ||
	     src->y >= dst->y + dst->height ||
	     src->y + src->height <= dst->y);
}

static inline cairo_bool_t
_cairo_rectangle_contains_rectangle (const cairo_rectangle_int_t *a,
				     const cairo_rectangle_int_t *b)
{
    return (a->x <= b->x &&
	    a->x + a->width >= b->x + b->width &&
	    a->y <= b->y &&
	    a->y + a->height >= b->y + b->height);
}

cairo_private void
_cairo_rectangle_int_from_double (cairo_rectangle_int_t *recti,
				  const cairo_rectangle_t *rectf);

/* Extends the dst rectangle to also contain src.
 * If one of the rectangles is empty, the result is undefined
 */
cairo_private void
_cairo_rectangle_union (cairo_rectangle_int_t *dst,
			const cairo_rectangle_int_t *src);

cairo_private cairo_bool_t
_cairo_box_intersects_line_segment (const cairo_box_t *box,
	                            cairo_line_t *line) cairo_pure;

cairo_private cairo_bool_t
_cairo_spline_intersects (const cairo_point_t *a,
			  const cairo_point_t *b,
			  const cairo_point_t *c,
			  const cairo_point_t *d,
			  const cairo_box_t *box) cairo_pure;

typedef struct {
    const cairo_user_data_key_t *key;
    void *user_data;
    cairo_destroy_func_t destroy;
} cairo_user_data_slot_t;

cairo_private void
_cairo_user_data_array_init (cairo_user_data_array_t *array);

cairo_private void
_cairo_user_data_array_fini (cairo_user_data_array_t *array);

cairo_private void *
_cairo_user_data_array_get_data (cairo_user_data_array_t     *array,
				 const cairo_user_data_key_t *key);

cairo_private cairo_status_t
_cairo_user_data_array_set_data (cairo_user_data_array_t     *array,
				 const cairo_user_data_key_t *key,
				 void			     *user_data,
				 cairo_destroy_func_t	      destroy);

cairo_private cairo_status_t
_cairo_user_data_array_copy (cairo_user_data_array_t		*dst,
			     const cairo_user_data_array_t	*src);

cairo_private void
_cairo_user_data_array_foreach (cairo_user_data_array_t     *array,
				void (*func) (const void *key,
					      void *elt,
					      void *closure),
				void *closure);

#define _CAIRO_HASH_INIT_VALUE 5381

cairo_private uintptr_t
_cairo_hash_string (const char *c);

cairo_private uintptr_t
_cairo_hash_bytes (uintptr_t hash,
		   const void *bytes,
		   unsigned int length);

/* We use bits 24-27 to store phases for subpixel positions */
#define _cairo_scaled_glyph_index(g) ((unsigned long)((g)->hash_entry.hash & 0xffffff))
#define _cairo_scaled_glyph_xphase(g) (int)(((g)->hash_entry.hash >> 24) & 3)
#define _cairo_scaled_glyph_yphase(g) (int)(((g)->hash_entry.hash >> 26) & 3)
#define _cairo_scaled_glyph_set_index(g, i)  ((g)->hash_entry.hash = (i))

#include "cairo-scaled-font-private.h"

struct _cairo_font_face {
    /* hash_entry must be first */
    cairo_hash_entry_t hash_entry;
    cairo_status_t status;
    cairo_reference_count_t ref_count;
    cairo_user_data_array_t user_data;
    const cairo_font_face_backend_t *backend;
};

cairo_private void
_cairo_default_context_reset_static_data (void);

cairo_private void
_cairo_toy_font_face_reset_static_data (void);

cairo_private void
_cairo_ft_font_reset_static_data (void);

cairo_private void
_cairo_win32_font_reset_static_data (void);

/* the font backend interface */

struct _cairo_unscaled_font_backend {
    cairo_bool_t (*destroy) (void	*unscaled_font);
};

/* #cairo_toy_font_face_t - simple family/slant/weight font faces used for
 * the built-in font API
 */

typedef struct _cairo_toy_font_face {
    cairo_font_face_t base;
    const char *family;
    cairo_bool_t owns_family;
    cairo_font_slant_t slant;
    cairo_font_weight_t weight;

    cairo_font_face_t *impl_face; /* The non-toy font face this actually uses */
} cairo_toy_font_face_t;

typedef enum _cairo_scaled_glyph_info {
    CAIRO_SCALED_GLYPH_INFO_METRICS	 = (1 << 0),
    CAIRO_SCALED_GLYPH_INFO_SURFACE	 = (1 << 1),
    CAIRO_SCALED_GLYPH_INFO_PATH	 = (1 << 2),
    CAIRO_SCALED_GLYPH_INFO_RECORDING_SURFACE = (1 << 3),
    CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE = (1 << 4)
} cairo_scaled_glyph_info_t;

typedef struct _cairo_scaled_font_subset {
    cairo_scaled_font_t *scaled_font;
    unsigned int font_id;
    unsigned int subset_id;

    /* Index of glyphs array is subset_glyph_index.
     * Value of glyphs array is scaled_font_glyph_index.
     */
    unsigned long *glyphs;
    char          **utf8;
    char          **glyph_names;
    int           *to_latin_char;
    unsigned long *latin_to_subset_glyph_index;
    unsigned int num_glyphs;
    cairo_bool_t is_composite;
    cairo_bool_t is_scaled;
    cairo_bool_t is_latin;
} cairo_scaled_font_subset_t;

struct _cairo_scaled_font_backend {
    cairo_font_type_t type;

    void
    (*fini)		(void			*scaled_font);

    /*
     * Get the requested glyph info.
     * @scaled_font: a #cairo_scaled_font_t
     * @scaled_glyph: a #cairo_scaled_glyph_t the glyph
     * @info: a #cairo_scaled_glyph_info_t which information to retrieve
     *  %CAIRO_SCALED_GLYPH_INFO_METRICS - glyph metrics and bounding box
     *  %CAIRO_SCALED_GLYPH_INFO_SURFACE - surface holding glyph image
     *  %CAIRO_SCALED_GLYPH_INFO_PATH - path holding glyph outline in device space
     *  %CAIRO_SCALED_GLYPH_INFO_RECORDING_SURFACE - surface holding recording of glyph
     *  %CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE - surface holding color glyph image
     * @foreground_color - foreground color to use when rendering color fonts. Use NULL
     * if not requesting CAIRO_SCALED_GLYPH_INFO_COLOR_SURFACE or foreground color is unknown.
     */
    cairo_warn cairo_int_status_t
    (*scaled_glyph_init)	(void			     *scaled_font,
				 cairo_scaled_glyph_t	     *scaled_glyph,
				 cairo_scaled_glyph_info_t    info,
                                 const cairo_color_t         *foreground_color);

    /* A backend only needs to implement this or ucs4_to_index(), not
     * both. This allows the backend to do something more sophisticated
     * then just converting characters one by one.
     */
    cairo_warn cairo_int_status_t
    (*text_to_glyphs) (void                       *scaled_font,
		       double		           x,
		       double		           y,
		       const char	          *utf8,
		       int		           utf8_len,
		       cairo_glyph_t	         **glyphs,
		       int		          *num_glyphs,
		       cairo_text_cluster_t      **clusters,
		       int		          *num_clusters,
		       cairo_text_cluster_flags_t *cluster_flags);

    /* Get the glyph index for the given unicode code point.
     * @scaled_font: a #cairo_scaled_font_t
     * @ucs4: unicode code point
     * Returns glyph index or 0 if not found.
     */
    unsigned long
    (*ucs4_to_index)		(void			     *scaled_font,
				 uint32_t		      ucs4);

    /* Read data from a sfnt font table.
     * @scaled_font: font
     * @tag: 4 byte table name specifying the table to read.
     * @offset: offset into the table
     * @buffer: buffer to write data into. Caller must ensure there is sufficient space.
     *          If NULL, return the size of the table in @length.
     * @length: If @buffer is NULL, the size of the table will be returned in @length.
     *          If @buffer is not null, @length specifies the number of bytes to read.
     *
     * If less than @length bytes are available to read this function
     * returns CAIRO_INT_STATUS_UNSUPPORTED. Note that requesting more
     * bytes than are available in the table may continue reading data
     * from the following table and return success. If this is
     * undesirable the caller should first query the table size. If an
     * error occurs the output value of @length is undefined.
     *
     * Returns CAIRO_INT_STATUS_UNSUPPORTED if not a sfnt style font or table not found.
     */
    cairo_warn cairo_int_status_t
    (*load_truetype_table)(void		        *scaled_font,
                           unsigned long         tag,
                           long                  offset,
                           unsigned char        *buffer,
                           unsigned long        *length);

    /* ucs4 is set to -1 if the unicode character could not be found
     * for the glyph */
    cairo_warn cairo_int_status_t
    (*index_to_ucs4)(void                       *scaled_font,
		     unsigned long               index,
                     uint32_t                   *ucs4);

    /* Determine if this scaled font differs from the outlines in the font tables.
     * eg synthesized bold/italic or a non default variant of a variable font.
     * @scaled_font: font
     * @is_sythetic: returns TRUE if scaled font is synthetic
     * Returns cairo status
     */
    cairo_warn cairo_int_status_t
    (*is_synthetic)(void                       *scaled_font,
		    cairo_bool_t               *is_synthetic);

    /* For type 1 fonts, return the glyph name for a given glyph index.
     * A glyph index and list of glyph names in the Type 1 fonts is provided.
     * The function returns the index of the glyph in the list of glyph names.
     * @scaled_font: font
     * @glyph_names: the names of each glyph in the Type 1 font in the
     *   order they appear in the CharStrings array
     * @num_glyph_names: the number of names in the glyph_names array
     * @glyph_index: the given glyph index
     * @glyph_array_index: (index into glyph_names) the glyph name corresponding
     *  to the glyph_index
     */
    cairo_warn cairo_int_status_t
    (*index_to_glyph_name)(void                 *scaled_font,
			   char                **glyph_names,
			   int                   num_glyph_names,
			   unsigned long         glyph_index,
			   unsigned long        *glyph_array_index);

    /* Read data from a PostScript font.
     * @scaled_font: font
     * @offset: offset into the table
     * @buffer: buffer to write data into. Caller must ensure there is sufficient space.
     *          If NULL, return the size of the table in @length.
     * @length: If @buffer is NULL, the size of the table will be returned in @length.
     *          If @buffer is not null, @length specifies the number of bytes to read.
     *
     * If less than @length bytes are available to read this function
     * returns CAIRO_INT_STATUS_UNSUPPORTED. If an error occurs the
     * output value of @length is undefined.
     *
     * Returns CAIRO_INT_STATUS_UNSUPPORTED if not a Type 1 font.
     */
    cairo_warn cairo_int_status_t
    (*load_type1_data)    (void		        *scaled_font,
                           long                  offset,
                           unsigned char        *buffer,
                           unsigned long        *length);

    /* Check if font has any color glyphs.
     * @scaled_font: font
     * Returns TRUE if font contains any color glyphs
     */
    cairo_bool_t
    (*has_color_glyphs)   (void                 *scaled_font);
};

struct _cairo_font_face_backend {
    cairo_font_type_t	type;

    cairo_warn cairo_status_t
    (*create_for_toy)  (cairo_toy_font_face_t	*toy_face,
			cairo_font_face_t      **font_face);

    /* The destroy() function is allowed to resurrect the font face
     * by re-referencing. This is needed for the FreeType backend.
     */
    cairo_bool_t
    (*destroy)     (void			*font_face);

    cairo_warn cairo_status_t
    (*scaled_font_create) (void				*font_face,
			   const cairo_matrix_t		*font_matrix,
			   const cairo_matrix_t		*ctm,
			   const cairo_font_options_t	*options,
			   cairo_scaled_font_t	       **scaled_font);

    cairo_font_face_t *
    (*get_implementation) (void				*font_face,
			   const cairo_matrix_t		*font_matrix,
			   const cairo_matrix_t		*ctm,
			   const cairo_font_options_t	*options);
};

extern const cairo_private struct _cairo_font_face_backend _cairo_user_font_face_backend;

/* concrete font backends */
#if CAIRO_HAS_FT_FONT

extern const cairo_private struct _cairo_font_face_backend _cairo_ft_font_face_backend;

#endif

#if CAIRO_HAS_WIN32_FONT

extern const cairo_private struct _cairo_font_face_backend _cairo_win32_font_face_backend;

#endif

#if CAIRO_HAS_DWRITE_FONT

extern const cairo_private struct _cairo_font_face_backend _cairo_dwrite_font_face_backend;

#endif

#if CAIRO_HAS_QUARTZ_FONT

extern const cairo_private struct _cairo_font_face_backend _cairo_quartz_font_face_backend;

#endif

#define CAIRO_EXTEND_SURFACE_DEFAULT CAIRO_EXTEND_NONE
#define CAIRO_EXTEND_GRADIENT_DEFAULT CAIRO_EXTEND_PAD
#define CAIRO_FILTER_DEFAULT CAIRO_FILTER_GOOD

extern const cairo_private cairo_solid_pattern_t _cairo_pattern_clear;
extern const cairo_private cairo_solid_pattern_t _cairo_pattern_black;
extern const cairo_private cairo_solid_pattern_t _cairo_pattern_white;

struct _cairo_surface_attributes {
    cairo_matrix_t matrix;
    cairo_extend_t extend;
    cairo_filter_t filter;
    cairo_bool_t has_component_alpha;
    int		   x_offset;
    int		   y_offset;
    void	   *extra;
};

#define CAIRO_FONT_SLANT_DEFAULT   CAIRO_FONT_SLANT_NORMAL
#define CAIRO_FONT_WEIGHT_DEFAULT  CAIRO_FONT_WEIGHT_NORMAL

#define CAIRO_WIN32_FONT_FAMILY_DEFAULT "Arial"
#define CAIRO_DWRITE_FONT_FAMILY_DEFAULT "Arial"
#define CAIRO_QUARTZ_FONT_FAMILY_DEFAULT  "Helvetica"
#define CAIRO_FT_FONT_FAMILY_DEFAULT     ""
#define CAIRO_USER_FONT_FAMILY_DEFAULT     "@cairo:"

#if   CAIRO_HAS_DWRITE_FONT

#define CAIRO_FONT_FAMILY_DEFAULT CAIRO_DWRITE_FONT_FAMILY_DEFAULT
#define CAIRO_FONT_FACE_BACKEND_DEFAULT &_cairo_dwrite_font_face_backend

#elif CAIRO_HAS_WIN32_FONT

#define CAIRO_FONT_FAMILY_DEFAULT CAIRO_WIN32_FONT_FAMILY_DEFAULT
#define CAIRO_FONT_FACE_BACKEND_DEFAULT &_cairo_win32_font_face_backend

#elif CAIRO_HAS_QUARTZ_FONT

#define CAIRO_FONT_FAMILY_DEFAULT CAIRO_QUARTZ_FONT_FAMILY_DEFAULT
#define CAIRO_FONT_FACE_BACKEND_DEFAULT &_cairo_quartz_font_face_backend

#elif CAIRO_HAS_FT_FONT

#define CAIRO_FONT_FAMILY_DEFAULT CAIRO_FT_FONT_FAMILY_DEFAULT
#define CAIRO_FONT_FACE_BACKEND_DEFAULT &_cairo_ft_font_face_backend

#else

#define CAIRO_FONT_FAMILY_DEFAULT CAIRO_FT_FONT_FAMILY_DEFAULT
#define CAIRO_FONT_FACE_BACKEND_DEFAULT &_cairo_user_font_face_backend

#endif

#define CAIRO_GSTATE_OPERATOR_DEFAULT	CAIRO_OPERATOR_OVER
#define CAIRO_GSTATE_TOLERANCE_DEFAULT	0.1
#define CAIRO_GSTATE_FILL_RULE_DEFAULT	CAIRO_FILL_RULE_WINDING
#define CAIRO_GSTATE_LINE_WIDTH_DEFAULT	2.0
#define CAIRO_GSTATE_LINE_CAP_DEFAULT	CAIRO_LINE_CAP_BUTT
#define CAIRO_GSTATE_LINE_JOIN_DEFAULT	CAIRO_LINE_JOIN_MITER
#define CAIRO_GSTATE_MITER_LIMIT_DEFAULT	10.0
#define CAIRO_GSTATE_DEFAULT_FONT_SIZE  10.0

#define CAIRO_SURFACE_RESOLUTION_DEFAULT 72.0
#define CAIRO_SURFACE_FALLBACK_RESOLUTION_DEFAULT 300.0

typedef struct _cairo_stroke_face {
    cairo_point_t ccw;
    cairo_point_t point;
    cairo_point_t cw;
    cairo_slope_t dev_vector;
    cairo_point_double_t dev_slope;
    cairo_point_double_t usr_vector;
    double length;
} cairo_stroke_face_t;

/* cairo.c */

static inline double cairo_const
_cairo_restrict_value (double value, double min, double max)
{
    if (value < min)
	return min;
    else if (value > max)
	return max;
    else
	return value;
}

/* C99 round() rounds to the nearest integral value with halfway cases rounded
 * away from 0. _cairo_round rounds halfway cases toward positive infinity.
 * This matches the rounding behaviour of _cairo_lround. */
static inline double cairo_const
_cairo_round (double r)
{
    return floor (r + .5);
}

#if DISABLE_SOME_FLOATING_POINT
cairo_private int
_cairo_lround (double d) cairo_const;
#else
static inline int cairo_const
_cairo_lround (double r)
{
    return _cairo_round (r);
}
#endif

cairo_private uint16_t
_cairo_half_from_float (float f) cairo_const;

cairo_private cairo_bool_t
_cairo_operator_bounded_by_mask (cairo_operator_t op) cairo_const;

cairo_private cairo_bool_t
_cairo_operator_bounded_by_source (cairo_operator_t op) cairo_const;

enum {
    CAIRO_OPERATOR_BOUND_BY_MASK = 1 << 1,
    CAIRO_OPERATOR_BOUND_BY_SOURCE = 1 << 2,
};

cairo_private uint32_t
_cairo_operator_bounded_by_either (cairo_operator_t op) cairo_const;
/* cairo-color.c */
cairo_private const cairo_color_t *
_cairo_stock_color (cairo_stock_t stock) cairo_pure;

#define CAIRO_COLOR_WHITE       _cairo_stock_color (CAIRO_STOCK_WHITE)
#define CAIRO_COLOR_BLACK       _cairo_stock_color (CAIRO_STOCK_BLACK)
#define CAIRO_COLOR_TRANSPARENT _cairo_stock_color (CAIRO_STOCK_TRANSPARENT)

cairo_private uint16_t
_cairo_color_double_to_short (double d) cairo_const;

cairo_private void
_cairo_color_init_rgba (cairo_color_t *color,
			double red, double green, double blue,
			double alpha);

cairo_private void
_cairo_color_multiply_alpha (cairo_color_t *color,
			     double	    alpha);

cairo_private void
_cairo_color_get_rgba (cairo_color_t *color,
		       double	     *red,
		       double	     *green,
		       double	     *blue,
		       double	     *alpha);

cairo_private void
_cairo_color_get_rgba_premultiplied (cairo_color_t *color,
				     double	   *red,
				     double	   *green,
				     double	   *blue,
				     double	   *alpha);

cairo_private cairo_bool_t
_cairo_color_equal (const cairo_color_t *color_a,
                    const cairo_color_t *color_b) cairo_pure;

cairo_private cairo_bool_t
_cairo_color_stop_equal (const cairo_color_stop_t *color_a,
			 const cairo_color_stop_t *color_b) cairo_pure;

cairo_private cairo_content_t
_cairo_color_get_content (const cairo_color_t *color) cairo_pure;

/* cairo-font-face.c */

extern const cairo_private cairo_font_face_t _cairo_font_face_nil;
extern const cairo_private cairo_font_face_t _cairo_font_face_nil_file_not_found;

cairo_private void
_cairo_font_face_init (cairo_font_face_t               *font_face,
		       const cairo_font_face_backend_t *backend);

cairo_private cairo_bool_t
_cairo_font_face_destroy (void *abstract_face);

cairo_private cairo_status_t
_cairo_font_face_set_error (cairo_font_face_t *font_face,
	                    cairo_status_t     status);

cairo_private void
_cairo_unscaled_font_init (cairo_unscaled_font_t               *font,
			   const cairo_unscaled_font_backend_t *backend);

cairo_private_no_warn cairo_unscaled_font_t *
_cairo_unscaled_font_reference (cairo_unscaled_font_t *font);

cairo_private void
_cairo_unscaled_font_destroy (cairo_unscaled_font_t *font);

/* cairo-font-face-twin.c */

cairo_private cairo_font_face_t *
_cairo_font_face_twin_create_fallback (void);

cairo_private cairo_status_t
_cairo_font_face_twin_create_for_toy (cairo_toy_font_face_t   *toy_face,
				      cairo_font_face_t      **font_face);

/* cairo-font-face-twin-data.c */

extern const cairo_private int8_t _cairo_twin_outlines[];
extern const cairo_private uint16_t _cairo_twin_charmap[128];

/* cairo-font-options.c */

cairo_private void
_cairo_font_options_init_default (cairo_font_options_t *options);

cairo_private void
_cairo_font_options_init_copy (cairo_font_options_t		*options,
			       const cairo_font_options_t	*other);

cairo_private void
_cairo_font_options_fini (cairo_font_options_t *options);

cairo_private void
_cairo_font_options_set_lcd_filter (cairo_font_options_t   *options,
				   cairo_lcd_filter_t  lcd_filter);

cairo_private cairo_lcd_filter_t
_cairo_font_options_get_lcd_filter (const cairo_font_options_t *options);

cairo_private void
_cairo_font_options_set_round_glyph_positions (cairo_font_options_t   *options,
					       cairo_round_glyph_positions_t  round);

cairo_private cairo_round_glyph_positions_t
_cairo_font_options_get_round_glyph_positions (const cairo_font_options_t *options);

/* cairo-hull.c */
cairo_private cairo_status_t
_cairo_hull_compute (cairo_pen_vertex_t *vertices, int *num_vertices);

/* cairo-lzw.c */
cairo_private unsigned char *
_cairo_lzw_compress (unsigned char *data, unsigned long *size_in_out);

/* cairo-misc.c */
cairo_private cairo_status_t
_cairo_validate_text_clusters (const char		   *utf8,
			       int			    utf8_len,
			       const cairo_glyph_t	   *glyphs,
			       int			    num_glyphs,
			       const cairo_text_cluster_t  *clusters,
			       int			    num_clusters,
			       cairo_text_cluster_flags_t   cluster_flags);

cairo_private unsigned long
_cairo_string_hash (const char *str, int len);

cairo_private cairo_status_t
_cairo_intern_string (const char **str_inout, int len);

cairo_private void
_cairo_intern_string_reset_static_data (void);

cairo_private const char *
_cairo_get_locale_decimal_point (void);

cairo_private double
_cairo_strtod (const char *nptr, char **endptr);

#ifdef HAVE_STRNDUP
#define _cairo_strndup strndup
#else
cairo_private char *
_cairo_strndup (const char *s, size_t n);
#endif

/* cairo-path-fixed.c */
cairo_private cairo_path_fixed_t *
_cairo_path_fixed_create (void);

cairo_private void
_cairo_path_fixed_init (cairo_path_fixed_t *path);

cairo_private cairo_status_t
_cairo_path_fixed_init_copy (cairo_path_fixed_t *path,
			     const cairo_path_fixed_t *other);

cairo_private void
_cairo_path_fixed_fini (cairo_path_fixed_t *path);

cairo_private void
_cairo_path_fixed_destroy (cairo_path_fixed_t *path);

cairo_private cairo_status_t
_cairo_path_fixed_move_to (cairo_path_fixed_t  *path,
			   cairo_fixed_t	x,
			   cairo_fixed_t	y);

cairo_private void
_cairo_path_fixed_new_sub_path (cairo_path_fixed_t *path);

cairo_private cairo_status_t
_cairo_path_fixed_rel_move_to (cairo_path_fixed_t *path,
			       cairo_fixed_t	   dx,
			       cairo_fixed_t	   dy);

cairo_private cairo_status_t
_cairo_path_fixed_line_to (cairo_path_fixed_t *path,
			   cairo_fixed_t	x,
			   cairo_fixed_t	y);

cairo_private cairo_status_t
_cairo_path_fixed_rel_line_to (cairo_path_fixed_t *path,
			       cairo_fixed_t	   dx,
			       cairo_fixed_t	   dy);

cairo_private cairo_status_t
_cairo_path_fixed_curve_to (cairo_path_fixed_t	*path,
			    cairo_fixed_t x0, cairo_fixed_t y0,
			    cairo_fixed_t x1, cairo_fixed_t y1,
			    cairo_fixed_t x2, cairo_fixed_t y2);

cairo_private cairo_status_t
_cairo_path_fixed_rel_curve_to (cairo_path_fixed_t *path,
				cairo_fixed_t dx0, cairo_fixed_t dy0,
				cairo_fixed_t dx1, cairo_fixed_t dy1,
				cairo_fixed_t dx2, cairo_fixed_t dy2);

cairo_private cairo_status_t
_cairo_path_fixed_close_path (cairo_path_fixed_t *path);

cairo_private cairo_bool_t
_cairo_path_fixed_get_current_point (cairo_path_fixed_t *path,
				     cairo_fixed_t	*x,
				     cairo_fixed_t	*y);

typedef cairo_status_t
(cairo_path_fixed_move_to_func_t) (void		 *closure,
				   const cairo_point_t *point);

typedef cairo_status_t
(cairo_path_fixed_line_to_func_t) (void		 *closure,
				   const cairo_point_t *point);

typedef cairo_status_t
(cairo_path_fixed_curve_to_func_t) (void	  *closure,
				    const cairo_point_t *p0,
				    const cairo_point_t *p1,
				    const cairo_point_t *p2);

typedef cairo_status_t
(cairo_path_fixed_close_path_func_t) (void *closure);

cairo_private cairo_status_t
_cairo_path_fixed_interpret (const cairo_path_fixed_t	  *path,
		       cairo_path_fixed_move_to_func_t	  *move_to,
		       cairo_path_fixed_line_to_func_t	  *line_to,
		       cairo_path_fixed_curve_to_func_t	  *curve_to,
		       cairo_path_fixed_close_path_func_t *close_path,
		       void				  *closure);

cairo_private cairo_status_t
_cairo_path_fixed_interpret_flat (const cairo_path_fixed_t *path,
		       cairo_path_fixed_move_to_func_t	  *move_to,
		       cairo_path_fixed_line_to_func_t	  *line_to,
		       cairo_path_fixed_close_path_func_t *close_path,
		       void				  *closure,
		       double				  tolerance);


cairo_private cairo_bool_t
_cairo_path_bounder_extents (const cairo_path_fixed_t *path,
			     cairo_box_t *box);

cairo_private cairo_bool_t
_cairo_path_fixed_extents (const cairo_path_fixed_t *path,
			   cairo_box_t *box);

cairo_private void
_cairo_path_fixed_approximate_clip_extents (const cairo_path_fixed_t	*path,
					    cairo_rectangle_int_t *extents);

cairo_private void
_cairo_path_fixed_approximate_fill_extents (const cairo_path_fixed_t *path,
					    cairo_rectangle_int_t *extents);

cairo_private void
_cairo_path_fixed_fill_extents (const cairo_path_fixed_t	*path,
				cairo_fill_rule_t	 fill_rule,
				double			 tolerance,
				cairo_rectangle_int_t	*extents);

cairo_private void
_cairo_path_fixed_approximate_stroke_extents (const cairo_path_fixed_t *path,
					      const cairo_stroke_style_t *style,
					      const cairo_matrix_t *ctm,
					      cairo_bool_t vector,
					      cairo_rectangle_int_t *extents);

cairo_private cairo_status_t
_cairo_path_fixed_stroke_extents (const cairo_path_fixed_t *path,
				  const cairo_stroke_style_t *style,
				  const cairo_matrix_t *ctm,
				  const cairo_matrix_t *ctm_inverse,
				  double tolerance,
				  cairo_rectangle_int_t *extents);

cairo_private void
_cairo_path_fixed_transform (cairo_path_fixed_t	*path,
			     const cairo_matrix_t	*matrix);

cairo_private cairo_bool_t
_cairo_path_fixed_is_box (const cairo_path_fixed_t *path,
                          cairo_box_t *box);

cairo_private cairo_bool_t
_cairo_path_fixed_is_rectangle (const cairo_path_fixed_t *path,
				cairo_box_t        *box);

/* cairo-path-in-fill.c */
cairo_private cairo_bool_t
_cairo_path_fixed_in_fill (const cairo_path_fixed_t	*path,
			   cairo_fill_rule_t	 fill_rule,
			   double		 tolerance,
			   double		 x,
			   double		 y);

/* cairo-path-fill.c */
cairo_private cairo_status_t
_cairo_path_fixed_fill_to_polygon (const cairo_path_fixed_t *path,
				   double              tolerance,
				   cairo_polygon_t      *polygon);

cairo_private cairo_status_t
_cairo_path_fixed_fill_rectilinear_to_polygon (const cairo_path_fixed_t *path,
					       cairo_antialias_t antialias,
					       cairo_polygon_t *polygon);

cairo_private cairo_status_t
_cairo_path_fixed_fill_rectilinear_to_boxes (const cairo_path_fixed_t *path,
					     cairo_fill_rule_t fill_rule,
					     cairo_antialias_t antialias,
					     cairo_boxes_t *boxes);

cairo_private cairo_region_t *
_cairo_path_fixed_fill_rectilinear_to_region (const cairo_path_fixed_t	*path,
					      cairo_fill_rule_t	 fill_rule,
					      const cairo_rectangle_int_t *extents);

cairo_private cairo_status_t
_cairo_path_fixed_fill_to_traps (const cairo_path_fixed_t   *path,
				 cairo_fill_rule_t	     fill_rule,
				 double			     tolerance,
				 cairo_traps_t		    *traps);

/* cairo-path-stroke.c */
cairo_private cairo_status_t
_cairo_path_fixed_stroke_to_polygon (const cairo_path_fixed_t	*path,
				     const cairo_stroke_style_t	*stroke_style,
				     const cairo_matrix_t	*ctm,
				     const cairo_matrix_t	*ctm_inverse,
				     double		 tolerance,
				     cairo_polygon_t	*polygon);

cairo_private cairo_int_status_t
_cairo_path_fixed_stroke_to_tristrip (const cairo_path_fixed_t	*path,
				      const cairo_stroke_style_t*style,
				      const cairo_matrix_t	*ctm,
				      const cairo_matrix_t	*ctm_inverse,
				      double			 tolerance,
				      cairo_tristrip_t		 *strip);

cairo_private cairo_status_t
_cairo_path_fixed_stroke_dashed_to_polygon (const cairo_path_fixed_t	*path,
					    const cairo_stroke_style_t	*stroke_style,
					    const cairo_matrix_t	*ctm,
					    const cairo_matrix_t	*ctm_inverse,
					    double		 tolerance,
					    cairo_polygon_t	*polygon);

cairo_private cairo_int_status_t
_cairo_path_fixed_stroke_rectilinear_to_boxes (const cairo_path_fixed_t	*path,
					       const cairo_stroke_style_t	*stroke_style,
					       const cairo_matrix_t	*ctm,
					       cairo_antialias_t	 antialias,
					       cairo_boxes_t		*boxes);

cairo_private cairo_int_status_t
_cairo_path_fixed_stroke_to_traps (const cairo_path_fixed_t	*path,
				   const cairo_stroke_style_t	*stroke_style,
				   const cairo_matrix_t	*ctm,
				   const cairo_matrix_t	*ctm_inverse,
				   double		 tolerance,
				   cairo_traps_t	*traps);

cairo_private cairo_int_status_t
_cairo_path_fixed_stroke_polygon_to_traps (const cairo_path_fixed_t	*path,
					   const cairo_stroke_style_t	*stroke_style,
					   const cairo_matrix_t	*ctm,
					   const cairo_matrix_t	*ctm_inverse,
					   double		 tolerance,
					   cairo_traps_t	*traps);

cairo_private cairo_status_t
_cairo_path_fixed_stroke_to_shaper (cairo_path_fixed_t	*path,
				   const cairo_stroke_style_t	*stroke_style,
				   const cairo_matrix_t	*ctm,
				   const cairo_matrix_t	*ctm_inverse,
				   double		 tolerance,
				   cairo_status_t (*add_triangle) (void *closure,
								   const cairo_point_t triangle[3]),
				   cairo_status_t (*add_triangle_fan) (void *closure,
								       const cairo_point_t *midpt,
								       const cairo_point_t *points,
								       int npoints),
				   cairo_status_t (*add_quad) (void *closure,
							       const cairo_point_t quad[4]),
				   void *closure);

/* cairo-scaled-font.c */

cairo_private void
_cairo_scaled_font_freeze_cache (cairo_scaled_font_t *scaled_font);

cairo_private void
_cairo_scaled_font_thaw_cache (cairo_scaled_font_t *scaled_font);

cairo_private void
_cairo_scaled_font_reset_cache (cairo_scaled_font_t *scaled_font);

cairo_private cairo_status_t
_cairo_scaled_font_set_error (cairo_scaled_font_t *scaled_font,
			      cairo_status_t status);

cairo_private cairo_scaled_font_t *
_cairo_scaled_font_create_in_error (cairo_status_t status);

cairo_private void
_cairo_scaled_font_reset_static_data (void);

cairo_private cairo_status_t
_cairo_scaled_font_register_placeholder_and_unlock_font_map (cairo_scaled_font_t *scaled_font);

cairo_private void
_cairo_scaled_font_unregister_placeholder_and_lock_font_map (cairo_scaled_font_t *scaled_font);

cairo_private cairo_status_t
_cairo_scaled_font_init (cairo_scaled_font_t               *scaled_font,
			 cairo_font_face_t		   *font_face,
			 const cairo_matrix_t              *font_matrix,
			 const cairo_matrix_t              *ctm,
			 const cairo_font_options_t	   *options,
			 const cairo_scaled_font_backend_t *backend);

cairo_private cairo_status_t
_cairo_scaled_font_set_metrics (cairo_scaled_font_t	    *scaled_font,
				cairo_font_extents_t	    *fs_metrics);

/* This should only be called on an error path by a scaled_font constructor */
cairo_private void
_cairo_scaled_font_fini (cairo_scaled_font_t *scaled_font);

cairo_private cairo_status_t
_cairo_scaled_font_font_extents (cairo_scaled_font_t  *scaled_font,
				 cairo_font_extents_t *extents);

cairo_private cairo_status_t
_cairo_scaled_font_glyph_device_extents (cairo_scaled_font_t	 *scaled_font,
					 const cairo_glyph_t	 *glyphs,
					 int                      num_glyphs,
					 cairo_rectangle_int_t   *extents,
					 cairo_bool_t		 *overlap);

cairo_private cairo_bool_t
_cairo_scaled_font_glyph_approximate_extents (cairo_scaled_font_t	 *scaled_font,
					      const cairo_glyph_t	 *glyphs,
					      int                      num_glyphs,
					      cairo_rectangle_int_t   *extents);

cairo_private cairo_status_t
_cairo_scaled_font_show_glyphs (cairo_scaled_font_t *scaled_font,
				cairo_operator_t     op,
				const cairo_pattern_t *source,
				cairo_surface_t	    *surface,
				int		     source_x,
				int		     source_y,
				int		     dest_x,
				int		     dest_y,
				unsigned int	     width,
				unsigned int	     height,
				cairo_glyph_t	    *glyphs,
				int		     num_glyphs,
				cairo_region_t	    *clip_region);

cairo_private cairo_status_t
_cairo_scaled_font_glyph_path (cairo_scaled_font_t *scaled_font,
			       const cairo_glyph_t *glyphs,
			       int                  num_glyphs,
			       cairo_path_fixed_t  *path);

cairo_private void
_cairo_scaled_glyph_set_metrics (cairo_scaled_glyph_t *scaled_glyph,
				 cairo_scaled_font_t *scaled_font,
				 cairo_text_extents_t *fs_metrics);

cairo_private void
_cairo_scaled_glyph_set_surface (cairo_scaled_glyph_t *scaled_glyph,
				 cairo_scaled_font_t *scaled_font,
				 cairo_image_surface_t *surface);

cairo_private void
_cairo_scaled_glyph_set_path (cairo_scaled_glyph_t *scaled_glyph,
			      cairo_scaled_font_t *scaled_font,
			      cairo_path_fixed_t *path);

cairo_private void
_cairo_scaled_glyph_set_recording_surface (cairo_scaled_glyph_t *scaled_glyph,
                                           cairo_scaled_font_t *scaled_font,
                                           cairo_surface_t *recording_surface,
					   const cairo_color_t *foreground_color);

cairo_private void
_cairo_scaled_glyph_set_color_surface (cairo_scaled_glyph_t *scaled_glyph,
		                       cairo_scaled_font_t *scaled_font,
		                       cairo_image_surface_t *surface,
                                       const cairo_color_t *foreground_color);

cairo_private cairo_int_status_t
_cairo_scaled_glyph_lookup (cairo_scaled_font_t *scaled_font,
			    unsigned long index,
			    cairo_scaled_glyph_info_t info,
                            const cairo_color_t   *foreground_color,
			    cairo_scaled_glyph_t **scaled_glyph_ret);

cairo_private double
_cairo_scaled_font_get_max_scale (cairo_scaled_font_t *scaled_font);

cairo_private void
_cairo_scaled_font_map_destroy (void);

/* cairo-stroke-style.c */

cairo_private void
_cairo_stroke_style_init (cairo_stroke_style_t *style);

cairo_private cairo_status_t
_cairo_stroke_style_init_copy (cairo_stroke_style_t *style,
			       const cairo_stroke_style_t *other);

cairo_private void
_cairo_stroke_style_fini (cairo_stroke_style_t *style);

cairo_private void
_cairo_stroke_style_max_distance_from_path (const cairo_stroke_style_t *style,
					    const cairo_path_fixed_t *path,
                                            const cairo_matrix_t *ctm,
                                            double *dx, double *dy);
cairo_private void
_cairo_stroke_style_max_line_distance_from_path (const cairo_stroke_style_t *style,
						 const cairo_path_fixed_t *path,
						 const cairo_matrix_t *ctm,
						 double *dx, double *dy);

cairo_private void
_cairo_stroke_style_max_join_distance_from_path (const cairo_stroke_style_t *style,
						 const cairo_path_fixed_t *path,
						 const cairo_matrix_t *ctm,
						 double *dx, double *dy);

cairo_private double
_cairo_stroke_style_dash_period (const cairo_stroke_style_t *style);

cairo_private double
_cairo_stroke_style_dash_stroked (const cairo_stroke_style_t *style);

cairo_private cairo_bool_t
_cairo_stroke_style_dash_can_approximate (const cairo_stroke_style_t *style,
					  const cairo_matrix_t *ctm,
					  double tolerance);

cairo_private void
_cairo_stroke_style_dash_approximate (const cairo_stroke_style_t *style,
				      const cairo_matrix_t *ctm,
				      double tolerance,
				      double *dash_offset,
				      double *dashes,
				      unsigned int *num_dashes);


/* cairo-surface.c */

cairo_private cairo_bool_t
_cairo_surface_has_mime_image (cairo_surface_t *surface);

cairo_private cairo_status_t
_cairo_surface_copy_mime_data (cairo_surface_t *dst,
			       cairo_surface_t *src);

cairo_private_no_warn cairo_int_status_t
_cairo_surface_set_error (cairo_surface_t	*surface,
			  cairo_int_status_t	 status);

cairo_private void
_cairo_surface_set_resolution (cairo_surface_t *surface,
                               double x_res,
                               double y_res);

cairo_private cairo_surface_t *
_cairo_surface_create_for_rectangle_int (cairo_surface_t *target,
					 const cairo_rectangle_int_t *extents);

cairo_private cairo_surface_t *
_cairo_surface_create_scratch (cairo_surface_t	    *other,
			       cairo_content_t	     content,
			       int		     width,
			       int		     height,
			       const cairo_color_t  *color);

cairo_private void
_cairo_surface_init (cairo_surface_t			*surface,
		     const cairo_surface_backend_t	*backend,
		     cairo_device_t			*device,
		     cairo_content_t			 content,
		     cairo_bool_t                        is_vector);

cairo_private void
_cairo_surface_set_font_options (cairo_surface_t       *surface,
				 cairo_font_options_t  *options);

cairo_private cairo_status_t
_cairo_surface_paint (cairo_surface_t	*surface,
		      cairo_operator_t	 op,
		      const cairo_pattern_t *source,
		      const cairo_clip_t	    *clip);

cairo_private cairo_image_surface_t *
_cairo_surface_map_to_image (cairo_surface_t  *surface,
			     const cairo_rectangle_int_t *extents);

cairo_private_no_warn cairo_int_status_t
_cairo_surface_unmap_image (cairo_surface_t       *surface,
			    cairo_image_surface_t *image);

cairo_private cairo_status_t
_cairo_surface_mask (cairo_surface_t	*surface,
		     cairo_operator_t	 op,
		     const cairo_pattern_t	*source,
		     const cairo_pattern_t	*mask,
		     const cairo_clip_t		*clip);

cairo_private cairo_status_t
_cairo_surface_fill_stroke (cairo_surface_t	    *surface,
			    cairo_operator_t	     fill_op,
			    const cairo_pattern_t   *fill_source,
			    cairo_fill_rule_t	     fill_rule,
			    double		     fill_tolerance,
			    cairo_antialias_t	     fill_antialias,
			    cairo_path_fixed_t	    *path,
			    cairo_operator_t	     stroke_op,
			    const cairo_pattern_t   *stroke_source,
			    const cairo_stroke_style_t    *stroke_style,
			    const cairo_matrix_t	    *stroke_ctm,
			    const cairo_matrix_t	    *stroke_ctm_inverse,
			    double		     stroke_tolerance,
			    cairo_antialias_t	     stroke_antialias,
			    const cairo_clip_t	    *clip);

cairo_private cairo_status_t
_cairo_surface_stroke (cairo_surface_t		*surface,
		       cairo_operator_t		 op,
		       const cairo_pattern_t	*source,
		       const cairo_path_fixed_t	*path,
		       const cairo_stroke_style_t	*style,
		       const cairo_matrix_t		*ctm,
		       const cairo_matrix_t		*ctm_inverse,
		       double			 tolerance,
		       cairo_antialias_t	 antialias,
		       const cairo_clip_t		*clip);

cairo_private cairo_status_t
_cairo_surface_fill (cairo_surface_t	*surface,
		     cairo_operator_t	 op,
		     const cairo_pattern_t *source,
		     const cairo_path_fixed_t	*path,
		     cairo_fill_rule_t	 fill_rule,
		     double		 tolerance,
		     cairo_antialias_t	 antialias,
		     const cairo_clip_t	*clip);

cairo_private cairo_status_t
_cairo_surface_show_text_glyphs (cairo_surface_t	    *surface,
				 cairo_operator_t	     op,
				 const cairo_pattern_t	    *source,
				 const char		    *utf8,
				 int			     utf8_len,
				 cairo_glyph_t		    *glyphs,
				 int			     num_glyphs,
				 const cairo_text_cluster_t *clusters,
				 int			     num_clusters,
				 cairo_text_cluster_flags_t  cluster_flags,
				 cairo_scaled_font_t	    *scaled_font,
				 const cairo_clip_t		    *clip);

cairo_private cairo_status_t
_cairo_surface_tag (cairo_surface_t	        *surface,
		    cairo_bool_t                 begin,
		    const char                  *tag_name,
		    const char                  *attributes);

cairo_private cairo_status_t
_cairo_surface_acquire_source_image (cairo_surface_t         *surface,
				     cairo_image_surface_t  **image_out,
				     void                   **image_extra);

cairo_private void
_cairo_surface_release_source_image (cairo_surface_t        *surface,
				     cairo_image_surface_t  *image,
				     void                   *image_extra);

cairo_private cairo_surface_t *
_cairo_surface_snapshot (cairo_surface_t *surface);

cairo_private void
_cairo_surface_attach_snapshot (cairo_surface_t *surface,
				cairo_surface_t *snapshot,
				cairo_surface_func_t detach_func);

cairo_private cairo_surface_t *
_cairo_surface_has_snapshot (cairo_surface_t *surface,
			     const cairo_surface_backend_t *backend);

cairo_private void
_cairo_surface_detach_snapshot (cairo_surface_t *snapshot);

cairo_private cairo_status_t
_cairo_surface_begin_modification (cairo_surface_t *surface);

cairo_private_no_warn cairo_bool_t
_cairo_surface_get_extents (cairo_surface_t         *surface,
			    cairo_rectangle_int_t   *extents);

cairo_private cairo_bool_t
_cairo_surface_has_device_transform (cairo_surface_t *surface) cairo_pure;

cairo_private void
_cairo_surface_release_device_reference (cairo_surface_t *surface);

/* cairo-image-surface.c */

/* XXX: In cairo 1.2.0 we added a new %CAIRO_FORMAT_RGB16_565 but
 * neglected to adjust this macro. The net effect is that it's
 * impossible to externally create an image surface with this
 * format. This is perhaps a good thing since we also neglected to fix
 * up things like cairo_surface_write_to_png() for the new format
 * (-Wswitch-enum will tell you where). Is it obvious that format was
 * added in haste?
 *
 * The reason for the new format was to allow the xlib backend to be
 * used on X servers with a 565 visual. So the new format did its job
 * for that, even without being considered "valid" for the sake of
 * things like cairo_image_surface_create().
 *
 * Since 1.2.0 we ran into the same situation with X servers with BGR
 * visuals. This time we invented #cairo_internal_format_t instead,
 * (see it for more discussion).
 *
 * The punchline is that %CAIRO_FORMAT_VALID must not consider any
 * internal format to be valid. Also we need to decide if the
 * RGB16_565 should be moved to instead be an internal format. If so,
 * this macro need not change for it. (We probably will need to leave
 * an RGB16_565 value in the header files for the sake of code that
 * might have that value in it.)
 *
 * If we do decide to start fully supporting RGB16_565 as an external
 * format, then %CAIRO_FORMAT_VALID needs to be adjusted to include
 * it. But that should not happen before all necessary code is fixed
 * to support it (at least cairo_surface_write_to_png() and a few spots
 * in cairo-xlib-surface.c--again see -Wswitch-enum).
 */
#define CAIRO_FORMAT_VALID(format) ((format) >= CAIRO_FORMAT_ARGB32 &&		\
                                    (format) <= CAIRO_FORMAT_RGBA128F)

/* pixman-required stride alignment in bytes. */
#define CAIRO_STRIDE_ALIGNMENT (sizeof (uint32_t))
#define CAIRO_STRIDE_FOR_WIDTH_BPP(w,bpp) \
   ((((bpp)*(w)+7)/8 + CAIRO_STRIDE_ALIGNMENT-1) & -CAIRO_STRIDE_ALIGNMENT)

#define CAIRO_CONTENT_VALID(content) ((content) && 			         \
				      (((content) & ~(CAIRO_CONTENT_COLOR |      \
						      CAIRO_CONTENT_ALPHA |      \
						      CAIRO_CONTENT_COLOR_ALPHA))\
				       == 0))

cairo_private int
_cairo_format_bits_per_pixel (cairo_format_t format) cairo_const;

cairo_private cairo_format_t
_cairo_format_from_content (cairo_content_t content) cairo_const;

cairo_private cairo_format_t
_cairo_format_from_pixman_format (pixman_format_code_t pixman_format);

cairo_private cairo_content_t
_cairo_content_from_format (cairo_format_t format) cairo_const;

cairo_private cairo_content_t
_cairo_content_from_pixman_format (pixman_format_code_t pixman_format);

cairo_private cairo_surface_t *
_cairo_image_surface_create_for_pixman_image (pixman_image_t		*pixman_image,
					      pixman_format_code_t	 pixman_format);

cairo_private pixman_format_code_t
_cairo_format_to_pixman_format_code (cairo_format_t format);

cairo_private cairo_bool_t
_pixman_format_from_masks (cairo_format_masks_t *masks,
			   pixman_format_code_t *format_ret);

cairo_private cairo_bool_t
_pixman_format_to_masks (pixman_format_code_t	 pixman_format,
			 cairo_format_masks_t	*masks);

cairo_private void
_cairo_image_scaled_glyph_fini (cairo_scaled_font_t *scaled_font,
				cairo_scaled_glyph_t *scaled_glyph);

cairo_private void
_cairo_image_reset_static_data (void);

cairo_private void
_cairo_image_compositor_reset_static_data (void);

cairo_private cairo_surface_t *
_cairo_image_surface_create_with_pixman_format (unsigned char		*data,
						pixman_format_code_t	 pixman_format,
						int			 width,
						int			 height,
						int			 stride);

cairo_private cairo_surface_t *
_cairo_image_surface_create_with_content (cairo_content_t	content,
					  int			width,
					  int			height);

cairo_private void
_cairo_image_surface_assume_ownership_of_data (cairo_image_surface_t *surface);

cairo_private cairo_image_surface_t *
_cairo_image_surface_coerce (cairo_image_surface_t	*surface);

cairo_private cairo_image_surface_t *
_cairo_image_surface_coerce_to_format (cairo_image_surface_t	*surface,
			               cairo_format_t		 format);

cairo_private cairo_image_transparency_t
_cairo_image_analyze_transparency (cairo_image_surface_t      *image);

cairo_private cairo_image_color_t
_cairo_image_analyze_color (cairo_image_surface_t      *image);

/* cairo-pen.c */
cairo_private int
_cairo_pen_vertices_needed (double	    tolerance,
			    double	    radius,
			    const cairo_matrix_t  *matrix);

cairo_private cairo_status_t
_cairo_pen_init (cairo_pen_t	*pen,
		 double		 radius,
		 double		 tolerance,
		 const cairo_matrix_t	*ctm);

cairo_private void
_cairo_pen_init_empty (cairo_pen_t *pen);

cairo_private cairo_status_t
_cairo_pen_init_copy (cairo_pen_t *pen, const cairo_pen_t *other);

cairo_private void
_cairo_pen_fini (cairo_pen_t *pen);

cairo_private cairo_status_t
_cairo_pen_add_points (cairo_pen_t *pen, cairo_point_t *point, int num_points);

cairo_private int
_cairo_pen_find_active_cw_vertex_index (const cairo_pen_t *pen,
					const cairo_slope_t *slope);

cairo_private int
_cairo_pen_find_active_ccw_vertex_index (const cairo_pen_t *pen,
					 const cairo_slope_t *slope);

cairo_private void
_cairo_pen_find_active_cw_vertices (const cairo_pen_t *pen,
				     const cairo_slope_t *in,
				     const cairo_slope_t *out,
				     int *start, int *stop);

cairo_private void
_cairo_pen_find_active_ccw_vertices (const cairo_pen_t *pen,
				     const cairo_slope_t *in,
				     const cairo_slope_t *out,
				     int *start, int *stop);

/* cairo-polygon.c */
cairo_private void
_cairo_polygon_init (cairo_polygon_t   *polygon,
		     const cairo_box_t *boxes,
		     int		num_boxes);

cairo_private void
_cairo_polygon_init_with_clip (cairo_polygon_t *polygon,
			       const cairo_clip_t *clip);

cairo_private cairo_status_t
_cairo_polygon_init_boxes (cairo_polygon_t *polygon,
			   const cairo_boxes_t *boxes);

cairo_private cairo_status_t
_cairo_polygon_init_box_array (cairo_polygon_t *polygon,
			       cairo_box_t *boxes,
			       int num_boxes);

cairo_private void
_cairo_polygon_limit (cairo_polygon_t *polygon,
		     const cairo_box_t *limits,
		     int num_limits);

cairo_private void
_cairo_polygon_limit_to_clip (cairo_polygon_t *polygon,
			      const cairo_clip_t *clip);

cairo_private void
_cairo_polygon_fini (cairo_polygon_t *polygon);

cairo_private_no_warn cairo_status_t
_cairo_polygon_add_line (cairo_polygon_t *polygon,
			 const cairo_line_t *line,
			 int top, int bottom,
			 int dir);

cairo_private_no_warn cairo_status_t
_cairo_polygon_add_external_edge (void *polygon,
				  const cairo_point_t *p1,
				  const cairo_point_t *p2);

cairo_private_no_warn cairo_status_t
_cairo_polygon_add_contour (cairo_polygon_t *polygon,
			    const cairo_contour_t *contour);

cairo_private void
_cairo_polygon_translate (cairo_polygon_t *polygon, int dx, int dy);

cairo_private cairo_status_t
_cairo_polygon_reduce (cairo_polygon_t *polygon,
		       cairo_fill_rule_t fill_rule);

cairo_private cairo_status_t
_cairo_polygon_intersect (cairo_polygon_t *a, int winding_a,
			  cairo_polygon_t *b, int winding_b);

cairo_private cairo_status_t
_cairo_polygon_intersect_with_boxes (cairo_polygon_t *polygon,
				     cairo_fill_rule_t *winding,
				     cairo_box_t *boxes,
				     int num_boxes);

static inline cairo_bool_t
_cairo_polygon_is_empty (const cairo_polygon_t *polygon)
{
    return
	polygon->num_edges == 0 ||
	polygon->extents.p2.x <= polygon->extents.p1.x;
}

#define _cairo_polygon_status(P) ((cairo_polygon_t *) (P))->status

/* cairo-spline.c */
cairo_private cairo_bool_t
_cairo_spline_init (cairo_spline_t *spline,
		    cairo_spline_add_point_func_t add_point_func,
		    void *closure,
		    const cairo_point_t *a, const cairo_point_t *b,
		    const cairo_point_t *c, const cairo_point_t *d);

cairo_private cairo_status_t
_cairo_spline_decompose (cairo_spline_t *spline, double tolerance);

cairo_private cairo_status_t
_cairo_spline_bound (cairo_spline_add_point_func_t add_point_func,
		     void *closure,
		     const cairo_point_t *p0, const cairo_point_t *p1,
		     const cairo_point_t *p2, const cairo_point_t *p3);

/* cairo-matrix.c */
cairo_private void
_cairo_matrix_get_affine (const cairo_matrix_t *matrix,
			  double *xx, double *yx,
			  double *xy, double *yy,
			  double *x0, double *y0);

cairo_private void
_cairo_matrix_transform_bounding_box (const cairo_matrix_t *matrix,
				      double *x1, double *y1,
				      double *x2, double *y2,
				      cairo_bool_t *is_tight);

cairo_private void
_cairo_matrix_transform_bounding_box_fixed (const cairo_matrix_t *matrix,
					    cairo_box_t          *bbox,
					    cairo_bool_t         *is_tight);

cairo_private cairo_bool_t
_cairo_matrix_is_invertible (const cairo_matrix_t *matrix) cairo_pure;

cairo_private cairo_bool_t
_cairo_matrix_is_scale_0 (const cairo_matrix_t *matrix) cairo_pure;

cairo_private double
_cairo_matrix_compute_determinant (const cairo_matrix_t *matrix) cairo_pure;

cairo_private cairo_status_t
_cairo_matrix_compute_basis_scale_factors (const cairo_matrix_t *matrix,
					   double *sx, double *sy, int x_major);

static inline cairo_bool_t
_cairo_matrix_is_identity (const cairo_matrix_t *matrix)
{
    return (matrix->xx == 1.0 && matrix->yx == 0.0 &&
	    matrix->xy == 0.0 && matrix->yy == 1.0 &&
	    matrix->x0 == 0.0 && matrix->y0 == 0.0);
}

static inline cairo_bool_t
_cairo_matrix_is_translation (const cairo_matrix_t *matrix)
{
    return (matrix->xx == 1.0 && matrix->yx == 0.0 &&
	    matrix->xy == 0.0 && matrix->yy == 1.0);
}

static inline cairo_bool_t
_cairo_matrix_is_scale (const cairo_matrix_t *matrix)
{
    return matrix->yx == 0.0 && matrix->xy == 0.0;
}

cairo_private cairo_bool_t
_cairo_matrix_is_integer_translation(const cairo_matrix_t *matrix,
				     int *itx, int *ity);

cairo_private cairo_bool_t
_cairo_matrix_has_unity_scale (const cairo_matrix_t *matrix);

cairo_private cairo_bool_t
_cairo_matrix_is_pixel_exact (const cairo_matrix_t *matrix) cairo_pure;

cairo_private double
_cairo_matrix_transformed_circle_major_axis (const cairo_matrix_t *matrix,
					     double radius) cairo_pure;

cairo_private cairo_bool_t
_cairo_matrix_is_pixman_translation (const cairo_matrix_t     *matrix,
				     cairo_filter_t            filter,
				     int                      *out_x_offset,
				     int                      *out_y_offset);

cairo_private cairo_status_t
_cairo_matrix_to_pixman_matrix_offset (const cairo_matrix_t	*matrix,
				       cairo_filter_t            filter,
				       double                    xc,
				       double                    yc,
				       pixman_transform_t	*out_transform,
				       int                      *out_x_offset,
				       int                      *out_y_offset);

cairo_private void
_cairo_debug_print_matrix (FILE *file, const cairo_matrix_t *matrix);

cairo_private void
_cairo_debug_print_rect (FILE *file, const cairo_rectangle_int_t *rect);

cairo_private const char *
_cairo_debug_operator_to_string (cairo_operator_t op);

cairo_private const char *
_cairo_debug_status_to_string (cairo_int_status_t status);

cairo_private cairo_status_t
_cairo_bentley_ottmann_tessellate_rectilinear_polygon (cairo_traps_t	 *traps,
						       const cairo_polygon_t *polygon,
						       cairo_fill_rule_t	  fill_rule);

cairo_private cairo_status_t
_cairo_bentley_ottmann_tessellate_polygon (cairo_traps_t         *traps,
					   const cairo_polygon_t *polygon,
					   cairo_fill_rule_t      fill_rule);

cairo_private cairo_status_t
_cairo_bentley_ottmann_tessellate_traps (cairo_traps_t *traps,
					 cairo_fill_rule_t fill_rule);

cairo_private cairo_status_t
_cairo_bentley_ottmann_tessellate_rectangular_traps (cairo_traps_t *traps,
						     cairo_fill_rule_t fill_rule);

cairo_private cairo_status_t
_cairo_bentley_ottmann_tessellate_boxes (const cairo_boxes_t *in,
					 cairo_fill_rule_t fill_rule,
					 cairo_boxes_t *out);

cairo_private cairo_status_t
_cairo_bentley_ottmann_tessellate_rectilinear_traps (cairo_traps_t *traps,
						     cairo_fill_rule_t fill_rule);

cairo_private cairo_status_t
_cairo_bentley_ottmann_tessellate_rectilinear_polygon_to_boxes (const cairo_polygon_t *polygon,
								cairo_fill_rule_t fill_rule,
								cairo_boxes_t *boxes);

cairo_private void
_cairo_trapezoid_array_translate_and_scale (cairo_trapezoid_t *offset_traps,
					    cairo_trapezoid_t *src_traps,
					    int num_traps,
					    double tx, double ty,
					    double sx, double sy);

cairo_private void
_cairo_clip_reset_static_data (void);

cairo_private void
_cairo_pattern_reset_static_data (void);

/* cairo-unicode.c */

cairo_private int
_cairo_utf8_get_char_validated (const char *p,
				uint32_t   *unicode);

cairo_private cairo_status_t
_cairo_utf8_to_ucs4 (const char *str,
		     int	 len,
		     uint32_t  **result,
		     int	*items_written);

cairo_private int
_cairo_ucs4_to_utf8 (uint32_t    unicode,
		     char       *utf8);

cairo_private int
_cairo_ucs4_to_utf16 (uint32_t    unicode,
		      uint16_t   *utf16);

#if _WIN32 || CAIRO_HAS_WIN32_FONT || CAIRO_HAS_QUARTZ_FONT || CAIRO_HAS_PDF_OPERATORS
# define CAIRO_HAS_UTF8_TO_UTF16 1
#endif
#if CAIRO_HAS_UTF8_TO_UTF16
cairo_private cairo_status_t
_cairo_utf8_to_utf16 (const char *str,
		      int	  len,
		      uint16_t  **result,
		      int	 *items_written);
#endif

cairo_private void
_cairo_matrix_multiply (cairo_matrix_t *r,
			const cairo_matrix_t *a,
			const cairo_matrix_t *b);

/* cairo-observer.c */

cairo_private void
_cairo_observers_notify (cairo_list_t *observers, void *arg);

/* Open a file with a UTF-8 filename */
cairo_private cairo_status_t
_cairo_fopen (const char *filename, const char *mode, FILE **file_out);

/* Avoid unnecessary PLT entries.  */
slim_hidden_proto (cairo_append_path);
slim_hidden_proto (cairo_arc);
slim_hidden_proto (cairo_arc_negative);
slim_hidden_proto (cairo_clip);
slim_hidden_proto (cairo_clip_extents);
slim_hidden_proto (cairo_clip_preserve);
slim_hidden_proto (cairo_close_path);
slim_hidden_proto (cairo_copy_path);
slim_hidden_proto (cairo_create);
slim_hidden_proto (cairo_curve_to);
slim_hidden_proto (cairo_destroy);
slim_hidden_proto (cairo_device_to_user);
slim_hidden_proto (cairo_fill);
slim_hidden_proto (cairo_fill_preserve);
slim_hidden_proto (cairo_font_face_destroy);
slim_hidden_proto (cairo_font_face_get_user_data);
slim_hidden_proto_no_warn (cairo_font_face_reference);
slim_hidden_proto (cairo_font_face_set_user_data);
slim_hidden_proto (cairo_font_options_equal);
slim_hidden_proto (cairo_font_options_get_custom_palette_color);
slim_hidden_proto (cairo_font_options_hash);
slim_hidden_proto (cairo_font_options_merge);
slim_hidden_proto (cairo_font_options_set_antialias);
slim_hidden_proto (cairo_font_options_set_hint_metrics);
slim_hidden_proto (cairo_font_options_set_hint_style);
slim_hidden_proto (cairo_font_options_set_subpixel_order);
slim_hidden_proto (cairo_font_options_status);
slim_hidden_proto (cairo_format_stride_for_width);
slim_hidden_proto (cairo_get_current_point);
slim_hidden_proto (cairo_get_hairline);
slim_hidden_proto (cairo_get_line_width);
slim_hidden_proto (cairo_get_matrix);
slim_hidden_proto (cairo_get_scaled_font);
slim_hidden_proto (cairo_get_source);
slim_hidden_proto (cairo_get_target);
slim_hidden_proto (cairo_get_tolerance);
slim_hidden_proto (cairo_glyph_allocate);
slim_hidden_proto (cairo_glyph_free);
slim_hidden_proto (cairo_has_current_point);
slim_hidden_proto (cairo_identity_matrix);
slim_hidden_proto (cairo_image_surface_create);
slim_hidden_proto (cairo_image_surface_create_for_data);
slim_hidden_proto (cairo_image_surface_get_data);
slim_hidden_proto (cairo_image_surface_get_format);
slim_hidden_proto (cairo_image_surface_get_height);
slim_hidden_proto (cairo_image_surface_get_stride);
slim_hidden_proto (cairo_image_surface_get_width);
slim_hidden_proto (cairo_line_to);
slim_hidden_proto (cairo_mask);
slim_hidden_proto (cairo_matrix_init);
slim_hidden_proto (cairo_matrix_init_identity);
slim_hidden_proto (cairo_matrix_init_rotate);
slim_hidden_proto (cairo_matrix_init_scale);
slim_hidden_proto (cairo_matrix_init_translate);
slim_hidden_proto (cairo_matrix_invert);
slim_hidden_proto (cairo_matrix_multiply);
slim_hidden_proto (cairo_matrix_rotate);
slim_hidden_proto (cairo_matrix_scale);
slim_hidden_proto (cairo_matrix_transform_distance);
slim_hidden_proto (cairo_matrix_transform_point);
slim_hidden_proto (cairo_matrix_translate);
slim_hidden_proto (cairo_mesh_pattern_begin_patch);
slim_hidden_proto (cairo_mesh_pattern_curve_to);
slim_hidden_proto (cairo_mesh_pattern_end_patch);
slim_hidden_proto (cairo_mesh_pattern_get_control_point);
slim_hidden_proto (cairo_mesh_pattern_get_corner_color_rgba);
slim_hidden_proto (cairo_mesh_pattern_get_patch_count);
slim_hidden_proto (cairo_mesh_pattern_get_path);
slim_hidden_proto (cairo_mesh_pattern_line_to);
slim_hidden_proto (cairo_mesh_pattern_move_to);
slim_hidden_proto (cairo_mesh_pattern_set_corner_color_rgba);
slim_hidden_proto (cairo_move_to);
slim_hidden_proto (cairo_new_path);
slim_hidden_proto (cairo_paint);
slim_hidden_proto (cairo_paint_with_alpha);
slim_hidden_proto_no_warn (cairo_path_destroy);
slim_hidden_proto (cairo_pattern_add_color_stop_rgba);
slim_hidden_proto (cairo_pattern_create_for_surface);
slim_hidden_proto (cairo_pattern_create_linear);
slim_hidden_proto (cairo_pattern_create_mesh);
slim_hidden_proto (cairo_pattern_create_radial);
slim_hidden_proto (cairo_pattern_create_rgb);
slim_hidden_proto (cairo_pattern_create_rgba);
slim_hidden_proto (cairo_pattern_destroy);
slim_hidden_proto (cairo_pattern_get_extend);
slim_hidden_proto (cairo_pattern_get_rgba);
slim_hidden_proto (cairo_pattern_get_type);
slim_hidden_proto_no_warn (cairo_pattern_reference);
slim_hidden_proto (cairo_pattern_set_extend);
slim_hidden_proto (cairo_pattern_set_matrix);
slim_hidden_proto (cairo_pop_group);
slim_hidden_proto (cairo_pop_group_to_source);
slim_hidden_proto (cairo_push_group);
slim_hidden_proto (cairo_push_group_with_content);
slim_hidden_proto (cairo_recording_surface_create);
slim_hidden_proto (cairo_recording_surface_ink_extents);
slim_hidden_proto (cairo_rectangle);
slim_hidden_proto (cairo_region_contains_point);
slim_hidden_proto (cairo_region_contains_rectangle);
slim_hidden_proto (cairo_region_copy);
slim_hidden_proto (cairo_region_create);
slim_hidden_proto (cairo_region_create_rectangle);
slim_hidden_proto (cairo_region_create_rectangles);
slim_hidden_proto (cairo_region_destroy);
slim_hidden_proto (cairo_region_equal);
slim_hidden_proto (cairo_region_get_extents);
slim_hidden_proto (cairo_region_get_rectangle);
slim_hidden_proto (cairo_region_intersect);
slim_hidden_proto (cairo_region_intersect_rectangle);
slim_hidden_proto (cairo_region_is_empty);
slim_hidden_proto (cairo_region_num_rectangles);
slim_hidden_proto (cairo_region_reference);
slim_hidden_proto (cairo_region_status);
slim_hidden_proto (cairo_region_subtract);
slim_hidden_proto (cairo_region_subtract_rectangle);
slim_hidden_proto (cairo_region_translate);
slim_hidden_proto (cairo_region_union);
slim_hidden_proto (cairo_region_union_rectangle);
slim_hidden_proto (cairo_region_xor);
slim_hidden_proto (cairo_region_xor_rectangle);
slim_hidden_proto (cairo_rel_line_to);
slim_hidden_proto (cairo_restore);
slim_hidden_proto (cairo_rotate);
slim_hidden_proto (cairo_save);
slim_hidden_proto (cairo_scale);
slim_hidden_proto (cairo_scaled_font_create);
slim_hidden_proto (cairo_scaled_font_destroy);
slim_hidden_proto (cairo_scaled_font_extents);
slim_hidden_proto (cairo_scaled_font_get_ctm);
slim_hidden_proto (cairo_scaled_font_get_font_face);
slim_hidden_proto (cairo_scaled_font_get_font_matrix);
slim_hidden_proto (cairo_scaled_font_get_font_options);
slim_hidden_proto (cairo_scaled_font_get_user_data);
slim_hidden_proto (cairo_scaled_font_glyph_extents);
slim_hidden_proto_no_warn (cairo_scaled_font_reference);
slim_hidden_proto (cairo_scaled_font_set_user_data);
slim_hidden_proto (cairo_scaled_font_status);
slim_hidden_proto (cairo_scaled_font_text_to_glyphs);
slim_hidden_proto (cairo_set_dash);
slim_hidden_proto (cairo_set_fill_rule);
slim_hidden_proto (cairo_set_font_matrix);
slim_hidden_proto (cairo_set_font_options);
slim_hidden_proto (cairo_set_font_size);
slim_hidden_proto (cairo_set_hairline);
slim_hidden_proto (cairo_set_line_cap);
slim_hidden_proto (cairo_set_line_join);
slim_hidden_proto (cairo_set_line_width);
slim_hidden_proto (cairo_set_matrix);
slim_hidden_proto (cairo_set_miter_limit);
slim_hidden_proto (cairo_set_operator);
slim_hidden_proto (cairo_set_source);
slim_hidden_proto (cairo_set_source_rgb);
slim_hidden_proto (cairo_set_source_rgba);
slim_hidden_proto (cairo_set_source_surface);
slim_hidden_proto (cairo_set_tolerance);
slim_hidden_proto (cairo_status);
slim_hidden_proto (cairo_stroke);
slim_hidden_proto (cairo_stroke_preserve);
slim_hidden_proto (cairo_surface_copy_page);
slim_hidden_proto (cairo_surface_create_similar_image);
slim_hidden_proto (cairo_surface_destroy);
slim_hidden_proto (cairo_surface_finish);
slim_hidden_proto (cairo_surface_flush);
slim_hidden_proto (cairo_surface_get_device_offset);
slim_hidden_proto (cairo_surface_get_device_scale);
slim_hidden_proto (cairo_surface_get_font_options);
slim_hidden_proto (cairo_surface_get_mime_data);
slim_hidden_proto (cairo_surface_has_show_text_glyphs);
slim_hidden_proto (cairo_surface_mark_dirty);
slim_hidden_proto (cairo_surface_mark_dirty_rectangle);
slim_hidden_proto_no_warn (cairo_surface_reference);
slim_hidden_proto (cairo_surface_set_device_offset);
slim_hidden_proto (cairo_surface_set_device_scale);
slim_hidden_proto (cairo_surface_set_fallback_resolution);
slim_hidden_proto (cairo_surface_set_mime_data);
slim_hidden_proto (cairo_surface_show_page);
slim_hidden_proto (cairo_surface_status);
slim_hidden_proto (cairo_surface_supports_mime_type);
slim_hidden_proto (cairo_text_cluster_allocate);
slim_hidden_proto (cairo_text_cluster_free);
slim_hidden_proto (cairo_toy_font_face_create);
slim_hidden_proto (cairo_toy_font_face_get_slant);
slim_hidden_proto (cairo_toy_font_face_get_weight);
slim_hidden_proto (cairo_transform);
slim_hidden_proto (cairo_translate);
slim_hidden_proto (cairo_user_font_face_create);
slim_hidden_proto (cairo_user_font_face_set_init_func);
slim_hidden_proto (cairo_user_font_face_set_render_color_glyph_func);
slim_hidden_proto (cairo_user_font_face_set_render_glyph_func);
slim_hidden_proto (cairo_user_font_face_set_unicode_to_glyph_func);
slim_hidden_proto (cairo_user_to_device);
slim_hidden_proto (cairo_user_to_device_distance);
slim_hidden_proto (cairo_version_string);

#if CAIRO_HAS_PNG_FUNCTIONS

slim_hidden_proto (cairo_image_surface_create_from_png_stream);
slim_hidden_proto (cairo_surface_write_to_png_stream);

#endif


#include "cairo-mutex-private.h"
#include "cairo-fixed-private.h"
#include "cairo-wideint-private.h"
#include "cairo-malloc-private.h"
#include "cairo-hash-private.h"

#if HAVE_VALGRIND
#include <memcheck.h>

#define VG(x) x

cairo_private void
_cairo_debug_check_image_surface_is_defined (const cairo_surface_t *surface);

#else

#define VG(x)
#define _cairo_debug_check_image_surface_is_defined(X)

#endif

cairo_private void
_cairo_debug_print_path (FILE *stream, const cairo_path_fixed_t *path);

cairo_private void
_cairo_debug_print_polygon (FILE *stream, cairo_polygon_t *polygon);

cairo_private void
_cairo_debug_print_traps (FILE *file, const cairo_traps_t *traps);

cairo_private void
_cairo_debug_print_clip (FILE *stream, const cairo_clip_t *clip);

#if 0
#define TRACE(x) fprintf (stderr, "%s: ", __FILE__), fprintf x
#define TRACE_(x) x
#else
#define TRACE(x)
#define TRACE_(x)
#endif

CAIRO_END_DECLS

#endif
