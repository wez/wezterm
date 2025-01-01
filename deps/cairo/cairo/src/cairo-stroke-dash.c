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

#include "cairoint.h"

#include "cairo-stroke-dash-private.h"

void
_cairo_stroker_dash_start (cairo_stroker_dash_t *dash)
{
    double offset;
    cairo_bool_t on = TRUE;
    unsigned int i = 0;

    if (! dash->dashed)
	return;

    offset = dash->dash_offset;

    /* We stop searching for a starting point as soon as the
       offset reaches zero.  Otherwise when an initial dash
       segment shrinks to zero it will be skipped over. */
    while (offset > 0.0 && offset >= dash->dashes[i]) {
	offset -= dash->dashes[i];
	on = !on;
	if (++i == dash->num_dashes)
	    i = 0;
    }

    dash->dash_index = i;
    dash->dash_on = dash->dash_starts_on = on;
    dash->dash_remain = dash->dashes[i] - offset;
}

void
_cairo_stroker_dash_step (cairo_stroker_dash_t *dash, double step)
{
    dash->dash_remain -= step;
    if (dash->dash_remain < CAIRO_FIXED_ERROR_DOUBLE) {
	if (++dash->dash_index == dash->num_dashes)
	    dash->dash_index = 0;

	dash->dash_on = ! dash->dash_on;
	dash->dash_remain += dash->dashes[dash->dash_index];
    }
}

void
_cairo_stroker_dash_init (cairo_stroker_dash_t *dash,
			  const cairo_stroke_style_t *style)
{
    dash->dashed = style->dash != NULL;
    if (! dash->dashed)
	return;

    dash->dashes = style->dash;
    dash->num_dashes = style->num_dashes;
    dash->dash_offset = style->dash_offset;

    _cairo_stroker_dash_start (dash);
}
