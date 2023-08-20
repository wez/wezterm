/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2004 Keith Packard
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
 * The Initial Developer of the Original Code is Keith Packard
 *
 * Contributor(s):
 *	Keith R. Packard <keithp@keithp.com>
 *
 */

#ifndef CAIRO_WIDEINT_TYPE_H
#define CAIRO_WIDEINT_TYPE_H

#include "cairo.h"

#include "config.h"

#if   HAVE_STDINT_H
# include <stdint.h>
#elif HAVE_INTTYPES_H
# include <inttypes.h>
#elif HAVE_SYS_INT_TYPES_H
# include <sys/int_types.h>
#elif defined(_MSC_VER)
  typedef __int8 int8_t;
  typedef unsigned __int8 uint8_t;
  typedef __int16 int16_t;
  typedef unsigned __int16 uint16_t;
  typedef __int32 int32_t;
  typedef unsigned __int32 uint32_t;
  typedef __int64 int64_t;
  typedef unsigned __int64 uint64_t;
# ifndef HAVE_UINT64_T
#  define HAVE_UINT64_T 1
# endif
#else
#error Cannot find definitions for fixed-width integral types (uint8_t, uint32_t, etc.)
#endif

#ifndef INT16_MIN
# define INT16_MIN	(-32767-1)
#endif
#ifndef INT16_MAX
# define INT16_MAX	(32767)
#endif
#ifndef UINT16_MAX
# define UINT16_MAX	(65535)
#endif
#ifndef INT32_MIN
# define INT32_MIN	(-2147483647-1)
#endif
#ifndef INT32_MAX
# define INT32_MAX	(2147483647)
#endif
#ifndef UINT32_MAX
# define UINT32_MAX     (4294967295U)
#endif

#if HAVE_BYTESWAP_H
# include <byteswap.h>
#endif
#ifndef bswap_16
# define bswap_16(p) \
	(((((uint16_t)(p)) & 0x00ff) << 8) | \
	  (((uint16_t)(p))           >> 8));
#endif
#ifndef bswap_32
# define bswap_32(p) \
         (((((uint32_t)(p)) & 0x000000ff) << 24) | \
	  ((((uint32_t)(p)) & 0x0000ff00) << 8)  | \
	  ((((uint32_t)(p)) & 0x00ff0000) >> 8)  | \
	  ((((uint32_t)(p)))              >> 24));
#endif


#if !HAVE_UINT64_T

typedef struct _cairo_uint64 {
    uint32_t	lo, hi;
} cairo_uint64_t, cairo_int64_t;

#else

typedef uint64_t    cairo_uint64_t;
typedef int64_t	    cairo_int64_t;

#endif

typedef struct _cairo_uquorem64 {
    cairo_uint64_t	quo;
    cairo_uint64_t	rem;
} cairo_uquorem64_t;

typedef struct _cairo_quorem64 {
    cairo_int64_t	quo;
    cairo_int64_t	rem;
} cairo_quorem64_t;

/* gcc has a non-standard name. */
#if HAVE___UINT128_T && !HAVE_UINT128_T
typedef __uint128_t uint128_t;
typedef __int128_t int128_t;
#define HAVE_UINT128_T 1
#endif

#if !HAVE_UINT128_T

typedef struct cairo_uint128 {
    cairo_uint64_t	lo, hi;
} cairo_uint128_t, cairo_int128_t;

#else

typedef uint128_t	cairo_uint128_t;
typedef int128_t	cairo_int128_t;

#endif

typedef struct _cairo_uquorem128 {
    cairo_uint128_t	quo;
    cairo_uint128_t	rem;
} cairo_uquorem128_t;

typedef struct _cairo_quorem128 {
    cairo_int128_t	quo;
    cairo_int128_t	rem;
} cairo_quorem128_t;


#endif /* CAIRO_WIDEINT_H */
