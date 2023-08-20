/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
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
 *	Carl D. Worth <cworth@cworth.org>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_STROKE_DASH_PRIVATE_H
#define CAIRO_STROKE_DASH_PRIVATE_H

#include "cairoint.h"

CAIRO_BEGIN_DECLS

typedef struct _cairo_stroker_dash {
    cairo_bool_t dashed;
    unsigned int dash_index;
    cairo_bool_t dash_on;
    cairo_bool_t dash_starts_on;
    double dash_remain;

    double dash_offset;
    const double *dashes;
    unsigned int num_dashes;
} cairo_stroker_dash_t;

cairo_private void
_cairo_stroker_dash_init (cairo_stroker_dash_t *dash,
			  const cairo_stroke_style_t *style);

cairo_private void
_cairo_stroker_dash_start (cairo_stroker_dash_t *dash);

cairo_private void
_cairo_stroker_dash_step (cairo_stroker_dash_t *dash, double step);

CAIRO_END_DECLS

#endif /* CAIRO_STROKE_DASH_PRIVATE_H */
