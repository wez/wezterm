/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2005 Red Hat, Inc
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
 *	Kristian Høgsberg <krh@redhat.com>
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#ifndef CAIRO_RECORDING_SURFACE_INLINE_H
#define CAIRO_RECORDING_SURFACE_INLINE_H

#include "cairo-recording-surface-private.h"

static inline cairo_bool_t
_cairo_recording_surface_get_bounds (cairo_surface_t *surface,
				     cairo_rectangle_t *extents)
{
    cairo_recording_surface_t *recording = (cairo_recording_surface_t *)surface;
    if (recording->unbounded)
	return FALSE;

    *extents = recording->extents_pixels;
    return TRUE;
}

/**
 * _cairo_surface_is_recording:
 * @surface: a #cairo_surface_t
 *
 * Checks if a surface is a #cairo_recording_surface_t
 *
 * Return value: %TRUE if the surface is a recording surface
 **/
static inline cairo_bool_t
_cairo_surface_is_recording (const cairo_surface_t *surface)
{
    return surface->backend->type == CAIRO_SURFACE_TYPE_RECORDING;
}

#endif /* CAIRO_RECORDING_SURFACE_INLINE_H */
