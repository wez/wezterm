/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2005 Red Hat, Inc
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
 * The Initial Developer of the Original Code is Red Hat, Inc.
 *
 * Contributor(s):
 *	Carl Worth <cworth@cworth.org>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

/* This surface supports redirecting all its input to multiple surfaces.
 */

#include "cairoint.h"

#include "cairo-tee.h"

#include "cairo-default-context-private.h"
#include "cairo-error-private.h"
#include "cairo-tee-surface-private.h"
#include "cairo-recording-surface-inline.h"
#include "cairo-surface-wrapper-private.h"
#include "cairo-array-private.h"
#include "cairo-image-surface-inline.h"

typedef struct _cairo_tee_surface {
    cairo_surface_t base;

    cairo_surface_wrapper_t master;
    cairo_array_t slaves;
} cairo_tee_surface_t;

slim_hidden_proto (cairo_tee_surface_create);
slim_hidden_proto (cairo_tee_surface_add);

static cairo_surface_t *
_cairo_tee_surface_create_similar (void			*abstract_surface,
				   cairo_content_t	 content,
				   int			 width,
				   int			 height)
{

    cairo_tee_surface_t *other = abstract_surface;
    cairo_surface_t *similar;
    cairo_surface_t *surface;
    cairo_surface_wrapper_t *slaves;
    int n, num_slaves;

    similar = _cairo_surface_wrapper_create_similar (&other->master,
						     content, width, height);
    surface = cairo_tee_surface_create (similar);
    cairo_surface_destroy (similar);
    if (unlikely (surface->status))
	return surface;

    num_slaves = _cairo_array_num_elements (&other->slaves);
    slaves = _cairo_array_index (&other->slaves, 0);
    for (n = 0; n < num_slaves; n++) {

	similar = _cairo_surface_wrapper_create_similar (&slaves[n],
							 content,
							 width, height);
	cairo_tee_surface_add (surface, similar);
	cairo_surface_destroy (similar);
    }

    if (unlikely (surface->status)) {
	cairo_status_t status = surface->status;
	cairo_surface_destroy (surface);
	surface = _cairo_surface_create_in_error (status);
    }

    return surface;
}

static cairo_status_t
_cairo_tee_surface_finish (void *abstract_surface)
{
    cairo_tee_surface_t *surface = abstract_surface;
    cairo_surface_wrapper_t *slaves;
    int n, num_slaves;

    _cairo_surface_wrapper_fini (&surface->master);

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++)
	_cairo_surface_wrapper_fini (&slaves[n]);

    _cairo_array_fini (&surface->slaves);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_surface_t *
_cairo_tee_surface_source (void	     *abstract_surface,
			   cairo_rectangle_int_t *extents)
{
    cairo_tee_surface_t *surface = abstract_surface;
    return _cairo_surface_get_source (surface->master.target, extents);
}

static cairo_status_t
_cairo_tee_surface_acquire_source_image (void	     *abstract_surface,
					 cairo_image_surface_t **image_out,
					 void		 **image_extra)
{
    cairo_tee_surface_t *surface = abstract_surface;
    cairo_surface_wrapper_t *slaves;
    int num_slaves, n;

    /* we prefer to use a real image surface if available */
    if (_cairo_surface_is_image (surface->master.target)) {
	return _cairo_surface_wrapper_acquire_source_image (&surface->master,
							    image_out, image_extra);
    }

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++) {
	if (_cairo_surface_is_image (slaves[n].target)) {
	    return _cairo_surface_wrapper_acquire_source_image (&slaves[n],
								image_out,
								image_extra);
	}
    }

    return _cairo_surface_wrapper_acquire_source_image (&surface->master,
							image_out, image_extra);
}

static void
_cairo_tee_surface_release_source_image (void	     *abstract_surface,
					 cairo_image_surface_t	*image,
					 void		  *image_extra)
{
    cairo_tee_surface_t *surface = abstract_surface;

    _cairo_surface_wrapper_release_source_image (&surface->master,
						 image, image_extra);
}

static cairo_surface_t *
_cairo_tee_surface_snapshot (void *abstract_surface)
{
    cairo_tee_surface_t *surface = abstract_surface;
    cairo_surface_wrapper_t *slaves;
    int num_slaves, n;

    /* we prefer to use a recording surface for our snapshots */
    if (_cairo_surface_is_recording (surface->master.target))
	return _cairo_surface_wrapper_snapshot (&surface->master);

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++) {
	if (_cairo_surface_is_recording (slaves[n].target))
	    return _cairo_surface_wrapper_snapshot (&slaves[n]);
    }

    return _cairo_surface_wrapper_snapshot (&surface->master);
}

static cairo_bool_t
_cairo_tee_surface_get_extents (void			*abstract_surface,
				cairo_rectangle_int_t	*rectangle)
{
    cairo_tee_surface_t *surface = abstract_surface;

    return _cairo_surface_wrapper_get_extents (&surface->master, rectangle);
}

static void
_cairo_tee_surface_get_font_options (void                  *abstract_surface,
				     cairo_font_options_t  *options)
{
    cairo_tee_surface_t *surface = abstract_surface;

    _cairo_surface_wrapper_get_font_options (&surface->master, options);
}

static cairo_int_status_t
_cairo_tee_surface_paint (void			*abstract_surface,
			  cairo_operator_t	 op,
			  const cairo_pattern_t	*source,
			  const cairo_clip_t	*clip)
{
    cairo_tee_surface_t *surface = abstract_surface;
    cairo_surface_wrapper_t *slaves;
    int n, num_slaves;
    cairo_int_status_t status;

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++) {
	status = _cairo_surface_wrapper_paint (&slaves[n], op, source, clip);
	if (unlikely (status))
	    return status;
    }

    return _cairo_surface_wrapper_paint (&surface->master, op, source, clip);
}

static cairo_int_status_t
_cairo_tee_surface_mask (void			*abstract_surface,
			 cairo_operator_t	 op,
			 const cairo_pattern_t	*source,
			 const cairo_pattern_t	*mask,
			 const cairo_clip_t	*clip)
{
    cairo_tee_surface_t *surface = abstract_surface;
    cairo_surface_wrapper_t *slaves;
    cairo_int_status_t status;
    int n, num_slaves;

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++) {
	status = _cairo_surface_wrapper_mask (&slaves[n],
					      op, source, mask, clip);
	if (unlikely (status))
	    return status;
    }

    return _cairo_surface_wrapper_mask (&surface->master,
					op, source, mask, clip);
}

static cairo_int_status_t
_cairo_tee_surface_stroke (void				*abstract_surface,
			   cairo_operator_t		 op,
			   const cairo_pattern_t	*source,
			   const cairo_path_fixed_t	*path,
			   const cairo_stroke_style_t	*style,
			   const cairo_matrix_t		*ctm,
			   const cairo_matrix_t		*ctm_inverse,
			   double			 tolerance,
			   cairo_antialias_t		 antialias,
			   const cairo_clip_t		*clip)
{
    cairo_tee_surface_t *surface = abstract_surface;
    cairo_surface_wrapper_t *slaves;
    cairo_int_status_t status;
    int n, num_slaves;

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++) {
	status = _cairo_surface_wrapper_stroke (&slaves[n],
						op, source,
						path, style,
						ctm, ctm_inverse,
						tolerance, antialias,
						clip);
	if (unlikely (status))
	    return status;
    }

    return _cairo_surface_wrapper_stroke (&surface->master,
					  op, source,
					  path, style,
					  ctm, ctm_inverse,
					  tolerance, antialias,
					  clip);
}

static cairo_int_status_t
_cairo_tee_surface_fill (void				*abstract_surface,
			 cairo_operator_t		 op,
			 const cairo_pattern_t		*source,
			 const cairo_path_fixed_t	*path,
			 cairo_fill_rule_t		 fill_rule,
			 double				 tolerance,
			 cairo_antialias_t		 antialias,
			 const cairo_clip_t		*clip)
{
    cairo_tee_surface_t *surface = abstract_surface;
    cairo_surface_wrapper_t *slaves;
    cairo_int_status_t status;
    int n, num_slaves;

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++) {
	status = _cairo_surface_wrapper_fill (&slaves[n],
					      op, source,
					      path, fill_rule,
					      tolerance, antialias,
					      clip);
	if (unlikely (status))
	    return status;
    }

    return _cairo_surface_wrapper_fill (&surface->master,
					op, source,
					path, fill_rule,
					tolerance, antialias,
					clip);
}

static cairo_bool_t
_cairo_tee_surface_has_show_text_glyphs (void *abstract_surface)
{
    return TRUE;
}

static cairo_int_status_t
_cairo_tee_surface_show_text_glyphs (void		    *abstract_surface,
				     cairo_operator_t	     op,
				     const cairo_pattern_t  *source,
				     const char		    *utf8,
				     int		     utf8_len,
				     cairo_glyph_t	    *glyphs,
				     int		     num_glyphs,
				     const cairo_text_cluster_t *clusters,
				     int		     num_clusters,
				     cairo_text_cluster_flags_t cluster_flags,
				     cairo_scaled_font_t    *scaled_font,
				     const cairo_clip_t	    *clip)
{
    cairo_tee_surface_t *surface = abstract_surface;
    cairo_surface_wrapper_t *slaves;
    cairo_int_status_t status;
    int n, num_slaves;
    cairo_glyph_t *glyphs_copy;

    /* XXX: This copying is ugly. */
    glyphs_copy = _cairo_malloc_ab (num_glyphs, sizeof (cairo_glyph_t));
    if (unlikely (glyphs_copy == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++) {
	memcpy (glyphs_copy, glyphs, sizeof (cairo_glyph_t) * num_glyphs);
	status = _cairo_surface_wrapper_show_text_glyphs (&slaves[n], op,
							  source,
							  utf8, utf8_len,
							  glyphs_copy, num_glyphs,
							  clusters, num_clusters,
							  cluster_flags,
							  scaled_font,
							  clip);
	if (unlikely (status))
	    goto CLEANUP;
    }

    memcpy (glyphs_copy, glyphs, sizeof (cairo_glyph_t) * num_glyphs);
    status = _cairo_surface_wrapper_show_text_glyphs (&surface->master, op,
						      source,
						      utf8, utf8_len,
						      glyphs_copy, num_glyphs,
						      clusters, num_clusters,
						      cluster_flags,
						      scaled_font,
						      clip);
CLEANUP:
    free (glyphs_copy);
    return status;
}

static const cairo_surface_backend_t cairo_tee_surface_backend = {
    CAIRO_SURFACE_TYPE_TEE,
    _cairo_tee_surface_finish,

    _cairo_default_context_create, /* XXX */

    _cairo_tee_surface_create_similar,
    NULL, /* create similar image */
    NULL, /* map to image */
    NULL, /* unmap image */

    _cairo_tee_surface_source,
    _cairo_tee_surface_acquire_source_image,
    _cairo_tee_surface_release_source_image,
    _cairo_tee_surface_snapshot,
    NULL, /* copy_page */
    NULL, /* show_page */
    _cairo_tee_surface_get_extents,
    _cairo_tee_surface_get_font_options,
    NULL, /* flush */
    NULL, /* mark_dirty_rectangle */

    _cairo_tee_surface_paint,
    _cairo_tee_surface_mask,
    _cairo_tee_surface_stroke,
    _cairo_tee_surface_fill,
    NULL, /* fill_stroke */

    NULL, /* show_glyphs */

    _cairo_tee_surface_has_show_text_glyphs,
    _cairo_tee_surface_show_text_glyphs
};

cairo_surface_t *
cairo_tee_surface_create (cairo_surface_t *master)
{
    cairo_tee_surface_t *surface;

    if (unlikely (master->status))
	return _cairo_surface_create_in_error (master->status);

    surface = _cairo_malloc (sizeof (cairo_tee_surface_t));
    if (unlikely (surface == NULL))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    _cairo_surface_init (&surface->base,
			 &cairo_tee_surface_backend,
			 master->device,
			 master->content,
			 TRUE); /* is_vector */

    _cairo_surface_wrapper_init (&surface->master, master);

    _cairo_array_init (&surface->slaves, sizeof (cairo_surface_wrapper_t));

    return &surface->base;
}
slim_hidden_def (cairo_tee_surface_create);

void
cairo_tee_surface_add (cairo_surface_t *abstract_surface,
		       cairo_surface_t *target)
{
    cairo_tee_surface_t *surface;
    cairo_surface_wrapper_t slave;
    cairo_status_t status;

    if (unlikely (abstract_surface->status))
	return;
    if (unlikely (abstract_surface->finished)) {
	status = _cairo_surface_set_error (abstract_surface,
					   _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return;
    }

    if (abstract_surface->backend != &cairo_tee_surface_backend) {
	status = _cairo_surface_set_error (abstract_surface,
					   _cairo_error (CAIRO_STATUS_SURFACE_TYPE_MISMATCH));
	return;
    }

    if (unlikely (target->status)) {
	status = _cairo_surface_set_error (abstract_surface, target->status);
	return;
    }

    surface = (cairo_tee_surface_t *) abstract_surface;

    _cairo_surface_wrapper_init (&slave, target);
    status = _cairo_array_append (&surface->slaves, &slave);
    if (unlikely (status)) {
	_cairo_surface_wrapper_fini (&slave);
	status = _cairo_surface_set_error (&surface->base, status);
    }
}
slim_hidden_def (cairo_tee_surface_add);

void
cairo_tee_surface_remove (cairo_surface_t *abstract_surface,
			  cairo_surface_t *target)
{
    cairo_tee_surface_t *surface;
    cairo_surface_wrapper_t *slaves;
    int n, num_slaves;

    if (unlikely (abstract_surface->status))
	return;
    if (unlikely (abstract_surface->finished)) {
	_cairo_surface_set_error (abstract_surface,
				  _cairo_error (CAIRO_STATUS_SURFACE_FINISHED));
	return;
    }

    if (abstract_surface->backend != &cairo_tee_surface_backend) {
	_cairo_surface_set_error (abstract_surface,
				  _cairo_error (CAIRO_STATUS_SURFACE_TYPE_MISMATCH));
	return;
    }

    surface = (cairo_tee_surface_t *) abstract_surface;
    if (target == surface->master.target) {
	_cairo_surface_set_error (abstract_surface,
				  _cairo_error (CAIRO_STATUS_INVALID_INDEX));
	return;
    }

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++) {
	if (slaves[n].target == target)
	    break;
    }

    if (n == num_slaves) {
	_cairo_surface_set_error (abstract_surface,
				  _cairo_error (CAIRO_STATUS_INVALID_INDEX));
	return;
    }

    _cairo_surface_wrapper_fini (&slaves[n]);
    for (n++; n < num_slaves; n++)
	slaves[n-1] = slaves[n];
    surface->slaves.num_elements--; /* XXX: cairo_array_remove()? */
}

cairo_surface_t *
cairo_tee_surface_index (cairo_surface_t *abstract_surface,
			 unsigned int index)
{
    cairo_tee_surface_t *surface;

    if (unlikely (abstract_surface->status))
	return _cairo_surface_create_in_error (abstract_surface->status);
    if (unlikely (abstract_surface->finished))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    if (abstract_surface->backend != &cairo_tee_surface_backend)
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_SURFACE_TYPE_MISMATCH));

    surface = (cairo_tee_surface_t *) abstract_surface;
    if (index == 0) {
	return surface->master.target;
    } else {
	cairo_surface_wrapper_t *slave;

	index--;

	if (index >= _cairo_array_num_elements (&surface->slaves))
	    return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_INVALID_INDEX));

	slave = _cairo_array_index (&surface->slaves, index);
	return slave->target;
    }
}

cairo_surface_t *
_cairo_tee_surface_find_match (void *abstract_surface,
			       const cairo_surface_backend_t *backend,
			       cairo_content_t content)
{
    cairo_tee_surface_t *surface = abstract_surface;
    cairo_surface_wrapper_t *slaves;
    int num_slaves, n;

    /* exact match first */
    if (surface->master.target->backend == backend &&
	surface->master.target->content == content)
    {
	return surface->master.target;
    }

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++) {
	if (slaves[n].target->backend == backend &&
	    slaves[n].target->content == content)
	{
	    return slaves[n].target;
	}
    }

    /* matching backend? */
    if (surface->master.target->backend == backend)
	return surface->master.target;

    num_slaves = _cairo_array_num_elements (&surface->slaves);
    slaves = _cairo_array_index (&surface->slaves, 0);
    for (n = 0; n < num_slaves; n++) {
	if (slaves[n].target->backend == backend)
	    return slaves[n].target;
    }

    return NULL;
}
