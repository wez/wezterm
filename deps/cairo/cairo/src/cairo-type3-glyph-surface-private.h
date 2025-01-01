/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
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

#ifndef CAIRO_TYPE3_GLYPH_SURFACE_PRIVATE_H
#define CAIRO_TYPE3_GLYPH_SURFACE_PRIVATE_H

#include "cairoint.h"

#if CAIRO_HAS_FONT_SUBSET

#include "cairo-surface-private.h"
#include "cairo-surface-clipper-private.h"
#include "cairo-pdf-operators-private.h"

typedef cairo_int_status_t
(*cairo_type3_glyph_surface_emit_image_t) (cairo_image_surface_t *image,
					   cairo_output_stream_t	*stream);

typedef struct cairo_type3_glyph_surface {
    cairo_surface_t base;

    cairo_scaled_font_t *scaled_font;
    cairo_output_stream_t *stream;
    cairo_pdf_operators_t pdf_operators;
    cairo_matrix_t cairo_to_pdf;
    cairo_type3_glyph_surface_emit_image_t emit_image;

    cairo_surface_clipper_t clipper;
} cairo_type3_glyph_surface_t;

cairo_private cairo_surface_t *
_cairo_type3_glyph_surface_create (cairo_scaled_font_t			 *scaled_font,
				   cairo_output_stream_t		 *stream,
				   cairo_type3_glyph_surface_emit_image_t emit_image,
				   cairo_scaled_font_subsets_t		 *font_subsets,
				   cairo_bool_t                           ps_output);

cairo_private void
_cairo_type3_glyph_surface_set_font_subsets_callback (void				    *abstract_surface,
						      cairo_pdf_operators_use_font_subset_t  use_font_subset,
						      void				    *closure);

cairo_private cairo_status_t
_cairo_type3_glyph_surface_analyze_glyph (void		     *abstract_surface,
					  unsigned long	      glyph_index);

cairo_private cairo_status_t
_cairo_type3_glyph_surface_emit_glyph (void		     *abstract_surface,
				       cairo_output_stream_t *stream,
				       unsigned long	      glyph_index,
				       cairo_box_t           *bbox,
				       double                *width);

#endif /* CAIRO_HAS_FONT_SUBSET */

#endif /* CAIRO_TYPE3_GLYPH_SURFACE_PRIVATE_H */
