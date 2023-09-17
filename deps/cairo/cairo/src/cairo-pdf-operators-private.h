/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2004 Red Hat, Inc
 * Copyright © 2006 Red Hat, Inc
 * Copyright © 2007 Adrian Johnson
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
 *	Kristian Høgsberg <krh@redhat.com>
 *	Carl Worth <cworth@cworth.org>
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#ifndef CAIRO_PDF_OPERATORS_H
#define CAIRO_PDF_OPERATORS_H

#include "cairo-compiler-private.h"
#include "cairo-error-private.h"
#include "cairo-types-private.h"

/* The glyph buffer size is based on the expected maximum glyphs in a
 * line so that an entire line can be emitted in as one string. If the
 * glyphs in a line exceeds this size the only downside is the slight
 * overhead of emitting two strings.
 */
#define PDF_GLYPH_BUFFER_SIZE 200

typedef cairo_int_status_t
(*cairo_pdf_operators_use_font_subset_t) (unsigned int  font_id,
					  unsigned int  subset_id,
					  void         *closure);

typedef struct _cairo_pdf_glyph {
    unsigned int glyph_index;
    double x_position;
    double x_advance;
} cairo_pdf_glyph_t;

typedef struct _cairo_pdf_operators {
    cairo_output_stream_t *stream;
    cairo_matrix_t cairo_to_pdf;
    cairo_scaled_font_subsets_t *font_subsets;
    cairo_pdf_operators_use_font_subset_t use_font_subset;
    void *use_font_subset_closure;
    cairo_bool_t ps_output; /* output is for PostScript */
    cairo_bool_t use_actual_text;
    cairo_bool_t in_text_object; /* inside BT/ET pair */

    /* PDF text state */
    cairo_bool_t is_new_text_object; /* text object started but matrix and font not yet selected */
    unsigned int font_id;
    unsigned int subset_id;
    cairo_matrix_t text_matrix; /* PDF text matrix (Tlm in the PDF reference) */
    cairo_matrix_t cairo_to_pdftext; /* translate cairo coords to PDF text space */
    cairo_matrix_t font_matrix_inverse;
    double cur_x; /* Current position in PDF text space (Tm in the PDF reference) */
    double cur_y;
    int hex_width;
    cairo_bool_t is_latin;
    int num_glyphs;
    double glyph_buf_x_pos;
    cairo_pdf_glyph_t glyphs[PDF_GLYPH_BUFFER_SIZE];

    /* PDF line style */
    cairo_bool_t         has_line_style;
    double		 line_width;
    cairo_line_cap_t	 line_cap;
    cairo_line_join_t	 line_join;
    double		 miter_limit;
    cairo_bool_t         has_dashes;
} cairo_pdf_operators_t;

cairo_private void
_cairo_pdf_operators_init (cairo_pdf_operators_t       *pdf_operators,
			   cairo_output_stream_t       *stream,
			   cairo_matrix_t 	       *cairo_to_pdf,
			   cairo_scaled_font_subsets_t *font_subsets,
			   cairo_bool_t                 ps);

cairo_private cairo_status_t
_cairo_pdf_operators_fini (cairo_pdf_operators_t       *pdf_operators);

cairo_private void
_cairo_pdf_operators_set_font_subsets_callback (cairo_pdf_operators_t 		     *pdf_operators,
						cairo_pdf_operators_use_font_subset_t use_font_subset,
						void				     *closure);

cairo_private void
_cairo_pdf_operators_set_stream (cairo_pdf_operators_t 	 *pdf_operators,
				 cairo_output_stream_t   *stream);


cairo_private void
_cairo_pdf_operators_set_cairo_to_pdf_matrix (cairo_pdf_operators_t *pdf_operators,
					      cairo_matrix_t 	    *cairo_to_pdf);

cairo_private void
_cairo_pdf_operators_enable_actual_text (cairo_pdf_operators_t *pdf_operators,
					 cairo_bool_t 	  	enable);

cairo_private cairo_status_t
_cairo_pdf_operators_flush (cairo_pdf_operators_t	 *pdf_operators);

cairo_private void
_cairo_pdf_operators_reset (cairo_pdf_operators_t	 *pdf_operators);

cairo_private cairo_int_status_t
_cairo_pdf_operators_clip (cairo_pdf_operators_t	*pdf_operators,
			   const cairo_path_fixed_t	*path,
			   cairo_fill_rule_t		 fill_rule);

cairo_private cairo_int_status_t
_cairo_pdf_operators_emit_stroke_style (cairo_pdf_operators_t		*pdf_operators,
					const cairo_stroke_style_t	*style,
					double				 scale);

cairo_private cairo_int_status_t
_cairo_pdf_operators_stroke (cairo_pdf_operators_t	*pdf_operators,
			     const cairo_path_fixed_t	*path,
			     const cairo_stroke_style_t	*style,
			     const cairo_matrix_t	*ctm,
			     const cairo_matrix_t	*ctm_inverse);

cairo_private cairo_int_status_t
_cairo_pdf_operators_fill (cairo_pdf_operators_t	*pdf_operators,
			   const cairo_path_fixed_t	*path,
			   cairo_fill_rule_t		fill_rule);

cairo_private cairo_int_status_t
_cairo_pdf_operators_fill_stroke (cairo_pdf_operators_t		*pdf_operators,
				  const cairo_path_fixed_t	*path,
				  cairo_fill_rule_t		 fill_rule,
				  const cairo_stroke_style_t	*style,
				  const cairo_matrix_t		*ctm,
				  const cairo_matrix_t		*ctm_inverse);

cairo_private cairo_int_status_t
_cairo_pdf_operators_show_text_glyphs (cairo_pdf_operators_t	  *pdf_operators,
				       const char                 *utf8,
				       int                         utf8_len,
				       cairo_glyph_t              *glyphs,
				       int                         num_glyphs,
				       const cairo_text_cluster_t *clusters,
				       int                         num_clusters,
				       cairo_text_cluster_flags_t  cluster_flags,
				       cairo_scaled_font_t	  *scaled_font);

cairo_private cairo_int_status_t
_cairo_pdf_operators_tag_begin (cairo_pdf_operators_t *pdf_operators,
				const char            *tag_name,
				int                    mcid);

cairo_private cairo_int_status_t
_cairo_pdf_operators_tag_end (cairo_pdf_operators_t *pdf_operators);

#endif /* CAIRO_PDF_OPERATORS_H */
