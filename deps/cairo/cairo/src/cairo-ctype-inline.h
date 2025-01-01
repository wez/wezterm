/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2009,2016,2021,2022 Adrian Johnson
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
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#ifndef _CAIRO_CTYPE_INLINE_H_
#define _CAIRO_CTYPE_INLINE_H_

#include "cairo-error-private.h"

CAIRO_BEGIN_DECLS

/* ASCII only versions of some ctype.h character classification functions.
 * The glibc versions are slow in UTF-8 locales.
 */

static inline int cairo_const
_cairo_isspace (int c)
{
    return (c == 0x20 || (c >= 0x09 && c <= 0x0d));
}

static inline int cairo_const
_cairo_isdigit (int c)
{
    return (c >= '0' && c <= '9');
}

static inline int cairo_const
_cairo_isxdigit (int c)
{
    return ((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F'));
}

static inline int cairo_const
_cairo_isalpha (int c)
{
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z');
}

static inline int cairo_const
_cairo_isprint (int c)
{
    return (c >= 0x20 && c <= 0x7e);
}

static inline int cairo_const
_cairo_isalnum (int c)
{
    return ((c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z'));
}

CAIRO_END_DECLS

#endif /* _CAIRO_CTYPE_INLINE_H_ */
