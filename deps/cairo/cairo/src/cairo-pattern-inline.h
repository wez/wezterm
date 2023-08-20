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
 *	Carl D. Worth <cworth@redhat.com>
 */

#ifndef CAIRO_PATTERN_INLINE_H
#define CAIRO_PATTERN_INLINE_H

#include "cairo-pattern-private.h"

#include "cairo-list-inline.h"

CAIRO_BEGIN_DECLS

static inline void
_cairo_pattern_add_observer (cairo_pattern_t *pattern,
			     cairo_pattern_observer_t *observer,
			     void (*func) (cairo_pattern_observer_t *,
					   cairo_pattern_t *,
					   unsigned int))
{
    observer->notify = func;
    cairo_list_add (&observer->link, &pattern->observers);
}

static inline cairo_surface_t *
_cairo_pattern_get_source (const cairo_surface_pattern_t *pattern,
			   cairo_rectangle_int_t *extents)
{
    return _cairo_surface_get_source (pattern->surface, extents);
}

CAIRO_END_DECLS

#endif /* CAIRO_PATTERN_INLINE_H */
