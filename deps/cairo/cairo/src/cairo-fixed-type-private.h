/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* Cairo - a vector graphics library with display and print output
 *
 * Copyright © 2007 Mozilla Corporation
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
 * The Initial Developer of the Original Code is Mozilla Foundation
 *
 * Contributor(s):
 *	Vladimir Vukicevic <vladimir@pobox.com>
 */

#ifndef CAIRO_FIXED_TYPE_PRIVATE_H
#define CAIRO_FIXED_TYPE_PRIVATE_H

#include "cairo-wideint-type-private.h"

/*
 * Fixed-point configuration
 */

typedef int32_t		cairo_fixed_16_16_t;
typedef cairo_int64_t	cairo_fixed_32_32_t;
typedef cairo_int64_t	cairo_fixed_48_16_t;
typedef cairo_int128_t	cairo_fixed_64_64_t;
typedef cairo_int128_t	cairo_fixed_96_32_t;

/* Eventually, we should allow changing this, but I think
 * there are some assumptions in the tessellator about the
 * size of a fixed type.  For now, it must be 32.
 */
#define CAIRO_FIXED_BITS	32

/* The number of fractional bits.  Changing this involves
 * making sure that you compute a double-to-fixed magic number.
 * (see below).
 */
#define CAIRO_FIXED_FRAC_BITS	8

/* A signed type %CAIRO_FIXED_BITS in size; the main fixed point type */
typedef int32_t cairo_fixed_t;

/* An unsigned type of the same size as #cairo_fixed_t */
typedef uint32_t cairo_fixed_unsigned_t;

typedef struct _cairo_point {
    cairo_fixed_t x;
    cairo_fixed_t y;
} cairo_point_t;

#endif /* CAIRO_FIXED_TYPE_PRIVATE_H */
