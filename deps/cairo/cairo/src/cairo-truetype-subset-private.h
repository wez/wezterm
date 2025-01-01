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
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#ifndef CAIRO_TRUETYPE_SUBSET_PRIVATE_H
#define CAIRO_TRUETYPE_SUBSET_PRIVATE_H

#include "cairoint.h"

#if CAIRO_HAS_FONT_SUBSET

/* The structs defined here should strictly follow the TrueType
 * specification and not be padded.  We use only 16-bit integer
 * in their definition to guarantee that.  The fields of type
 * "FIXED" in the TT spec are broken into two *_1 and *_2 16-bit
 * parts, and 64-bit members are broken into four.
 *
 * The test truetype-tables in the test suite makes sure that
 * these tables have the right size.  Please update that test
 * if you add new tables/structs that should be packed.
 */

#define MAKE_TT_TAG(a, b, c, d)    ((int)((uint32_t)a<<24 | b<<16 | c<<8 | d))
#define TT_TAG_CFF    MAKE_TT_TAG('C','F','F',' ')
#define TT_TAG_cmap   MAKE_TT_TAG('c','m','a','p')
#define TT_TAG_cvt    MAKE_TT_TAG('c','v','t',' ')
#define TT_TAG_fpgm   MAKE_TT_TAG('f','p','g','m')
#define TT_TAG_glyf   MAKE_TT_TAG('g','l','y','f')
#define TT_TAG_head   MAKE_TT_TAG('h','e','a','d')
#define TT_TAG_hhea   MAKE_TT_TAG('h','h','e','a')
#define TT_TAG_hmtx   MAKE_TT_TAG('h','m','t','x')
#define TT_TAG_loca   MAKE_TT_TAG('l','o','c','a')
#define TT_TAG_maxp   MAKE_TT_TAG('m','a','x','p')
#define TT_TAG_name   MAKE_TT_TAG('n','a','m','e')
#define TT_TAG_OS2    MAKE_TT_TAG('O','S','/','2')
#define TT_TAG_post   MAKE_TT_TAG('p','o','s','t')
#define TT_TAG_prep   MAKE_TT_TAG('p','r','e','p')

/* All tt_* structs are big-endian */
typedef struct _tt_cmap_index {
    uint16_t platform;
    uint16_t encoding;
    uint32_t offset;
} tt_cmap_index_t;

typedef struct _tt_cmap {
    uint16_t        version;
    uint16_t        num_tables;
    tt_cmap_index_t index[1];
} tt_cmap_t;

typedef struct _segment_map {
    uint16_t format;
    uint16_t length;
    uint16_t version;
    uint16_t segCountX2;
    uint16_t searchRange;
    uint16_t entrySelector;
    uint16_t rangeShift;
    uint16_t endCount[1];
} tt_segment_map_t;

typedef struct _tt_head {
    int16_t     version_1;
    int16_t     version_2;
    int16_t     revision_1;
    int16_t     revision_2;
    uint16_t    checksum_1;
    uint16_t    checksum_2;
    uint16_t    magic_1;
    uint16_t    magic_2;
    uint16_t    flags;
    uint16_t    units_per_em;
    int16_t     created_1;
    int16_t     created_2;
    int16_t     created_3;
    int16_t     created_4;
    int16_t     modified_1;
    int16_t     modified_2;
    int16_t     modified_3;
    int16_t     modified_4;
    int16_t     x_min;                  /* FWORD */
    int16_t     y_min;                  /* FWORD */
    int16_t     x_max;                  /* FWORD */
    int16_t     y_max;                  /* FWORD */
    uint16_t    mac_style;
    uint16_t    lowest_rec_pppem;
    int16_t     font_direction_hint;
    int16_t     index_to_loc_format;
    int16_t     glyph_data_format;
} tt_head_t;

typedef struct _tt_hhea {
    int16_t     version_1;
    int16_t     version_2;
    int16_t     ascender;               /* FWORD */
    int16_t     descender;              /* FWORD */
    int16_t     line_gap;               /* FWORD */
    uint16_t    advance_max_width;      /* UFWORD */
    int16_t     min_left_side_bearing;  /* FWORD */
    int16_t     min_right_side_bearing; /* FWORD */
    int16_t     x_max_extent;           /* FWORD */
    int16_t     caret_slope_rise;
    int16_t     caret_slope_run;
    int16_t     reserved[5];
    int16_t     metric_data_format;
    uint16_t    num_hmetrics;
} tt_hhea_t;

typedef struct _tt_maxp {
    int16_t     version_1;
    int16_t     version_2;
    uint16_t    num_glyphs;
    uint16_t    max_points;
    uint16_t    max_contours;
    uint16_t    max_composite_points;
    uint16_t    max_composite_contours;
    uint16_t    max_zones;
    uint16_t    max_twilight_points;
    uint16_t    max_storage;
    uint16_t    max_function_defs;
    uint16_t    max_instruction_defs;
    uint16_t    max_stack_elements;
    uint16_t    max_size_of_instructions;
    uint16_t    max_component_elements;
    uint16_t    max_component_depth;
} tt_maxp_t;

typedef struct _tt_name_record {
    uint16_t platform;
    uint16_t encoding;
    uint16_t language;
    uint16_t name;
    uint16_t length;
    uint16_t offset;
} tt_name_record_t;

typedef struct _tt_name {
    uint16_t   format;
    uint16_t   num_records;
    uint16_t   strings_offset;
    tt_name_record_t records[1];
} tt_name_t;


/* bitmask for fsSelection field */
#define TT_FS_SELECTION_ITALIC   1
#define TT_FS_SELECTION_BOLD    32

/* _unused fields are defined in TT spec but not used by cairo */
typedef struct _tt_os2 {
    uint16_t   _unused1[2];
    uint16_t   usWeightClass;
    uint16_t   _unused2[28];
    uint16_t   fsSelection;
    uint16_t   _unused3[11];
} tt_os2_t;

/* composite_glyph_t flags */
#define TT_ARG_1_AND_2_ARE_WORDS     0x0001
#define TT_WE_HAVE_A_SCALE           0x0008
#define TT_MORE_COMPONENTS           0x0020
#define TT_WE_HAVE_AN_X_AND_Y_SCALE  0x0040
#define TT_WE_HAVE_A_TWO_BY_TWO      0x0080

typedef struct _tt_composite_glyph {
    uint16_t flags;
    uint16_t index;
    uint16_t args[6]; /* 1 to 6 arguments depending on value of flags */
} tt_composite_glyph_t;

typedef struct _tt_glyph_data {
    int16_t           num_contours;
    int8_t            data[8];
    tt_composite_glyph_t glyph;
} tt_glyph_data_t;

#endif /* CAIRO_HAS_FONT_SUBSET */

#endif /* CAIRO_TRUETYPE_SUBSET_PRIVATE_H */
