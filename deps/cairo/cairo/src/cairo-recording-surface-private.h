/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
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
 * Contributor(s):
 *	Kristian Høgsberg <krh@redhat.com>
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#ifndef CAIRO_RECORDING_SURFACE_H
#define CAIRO_RECORDING_SURFACE_H

#include "cairoint.h"
#include "cairo-path-fixed-private.h"
#include "cairo-pattern-private.h"
#include "cairo-surface-backend-private.h"

typedef enum {
    /* The 5 basic drawing operations. */
    CAIRO_COMMAND_PAINT,
    CAIRO_COMMAND_MASK,
    CAIRO_COMMAND_STROKE,
    CAIRO_COMMAND_FILL,
    CAIRO_COMMAND_SHOW_TEXT_GLYPHS,

    /* cairo_tag_begin()/cairo_tag_end() */
    CAIRO_COMMAND_TAG,
} cairo_command_type_t;

typedef enum {
    CAIRO_RECORDING_REGION_ALL = 0,
    CAIRO_RECORDING_REGION_NATIVE,
    CAIRO_RECORDING_REGION_IMAGE_FALLBACK
} cairo_recording_region_type_t;

typedef enum {
    CAIRO_RECORDING_REPLAY,
    CAIRO_RECORDING_CREATE_REGIONS,
    CAIRO_RECORDING_REPLAY_REGION
} cairo_recording_replay_type_t;

typedef struct _cairo_command_header {
    cairo_command_type_t	 type;
    cairo_operator_t		 op;
    cairo_rectangle_int_t	 extents;
    cairo_clip_t		*clip;

    int index;
    struct _cairo_command_header *chain;
} cairo_command_header_t;

typedef struct _cairo_command_paint {
    cairo_command_header_t       header;
    cairo_pattern_union_t	 source;
} cairo_command_paint_t;

typedef struct _cairo_command_mask {
    cairo_command_header_t       header;
    cairo_pattern_union_t	 source;
    cairo_pattern_union_t	 mask;
} cairo_command_mask_t;

typedef struct _cairo_command_stroke {
    cairo_command_header_t       header;
    cairo_pattern_union_t	 source;
    cairo_path_fixed_t		 path;
    cairo_stroke_style_t	 style;
    cairo_matrix_t		 ctm;
    cairo_matrix_t		 ctm_inverse;
    double			 tolerance;
    cairo_antialias_t		 antialias;
} cairo_command_stroke_t;

typedef struct _cairo_command_fill {
    cairo_command_header_t       header;
    cairo_pattern_union_t	 source;
    cairo_path_fixed_t		 path;
    cairo_fill_rule_t		 fill_rule;
    double			 tolerance;
    cairo_antialias_t		 antialias;
} cairo_command_fill_t;

typedef struct _cairo_command_show_text_glyphs {
    cairo_command_header_t       header;
    cairo_pattern_union_t	 source;
    char			*utf8;
    int				 utf8_len;
    cairo_glyph_t		*glyphs;
    unsigned int		 num_glyphs;
    cairo_text_cluster_t	*clusters;
    int				 num_clusters;
    cairo_text_cluster_flags_t   cluster_flags;
    cairo_scaled_font_t		*scaled_font;
} cairo_command_show_text_glyphs_t;

typedef struct _cairo_command_tag {
    cairo_command_header_t       header;
    cairo_bool_t                 begin;
    char                        *tag_name;
    char                        *attributes;
} cairo_command_tag_t;

typedef union _cairo_command {
    cairo_command_header_t      header;

    cairo_command_paint_t			paint;
    cairo_command_mask_t			mask;
    cairo_command_stroke_t			stroke;
    cairo_command_fill_t			fill;
    cairo_command_show_text_glyphs_t		show_text_glyphs;
    cairo_command_tag_t                         tag;
} cairo_command_t;

typedef struct _cairo_recording_surface {
    cairo_surface_t base;

    /* A recording-surface is logically unbounded, but when used as a
     * source we need to render it to an image, so we need a size at
     * which to create that image. */
    cairo_rectangle_t extents_pixels;
    cairo_rectangle_int_t extents;
    cairo_bool_t unbounded;

    cairo_array_t commands;
    unsigned int *indices;
    unsigned int num_indices;
    cairo_bool_t optimize_clears;
    cairo_bool_t has_bilevel_alpha;
    cairo_bool_t has_only_op_over;

    struct bbtree {
	cairo_box_t extents;
	struct bbtree *left, *right;
	cairo_command_header_t *chain;
    } bbtree;

    /* The mutex protects modification to all subsequent fields. */
    cairo_mutex_t mutex;

    cairo_list_t region_array_list;

} cairo_recording_surface_t;

typedef struct _cairo_recording_region_element {
    cairo_recording_region_type_t region;
    unsigned int source_id;
    unsigned int mask_id;
} cairo_recording_region_element_t;

typedef struct _cairo_recording_region_array {
    unsigned int id;
    cairo_reference_count_t ref_count;
    cairo_array_t regions; /* cairo_recording_region_element_t */
    cairo_list_t link;
} cairo_recording_regions_array_t;

slim_hidden_proto (cairo_recording_surface_create);

cairo_private cairo_int_status_t
_cairo_recording_surface_get_path (cairo_surface_t	 *surface,
				   cairo_path_fixed_t *path);

cairo_private cairo_status_t
_cairo_recording_surface_replay_one (cairo_recording_surface_t	*surface,
				     long unsigned index,
				     cairo_surface_t *target);

cairo_private cairo_status_t
_cairo_recording_surface_replay (cairo_surface_t *surface,
				 cairo_surface_t *target);

cairo_private cairo_status_t
_cairo_recording_surface_replay_with_foreground_color (cairo_surface_t     *surface,
                                                       cairo_surface_t     *target,
                                                       const cairo_color_t *foreground_color,
                                                       cairo_bool_t        *foreground_used);

cairo_private cairo_status_t
_cairo_recording_surface_replay_with_clip (cairo_surface_t *surface,
					   const cairo_matrix_t *surface_transform,
					   cairo_surface_t *target,
					   const cairo_clip_t *target_clip,
                                           cairo_bool_t surface_is_unbounded);

cairo_private cairo_status_t
_cairo_recording_surface_replay_and_create_regions (cairo_surface_t      *surface,
                                                    unsigned int          regions_id,
						    const cairo_matrix_t *surface_transform,
						    cairo_surface_t      *target,
						    cairo_bool_t          surface_is_unbounded);
cairo_private cairo_status_t
_cairo_recording_surface_replay_region (cairo_surface_t			*surface,
                                        unsigned int                     regions_id,
					const cairo_rectangle_int_t     *surface_extents,
					cairo_surface_t			*target,
					cairo_recording_region_type_t	 region);

cairo_private cairo_status_t
_cairo_recording_surface_get_bbox (cairo_recording_surface_t *recording,
				   cairo_box_t *bbox,
				   const cairo_matrix_t *transform);

cairo_private cairo_status_t
_cairo_recording_surface_get_ink_bbox (cairo_recording_surface_t *surface,
				       cairo_box_t *bbox,
				       const cairo_matrix_t *transform);

cairo_private cairo_bool_t
_cairo_recording_surface_has_only_bilevel_alpha (cairo_recording_surface_t *surface);

cairo_private cairo_bool_t
_cairo_recording_surface_has_only_op_over (cairo_recording_surface_t *surface);

cairo_private cairo_status_t
_cairo_recording_surface_region_array_attach (cairo_surface_t *surface,
                                              unsigned int    *id);

cairo_private void
_cairo_recording_surface_region_array_reference (cairo_surface_t *surface,
                                                 unsigned int     id);

cairo_private void
_cairo_recording_surface_region_array_remove (cairo_surface_t *surface,
                                              unsigned int     id);

cairo_private void
_cairo_debug_print_recording_surface (FILE            *file,
				      cairo_surface_t *surface,
                                      unsigned int     regions_id,
				      int              indent,
				      cairo_bool_t     recurse);

#endif /* CAIRO_RECORDING_SURFACE_H */
