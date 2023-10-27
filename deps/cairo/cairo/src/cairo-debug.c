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
 *	Carl D. Worth <cworth@cworth.org>
 */

#include "cairoint.h"
#include "cairo-image-surface-private.h"

/**
 * cairo_debug_reset_static_data:
 *
 * Resets all static data within cairo to its original state,
 * (ie. identical to the state at the time of program invocation). For
 * example, all caches within cairo will be flushed empty.
 *
 * This function is intended to be useful when using memory-checking
 * tools such as valgrind. When valgrind's memcheck analyzes a
 * cairo-using program without a call to cairo_debug_reset_static_data(),
 * it will report all data reachable via cairo's static objects as
 * "still reachable". Calling cairo_debug_reset_static_data() just prior
 * to program termination will make it easier to get squeaky clean
 * reports from valgrind.
 *
 * WARNING: It is only safe to call this function when there are no
 * active cairo objects remaining, (ie. the appropriate destroy
 * functions have been called as necessary). If there are active cairo
 * objects, this call is likely to cause a crash, (eg. an assertion
 * failure due to a hash table being destroyed when non-empty).
 *
 * Since: 1.0
 **/
void
cairo_debug_reset_static_data (void)
{
    CAIRO_MUTEX_INITIALIZE ();

    _cairo_scaled_font_map_destroy ();

    _cairo_toy_font_face_reset_static_data ();

#if CAIRO_HAS_FT_FONT
    _cairo_ft_font_reset_static_data ();
#endif

#if CAIRO_HAS_WIN32_FONT
    _cairo_win32_font_reset_static_data ();
#endif

    _cairo_intern_string_reset_static_data ();

    _cairo_scaled_font_reset_static_data ();

    _cairo_pattern_reset_static_data ();

    _cairo_clip_reset_static_data ();

    _cairo_image_reset_static_data ();

    _cairo_image_compositor_reset_static_data ();

    _cairo_default_context_reset_static_data ();

    CAIRO_MUTEX_FINALIZE ();
}

#if HAVE_VALGRIND
void
_cairo_debug_check_image_surface_is_defined (const cairo_surface_t *surface)
{
    const cairo_image_surface_t *image = (cairo_image_surface_t *) surface;
    const uint8_t *bits;
    int row, width;

    if (surface == NULL)
	return;

    if (! RUNNING_ON_VALGRIND)
	return;

    bits = image->data;
    switch (image->format) {
    case CAIRO_FORMAT_A1:
	width = (image->width + 7)/8;
	break;
    case CAIRO_FORMAT_A8:
	width = image->width;
	break;
    case CAIRO_FORMAT_RGB16_565:
	width = image->width*2;
	break;
    case CAIRO_FORMAT_RGB24:
    case CAIRO_FORMAT_RGB30:
    case CAIRO_FORMAT_ARGB32:
	width = image->width*4;
	break;
    case CAIRO_FORMAT_RGB96F:
	width = image->width*12;
	break;
    case CAIRO_FORMAT_RGBA128F:
	width = image->width*16;
	break;
    case CAIRO_FORMAT_INVALID:
    default:
	/* XXX compute width from pixman bpp */
	return;
    }

    for (row = 0; row < image->height; row++) {
	VALGRIND_CHECK_MEM_IS_DEFINED (bits, width);
	/* and then silence any future valgrind warnings */
	VALGRIND_MAKE_MEM_DEFINED (bits, width);
	bits += image->stride;
    }
}
#endif


#if 0
void
_cairo_image_surface_write_to_ppm (cairo_image_surface_t *isurf, const char *fn)
{
    char *fmt;
    if (isurf->format == CAIRO_FORMAT_ARGB32 || isurf->format == CAIRO_FORMAT_RGB24)
        fmt = "P6";
    else if (isurf->format == CAIRO_FORMAT_A8)
        fmt = "P5";
    else
        return;

    FILE *fp = fopen(fn, "wb");
    if (!fp)
        return;

    fprintf (fp, "%s %d %d 255\n", fmt,isurf->width, isurf->height);
    for (int j = 0; j < isurf->height; j++) {
        unsigned char *row = isurf->data + isurf->stride * j;
        for (int i = 0; i < isurf->width; i++) {
            if (isurf->format == CAIRO_FORMAT_ARGB32 || isurf->format == CAIRO_FORMAT_RGB24) {
                unsigned char r = *row++;
                unsigned char g = *row++;
                unsigned char b = *row++;
                *row++;
                putc(r, fp);
                putc(g, fp);
                putc(b, fp);
            } else {
                unsigned char a = *row++;
                putc(a, fp);
            }
        }
    }

    fclose (fp);

    fprintf (stderr, "Wrote %s\n", fn);
}
#endif

static cairo_status_t
_print_move_to (void *closure,
		const cairo_point_t *point)
{
    fprintf (closure,
	     " %f %f m",
	     _cairo_fixed_to_double (point->x),
	     _cairo_fixed_to_double (point->y));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_print_line_to (void *closure,
		const cairo_point_t *point)
{
    fprintf (closure,
	     " %f %f l",
	     _cairo_fixed_to_double (point->x),
	     _cairo_fixed_to_double (point->y));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_print_curve_to (void *closure,
		 const cairo_point_t *p1,
		 const cairo_point_t *p2,
		 const cairo_point_t *p3)
{
    fprintf (closure,
	     " %f %f %f %f %f %f c",
	     _cairo_fixed_to_double (p1->x),
	     _cairo_fixed_to_double (p1->y),
	     _cairo_fixed_to_double (p2->x),
	     _cairo_fixed_to_double (p2->y),
	     _cairo_fixed_to_double (p3->x),
	     _cairo_fixed_to_double (p3->y));

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_print_close (void *closure)
{
    fprintf (closure, " h");

    return CAIRO_STATUS_SUCCESS;
}

void
_cairo_debug_print_path (FILE *stream, const cairo_path_fixed_t *path)
{
    cairo_status_t status;
    cairo_box_t box;

    fprintf (stream,
	     "path: extents=(%f, %f), (%f, %f)\n",
	    _cairo_fixed_to_double (path->extents.p1.x),
	    _cairo_fixed_to_double (path->extents.p1.y),
	    _cairo_fixed_to_double (path->extents.p2.x),
	    _cairo_fixed_to_double (path->extents.p2.y));

    status = _cairo_path_fixed_interpret (path,
					  _print_move_to,
					  _print_line_to,
					  _print_curve_to,
					  _print_close,
					  stream);
    assert (status == CAIRO_STATUS_SUCCESS);

    if (_cairo_path_fixed_is_box (path, &box)) {
	fprintf (stream, "[box (%d, %d), (%d, %d)]",
		 box.p1.x, box.p1.y, box.p2.x, box.p2.y);
    }

    fprintf (stream, "\n");
}

void
_cairo_debug_print_polygon (FILE *stream, cairo_polygon_t *polygon)
{
    int n;

    fprintf (stream,
	     "polygon: extents=(%f, %f), (%f, %f)\n",
	    _cairo_fixed_to_double (polygon->extents.p1.x),
	    _cairo_fixed_to_double (polygon->extents.p1.y),
	    _cairo_fixed_to_double (polygon->extents.p2.x),
	    _cairo_fixed_to_double (polygon->extents.p2.y));
    if (polygon->num_limits) {
	fprintf (stream,
		 "       : limit=(%f, %f), (%f, %f) x %d\n",
		 _cairo_fixed_to_double (polygon->limit.p1.x),
		 _cairo_fixed_to_double (polygon->limit.p1.y),
		 _cairo_fixed_to_double (polygon->limit.p2.x),
		 _cairo_fixed_to_double (polygon->limit.p2.y),
		 polygon->num_limits);
    }

    for (n = 0; n < polygon->num_edges; n++) {
	cairo_edge_t *edge = &polygon->edges[n];

	fprintf (stream,
		 "  [%d] = [(%f, %f), (%f, %f)], top=%f, bottom=%f, dir=%d\n",
		 n,
		 _cairo_fixed_to_double (edge->line.p1.x),
		 _cairo_fixed_to_double (edge->line.p1.y),
		 _cairo_fixed_to_double (edge->line.p2.x),
		 _cairo_fixed_to_double (edge->line.p2.y),
		 _cairo_fixed_to_double (edge->top),
		 _cairo_fixed_to_double (edge->bottom),
		 edge->dir);

    }
}

void
_cairo_debug_print_matrix (FILE *file, const cairo_matrix_t *matrix)
{
    fprintf (file, "[%g %g %g %g %g %g]\n",
	     matrix->xx, matrix->yx,
	     matrix->xy, matrix->yy,
	     matrix->x0, matrix->y0);
}

void
_cairo_debug_print_rect (FILE *file, const cairo_rectangle_int_t *rect)
{
    fprintf (file, "x: %d y: %d width: %d height: %d\n",
	     rect->x, rect->y,
	     rect->width, rect->height);
}

const char *
_cairo_debug_operator_to_string (cairo_operator_t op)
{
    switch (op) {
        case CAIRO_OPERATOR_CLEAR: return "CLEAR";
        case CAIRO_OPERATOR_SOURCE: return "SOURCE";
        case CAIRO_OPERATOR_OVER: return "OVER";
        case CAIRO_OPERATOR_IN: return "IN";
        case CAIRO_OPERATOR_OUT: return "OUT";
        case CAIRO_OPERATOR_ATOP: return "ATOP";
        case CAIRO_OPERATOR_DEST: return "DEST";
        case CAIRO_OPERATOR_DEST_OVER: return "DEST_OVER";
        case CAIRO_OPERATOR_DEST_IN: return "DEST_IN";
        case CAIRO_OPERATOR_DEST_OUT: return "DEST_OUT";
        case CAIRO_OPERATOR_DEST_ATOP: return "DEST_ATOP";
        case CAIRO_OPERATOR_XOR: return "XOR";
        case CAIRO_OPERATOR_ADD: return "ADD";
        case CAIRO_OPERATOR_SATURATE: return "SATURATE";
        case CAIRO_OPERATOR_MULTIPLY: return "MULTIPLY";
        case CAIRO_OPERATOR_SCREEN: return "SCREEN";
        case CAIRO_OPERATOR_OVERLAY: return "OVERLAY";
        case CAIRO_OPERATOR_DARKEN: return "DARKEN";
        case CAIRO_OPERATOR_LIGHTEN: return "LIGHTEN";
        case CAIRO_OPERATOR_COLOR_DODGE: return "COLOR_DODGE";
        case CAIRO_OPERATOR_COLOR_BURN: return "COLOR_BURN";
        case CAIRO_OPERATOR_HARD_LIGHT: return "HARD_LIGHT";
        case CAIRO_OPERATOR_SOFT_LIGHT: return "SOFT_LIGHT";
        case CAIRO_OPERATOR_DIFFERENCE: return "DIFFERENCE";
        case CAIRO_OPERATOR_EXCLUSION: return "EXCLUSION";
        case CAIRO_OPERATOR_HSL_HUE: return "HSL_HUE";
        case CAIRO_OPERATOR_HSL_SATURATION: return "HSL_SATURATION";
        case CAIRO_OPERATOR_HSL_COLOR: return "HSL_COLOR";
        case CAIRO_OPERATOR_HSL_LUMINOSITY: return "HSL_LUMINOSITY";
    }
    return "UNKNOWN";
}

const char *
_cairo_debug_status_to_string (cairo_int_status_t status)
{
    switch (status) {
	case CAIRO_INT_STATUS_SUCCESS: return "SUCCESS";
	case CAIRO_INT_STATUS_NO_MEMORY: return "NO_MEMORY";
	case CAIRO_INT_STATUS_INVALID_RESTORE: return "INVALID_RESTORE";
	case CAIRO_INT_STATUS_INVALID_POP_GROUP: return "INVALID_POP_GROUP";
	case CAIRO_INT_STATUS_NO_CURRENT_POINT: return "NO_CURRENT_POINT";
	case CAIRO_INT_STATUS_INVALID_MATRIX: return "INVALID_MATRIX";
	case CAIRO_INT_STATUS_INVALID_STATUS: return "INVALID_STATUS";
	case CAIRO_INT_STATUS_NULL_POINTER: return "NULL_POINTER";
	case CAIRO_INT_STATUS_INVALID_STRING: return "INVALID_STRING";
	case CAIRO_INT_STATUS_INVALID_PATH_DATA: return "INVALID_PATH_DATA";
	case CAIRO_INT_STATUS_READ_ERROR: return "READ_ERROR";
	case CAIRO_INT_STATUS_WRITE_ERROR: return "WRITE_ERROR";
	case CAIRO_INT_STATUS_SURFACE_FINISHED: return "SURFACE_FINISHED";
	case CAIRO_INT_STATUS_SURFACE_TYPE_MISMATCH: return "SURFACE_TYPE_MISMATCH";
	case CAIRO_INT_STATUS_PATTERN_TYPE_MISMATCH: return "PATTERN_TYPE_MISMATCH";
	case CAIRO_INT_STATUS_INVALID_CONTENT: return "INVALID_CONTENT";
	case CAIRO_INT_STATUS_INVALID_FORMAT: return "INVALID_FORMAT";
	case CAIRO_INT_STATUS_INVALID_VISUAL: return "INVALID_VISUAL";
	case CAIRO_INT_STATUS_FILE_NOT_FOUND: return "FILE_NOT_FOUND";
	case CAIRO_INT_STATUS_INVALID_DASH: return "INVALID_DASH";
	case CAIRO_INT_STATUS_INVALID_DSC_COMMENT: return "INVALID_DSC_COMMENT";
	case CAIRO_INT_STATUS_INVALID_INDEX: return "INVALID_INDEX";
	case CAIRO_INT_STATUS_CLIP_NOT_REPRESENTABLE: return "CLIP_NOT_REPRESENTABLE";
	case CAIRO_INT_STATUS_TEMP_FILE_ERROR: return "TEMP_FILE_ERROR";
	case CAIRO_INT_STATUS_INVALID_STRIDE: return "INVALID_STRIDE";
	case CAIRO_INT_STATUS_FONT_TYPE_MISMATCH: return "FONT_TYPE_MISMATCH";
	case CAIRO_INT_STATUS_USER_FONT_IMMUTABLE: return "USER_FONT_IMMUTABLE";
	case CAIRO_INT_STATUS_USER_FONT_ERROR: return "USER_FONT_ERROR";
	case CAIRO_INT_STATUS_NEGATIVE_COUNT: return "NEGATIVE_COUNT";
	case CAIRO_INT_STATUS_INVALID_CLUSTERS: return "INVALID_CLUSTERS";
	case CAIRO_INT_STATUS_INVALID_SLANT: return "INVALID_SLANT";
	case CAIRO_INT_STATUS_INVALID_WEIGHT: return "INVALID_WEIGHT";
	case CAIRO_INT_STATUS_INVALID_SIZE: return "INVALID_SIZE";
	case CAIRO_INT_STATUS_USER_FONT_NOT_IMPLEMENTED: return "USER_FONT_NOT_IMPLEMENTED";
	case CAIRO_INT_STATUS_DEVICE_TYPE_MISMATCH: return "DEVICE_TYPE_MISMATCH";
	case CAIRO_INT_STATUS_DEVICE_ERROR: return "DEVICE_ERROR";
	case CAIRO_INT_STATUS_INVALID_MESH_CONSTRUCTION: return "INVALID_MESH_CONSTRUCTION";
	case CAIRO_INT_STATUS_DEVICE_FINISHED: return "DEVICE_FINISHED";
	case CAIRO_INT_STATUS_JBIG2_GLOBAL_MISSING: return "JBIG2_GLOBAL_MISSING";
	case CAIRO_INT_STATUS_PNG_ERROR: return "PNG_ERROR";
	case CAIRO_INT_STATUS_FREETYPE_ERROR: return "FREETYPE_ERROR";
	case CAIRO_INT_STATUS_WIN32_GDI_ERROR: return "WIN32_GDI_ERROR";
	case CAIRO_INT_STATUS_TAG_ERROR: return "TAG_ERROR";
	case CAIRO_INT_STATUS_DWRITE_ERROR: return "DWRITE_ERROR";
	case CAIRO_INT_STATUS_SVG_FONT_ERROR: return "SVG_FONT_ERROR";

	case CAIRO_INT_STATUS_LAST_STATUS: return "LAST_STATUS";

	case CAIRO_INT_STATUS_UNSUPPORTED: return "UNSUPPORTED";
	case CAIRO_INT_STATUS_DEGENERATE: return "DEGENERATE";
	case CAIRO_INT_STATUS_NOTHING_TO_DO: return "NOTHING_TO_DO";
	case CAIRO_INT_STATUS_FLATTEN_TRANSPARENCY: return "FLATTEN_TRANSPARENCY";
	case CAIRO_INT_STATUS_IMAGE_FALLBACK: return "IMAGE_FALLBACK";
	case CAIRO_INT_STATUS_ANALYZE_RECORDING_SURFACE_PATTERN: return "ANALYZE_RECORDING_SURFACE_PATTERN";
    }
    return "UNKNOWN";
}
