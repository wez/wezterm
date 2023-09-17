/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2023 Adrian Johnson
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
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#ifndef _CAIRO_DWRITE_H_
#define _CAIRO_DWRITE_H_

#include "cairo.h"

#if CAIRO_HAS_DWRITE_FONT

#ifdef __cplusplus

#include <dwrite.h>

CAIRO_BEGIN_DECLS

cairo_public cairo_font_face_t *
cairo_dwrite_font_face_create_for_dwrite_fontface (IDWriteFontFace *dwrite_font_face);

cairo_public IDWriteRenderingParams *
cairo_dwrite_font_face_get_rendering_params (cairo_font_face_t *font_face);

cairo_public void
cairo_dwrite_font_face_set_rendering_params (cairo_font_face_t *font_face, IDWriteRenderingParams *param);

cairo_public DWRITE_MEASURING_MODE
cairo_dwrite_font_face_get_measuring_mode (cairo_font_face_t *font_face);

cairo_public void
cairo_dwrite_font_face_set_measuring_mode (cairo_font_face_t *font_face, DWRITE_MEASURING_MODE mode);

CAIRO_END_DECLS

#else  /* __cplusplus */
#error DWrite font backend requires C++
#endif /* __cplusplus */

#else  /* CAIRO_HAS_DWRITE_FONT */
# error Cairo was not compiled with support for DWrite font backend
#endif /* CAIRO_HAS_DWRITE_FONT */

#endif /* _CAIRO_DWRITE_H_ */
