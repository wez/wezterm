/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
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
 *      Graydon Hoare <graydon@redhat.com>
 *	Owen Taylor <otaylor@redhat.com>
 */

#ifndef CAIRO_FT_PRIVATE_H
#define CAIRO_FT_PRIVATE_H

#include "cairoint.h"
#include "cairo-ft.h"

#if CAIRO_HAS_FT_FONT

CAIRO_BEGIN_DECLS

typedef struct _cairo_ft_unscaled_font cairo_ft_unscaled_font_t;

cairo_private cairo_bool_t
_cairo_scaled_font_is_ft (cairo_scaled_font_t *scaled_font);

/* These functions are needed by the PDF backend, which needs to keep track of the
 * the different fonts-on-disk used by a document, so it can embed them
 */
cairo_private unsigned int
_cairo_ft_scaled_font_get_load_flags (cairo_scaled_font_t *scaled_font);

cairo_private cairo_status_t
_cairo_ft_to_cairo_error (FT_Error error);

cairo_private cairo_status_t
_cairo_ft_face_decompose_glyph_outline (FT_Face		     face,
					cairo_path_fixed_t **pathp);

#if HAVE_FT_SVG_DOCUMENT

typedef struct FT_Color_ FT_Color;

cairo_private cairo_status_t
_cairo_render_svg_glyph (const char           *svg_document,
                         unsigned long         first_glyph,
                         unsigned long         last_glyph,
                         unsigned long         glyph,
                         double                units_per_em,
                         FT_Color             *palette,
                         int                   num_palette_entries,
                         cairo_t              *cr,
                         cairo_pattern_t      *foreground_source,
                         cairo_bool_t         *foreground_source_used);
#endif

#if HAVE_FT_COLR_V1
cairo_private cairo_status_t
_cairo_render_colr_v1_glyph (FT_Face                 face,
                             unsigned long           glyph,
                             FT_Color               *palette,
                             int                     num_palette_entries,
                             cairo_t                *cr,
                             cairo_pattern_t        *foreground_source,
                             cairo_bool_t           *foreground_source_used);
#endif

CAIRO_END_DECLS

#endif /* CAIRO_HAS_FT_FONT */
#endif /* CAIRO_FT_PRIVATE_H */
