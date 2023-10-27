/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2004 Calum Robinson
 * Copyright (C) 2006,2007 Mozilla Corporation
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
 * The Initial Developer of the Original Code is Calum Robinson
 *
 * Contributor(s):
 *    Calum Robinson <calumr@mac.com>
 *    Vladimir Vukicevic <vladimir@mozilla.com>
 */

#ifndef CAIRO_QUARTZ_PRIVATE_H
#define CAIRO_QUARTZ_PRIVATE_H

#include "cairoint.h"

#if CAIRO_HAS_QUARTZ_SURFACE
#include "cairo-quartz.h"
#include "cairo-surface-clipper-private.h"

#ifndef CGFLOAT_DEFINED
/* On 10.4, Quartz APIs used float instead of CGFloat */
typedef float CGFloat;
#endif

typedef CGFloat cairo_quartz_float_t;

typedef enum {
    DO_DIRECT,
    DO_SHADING,
    DO_IMAGE,
    DO_LAYER
} cairo_quartz_action_t;

/* define CTFontRef for pre-10.5 SDKs */
typedef const struct __CTFont *CTFontRef;

typedef struct cairo_quartz_surface {
    cairo_surface_t base;

    CGContextRef cgContext;
    CGAffineTransform cgContextBaseCTM;

#if MAC_OS_X_VERSION_MIN_REQUIRED < 10600
    void *imageData;
#endif

    cairo_surface_clipper_t clipper;
    cairo_rectangle_int_t extents;
    cairo_rectangle_int_t virtual_extents;
    CGLayerRef cgLayer;
} cairo_quartz_surface_t;

typedef struct cairo_quartz_image_surface {
    cairo_surface_t base;

    int width, height;

    CGImageRef image;
    cairo_image_surface_t *imageSurface;
} cairo_quartz_image_surface_t;

cairo_private cairo_bool_t
_cairo_quartz_verify_surface_size(int width, int height);

cairo_private cairo_bool_t
_cairo_surface_is_quartz (const cairo_surface_t *surface);

cairo_private CGImageRef
CairoQuartzCreateCGImage (cairo_format_t format,
			      unsigned int width,
			      unsigned int height,
			      unsigned int stride,
			      void *data,
			      cairo_bool_t interpolate,
			      CGColorSpaceRef colorSpaceOverride,
			      CGDataProviderReleaseDataCallback releaseCallback,
			      void *releaseInfo);

cairo_private CGFontRef
_cairo_quartz_scaled_font_get_cg_font_ref (cairo_scaled_font_t *sfont);
cairo_private CTFontRef
_cairo_quartz_scaled_font_get_ct_font (cairo_scaled_font_t *sfont);
cairo_private cairo_font_face_t*
_cairo_quartz_font_face_create_for_ctfont (CTFontRef ctFont);
cairo_private void
_cairo_quartz_set_antialiasing (CGContextRef context, cairo_antialias_t antialias);

#else

# error Cairo was not compiled with support for the quartz backend

#endif /* CAIRO_HAS_QUARTZ_SURFACE */

#endif /* CAIRO_QUARTZ_PRIVATE_H */
