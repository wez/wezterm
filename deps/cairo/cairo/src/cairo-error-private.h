/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
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
 * The Initial Developer of the Original Code is University of Southern
 * California.
 *
 * Contributor(s):
 *	Carl D. Worth <cworth@cworth.org>
 */

#ifndef _CAIRO_ERROR_PRIVATE_H_
#define _CAIRO_ERROR_PRIVATE_H_

#include "cairo.h"
#include "cairo-compiler-private.h"
#include "cairo-types-private.h"

#include <assert.h>

CAIRO_BEGIN_DECLS

/* _cairo_int_status: internal status
 *
 * Sure wish C had a real enum type so that this would be distinct
 * from #cairo_status_t. Oh well, without that, I'll use this bogus 100
 * offset.  We want to keep it fit in int8_t as the compiler may choose
 * that for #cairo_status_t
 */
enum _cairo_int_status {
    CAIRO_INT_STATUS_SUCCESS = 0,

    CAIRO_INT_STATUS_NO_MEMORY,
    CAIRO_INT_STATUS_INVALID_RESTORE,
    CAIRO_INT_STATUS_INVALID_POP_GROUP,
    CAIRO_INT_STATUS_NO_CURRENT_POINT,
    CAIRO_INT_STATUS_INVALID_MATRIX,
    CAIRO_INT_STATUS_INVALID_STATUS,
    CAIRO_INT_STATUS_NULL_POINTER,
    CAIRO_INT_STATUS_INVALID_STRING,
    CAIRO_INT_STATUS_INVALID_PATH_DATA,
    CAIRO_INT_STATUS_READ_ERROR,
    CAIRO_INT_STATUS_WRITE_ERROR,
    CAIRO_INT_STATUS_SURFACE_FINISHED,
    CAIRO_INT_STATUS_SURFACE_TYPE_MISMATCH,
    CAIRO_INT_STATUS_PATTERN_TYPE_MISMATCH,
    CAIRO_INT_STATUS_INVALID_CONTENT,
    CAIRO_INT_STATUS_INVALID_FORMAT,
    CAIRO_INT_STATUS_INVALID_VISUAL,
    CAIRO_INT_STATUS_FILE_NOT_FOUND,
    CAIRO_INT_STATUS_INVALID_DASH,
    CAIRO_INT_STATUS_INVALID_DSC_COMMENT,
    CAIRO_INT_STATUS_INVALID_INDEX,
    CAIRO_INT_STATUS_CLIP_NOT_REPRESENTABLE,
    CAIRO_INT_STATUS_TEMP_FILE_ERROR,
    CAIRO_INT_STATUS_INVALID_STRIDE,
    CAIRO_INT_STATUS_FONT_TYPE_MISMATCH,
    CAIRO_INT_STATUS_USER_FONT_IMMUTABLE,
    CAIRO_INT_STATUS_USER_FONT_ERROR,
    CAIRO_INT_STATUS_NEGATIVE_COUNT,
    CAIRO_INT_STATUS_INVALID_CLUSTERS,
    CAIRO_INT_STATUS_INVALID_SLANT,
    CAIRO_INT_STATUS_INVALID_WEIGHT,
    CAIRO_INT_STATUS_INVALID_SIZE,
    CAIRO_INT_STATUS_USER_FONT_NOT_IMPLEMENTED,
    CAIRO_INT_STATUS_DEVICE_TYPE_MISMATCH,
    CAIRO_INT_STATUS_DEVICE_ERROR,
    CAIRO_INT_STATUS_INVALID_MESH_CONSTRUCTION,
    CAIRO_INT_STATUS_DEVICE_FINISHED,
    CAIRO_INT_STATUS_JBIG2_GLOBAL_MISSING,
    CAIRO_INT_STATUS_PNG_ERROR,
    CAIRO_INT_STATUS_FREETYPE_ERROR,
    CAIRO_INT_STATUS_WIN32_GDI_ERROR,
    CAIRO_INT_STATUS_TAG_ERROR,
    CAIRO_INT_STATUS_DWRITE_ERROR,
    CAIRO_INT_STATUS_SVG_FONT_ERROR,

    CAIRO_INT_STATUS_LAST_STATUS,

    CAIRO_INT_STATUS_UNSUPPORTED = 100,
    CAIRO_INT_STATUS_DEGENERATE,
    CAIRO_INT_STATUS_NOTHING_TO_DO,
    CAIRO_INT_STATUS_FLATTEN_TRANSPARENCY,
    CAIRO_INT_STATUS_IMAGE_FALLBACK,
    CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN,
};

typedef enum _cairo_int_status cairo_int_status_t;

#define _cairo_status_is_error(status) \
    ((status) != CAIRO_STATUS_SUCCESS && (status) < CAIRO_STATUS_LAST_STATUS)

#define _cairo_int_status_is_error(status) \
    ((status) != CAIRO_INT_STATUS_SUCCESS && (status) < CAIRO_INT_STATUS_LAST_STATUS)

cairo_private cairo_status_t
_cairo_error (cairo_status_t status);

/* hide compiler warnings when discarding the return value */
#define _cairo_error_throw(status) do { \
    cairo_status_t status__ = _cairo_error (status); \
    (void) status__; \
} while (0)

CAIRO_END_DECLS

#endif /* _CAIRO_ERROR_PRIVATE_H_ */
