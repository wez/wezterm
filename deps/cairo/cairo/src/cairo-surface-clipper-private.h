/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2009 Chris Wilson
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
 *	Chris Wilson <chris@chris-wilson.co.u>
 */

#ifndef CAIRO_SURFACE_CLIPPER_PRIVATE_H
#define CAIRO_SURFACE_CLIPPER_PRIVATE_H

#include "cairo-types-private.h"
#include "cairo-clip-private.h"

CAIRO_BEGIN_DECLS

typedef struct _cairo_surface_clipper cairo_surface_clipper_t;

typedef cairo_status_t
(*cairo_surface_clipper_intersect_clip_path_func_t) (cairo_surface_clipper_t *,
						     cairo_path_fixed_t *,
						     cairo_fill_rule_t,
						     double,
						     cairo_antialias_t);
struct _cairo_surface_clipper {
    cairo_clip_t *clip;
    cairo_surface_clipper_intersect_clip_path_func_t intersect_clip_path;
};

cairo_private cairo_status_t
_cairo_surface_clipper_set_clip (cairo_surface_clipper_t *clipper,
				 const cairo_clip_t *clip);

cairo_private void
_cairo_surface_clipper_init (cairo_surface_clipper_t *clipper,
			     cairo_surface_clipper_intersect_clip_path_func_t intersect);

cairo_private void
_cairo_surface_clipper_reset (cairo_surface_clipper_t *clipper);

CAIRO_END_DECLS

#endif /* CAIRO_SURFACE_CLIPPER_PRIVATE_H */
