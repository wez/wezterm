/* Cairo - a vector graphics library with display and print output
 *
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
 * The Initial Developer of the Original Code is Intel Corporation.
 *
 * Contributors(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#include "cairoint.h"
#include "cairo-device-private.h"
#include "cairo-error-private.h"

/**
 * SECTION:cairo-device
 * @Title: cairo_device_t
 * @Short_Description: interface to underlying rendering system
 * @See_Also: #cairo_surface_t
 *
 * Devices are the abstraction Cairo employs for the rendering system
 * used by a #cairo_surface_t. You can get the device of a surface using
 * cairo_surface_get_device().
 *
 * Devices are created using custom functions specific to the rendering
 * system you want to use. See the documentation for the surface types
 * for those functions.
 *
 * An important function that devices fulfill is sharing access to the
 * rendering system between Cairo and your application. If you want to
 * access a device directly that you used to draw to with Cairo, you must
 * first call cairo_device_flush() to ensure that Cairo finishes all
 * operations on the device and resets it to a clean state.
 *
 * Cairo also provides the functions cairo_device_acquire() and
 * cairo_device_release() to synchronize access to the rendering system
 * in a multithreaded environment. This is done internally, but can also
 * be used by applications.
 *
 * Putting this all together, a function that works with devices should
 * look something like this:
 * <informalexample><programlisting>
 * void
 * my_device_modifying_function (cairo_device_t *device)
 * {
 *   cairo_status_t status;
 *
 *   // Ensure the device is properly reset
 *   cairo_device_flush (device);
 *   // Try to acquire the device
 *   status = cairo_device_acquire (device);
 *   if (status != CAIRO_STATUS_SUCCESS) {
 *     printf ("Failed to acquire the device: %s\n", cairo_status_to_string (status));
 *     return;
 *   }
 *
 *   // Do the custom operations on the device here.
 *   // But do not call any Cairo functions that might acquire devices.
 *   
 *   // Release the device when done.
 *   cairo_device_release (device);
 * }
 * </programlisting></informalexample>
 *
 * <note><para>Please refer to the documentation of each backend for
 * additional usage requirements, guarantees provided, and
 * interactions with existing surface API of the device functions for
 * surfaces of that type.
 * </para></note>
 **/

static const cairo_device_t _nil_device = {
    CAIRO_REFERENCE_COUNT_INVALID,
    CAIRO_STATUS_NO_MEMORY,
};

static const cairo_device_t _mismatch_device = {
    CAIRO_REFERENCE_COUNT_INVALID,
    CAIRO_STATUS_DEVICE_TYPE_MISMATCH,
};

static const cairo_device_t _invalid_device = {
    CAIRO_REFERENCE_COUNT_INVALID,
    CAIRO_STATUS_DEVICE_ERROR,
};

cairo_device_t *
_cairo_device_create_in_error (cairo_status_t status)
{
    switch (status) {
    case CAIRO_STATUS_NO_MEMORY:
	return (cairo_device_t *) &_nil_device;
    case CAIRO_STATUS_DEVICE_ERROR:
	return (cairo_device_t *) &_invalid_device;
    case CAIRO_STATUS_DEVICE_TYPE_MISMATCH:
	return (cairo_device_t *) &_mismatch_device;

    case CAIRO_STATUS_SUCCESS:
    case CAIRO_STATUS_LAST_STATUS:
	ASSERT_NOT_REACHED;
	/* fall-through */
    case CAIRO_STATUS_SURFACE_TYPE_MISMATCH:
    case CAIRO_STATUS_INVALID_STATUS:
    case CAIRO_STATUS_INVALID_FORMAT:
    case CAIRO_STATUS_INVALID_VISUAL:
    case CAIRO_STATUS_READ_ERROR:
    case CAIRO_STATUS_WRITE_ERROR:
    case CAIRO_STATUS_FILE_NOT_FOUND:
    case CAIRO_STATUS_TEMP_FILE_ERROR:
    case CAIRO_STATUS_INVALID_STRIDE:
    case CAIRO_STATUS_INVALID_SIZE:
    case CAIRO_STATUS_INVALID_RESTORE:
    case CAIRO_STATUS_INVALID_POP_GROUP:
    case CAIRO_STATUS_NO_CURRENT_POINT:
    case CAIRO_STATUS_INVALID_MATRIX:
    case CAIRO_STATUS_NULL_POINTER:
    case CAIRO_STATUS_INVALID_STRING:
    case CAIRO_STATUS_INVALID_PATH_DATA:
    case CAIRO_STATUS_SURFACE_FINISHED:
    case CAIRO_STATUS_PATTERN_TYPE_MISMATCH:
    case CAIRO_STATUS_INVALID_DASH:
    case CAIRO_STATUS_INVALID_DSC_COMMENT:
    case CAIRO_STATUS_INVALID_INDEX:
    case CAIRO_STATUS_CLIP_NOT_REPRESENTABLE:
    case CAIRO_STATUS_FONT_TYPE_MISMATCH:
    case CAIRO_STATUS_USER_FONT_IMMUTABLE:
    case CAIRO_STATUS_USER_FONT_ERROR:
    case CAIRO_STATUS_NEGATIVE_COUNT:
    case CAIRO_STATUS_INVALID_CLUSTERS:
    case CAIRO_STATUS_INVALID_SLANT:
    case CAIRO_STATUS_INVALID_WEIGHT:
    case CAIRO_STATUS_USER_FONT_NOT_IMPLEMENTED:
    case CAIRO_STATUS_INVALID_CONTENT:
    case CAIRO_STATUS_INVALID_MESH_CONSTRUCTION:
    case CAIRO_STATUS_DEVICE_FINISHED:
    case CAIRO_STATUS_JBIG2_GLOBAL_MISSING:
    case CAIRO_STATUS_PNG_ERROR:
    case CAIRO_STATUS_FREETYPE_ERROR:
    case CAIRO_STATUS_WIN32_GDI_ERROR:
    case CAIRO_STATUS_TAG_ERROR:
    case CAIRO_STATUS_DWRITE_ERROR:
    case CAIRO_STATUS_SVG_FONT_ERROR:
    default:
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_device_t *) &_nil_device;
    }
}

void
_cairo_device_init (cairo_device_t *device,
		    const cairo_device_backend_t *backend)
{
    CAIRO_REFERENCE_COUNT_INIT (&device->ref_count, 1);
    device->status = CAIRO_STATUS_SUCCESS;

    device->backend = backend;

    CAIRO_RECURSIVE_MUTEX_INIT (device->mutex);
    device->mutex_depth = 0;

    device->finished = FALSE;

    _cairo_user_data_array_init (&device->user_data);
}

/**
 * cairo_device_reference:
 * @device: a #cairo_device_t
 *
 * Increases the reference count on @device by one. This prevents
 * @device from being destroyed until a matching call to
 * cairo_device_destroy() is made.
 *
 * Use cairo_device_get_reference_count() to get the number of references
 * to a #cairo_device_t.
 *
 * Return value: the referenced #cairo_device_t.
 *
 * Since: 1.10
 **/
cairo_device_t *
cairo_device_reference (cairo_device_t *device)
{
    if (device == NULL ||
	CAIRO_REFERENCE_COUNT_IS_INVALID (&device->ref_count))
    {
	return device;
    }

    assert (CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&device->ref_count));
    _cairo_reference_count_inc (&device->ref_count);

    return device;
}
slim_hidden_def (cairo_device_reference);

/**
 * cairo_device_status:
 * @device: a #cairo_device_t
 *
 * Checks whether an error has previously occurred for this
 * device.
 *
 * Return value: %CAIRO_STATUS_SUCCESS on success or an error code if
 *               the device is in an error state.
 *
 * Since: 1.10
 **/
cairo_status_t
cairo_device_status (cairo_device_t *device)
{
    if (device == NULL)
	return CAIRO_STATUS_NULL_POINTER;

    return device->status;
}

/**
 * cairo_device_flush:
 * @device: a #cairo_device_t
 *
 * Finish any pending operations for the device and also restore any
 * temporary modifications cairo has made to the device's state.
 * This function must be called before switching from using the 
 * device with Cairo to operating on it directly with native APIs.
 * If the device doesn't support direct access, then this function
 * does nothing.
 *
 * This function may acquire devices.
 *
 * Since: 1.10
 **/
void
cairo_device_flush (cairo_device_t *device)
{
    cairo_status_t status;

    if (device == NULL || device->status)
	return;

    if (device->finished)
	return;

    if (device->backend->flush != NULL) {
	status = device->backend->flush (device);
	if (unlikely (status))
	    status = _cairo_device_set_error (device, status);
    }
}
slim_hidden_def (cairo_device_flush);

/**
 * cairo_device_finish:
 * @device: the #cairo_device_t to finish
 *
 * This function finishes the device and drops all references to
 * external resources. All surfaces, fonts and other objects created
 * for this @device will be finished, too.
 * Further operations on the @device will not affect the @device but
 * will instead trigger a %CAIRO_STATUS_DEVICE_FINISHED error.
 *
 * When the last call to cairo_device_destroy() decreases the
 * reference count to zero, cairo will call cairo_device_finish() if
 * it hasn't been called already, before freeing the resources
 * associated with the device.
 *
 * This function may acquire devices.
 *
 * Since: 1.10
 **/
void
cairo_device_finish (cairo_device_t *device)
{
    if (device == NULL ||
	CAIRO_REFERENCE_COUNT_IS_INVALID (&device->ref_count))
    {
	return;
    }

    if (device->finished)
	return;

    cairo_device_flush (device);

    if (device->backend->finish != NULL)
	device->backend->finish (device);

    /* We only finish the device after the backend's callback returns because
     * the device might still be needed during the callback
     * (e.g. for cairo_device_acquire ()).
     */
    device->finished = TRUE;
}
slim_hidden_def (cairo_device_finish);

/**
 * cairo_device_destroy:
 * @device: a #cairo_device_t
 *
 * Decreases the reference count on @device by one. If the result is
 * zero, then @device and all associated resources are freed.  See
 * cairo_device_reference().
 *
 * This function may acquire devices if the last reference was dropped.
 *
 * Since: 1.10
 **/
void
cairo_device_destroy (cairo_device_t *device)
{
    cairo_user_data_array_t user_data;

    if (device == NULL ||
	CAIRO_REFERENCE_COUNT_IS_INVALID (&device->ref_count))
    {
	return;
    }

    assert (CAIRO_REFERENCE_COUNT_HAS_REFERENCE (&device->ref_count));
    if (! _cairo_reference_count_dec_and_test (&device->ref_count))
	return;

    cairo_device_finish (device);

    assert (device->mutex_depth == 0);
    CAIRO_MUTEX_FINI (device->mutex);

    user_data = device->user_data;

    device->backend->destroy (device);

    _cairo_user_data_array_fini (&user_data);

}
slim_hidden_def (cairo_device_destroy);

/**
 * cairo_device_get_type:
 * @device: a #cairo_device_t
 *
 * This function returns the type of the device. See #cairo_device_type_t
 * for available types.
 *
 * Return value: The type of @device.
 *
 * Since: 1.10
 **/
cairo_device_type_t
cairo_device_get_type (cairo_device_t *device)
{
    if (device == NULL ||
	CAIRO_REFERENCE_COUNT_IS_INVALID (&device->ref_count))
    {
	return CAIRO_DEVICE_TYPE_INVALID;
    }

    return device->backend->type;
}

/**
 * cairo_device_acquire:
 * @device: a #cairo_device_t
 *
 * Acquires the @device for the current thread. This function will block
 * until no other thread has acquired the device.
 *
 * If the return value is %CAIRO_STATUS_SUCCESS, you successfully acquired the
 * device. From now on your thread owns the device and no other thread will be
 * able to acquire it until a matching call to cairo_device_release(). It is
 * allowed to recursively acquire the device multiple times from the same
 * thread.
 *
 * <note><para>You must never acquire two different devices at the same time
 * unless this is explicitly allowed. Otherwise the possibility of deadlocks
 * exist.
 *
 * As various Cairo functions can acquire devices when called, these functions
 * may also cause deadlocks when you call them with an acquired device. So you
 * must not have a device acquired when calling them. These functions are
 * marked in the documentation.
 * </para></note>
 *
 * Return value: %CAIRO_STATUS_SUCCESS on success or an error code if
 *               the device is in an error state and could not be
 *               acquired. After a successful call to cairo_device_acquire(),
 *               a matching call to cairo_device_release() is required.
 *
 * Since: 1.10
 **/
cairo_status_t
cairo_device_acquire (cairo_device_t *device)
{
    if (device == NULL)
	return CAIRO_STATUS_SUCCESS;

    if (unlikely (device->status))
	return device->status;

    if (unlikely (device->finished))
	return _cairo_device_set_error (device, CAIRO_STATUS_DEVICE_FINISHED);

    CAIRO_MUTEX_LOCK (device->mutex);
    if (device->mutex_depth++ == 0) {
	if (device->backend->lock != NULL)
	    device->backend->lock (device);
    }

    return CAIRO_STATUS_SUCCESS;
}
slim_hidden_def (cairo_device_acquire);

/**
 * cairo_device_release:
 * @device: a #cairo_device_t
 *
 * Releases a @device previously acquired using cairo_device_acquire(). See
 * that function for details.
 *
 * Since: 1.10
 **/
void
cairo_device_release (cairo_device_t *device)
{
    if (device == NULL)
	return;

    assert (device->mutex_depth > 0);

    if (--device->mutex_depth == 0) {
	if (device->backend->unlock != NULL)
	    device->backend->unlock (device);
    }

    CAIRO_MUTEX_UNLOCK (device->mutex);
}
slim_hidden_def (cairo_device_release);

cairo_status_t
_cairo_device_set_error (cairo_device_t *device,
			 cairo_status_t  status)
{
    if (status == CAIRO_STATUS_SUCCESS)
        return CAIRO_STATUS_SUCCESS;

    _cairo_status_set_error (&device->status, status);

    return _cairo_error (status);
}

/**
 * cairo_device_get_reference_count:
 * @device: a #cairo_device_t
 *
 * Returns the current reference count of @device.
 *
 * Return value: the current reference count of @device.  If the
 * object is a nil object, 0 will be returned.
 *
 * Since: 1.10
 **/
unsigned int
cairo_device_get_reference_count (cairo_device_t *device)
{
    if (device == NULL ||
	CAIRO_REFERENCE_COUNT_IS_INVALID (&device->ref_count))
	return 0;

    return CAIRO_REFERENCE_COUNT_GET_VALUE (&device->ref_count);
}

/**
 * cairo_device_get_user_data:
 * @device: a #cairo_device_t
 * @key: the address of the #cairo_user_data_key_t the user data was
 * attached to
 *
 * Return user data previously attached to @device using the
 * specified key.  If no user data has been attached with the given
 * key this function returns %NULL.
 *
 * Return value: the user data previously attached or %NULL.
 *
 * Since: 1.10
 **/
void *
cairo_device_get_user_data (cairo_device_t		 *device,
			    const cairo_user_data_key_t *key)
{
    return _cairo_user_data_array_get_data (&device->user_data,
					    key);
}

/**
 * cairo_device_set_user_data:
 * @device: a #cairo_device_t
 * @key: the address of a #cairo_user_data_key_t to attach the user data to
 * @user_data: the user data to attach to the #cairo_device_t
 * @destroy: a #cairo_destroy_func_t which will be called when the
 * #cairo_t is destroyed or when new user data is attached using the
 * same key.
 *
 * Attach user data to @device.  To remove user data from a surface,
 * call this function with the key that was used to set it and %NULL
 * for @data.
 *
 * Return value: %CAIRO_STATUS_SUCCESS or %CAIRO_STATUS_NO_MEMORY if a
 * slot could not be allocated for the user data.
 *
 * Since: 1.10
 **/
cairo_status_t
cairo_device_set_user_data (cairo_device_t		 *device,
			    const cairo_user_data_key_t *key,
			    void			 *user_data,
			    cairo_destroy_func_t	  destroy)
{
    if (CAIRO_REFERENCE_COUNT_IS_INVALID (&device->ref_count))
	return device->status;

    return _cairo_user_data_array_set_data (&device->user_data,
					    key, user_data, destroy);
}
