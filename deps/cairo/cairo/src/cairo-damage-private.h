/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2012 Intel Corporation
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
 * Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA 02111-1307 USA
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
 * The Initial Developer of the Original Code is Chris Wilson
 *
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_DAMAGE_PRIVATE_H
#define CAIRO_DAMAGE_PRIVATE_H

#include "cairo-types-private.h"

#include <pixman.h>

CAIRO_BEGIN_DECLS

struct _cairo_damage {
    cairo_status_t status;
    cairo_region_t *region;

    int dirty, remain;
    struct _cairo_damage_chunk {
	struct _cairo_damage_chunk *next;
	cairo_box_t *base;
	int count;
	int size;
    } chunks, *tail;
    cairo_box_t boxes[32];
};

cairo_private cairo_damage_t *
_cairo_damage_create (void);

cairo_private cairo_damage_t *
_cairo_damage_create_in_error (cairo_status_t status);

cairo_private cairo_damage_t *
_cairo_damage_add_box (cairo_damage_t *damage,
		       const cairo_box_t *box);

cairo_private cairo_damage_t *
_cairo_damage_add_rectangle (cairo_damage_t *damage,
			     const cairo_rectangle_int_t *rect);

cairo_private cairo_damage_t *
_cairo_damage_add_region (cairo_damage_t *damage,
			  const cairo_region_t *region);

cairo_private cairo_damage_t *
_cairo_damage_reduce (cairo_damage_t *damage);

cairo_private void
_cairo_damage_destroy (cairo_damage_t *damage);

CAIRO_END_DECLS

#endif /* CAIRO_DAMAGE_PRIVATE_H */
