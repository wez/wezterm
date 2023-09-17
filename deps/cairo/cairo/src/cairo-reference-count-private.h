/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2007 Chris Wilson
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
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_REFRENCE_COUNT_PRIVATE_H
#define CAIRO_REFRENCE_COUNT_PRIVATE_H

#include "cairo-atomic-private.h"

/* Encapsulate operations on the object's reference count */
typedef struct {
    cairo_atomic_int_t ref_count;
} cairo_reference_count_t;

#define _cairo_reference_count_inc(RC) _cairo_atomic_int_inc (&(RC)->ref_count)
#define _cairo_reference_count_dec(RC) _cairo_atomic_int_dec (&(RC)->ref_count)
#define _cairo_reference_count_dec_and_test(RC) _cairo_atomic_int_dec_and_test (&(RC)->ref_count)

#define CAIRO_REFERENCE_COUNT_INIT(RC, VALUE) ((RC)->ref_count = (VALUE))

#define CAIRO_REFERENCE_COUNT_GET_VALUE(RC) _cairo_atomic_int_get (&(RC)->ref_count)

#define CAIRO_REFERENCE_COUNT_INVALID_VALUE ((cairo_atomic_int_t) -1)
#define CAIRO_REFERENCE_COUNT_INVALID {CAIRO_REFERENCE_COUNT_INVALID_VALUE}

#define CAIRO_REFERENCE_COUNT_IS_INVALID(RC) (CAIRO_REFERENCE_COUNT_GET_VALUE (RC) == CAIRO_REFERENCE_COUNT_INVALID_VALUE)

#define CAIRO_REFERENCE_COUNT_HAS_REFERENCE(RC) (CAIRO_REFERENCE_COUNT_GET_VALUE (RC) > 0)

#endif
