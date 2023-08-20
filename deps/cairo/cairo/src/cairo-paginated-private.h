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

#ifndef CAIRO_PAGINATED_H
#define CAIRO_PAGINATED_H

#include "cairoint.h"

struct _cairo_paginated_surface_backend {
    /* Optional. Will be called once for each page.
     *
     * Note: With respect to the order of drawing operations as seen
     * by the target, this call will occur before any drawing
     * operations for the relevant page. However, with respect to the
     * function calls as made by the user, this call will be *after*
     * any drawing operations for the page, (that is, it will occur
     * during the user's call to cairo_show_page or cairo_copy_page).
     */
    cairo_warn cairo_int_status_t
    (*start_page)		(void			*surface);

    /* Required. Will be called twice for each page, once with an
     * argument of CAIRO_PAGINATED_MODE_ANALYZE and once with
     * CAIRO_PAGINATED_MODE_RENDER. See more details in the
     * documentation for _cairo_paginated_surface_create below.
     */
    cairo_warn cairo_int_status_t
    (*set_paginated_mode)	(void			*surface,
				 cairo_paginated_mode_t	 mode);

    /* Optional. Specifies the smallest box that encloses all objects
     * on the page. Will be called at the end of the ANALYZE phase but
     * before the mode is changed to RENDER.
     */
    cairo_warn cairo_int_status_t
    (*set_bounding_box)	(void		*surface,
			 cairo_box_t	*bbox);

    /* Optional. Indicates whether the page requires fallback images.
     * Will be called at the end of the ANALYZE phase but before the
     * mode is changed to RENDER.
     */
    cairo_warn cairo_int_status_t
    (*set_fallback_images_required) (void	    *surface,
				     cairo_bool_t    fallbacks_required);

    cairo_bool_t
    (*supports_fine_grained_fallbacks) (void	    *surface);

    /* Optional. Indicates whether the page requires a thumbnail image to be
     * supplied. If a thumbnail is required, set width, height to size required
     * and return TRUE.
     */
    cairo_bool_t
    (*requires_thumbnail_image) (void	*surface,
				 int    *width,
				 int    *height);

    /* If thumbbail image requested, this function will be called before
     * _show_page().
     */
    cairo_warn cairo_int_status_t
    (*set_thumbnail_image) (void	          *surface,
			    cairo_image_surface_t *image);
};

/* A #cairo_paginated_surface_t provides a very convenient wrapper that
 * is well-suited for doing the analysis common to most surfaces that
 * have paginated output, (that is, things directed at printers, or
 * for saving content in files such as PostScript or PDF files).
 *
 * To use the paginated surface, you'll first need to create your
 * 'real' surface using _cairo_surface_init() and the standard
 * #cairo_surface_backend_t. Then you also call
 * _cairo_paginated_surface_create which takes its own, much simpler,
 * #cairo_paginated_surface_backend_t. You are free to return the result
 * of _cairo_paginated_surface_create() from your public
 * cairo_<foo>_surface_create(). The paginated backend will be careful
 * to not let the user see that they really got a "wrapped"
 * surface. See test-paginated-surface.c for a fairly minimal example
 * of a paginated-using surface. That should be a reasonable example
 * to follow.
 *
 * What the paginated surface does is first save all drawing
 * operations for a page into a recording-surface. Then when the user calls
 * cairo_show_page(), the paginated surface performs the following
 * sequence of operations (using the backend functions passed to
 * cairo_paginated_surface_create()):
 *
 * 1. Calls start_page() (if not %NULL). At this point, it is appropriate
 *    for the target to emit any page-specific header information into
 *    its output.
 *
 * 2. Calls set_paginated_mode() with an argument of %CAIRO_PAGINATED_MODE_ANALYZE
 *
 * 3. Replays the recording-surface to the target surface, (with an
 *    analysis surface inserted between which watches the return value
 *    from each operation). This analysis stage is used to decide which
 *    operations will require fallbacks.
 *
 * 4. Calls set_bounding_box() to provide the target surface with the
 *    tight bounding box of the page.
 *
 * 5. Calls set_paginated_mode() with an argument of %CAIRO_PAGINATED_MODE_RENDER
 *
 * 6. Replays a subset of the recording-surface operations to the target surface
 *
 * 7. Calls set_paginated_mode() with an argument of %CAIRO_PAGINATED_MODE_FALLBACK
 *
 * 8. Replays the remaining operations to an image surface, sets an
 *    appropriate clip on the target, then paints the resulting image
 *    surface to the target.
 *
 * So, the target will see drawing operations during three separate
 * stages, (ANALYZE, RENDER and FALLBACK). During the ANALYZE phase
 * the target should not actually perform any rendering, (for example,
 * if performing output to a file, no output should be generated
 * during this stage). Instead the drawing functions simply need to
 * return %CAIRO_STATUS_SUCCESS or %CAIRO_INT_STATUS_UNSUPPORTED to
 * indicate whether rendering would be supported. And it should do
 * this as quickly as possible. The FALLBACK phase allows the surface
 * to distinguish fallback images from native rendering in case they
 * need to be handled as a special case.
 *
 * Note: The paginated surface layer assumes that the target surface
 * is "blank" by default at the beginning of each page, without any
 * need for an explicit erase operation, (as opposed to an image
 * surface, for example, which might have uninitialized content
 * originally). As such, it optimizes away CLEAR operations that
 * happen at the beginning of each page---the target surface will not
 * even see these operations.
 */
cairo_private cairo_surface_t *
_cairo_paginated_surface_create (cairo_surface_t				*target,
				 cairo_content_t				 content,
				 const cairo_paginated_surface_backend_t	*backend);

cairo_private cairo_surface_t *
_cairo_paginated_surface_get_target (cairo_surface_t *surface);

cairo_private cairo_surface_t *
_cairo_paginated_surface_get_recording (cairo_surface_t *surface);

cairo_private cairo_bool_t
_cairo_surface_is_paginated (cairo_surface_t *surface);

cairo_private cairo_status_t
_cairo_paginated_surface_set_size (cairo_surface_t 	*surface,
				   double		 width,
				   double		 height);

#endif /* CAIRO_PAGINATED_H */
