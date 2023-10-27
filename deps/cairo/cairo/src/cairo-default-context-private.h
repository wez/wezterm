/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2005 Red Hat, Inc.
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
 *	Carl D. Worth <cworth@redhat.com>
 */

#ifndef CAIRO_DEFAULT_CONTEXT_PRIVATE_H
#define CAIRO_DEFAULT_CONTEXT_PRIVATE_H

#include "cairo-private.h"
#include "cairo-gstate-private.h"
#include "cairo-path-fixed-private.h"

CAIRO_BEGIN_DECLS

typedef struct _cairo_default_context cairo_default_context_t;

struct _cairo_default_context {
    cairo_t base;

    cairo_gstate_t *gstate;
    cairo_gstate_t  gstate_tail[2];
    cairo_gstate_t *gstate_freelist;

    cairo_path_fixed_t path[1];
};

cairo_private cairo_t *
_cairo_default_context_create (void *target);

cairo_private cairo_status_t
_cairo_default_context_init (cairo_default_context_t *cr, void *target);

cairo_private void
_cairo_default_context_fini (cairo_default_context_t *cr);

CAIRO_END_DECLS

#endif /* CAIRO_DEFAULT_CONTEXT_PRIVATE_H */
