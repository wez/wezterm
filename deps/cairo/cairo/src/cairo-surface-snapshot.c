/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2005 Red Hat, Inc.
 * Copyright © 2009 Intel Corporation
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
 *	Carl D. Worth <cworth@cworth.org>
 *      Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"

#include "cairo-error-private.h"
#include "cairo-image-surface-private.h"
#include "cairo-surface-snapshot-inline.h"

static cairo_status_t
_cairo_surface_snapshot_finish (void *abstract_surface)
{
    cairo_surface_snapshot_t *surface = abstract_surface;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (surface->clone != NULL) {
	cairo_surface_finish (surface->clone);
	status = surface->clone->status;

	cairo_surface_destroy (surface->clone);
    }

    CAIRO_MUTEX_FINI (surface->mutex);

    return status;
}

static cairo_status_t
_cairo_surface_snapshot_flush (void *abstract_surface, unsigned flags)
{
    cairo_surface_snapshot_t *surface = abstract_surface;
    cairo_surface_t *target;
    cairo_status_t status;

    target = _cairo_surface_snapshot_get_target (&surface->base);
    status = target->status;
    if (status == CAIRO_STATUS_SUCCESS)
	status = _cairo_surface_flush (target, flags);
    cairo_surface_destroy (target);

    return status;
}

static cairo_surface_t *
_cairo_surface_snapshot_source (void                    *abstract_surface,
				cairo_rectangle_int_t *extents)
{
    cairo_surface_snapshot_t *surface = abstract_surface;
    return _cairo_surface_get_source (surface->target, extents); /* XXX racy */
}

struct snapshot_extra {
    cairo_surface_t *target;
    void *extra;
};

static cairo_status_t
_cairo_surface_snapshot_acquire_source_image (void                    *abstract_surface,
					      cairo_image_surface_t  **image_out,
					      void                   **extra_out)
{
    cairo_surface_snapshot_t *surface = abstract_surface;
    struct snapshot_extra *extra;
    cairo_status_t status;

    extra = _cairo_malloc (sizeof (*extra));
    if (unlikely (extra == NULL)) {
	*extra_out = NULL;
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    extra->target = _cairo_surface_snapshot_get_target (&surface->base);
    status =  _cairo_surface_acquire_source_image (extra->target, image_out, &extra->extra);
    if (unlikely (status)) {
	cairo_surface_destroy (extra->target);
	free (extra);
	extra = NULL;
    }

    *extra_out = extra;
    return status;
}

static void
_cairo_surface_snapshot_release_source_image (void                   *abstract_surface,
					      cairo_image_surface_t  *image,
					      void                   *_extra)
{
    struct snapshot_extra *extra = _extra;

    _cairo_surface_release_source_image (extra->target, image, extra->extra);
    cairo_surface_destroy (extra->target);
    free (extra);
}

static cairo_bool_t
_cairo_surface_snapshot_get_extents (void                  *abstract_surface,
				     cairo_rectangle_int_t *extents)
{
    cairo_surface_snapshot_t *surface = abstract_surface;
    cairo_surface_t *target;
    cairo_bool_t bounded;

    target = _cairo_surface_snapshot_get_target (&surface->base);
    bounded = _cairo_surface_get_extents (target, extents);
    cairo_surface_destroy (target);

    return bounded;
}

static const cairo_surface_backend_t _cairo_surface_snapshot_backend = {
    CAIRO_INTERNAL_SURFACE_TYPE_SNAPSHOT,
    _cairo_surface_snapshot_finish,
    NULL,

    NULL, /* create similar */
    NULL, /* create similar image  */
    NULL, /* map to image */
    NULL, /* unmap image  */

    _cairo_surface_snapshot_source,
    _cairo_surface_snapshot_acquire_source_image,
    _cairo_surface_snapshot_release_source_image,
    NULL, /* snapshot */

    NULL, /* copy_page */
    NULL, /* show_page */

    _cairo_surface_snapshot_get_extents,
    NULL, /* get-font-options */

    _cairo_surface_snapshot_flush,
};

static void
_cairo_surface_snapshot_copy_on_write (cairo_surface_t *surface)
{
    cairo_surface_snapshot_t *snapshot = (cairo_surface_snapshot_t *) surface;
    cairo_image_surface_t *image;
    cairo_surface_t *clone;
    void *extra;
    cairo_status_t status;

    TRACE ((stderr, "%s: target=%d\n",
	    __FUNCTION__, snapshot->target->unique_id));

    /* We need to make an image copy of the original surface since the
     * snapshot may exceed the lifetime of the original device, i.e.
     * when we later need to use the snapshot the data may have already
     * been lost.
     */

    CAIRO_MUTEX_LOCK (snapshot->mutex);

    if (snapshot->target->backend->snapshot != NULL) {
	clone = snapshot->target->backend->snapshot (snapshot->target);
	if (clone != NULL) {
	    assert (clone->status || ! _cairo_surface_is_snapshot (clone));
	    goto done;
	}
    }

    /* XXX copy to a similar surface, leave acquisition till later?
     * We should probably leave such decisions to the backend in case we
     * rely upon devices/connections like Xlib.
    */
    status = _cairo_surface_acquire_source_image (snapshot->target, &image, &extra);
    if (unlikely (status)) {
	snapshot->target = _cairo_surface_create_in_error (status);
	status = _cairo_surface_set_error (surface, status);
	goto unlock;
    }
    clone = image->base.backend->snapshot (&image->base);
    _cairo_surface_release_source_image (snapshot->target, image, extra);

done:
    status = _cairo_surface_set_error (surface, clone->status);
    snapshot->target = snapshot->clone = clone;
    snapshot->base.type = clone->type;
unlock:
    CAIRO_MUTEX_UNLOCK (snapshot->mutex);
}

/**
 * _cairo_surface_snapshot:
 * @surface: a #cairo_surface_t
 *
 * Make an immutable reference to @surface. It is an error to call a
 * surface-modifying function on the result of this function. The
 * resulting 'snapshot' is a lazily copied-on-write surface i.e. it
 * remains a reference to the original surface until that surface is
 * written to again, at which time a copy is made of the original surface
 * and the snapshot then points to that instead. Multiple snapshots of the
 * same unmodified surface point to the same copy.
 *
 * The caller owns the return value and should call
 * cairo_surface_destroy() when finished with it. This function will not
 * return %NULL, but will return a nil surface instead.
 *
 * Return value: The snapshot surface. Note that the return surface
 * may not necessarily be of the same type as @surface.
 **/
cairo_surface_t *
_cairo_surface_snapshot (cairo_surface_t *surface)
{
    cairo_surface_snapshot_t *snapshot;
    cairo_status_t status;

    TRACE ((stderr, "%s: target=%d\n", __FUNCTION__, surface->unique_id));

    if (unlikely (surface->status))
	return _cairo_surface_create_in_error (surface->status);

    if (unlikely (surface->finished))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    if (surface->snapshot_of != NULL)
	return cairo_surface_reference (surface);

    if (_cairo_surface_is_snapshot (surface))
	return cairo_surface_reference (surface);

    snapshot = (cairo_surface_snapshot_t *)
	_cairo_surface_has_snapshot (surface, &_cairo_surface_snapshot_backend);
    if (snapshot != NULL)
	return cairo_surface_reference (&snapshot->base);

    snapshot = _cairo_malloc (sizeof (cairo_surface_snapshot_t));
    if (unlikely (snapshot == NULL))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_SURFACE_FINISHED));

    _cairo_surface_init (&snapshot->base,
			 &_cairo_surface_snapshot_backend,
			 NULL, /* device */
			 surface->content,
			 surface->is_vector);
    snapshot->base.type = surface->type;

    CAIRO_MUTEX_INIT (snapshot->mutex);
    snapshot->target = surface;
    snapshot->clone = NULL;

    status = _cairo_surface_copy_mime_data (&snapshot->base, surface);
    if (unlikely (status)) {
	cairo_surface_destroy (&snapshot->base);
	return _cairo_surface_create_in_error (status);
    }

    snapshot->base.device_transform = surface->device_transform;
    snapshot->base.device_transform_inverse = surface->device_transform_inverse;

    _cairo_surface_attach_snapshot (surface,
				    &snapshot->base,
				    _cairo_surface_snapshot_copy_on_write);

    return &snapshot->base;
}
