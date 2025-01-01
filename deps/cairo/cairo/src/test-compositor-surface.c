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
 * The Initial Developer of the Original Code is Intel Corporation
 *
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "test-compositor-surface-private.h"

#include "cairo-compositor-private.h"
#include "cairo-default-context-private.h"
#include "cairo-error-private.h"
#include "cairo-image-surface-private.h"
#include "cairo-surface-backend-private.h"

typedef struct _test_compositor_surface {
    cairo_image_surface_t base;
} test_compositor_surface_t;

static const cairo_surface_backend_t test_compositor_surface_backend;

cairo_surface_t *
test_compositor_surface_create (const cairo_compositor_t *compositor,
				cairo_content_t	content,
				int		width,
				int		height)
{
    test_compositor_surface_t *surface;
    pixman_image_t *pixman_image;
    pixman_format_code_t pixman_format;

    switch (content) {
    case CAIRO_CONTENT_ALPHA:
	pixman_format = PIXMAN_a8;
	break;
    case CAIRO_CONTENT_COLOR:
	pixman_format = PIXMAN_x8r8g8b8;
	break;
    case CAIRO_CONTENT_COLOR_ALPHA:
	pixman_format = PIXMAN_a8r8g8b8;
	break;
    default:
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_INVALID_CONTENT));
    }

    pixman_image = pixman_image_create_bits (pixman_format, width, height,
					     NULL, 0);
    if (unlikely (pixman_image == NULL))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    surface = _cairo_malloc (sizeof (test_compositor_surface_t));
    if (unlikely (surface == NULL)) {
	pixman_image_unref (pixman_image);
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));
    }

    _cairo_surface_init (&surface->base.base,
			 &test_compositor_surface_backend,
			 NULL, /* device */
			 content,
			 FALSE); /* is_vector */
    _cairo_image_surface_init (&surface->base, pixman_image, pixman_format);

    surface->base.compositor = compositor;

    return &surface->base.base;
}

static cairo_surface_t *
test_compositor_surface_create_similar (void		*abstract_surface,
					cairo_content_t	 content,
					int		 width,
					int		 height)
{
    test_compositor_surface_t *surface = abstract_surface;

    return test_compositor_surface_create (surface->base.compositor,
					   content, width, height);
}

static cairo_int_status_t
test_compositor_surface_paint (void			*_surface,
			       cairo_operator_t		 op,
			       const cairo_pattern_t	*source,
			       const cairo_clip_t	*clip)
{
    test_compositor_surface_t *surface = _surface;
    return _cairo_compositor_paint (surface->base.compositor,
				    _surface, op, source,
				    clip);
}

static cairo_int_status_t
test_compositor_surface_mask (void			*_surface,
			      cairo_operator_t		 op,
			      const cairo_pattern_t	*source,
			      const cairo_pattern_t	*mask,
			      const cairo_clip_t	*clip)
{
    test_compositor_surface_t *surface = _surface;
    return _cairo_compositor_mask (surface->base.compositor,
				   _surface, op, source, mask,
				    clip);
}

static cairo_int_status_t
test_compositor_surface_stroke (void				*_surface,
				cairo_operator_t		 op,
				const cairo_pattern_t		*source,
				const cairo_path_fixed_t	*path,
				const cairo_stroke_style_t	*style,
				const cairo_matrix_t		*ctm,
				const cairo_matrix_t		*ctm_inverse,
				double				 tolerance,
				cairo_antialias_t		 antialias,
				const cairo_clip_t		*clip)
{
    test_compositor_surface_t *surface = _surface;
    if (antialias == CAIRO_ANTIALIAS_DEFAULT)
	antialias = CAIRO_ANTIALIAS_BEST;
    return _cairo_compositor_stroke (surface->base.compositor,
				     _surface, op, source,
				     path, style, ctm, ctm_inverse,
				     tolerance, antialias,
				     clip);
}

static cairo_int_status_t
test_compositor_surface_fill (void			*_surface,
			      cairo_operator_t		 op,
			      const cairo_pattern_t	*source,
			      const cairo_path_fixed_t	*path,
			      cairo_fill_rule_t		 fill_rule,
			      double			 tolerance,
			      cairo_antialias_t		 antialias,
			      const cairo_clip_t	*clip)
{
    test_compositor_surface_t *surface = _surface;
    if (antialias == CAIRO_ANTIALIAS_DEFAULT)
	antialias = CAIRO_ANTIALIAS_BEST;
    return _cairo_compositor_fill (surface->base.compositor,
				   _surface, op, source,
				   path, fill_rule, tolerance, antialias,
				   clip);
}

static cairo_int_status_t
test_compositor_surface_glyphs (void			*_surface,
				cairo_operator_t	 op,
				const cairo_pattern_t	*source,
				cairo_glyph_t		*glyphs,
				int			 num_glyphs,
				cairo_scaled_font_t	*scaled_font,
				const cairo_clip_t	*clip)
{
    test_compositor_surface_t *surface = _surface;
    return _cairo_compositor_glyphs (surface->base.compositor,
				     _surface, op, source,
				     glyphs, num_glyphs, scaled_font,
				     clip);
}

static const cairo_surface_backend_t test_compositor_surface_backend = {
    CAIRO_SURFACE_TYPE_IMAGE,
    _cairo_image_surface_finish,
    _cairo_default_context_create,

    test_compositor_surface_create_similar,
    NULL, /* create similar image */
    _cairo_image_surface_map_to_image,
    _cairo_image_surface_unmap_image,

    _cairo_image_surface_source,
    _cairo_image_surface_acquire_source_image,
    _cairo_image_surface_release_source_image,
    _cairo_image_surface_snapshot,

    NULL, /* copy_page */
    NULL, /* show_page */

    _cairo_image_surface_get_extents,
    _cairo_image_surface_get_font_options,

    NULL, /* flush */
    NULL, /* mark_dirty_rectangle */

    test_compositor_surface_paint,
    test_compositor_surface_mask,
    test_compositor_surface_stroke,
    test_compositor_surface_fill,
    NULL, /* fill/stroke */
    test_compositor_surface_glyphs,
};

static const cairo_compositor_t *
get_fallback_compositor (void)
{
    return &_cairo_fallback_compositor;
}

cairo_surface_t *
_cairo_test_fallback_compositor_surface_create (cairo_content_t	content,
						int		width,
						int		height)
{
    return test_compositor_surface_create (get_fallback_compositor(),
					   content, width, height);
}

cairo_surface_t *
_cairo_test_mask_compositor_surface_create (cairo_content_t	content,
						int		width,
						int		height)
{
    return test_compositor_surface_create (_cairo_image_mask_compositor_get(),
					   content, width, height);
}

cairo_surface_t *
_cairo_test_traps_compositor_surface_create (cairo_content_t	content,
					     int		width,
					     int		height)
{
    return test_compositor_surface_create (_cairo_image_traps_compositor_get(),
					   content, width, height);
}

cairo_surface_t *
_cairo_test_spans_compositor_surface_create (cairo_content_t	content,
					     int		width,
					     int		height)
{
    return test_compositor_surface_create (_cairo_image_spans_compositor_get(),
					   content, width, height);
}
