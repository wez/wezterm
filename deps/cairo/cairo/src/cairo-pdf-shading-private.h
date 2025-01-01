/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2009 Adrian Johnson
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

#ifndef CAIRO_PDF_SHADING_H
#define CAIRO_PDF_SHADING_H

#include "cairo-compiler-private.h"
#include "cairo-types-private.h"
#include "cairo-pattern-private.h"


typedef struct _cairo_pdf_shading {
    int shading_type;
    int bits_per_coordinate;
    int bits_per_component;
    int bits_per_flag;
    double *decode_array;
    int decode_array_length;
    unsigned char *data;
    unsigned long data_length;
} cairo_pdf_shading_t;


/**
 * _cairo_pdf_shading_init_color:
 * @shading: a #cairo_pdf_shading_t to initialize
 * @pattern: the #cairo_mesh_pattern_t to initialize from
 *
 * Generate the PDF shading dictionary data for the a PDF type 7
 * shading from RGB part of the specified mesh pattern.
 *
 * Return value: %CAIRO_STATUS_SUCCESS if successful, possible errors
 * include %CAIRO_STATUS_NO_MEMORY.
 **/
cairo_private cairo_status_t
_cairo_pdf_shading_init_color (cairo_pdf_shading_t        *shading,
			       const cairo_mesh_pattern_t *pattern);


/**
 * _cairo_pdf_shading_init_alpha:
 * @shading: a #cairo_pdf_shading_t to initialize
 * @pattern: the #cairo_mesh_pattern_t to initialize from
 *
 * Generate the PDF shading dictionary data for a PDF type 7
 * shading from alpha part of the specified mesh pattern.
 *
 * Return value: %CAIRO_STATUS_SUCCESS if successful, possible errors
 * include %CAIRO_STATUS_NO_MEMORY.
 **/
cairo_private cairo_status_t
_cairo_pdf_shading_init_alpha (cairo_pdf_shading_t        *shading,
			       const cairo_mesh_pattern_t *pattern);

/**
 * _cairo_pdf_shading_fini:
 * @shading: a #cairo_pdf_shading_t
 *
 * Free all resources associated with @shading.  After this call,
 * @shading should not be used again without a subsequent call to
 * _cairo_pdf_shading_init() again first.
 **/
cairo_private void
_cairo_pdf_shading_fini (cairo_pdf_shading_t *shading);


#endif /* CAIRO_PDF_SHADING_H */
