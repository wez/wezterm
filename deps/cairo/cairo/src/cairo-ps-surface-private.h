/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2003 University of Southern California
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
 * The Initial Developer of the Original Code is University of Southern
 * California.
 *
 * Contributor(s):
 *	Carl D. Worth <cworth@cworth.org>
 *	Kristian Høgsberg <krh@redhat.com>
 *	Keith Packard <keithp@keithp.com>
 */

#ifndef CAIRO_PS_SURFACE_PRIVATE_H
#define CAIRO_PS_SURFACE_PRIVATE_H

#include "cairo-ps.h"

#include "cairo-surface-private.h"
#include "cairo-surface-clipper-private.h"
#include "cairo-pdf-operators-private.h"

#include <time.h>

typedef struct _cairo_ps_form {
    cairo_hash_entry_t base;
    unsigned char *unique_id;
    unsigned long unique_id_length;
    cairo_bool_t is_image;
    int id;
    cairo_surface_t *src_surface;
    unsigned int regions_id;
    cairo_rectangle_int_t src_surface_extents;
    cairo_bool_t src_surface_bounded;
    cairo_filter_t filter;

    /* Union of source extents required for all operations using this form */
    cairo_rectangle_int_t required_extents;
} cairo_ps_form_t;

typedef struct cairo_ps_surface {
    cairo_surface_t base;

    /* Here final_stream corresponds to the stream/file passed to
     * cairo_ps_surface_create surface is built. Meanwhile stream is a
     * temporary stream in which the file output is built, (so that
     * the header can be built and inserted into the target stream
     * before the contents of the temporary stream are copied). */
    cairo_output_stream_t *final_stream;

    FILE *tmpfile;
    cairo_output_stream_t *stream;

    cairo_bool_t eps;
    cairo_bool_t contains_eps;
    cairo_content_t content;
    double width;
    double height;
    cairo_point_int_t document_bbox_p1, document_bbox_p2; /* in PS coordinates */
    cairo_rectangle_int_t surface_extents;
    cairo_bool_t surface_bounded;
    cairo_matrix_t cairo_to_ps;
    cairo_bool_t paint_proc; /* TRUE if surface will be used in a PaintProc */

    cairo_bool_t current_pattern_is_solid_color;
    cairo_color_t current_color;

    int num_pages;

    cairo_paginated_mode_t paginated_mode;

    cairo_bool_t force_fallbacks;
    cairo_bool_t has_creation_date;
    time_t creation_date;

    cairo_scaled_font_subsets_t *font_subsets;

    cairo_list_t document_media;
    cairo_array_t dsc_header_comments;
    cairo_array_t dsc_setup_comments;
    cairo_array_t dsc_page_setup_comments;

    cairo_array_t recording_surf_stack;

    cairo_array_t *dsc_comment_target;

    cairo_ps_level_t ps_level;
    cairo_ps_level_t ps_level_used;

    cairo_surface_clipper_t clipper;

    cairo_pdf_operators_t pdf_operators;
    cairo_surface_t *paginated_surface;
    cairo_hash_table_t *forms;
    int num_forms;
    long total_form_size;
} cairo_ps_surface_t;

#endif /* CAIRO_PS_SURFACE_PRIVATE_H */
