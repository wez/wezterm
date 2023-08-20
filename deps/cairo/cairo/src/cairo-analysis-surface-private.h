/*
 * Copyright © 2005 Keith Packard
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
 * The Initial Developer of the Original Code is Keith Packard
 *
 * Contributor(s):
 *      Keith Packard <keithp@keithp.com>
 */

#ifndef CAIRO_ANALYSIS_SURFACE_H
#define CAIRO_ANALYSIS_SURFACE_H

#include "cairoint.h"

cairo_private cairo_surface_t *
_cairo_analysis_surface_create (cairo_surface_t		*target,
				cairo_bool_t             create_region_ids);

cairo_private void
_cairo_analysis_surface_set_ctm (cairo_surface_t *surface,
				 const cairo_matrix_t  *ctm);

cairo_private void
_cairo_analysis_surface_get_ctm (cairo_surface_t *surface,
				 cairo_matrix_t  *ctm);

cairo_private cairo_region_t *
_cairo_analysis_surface_get_supported (cairo_surface_t *surface);

cairo_private cairo_region_t *
_cairo_analysis_surface_get_unsupported (cairo_surface_t *surface);

cairo_private cairo_bool_t
_cairo_analysis_surface_has_supported (cairo_surface_t *surface);

cairo_private cairo_bool_t
_cairo_analysis_surface_has_unsupported (cairo_surface_t *surface);

cairo_private void
_cairo_analysis_surface_get_bounding_box (cairo_surface_t *surface,
					  cairo_box_t     *bbox);

cairo_private unsigned int
_cairo_analysis_surface_get_source_region_id (cairo_surface_t *surface);

cairo_private unsigned int
_cairo_analysis_surface_get_mask_region_id (cairo_surface_t *surface);

cairo_private cairo_int_status_t
_cairo_analysis_surface_merge_status (cairo_int_status_t status_a,
				      cairo_int_status_t status_b);

cairo_private cairo_surface_t *
_cairo_null_surface_create (cairo_content_t content);

static inline cairo_bool_t
_cairo_surface_is_analysis (const cairo_surface_t *surface)
{
    return (cairo_internal_surface_type_t)surface->backend->type == CAIRO_INTERNAL_SURFACE_TYPE_ANALYSIS;
}

#endif /* CAIRO_ANALYSIS_SURFACE_H */
