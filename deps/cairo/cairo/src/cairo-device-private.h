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

#ifndef _CAIRO_DEVICE_PRIVATE_H_
#define _CAIRO_DEVICE_PRIVATE_H_

#include "cairo-compiler-private.h"
#include "cairo-mutex-private.h"
#include "cairo-reference-count-private.h"
#include "cairo-types-private.h"

struct _cairo_device {
    cairo_reference_count_t ref_count;
    cairo_status_t status;
    cairo_user_data_array_t user_data;

    const cairo_device_backend_t *backend;

    cairo_recursive_mutex_t mutex;
    unsigned mutex_depth;

    cairo_bool_t finished;
};

struct _cairo_device_backend {
    cairo_device_type_t type;

    void (*lock) (void *device);
    void (*unlock) (void *device);

    cairo_warn cairo_status_t (*flush) (void *device);
    void (*finish) (void *device);
    void (*destroy) (void *device);
};

cairo_private cairo_device_t *
_cairo_device_create_in_error (cairo_status_t status);

cairo_private void
_cairo_device_init (cairo_device_t *device,
		    const cairo_device_backend_t *backend);

cairo_private cairo_status_t
_cairo_device_set_error (cairo_device_t *device,
		         cairo_status_t error);

slim_hidden_proto_no_warn (cairo_device_reference);
slim_hidden_proto (cairo_device_acquire);
slim_hidden_proto (cairo_device_release);
slim_hidden_proto (cairo_device_flush);
slim_hidden_proto (cairo_device_finish);
slim_hidden_proto (cairo_device_destroy);

#endif /* _CAIRO_DEVICE_PRIVATE_H_ */
