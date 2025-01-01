/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2011 Intel Corporation
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

#ifndef CAIRO_TRISTRIP_PRIVATE_H
#define CAIRO_TRISTRIP_PRIVATE_H

#include "cairo-compiler-private.h"
#include "cairo-error-private.h"
#include "cairo-types-private.h"

CAIRO_BEGIN_DECLS

struct _cairo_tristrip {
    cairo_status_t status;

    /* XXX clipping */

    const cairo_box_t *limits;
    int num_limits;

    int num_points;
    int size_points;
    cairo_point_t *points;
    cairo_point_t  points_embedded[64];
};

cairo_private void
_cairo_tristrip_init (cairo_tristrip_t *strip);

cairo_private void
_cairo_tristrip_limit (cairo_tristrip_t	*strip,
		       const cairo_box_t	*limits,
		       int			 num_limits);

cairo_private void
_cairo_tristrip_init_with_clip (cairo_tristrip_t *strip,
				const cairo_clip_t *clip);

cairo_private void
_cairo_tristrip_translate (cairo_tristrip_t *strip, int x, int y);

cairo_private void
_cairo_tristrip_move_to (cairo_tristrip_t *strip,
			 const cairo_point_t *point);

cairo_private void
_cairo_tristrip_add_point (cairo_tristrip_t *strip,
			   const cairo_point_t *point);

cairo_private void
_cairo_tristrip_extents (const cairo_tristrip_t *strip,
			 cairo_box_t         *extents);

cairo_private void
_cairo_tristrip_fini (cairo_tristrip_t *strip);

#define _cairo_tristrip_status(T) ((T)->status)

CAIRO_END_DECLS

#endif /* CAIRO_TRISTRIP_PRIVATE_H */
