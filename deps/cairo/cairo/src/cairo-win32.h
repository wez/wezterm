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
 *	Owen Taylor <otaylor@redhat.com>
 */

#ifndef _CAIRO_WIN32_H_
#define _CAIRO_WIN32_H_

#include "cairo.h"

#if CAIRO_HAS_WIN32_SURFACE

#include <windows.h>

CAIRO_BEGIN_DECLS

cairo_public cairo_surface_t *
cairo_win32_surface_create (HDC hdc);

cairo_public cairo_surface_t *
cairo_win32_surface_create_with_format (HDC hdc,
                                        cairo_format_t format);

cairo_public cairo_surface_t *
cairo_win32_printing_surface_create (HDC hdc);

cairo_public cairo_surface_t *
cairo_win32_surface_create_with_ddb (HDC hdc,
                                     cairo_format_t format,
                                     int width,
                                     int height);

cairo_public cairo_surface_t *
cairo_win32_surface_create_with_dib (cairo_format_t format,
                                     int width,
                                     int height);

cairo_public HDC
cairo_win32_surface_get_dc (cairo_surface_t *surface);

cairo_public cairo_surface_t *
cairo_win32_surface_get_image (cairo_surface_t *surface);

#if CAIRO_HAS_WIN32_FONT

/*
 * Win32 font support
 */

cairo_public cairo_font_face_t *
cairo_win32_font_face_create_for_logfontw (LOGFONTW *logfont);

cairo_public cairo_font_face_t *
cairo_win32_font_face_create_for_hfont (HFONT font);

cairo_public cairo_font_face_t *
cairo_win32_font_face_create_for_logfontw_hfont (LOGFONTW *logfont, HFONT font);

cairo_public cairo_status_t
cairo_win32_scaled_font_select_font (cairo_scaled_font_t *scaled_font,
				     HDC                  hdc);

cairo_public void
cairo_win32_scaled_font_done_font (cairo_scaled_font_t *scaled_font);

cairo_public double
cairo_win32_scaled_font_get_metrics_factor (cairo_scaled_font_t *scaled_font);

cairo_public void
cairo_win32_scaled_font_get_logical_to_device (cairo_scaled_font_t *scaled_font,
					       cairo_matrix_t *logical_to_device);

cairo_public void
cairo_win32_scaled_font_get_device_to_logical (cairo_scaled_font_t *scaled_font,
					       cairo_matrix_t *device_to_logical);

#endif /* CAIRO_HAS_WIN32_FONT */

CAIRO_END_DECLS

#else  /* CAIRO_HAS_WIN32_SURFACE */
# error Cairo was not compiled with support for the win32 backend
#endif /* CAIRO_HAS_WIN32_SURFACE */

#endif /* _CAIRO_WIN32_H_ */
