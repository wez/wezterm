/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2005 Red Hat, Inc.
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
 *      Vladimir Vukicevic <vladimir@pobox.com>
 *      Søren Sandmann <sandmann@daimi.au.dk>
 */

#ifndef CAIRO_REGION_PRIVATE_H
#define CAIRO_REGION_PRIVATE_H

#include "cairo-types-private.h"
#include "cairo-reference-count-private.h"

#include <pixman.h>

CAIRO_BEGIN_DECLS

struct _cairo_region {
    cairo_reference_count_t ref_count;
    cairo_status_t status;

    pixman_region32_t rgn;
};

cairo_private cairo_region_t *
_cairo_region_create_in_error (cairo_status_t status);

cairo_private void
_cairo_region_init (cairo_region_t *region);

cairo_private void
_cairo_region_init_rectangle (cairo_region_t *region,
			      const cairo_rectangle_int_t *rectangle);

cairo_private void
_cairo_region_fini (cairo_region_t *region);

cairo_private cairo_region_t *
_cairo_region_create_from_boxes (const cairo_box_t *boxes, int count);

cairo_private cairo_box_t *
_cairo_region_get_boxes (const cairo_region_t *region, int *nbox);

CAIRO_END_DECLS

#endif /* CAIRO_REGION_PRIVATE_H */
