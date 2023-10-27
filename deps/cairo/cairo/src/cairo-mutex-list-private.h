/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2007 Mathias Hasselmann
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
 * Contributor(s):
 *	Mathias Hasselmann <mathias.hasselmann@gmx.de>
 */

#ifndef CAIRO_FEATURES_H
/* This block is to just make this header file standalone */
#define CAIRO_MUTEX_DECLARE(mutex)
#endif

CAIRO_MUTEX_DECLARE (_cairo_pattern_solid_surface_cache_lock)

CAIRO_MUTEX_DECLARE (_cairo_image_solid_cache_mutex)

CAIRO_MUTEX_DECLARE (_cairo_toy_font_face_mutex)
CAIRO_MUTEX_DECLARE (_cairo_intern_string_mutex)
CAIRO_MUTEX_DECLARE (_cairo_scaled_font_map_mutex)
CAIRO_MUTEX_DECLARE (_cairo_scaled_glyph_page_cache_mutex)
CAIRO_MUTEX_DECLARE (_cairo_scaled_font_error_mutex)
CAIRO_MUTEX_DECLARE (_cairo_glyph_cache_mutex)

#if CAIRO_HAS_FT_FONT
CAIRO_MUTEX_DECLARE (_cairo_ft_unscaled_font_map_mutex)
#endif

#if CAIRO_HAS_WIN32_FONT
CAIRO_MUTEX_DECLARE (_cairo_win32_font_face_mutex)
CAIRO_MUTEX_DECLARE (_cairo_win32_font_dc_mutex)
#endif

#if CAIRO_HAS_XLIB_SURFACE
CAIRO_MUTEX_DECLARE (_cairo_xlib_display_mutex)
#endif

#if CAIRO_HAS_XCB_SURFACE
CAIRO_MUTEX_DECLARE (_cairo_xcb_connections_mutex)
#endif

#if !defined (HAS_ATOMIC_OPS) || defined (ATOMIC_OP_NEEDS_MEMORY_BARRIER)
CAIRO_MUTEX_DECLARE (_cairo_atomic_mutex)
#endif

/* Undefine, to err on unintended inclusion */
#undef   CAIRO_MUTEX_DECLARE
