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

#include "test-null-compositor-surface.h"

#include "cairo-compositor-private.h"
#include "cairo-default-context-private.h"
#include "cairo-error-private.h"
#include "cairo-image-surface-private.h"
#include "cairo-surface-backend-private.h"
#include "cairo-spans-compositor-private.h"
#include "cairo-spans-private.h"

typedef struct _test_compositor_surface {
    cairo_image_surface_t base;
} test_compositor_surface_t;

static const cairo_surface_backend_t test_compositor_surface_backend;

static cairo_surface_t *
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
    NULL, /* snapshot */

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

static cairo_int_status_t
acquire (void *abstract_dst)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
release (void *abstract_dst)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
set_clip_region (void *_surface,
		 cairo_region_t *region)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_surface_t *
pattern_to_surface (cairo_surface_t *dst,
		    const cairo_pattern_t *pattern,
		    cairo_bool_t is_mask,
		    const cairo_rectangle_int_t *extents,
		    const cairo_rectangle_int_t *sample,
		    int *src_x, int *src_y)
{
    return cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 0, 0);
}

static cairo_int_status_t
fill_boxes (void		*_dst,
	    cairo_operator_t	 op,
	    const cairo_color_t	*color,
	    cairo_boxes_t	*boxes)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
draw_image_boxes (void *_dst,
		  cairo_image_surface_t *image,
		  cairo_boxes_t *boxes,
		  int dx, int dy)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
composite (void			*_dst,
	   cairo_operator_t	op,
	   cairo_surface_t	*abstract_src,
	   cairo_surface_t	*abstract_mask,
	   int			src_x,
	   int			src_y,
	   int			mask_x,
	   int			mask_y,
	   int			dst_x,
	   int			dst_y,
	   unsigned int		width,
	   unsigned int		height)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
lerp (void			*_dst,
      cairo_surface_t		*abstract_src,
      cairo_surface_t		*abstract_mask,
      int			src_x,
      int			src_y,
      int			mask_x,
      int			mask_y,
      int			dst_x,
      int			dst_y,
      unsigned int		width,
      unsigned int		height)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
composite_boxes (void			*_dst,
		 cairo_operator_t	op,
		 cairo_surface_t	*abstract_src,
		 cairo_surface_t	*abstract_mask,
		 int			src_x,
		 int			src_y,
		 int			mask_x,
		 int			mask_y,
		 int			dst_x,
		 int			dst_y,
		 cairo_boxes_t		*boxes,
		 const cairo_rectangle_int_t  *extents)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
composite_traps (void			*_dst,
		 cairo_operator_t	op,
		 cairo_surface_t	*abstract_src,
		 int			src_x,
		 int			src_y,
		 int			dst_x,
		 int			dst_y,
		 const cairo_rectangle_int_t *extents,
		 cairo_antialias_t	antialias,
		 cairo_traps_t		*traps)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
check_composite_glyphs (const cairo_composite_rectangles_t *extents,
			cairo_scaled_font_t *scaled_font,
			cairo_glyph_t *glyphs,
			int *num_glyphs)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
composite_glyphs (void				*_dst,
		  cairo_operator_t		 op,
		  cairo_surface_t		*_src,
		  int				 src_x,
		  int				 src_y,
		  int				 dst_x,
		  int				 dst_y,
		  cairo_composite_glyphs_info_t *info)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
spans (void *abstract_renderer,
       int y, int height,
       const cairo_half_open_span_t *spans,
       unsigned num_spans)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
finish_spans (void *abstract_renderer)
{
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
span_renderer_init (cairo_abstract_span_renderer_t	*_r,
		    const cairo_composite_rectangles_t *composite,
		    cairo_antialias_t			antialias,
		    cairo_bool_t			needs_clip)
{
    cairo_span_renderer_t *r = (cairo_span_renderer_t *)_r;
    r->render_rows = spans;
    r->finish = finish_spans;
    return CAIRO_STATUS_SUCCESS;
}

static void
span_renderer_fini (cairo_abstract_span_renderer_t *_r,
		    cairo_int_status_t status)
{
}

static const cairo_compositor_t *
no_fallback_compositor_get (void)
{
    return &__cairo_no_compositor;
}

static cairo_int_status_t
check_composite (const cairo_composite_rectangles_t *extents)
{
    return CAIRO_STATUS_SUCCESS;
}

static const cairo_compositor_t *
no_traps_compositor_get (void)
{
    static cairo_atomic_once_t once = CAIRO_ATOMIC_ONCE_INIT;
    static cairo_traps_compositor_t compositor;

    if (_cairo_atomic_init_once_enter(&once)) {
	_cairo_traps_compositor_init (&compositor,
				      no_fallback_compositor_get ());

	compositor.acquire = acquire;
	compositor.release = release;
	compositor.set_clip_region = set_clip_region;
	compositor.pattern_to_surface = pattern_to_surface;
	compositor.draw_image_boxes = draw_image_boxes;
	//compositor.copy_boxes = copy_boxes;
	compositor.fill_boxes = fill_boxes;
	compositor.check_composite = check_composite;
	compositor.composite = composite;
	compositor.lerp = lerp;
	//compositor.check_composite_boxes = check_composite_boxes;
	compositor.composite_boxes = composite_boxes;
	//compositor.check_composite_traps = check_composite_traps;
	compositor.composite_traps = composite_traps;
	compositor.check_composite_glyphs = check_composite_glyphs;
	compositor.composite_glyphs = composite_glyphs;

	_cairo_atomic_init_once_leave(&once);
    }

    return &compositor.base;
}

static const cairo_compositor_t *
no_spans_compositor_get (void)
{
    static cairo_atomic_once_t once = CAIRO_ATOMIC_ONCE_INIT;
    static cairo_spans_compositor_t compositor;

    if (_cairo_atomic_init_once_enter(&once)) {
	_cairo_spans_compositor_init (&compositor,
				      no_traps_compositor_get());

	//compositor.acquire = acquire;
	//compositor.release = release;
	compositor.fill_boxes = fill_boxes;
	//compositor.check_composite_boxes = check_composite_boxes;
	compositor.composite_boxes = composite_boxes;
	//compositor.check_span_renderer = check_span_renderer;
	compositor.renderer_init = span_renderer_init;
	compositor.renderer_fini = span_renderer_fini;

	_cairo_atomic_init_once_leave(&once);
    }

    return &compositor.base;
}

cairo_surface_t *
_cairo_test_no_fallback_compositor_surface_create (cairo_content_t	content,
						   int		width,
						   int		height)
{
    return test_compositor_surface_create (no_fallback_compositor_get(),
					   content, width, height);
}

cairo_surface_t *
_cairo_test_no_traps_compositor_surface_create (cairo_content_t	content,
						int		width,
						int		height)
{
    return test_compositor_surface_create (no_traps_compositor_get(),
					   content, width, height);
}

cairo_surface_t *
_cairo_test_no_spans_compositor_surface_create (cairo_content_t	content,
					     int		width,
					     int		height)
{
    return test_compositor_surface_create (no_spans_compositor_get(),
					   content, width, height);
}
