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
 *	Carl Worth <cworth@cworth.org>
 */

/* This isn't a "real" surface, but just something to be used by the
 * test suite to help exercise the paginated-surface paths in cairo.
 *
 * The defining feature of this backend is that it uses a paginated
 * surface to record all operations, and then replays everything to an
 * image surface.
 *
 * It's possible that this code might serve as a good starting point
 * for someone working on bringing up a new paginated-surface-based
 * backend.
 */

#include "cairoint.h"

#include "test-paginated-surface.h"

#include "cairo-default-context-private.h"
#include "cairo-error-private.h"
#include "cairo-paginated-private.h"
#include "cairo-surface-backend-private.h"

typedef struct _test_paginated_surface {
    cairo_surface_t base;
    cairo_surface_t *target;
    cairo_paginated_mode_t paginated_mode;
} test_paginated_surface_t;

static const cairo_surface_backend_t test_paginated_surface_backend;
static const cairo_paginated_surface_backend_t test_paginated_surface_paginated_backend;

cairo_surface_t *
_cairo_test_paginated_surface_create (cairo_surface_t *target)
{
    cairo_status_t status;
    cairo_surface_t *paginated;
    test_paginated_surface_t *surface;

    status = cairo_surface_status (target);
    if (unlikely (status))
	return _cairo_surface_create_in_error (status);

    surface = _cairo_malloc (sizeof (test_paginated_surface_t));
    if (unlikely (surface == NULL))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    _cairo_surface_init (&surface->base,
			 &test_paginated_surface_backend,
			 NULL, /* device */
			 target->content,
			 TRUE); /* is_vector */

    surface->target = cairo_surface_reference (target);

    paginated =  _cairo_paginated_surface_create (&surface->base,
						  target->content,
						  &test_paginated_surface_paginated_backend);
    status = paginated->status;
    if (status == CAIRO_STATUS_SUCCESS) {
	/* paginated keeps the only reference to surface now, drop ours */
	cairo_surface_destroy (&surface->base);
	return paginated;
    }

    cairo_surface_destroy (target);
    free (surface);
    return _cairo_surface_create_in_error (status);
}

static cairo_status_t
_test_paginated_surface_finish (void *abstract_surface)
{
    test_paginated_surface_t *surface = abstract_surface;

    cairo_surface_destroy (surface->target);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
_test_paginated_surface_get_extents (void			*abstract_surface,
				     cairo_rectangle_int_t	*rectangle)
{
    test_paginated_surface_t *surface = abstract_surface;

    return _cairo_surface_get_extents (surface->target, rectangle);
}

static cairo_int_status_t
_test_paginated_surface_paint (void		*abstract_surface,
			       cairo_operator_t	 op,
			       const cairo_pattern_t	*source,
			       const cairo_clip_t	*clip)
{
    test_paginated_surface_t *surface = abstract_surface;

    if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE)
	return CAIRO_STATUS_SUCCESS;

    return _cairo_surface_paint (surface->target, op, source, clip);
}

static cairo_int_status_t
_test_paginated_surface_mask (void		*abstract_surface,
			      cairo_operator_t	 op,
			      const cairo_pattern_t	*source,
			      const cairo_pattern_t	*mask,
			      const cairo_clip_t	*clip)
{
    test_paginated_surface_t *surface = abstract_surface;

    if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE)
	return CAIRO_STATUS_SUCCESS;

    return _cairo_surface_mask (surface->target,
				op, source, mask, clip);
}

static cairo_int_status_t
_test_paginated_surface_stroke (void				*abstract_surface,
				cairo_operator_t		 op,
				const cairo_pattern_t		*source,
				const cairo_path_fixed_t		*path,
				const cairo_stroke_style_t		*style,
				const cairo_matrix_t			*ctm,
				const cairo_matrix_t			*ctm_inverse,
				double				 tolerance,
				cairo_antialias_t		 antialias,
				const cairo_clip_t		*clip)
{
    test_paginated_surface_t *surface = abstract_surface;

    if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE)
	return CAIRO_STATUS_SUCCESS;

    return _cairo_surface_stroke (surface->target, op, source,
				  path, style,
				  ctm, ctm_inverse,
				  tolerance, antialias,
				  clip);
}

static cairo_int_status_t
_test_paginated_surface_fill (void				*abstract_surface,
			      cairo_operator_t			 op,
			      const cairo_pattern_t		*source,
			      const cairo_path_fixed_t		*path,
			      cairo_fill_rule_t			 fill_rule,
			      double				 tolerance,
			      cairo_antialias_t			 antialias,
			      const cairo_clip_t		*clip)
{
    test_paginated_surface_t *surface = abstract_surface;

    if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE)
	return CAIRO_STATUS_SUCCESS;

    return _cairo_surface_fill (surface->target, op, source,
				path, fill_rule,
				tolerance, antialias,
				clip);
}

static cairo_bool_t
_test_paginated_surface_has_show_text_glyphs (void *abstract_surface)
{
    test_paginated_surface_t *surface = abstract_surface;

    return cairo_surface_has_show_text_glyphs (surface->target);
}

static cairo_int_status_t
_test_paginated_surface_show_text_glyphs (void			    *abstract_surface,
					  cairo_operator_t	     op,
					  const cairo_pattern_t	    *source,
					  const char		    *utf8,
					  int			     utf8_len,
					  cairo_glyph_t		    *glyphs,
					  int			     num_glyphs,
					  const cairo_text_cluster_t *clusters,
					  int			     num_clusters,
					  cairo_text_cluster_flags_t cluster_flags,
					  cairo_scaled_font_t	    *scaled_font,
					  const cairo_clip_t	    *clip)
{
    test_paginated_surface_t *surface = abstract_surface;

    if (surface->paginated_mode == CAIRO_PAGINATED_MODE_ANALYZE)
	return CAIRO_STATUS_SUCCESS;

    return _cairo_surface_show_text_glyphs (surface->target, op, source,
					    utf8, utf8_len,
					    glyphs, num_glyphs,
					    clusters, num_clusters,
					    cluster_flags,
					    scaled_font,
					    clip);
}


static cairo_int_status_t
_test_paginated_surface_set_paginated_mode (void			*abstract_surface,
					    cairo_paginated_mode_t	 mode)
{
    test_paginated_surface_t *surface = abstract_surface;

    surface->paginated_mode = mode;

    return CAIRO_STATUS_SUCCESS;
}

static const cairo_surface_backend_t test_paginated_surface_backend = {
    CAIRO_INTERNAL_SURFACE_TYPE_TEST_PAGINATED,
    _test_paginated_surface_finish,
    _cairo_default_context_create,

    /* Since we are a paginated user, we get to regard most of the
     * surface backend interface as historical cruft and ignore it. */

    NULL, /* create_similar */
    NULL, /* create similar image */
    NULL, /* map to image */
    NULL, /* unmap image */

    _cairo_surface_default_source,
    NULL, /* acquire_source_image */
    NULL, /* release_source_image */
    NULL, /* snapshot */

    NULL, /* copy_page */
    NULL, /* show_page */

    _test_paginated_surface_get_extents,
    NULL, /* get_font_options */

    NULL, /* flush */
    NULL, /* mark_dirty_rectangle */

    /* Here is the more "modern" section of the surface backend
     * interface which is mostly just drawing functions */

    _test_paginated_surface_paint,
    _test_paginated_surface_mask,
    _test_paginated_surface_stroke,
    _test_paginated_surface_fill,
    NULL, /* fill-stroke */
    NULL, /* replaced by show_text_glyphs */
    _test_paginated_surface_has_show_text_glyphs,
    _test_paginated_surface_show_text_glyphs
};

static const cairo_paginated_surface_backend_t test_paginated_surface_paginated_backend = {
    NULL, /* start_page */
    _test_paginated_surface_set_paginated_mode
};
