/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright � 2006, 2007 Mozilla Corporation
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
 * The Initial Developer of the Original Code is Mozilla Foundation.
 *
 * Contributor(s):
 *	Vladimir Vukicevic <vladimir@mozilla.com>
 */

#include "cairoint.h"

#include "cairo-quartz-private.h"

#include "cairo-composite-rectangles-private.h"
#include "cairo-compositor-private.h"
#include "cairo-default-context-private.h"
#include "cairo-error-private.h"
#include "cairo-image-surface-inline.h"
#include "cairo-pattern-private.h"
#include "cairo-surface-backend-private.h"
#include "cairo-surface-clipper-private.h"
#include "cairo-recording-surface-private.h"

#include <dlfcn.h>

#ifndef RTLD_DEFAULT
#define RTLD_DEFAULT ((void *) 0)
#endif

#include <limits.h>

#undef QUARTZ_DEBUG

#ifdef QUARTZ_DEBUG
#define ND(_x)	fprintf _x
#else
#define ND(_x)	do {} while(0)
#endif

#define IS_EMPTY(s) ((s)->extents.width == 0 || (s)->extents.height == 0)
#if MAC_OS_X_VERSION_MIN_REQUIRED < 1080
#define FONT_ORIENTATION_HORIZONTAL kCTFontHorizontalOrientation
#else
#define FONT_ORIENTATION_HORIZONTAL kCTFontOrientationHorizontal
#endif


/**
 * SECTION:cairo-quartz
 * @Title: Quartz Surfaces
 * @Short_Description: Rendering to Quartz surfaces
 * @See_Also: #cairo_surface_t
 *
 * The Quartz surface is used to render cairo graphics targeting the
 * Apple OS X Quartz rendering system.
 **/

/**
 * CAIRO_HAS_QUARTZ_SURFACE:
 *
 * Defined if the Quartz surface backend is available.
 * This macro can be used to conditionally compile backend-specific code.
 *
 * Since: 1.6
 **/

/*
 * macOS Private functions
 */
static bool (*CGContextGetAllowsFontSmoothingPtr) (CGContextRef) = NULL;
static unsigned int (*CGContextGetTypePtr) (CGContextRef) = NULL;
static void
quartz_ensure_symbols()
{
    static cairo_bool_t symbol_lookup_done = FALSE;
    if (!symbol_lookup_done) {
	CGContextGetTypePtr = dlsym (RTLD_DEFAULT, "CGContextGetType");
	CGContextGetAllowsFontSmoothingPtr =
	    dlsym (RTLD_DEFAULT, "CGContextGetAllowsFontSmoothing");
	symbol_lookup_done = TRUE;
    }
}

#ifdef QUARTZ_DEBUG
static void quartz_surface_to_png (cairo_quartz_surface_t *nq, const char *dest);
static void quartz_image_to_png (CGImageRef, const char *dest);
#endif

typedef struct
{
    cairo_surface_t base;
    CGImageRef image;
} cairo_quartz_snapshot_t;

static cairo_surface_t* _cairo_quartz_snapshot_create (cairo_quartz_surface_t *surface);
static cairo_status_t _cairo_quartz_snapshot_finish (void* surface);
static CGImageRef _cairo_quartz_surface_snapshot_get_image (cairo_quartz_surface_t *surface);

static const cairo_surface_backend_t cairo_quartz_snapshot_backend = {
    CAIRO_INTERNAL_SURFACE_TYPE_QUARTZ_SNAPSHOT,
    _cairo_quartz_snapshot_finish,
};

static cairo_quartz_surface_t *
_cairo_quartz_surface_create_internal (CGContextRef cgContext,
				       cairo_content_t content,
				       unsigned int width,
				       unsigned int height);

CGImageRef
CairoQuartzCreateCGImage (cairo_format_t format,
			  unsigned int width,
			  unsigned int height,
			  unsigned int stride,
			  void *data,
			  cairo_bool_t interpolate,
			  CGColorSpaceRef colorSpaceOverride,
			  CGDataProviderReleaseDataCallback releaseCallback,
			  void *releaseInfo)
{
    CGImageRef image = NULL;
    CGDataProviderRef dataProvider = NULL;
    CGColorSpaceRef colorSpace = colorSpaceOverride;
    CGBitmapInfo bitinfo = kCGBitmapByteOrder32Host;
    int bitsPerComponent, bitsPerPixel;

    switch (format) {
	case CAIRO_FORMAT_ARGB32:
	    if (colorSpace == NULL)
		colorSpace = CGColorSpaceCreateDeviceRGB ();
	    bitinfo |= kCGImageAlphaPremultipliedFirst;
	    bitsPerComponent = 8;
	    bitsPerPixel = 32;
	    break;

	case CAIRO_FORMAT_RGB24:
	    if (colorSpace == NULL)
		colorSpace = CGColorSpaceCreateDeviceRGB ();
	    bitinfo |= kCGImageAlphaNoneSkipFirst;
	    bitsPerComponent = 8;
	    bitsPerPixel = 32;
	    break;

	case CAIRO_FORMAT_A8:
	    bitsPerComponent = 8;
	    bitsPerPixel = 8;
	    break;

	case CAIRO_FORMAT_A1:
#ifdef WORDS_BIGENDIAN
	    bitsPerComponent = 1;
	    bitsPerPixel = 1;
	    break;
#endif

	case CAIRO_FORMAT_RGB30:
	case CAIRO_FORMAT_RGB16_565:
        case CAIRO_FORMAT_RGB96F:
        case CAIRO_FORMAT_RGBA128F:
	case CAIRO_FORMAT_INVALID:
	default:
	    return NULL;
    }

    dataProvider = CGDataProviderCreateWithData (releaseInfo,
						 data,
						 height * stride,
						 releaseCallback);

    if (unlikely (!dataProvider)) {
	// manually release
	if (releaseCallback)
	    releaseCallback (releaseInfo, data, height * stride);
	goto FINISH;
    }

    if (format == CAIRO_FORMAT_A8 || format == CAIRO_FORMAT_A1) {
	cairo_quartz_float_t decode[] = {1.0, 0.0};
	image = CGImageMaskCreate (width, height,
				   bitsPerComponent,
				   bitsPerPixel,
				   stride,
				   dataProvider,
				   decode,
				   interpolate);
    } else
	image = CGImageCreate (width, height,
			       bitsPerComponent,
			       bitsPerPixel,
			       stride,
			       colorSpace,
			       bitinfo,
			       dataProvider,
			       NULL,
			       interpolate,
			       kCGRenderingIntentDefault);

FINISH:

    CGDataProviderRelease (dataProvider);

    if (colorSpace != colorSpaceOverride)
	CGColorSpaceRelease (colorSpace);

    return image;
}

static inline cairo_bool_t
_cairo_quartz_is_cgcontext_bitmap_context (CGContextRef cgc)
{
    if (unlikely (cgc == NULL))
	return FALSE;

    quartz_ensure_symbols ();
    if (likely (CGContextGetTypePtr)) {
	/* 4 is the type value of a bitmap context */
	return CGContextGetTypePtr (cgc) == 4;
    }

    /* This will cause a (harmless) warning to be printed if called on a non-bitmap context */
    return CGBitmapContextGetBitsPerPixel (cgc) != 0;
}

/* CoreGraphics limitation with flipped CTM surfaces: height must be less than signed 16-bit max */

#define CG_MAX_HEIGHT   SHRT_MAX
#define CG_MAX_WIDTH    USHRT_MAX

/* is the desired size of the surface within bounds? */
cairo_bool_t
_cairo_quartz_verify_surface_size (int width, int height)
{
    /* hmmm, allow width, height == 0 ? */
    if (width < 0 || height < 0)
	return FALSE;

    if (width > CG_MAX_WIDTH || height > CG_MAX_HEIGHT)
	return FALSE;

    return TRUE;
}

/*
 * Cairo path -> Quartz path conversion helpers
 */

/* cairo path -> execute in context */
static cairo_status_t
_cairo_path_to_quartz_context_move_to (void *closure,
				       const cairo_point_t *point)
{
    //ND ((stderr, "moveto: %f %f\n", _cairo_fixed_to_double (point->x), _cairo_fixed_to_double (point->y)));
    double x = _cairo_fixed_to_double (point->x);
    double y = _cairo_fixed_to_double (point->y);

    CGContextMoveToPoint (closure, x, y);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_path_to_quartz_context_line_to (void *closure,
				       const cairo_point_t *point)
{
    //ND ((stderr, "lineto: %f %f\n",  _cairo_fixed_to_double (point->x), _cairo_fixed_to_double (point->y)));
    double x = _cairo_fixed_to_double (point->x);
    double y = _cairo_fixed_to_double (point->y);

    CGContextAddLineToPoint (closure, x, y);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_path_to_quartz_context_curve_to (void *closure,
					const cairo_point_t *p0,
					const cairo_point_t *p1,
					const cairo_point_t *p2)
{
    //ND ((stderr, "curveto: %f,%f %f,%f %f,%f\n",
    //		   _cairo_fixed_to_double (p0->x), _cairo_fixed_to_double (p0->y),
    //		   _cairo_fixed_to_double (p1->x), _cairo_fixed_to_double (p1->y),
    //		   _cairo_fixed_to_double (p2->x), _cairo_fixed_to_double (p2->y)));
    double x0 = _cairo_fixed_to_double (p0->x);
    double y0 = _cairo_fixed_to_double (p0->y);
    double x1 = _cairo_fixed_to_double (p1->x);
    double y1 = _cairo_fixed_to_double (p1->y);
    double x2 = _cairo_fixed_to_double (p2->x);
    double y2 = _cairo_fixed_to_double (p2->y);

    CGContextAddCurveToPoint (closure, x0, y0, x1, y1, x2, y2);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_path_to_quartz_context_close_path (void *closure)
{
    //ND ((stderr, "closepath\n"));
    CGContextClosePath (closure);
    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_quartz_cairo_path_to_quartz_context (const cairo_path_fixed_t *path,
					    CGContextRef closure)
{
    cairo_status_t status;

    CGContextBeginPath (closure);
    status = _cairo_path_fixed_interpret (path,
					  _cairo_path_to_quartz_context_move_to,
					  _cairo_path_to_quartz_context_line_to,
					  _cairo_path_to_quartz_context_curve_to,
					  _cairo_path_to_quartz_context_close_path,
					  closure);

    assert (status == CAIRO_STATUS_SUCCESS);
}

/*
 * Misc helpers/callbacks
 */


static CGBlendMode
_cairo_quartz_cairo_operator_to_quartz_blend (cairo_operator_t op)
{
    switch (op) {
	case CAIRO_OPERATOR_MULTIPLY:
	    return kCGBlendModeMultiply;
	case CAIRO_OPERATOR_SCREEN:
	    return kCGBlendModeScreen;
	case CAIRO_OPERATOR_OVERLAY:
	    return kCGBlendModeOverlay;
	case CAIRO_OPERATOR_DARKEN:
	    return kCGBlendModeDarken;
	case CAIRO_OPERATOR_LIGHTEN:
	    return kCGBlendModeLighten;
	case CAIRO_OPERATOR_COLOR_DODGE:
	    return kCGBlendModeColorDodge;
	case CAIRO_OPERATOR_COLOR_BURN:
	    return kCGBlendModeColorBurn;
	case CAIRO_OPERATOR_HARD_LIGHT:
	    return kCGBlendModeHardLight;
	case CAIRO_OPERATOR_SOFT_LIGHT:
	    return kCGBlendModeSoftLight;
	case CAIRO_OPERATOR_DIFFERENCE:
	    return kCGBlendModeDifference;
	case CAIRO_OPERATOR_EXCLUSION:
	    return kCGBlendModeExclusion;
	case CAIRO_OPERATOR_HSL_HUE:
	    return kCGBlendModeHue;
	case CAIRO_OPERATOR_HSL_SATURATION:
	    return kCGBlendModeSaturation;
	case CAIRO_OPERATOR_HSL_COLOR:
	    return kCGBlendModeColor;
	case CAIRO_OPERATOR_HSL_LUMINOSITY:
	    return kCGBlendModeLuminosity;

	case CAIRO_OPERATOR_CLEAR:
	    return kCGBlendModeClear;
	case CAIRO_OPERATOR_SOURCE:
	    return kCGBlendModeCopy;
	case CAIRO_OPERATOR_OVER:
	    return kCGBlendModeNormal;
	case CAIRO_OPERATOR_IN:
	    return kCGBlendModeSourceIn;
	case CAIRO_OPERATOR_OUT:
	    return kCGBlendModeSourceOut;
	case CAIRO_OPERATOR_ATOP:
	    return kCGBlendModeSourceAtop;
	case CAIRO_OPERATOR_DEST_OVER:
	    return kCGBlendModeDestinationOver;
	case CAIRO_OPERATOR_DEST_IN:
	    return kCGBlendModeDestinationIn;
	case CAIRO_OPERATOR_DEST_OUT:
	    return kCGBlendModeDestinationOut;
	case CAIRO_OPERATOR_DEST_ATOP:
	    return kCGBlendModeDestinationAtop;
	case CAIRO_OPERATOR_XOR:
	    return kCGBlendModeXOR;
	case CAIRO_OPERATOR_ADD:
	    return kCGBlendModePlusLighter;
	case CAIRO_OPERATOR_DEST:
	case CAIRO_OPERATOR_SATURATE:
        default:
	    ASSERT_NOT_REACHED;
    }
}

static cairo_int_status_t
_cairo_cgcontext_set_cairo_operator (CGContextRef context, cairo_operator_t op)
{
    CGBlendMode blendmode;

    assert (op != CAIRO_OPERATOR_DEST);

    /* Quartz doesn't support SATURATE at all. COLOR_DODGE and
     * COLOR_BURN in Quartz follow the ISO32000 definition, but cairo
     * uses the definition from the Adobe Supplement.  Also fallback
     * on SOFT_LIGHT and HSL_HUE, because their results are
     * significantly different from those provided by pixman.
     */
    if (op == CAIRO_OPERATOR_SATURATE ||
	op == CAIRO_OPERATOR_SOFT_LIGHT ||
	op == CAIRO_OPERATOR_HSL_HUE ||
	op == CAIRO_OPERATOR_COLOR_DODGE ||
	op == CAIRO_OPERATOR_COLOR_BURN)
    {
	return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    blendmode = _cairo_quartz_cairo_operator_to_quartz_blend (op);
    CGContextSetBlendMode (context, blendmode);
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_quartz_surface_set_cairo_operator (cairo_quartz_surface_t *surface, cairo_operator_t op)
{
    ND((stderr, "%p _cairo_quartz_surface_set_cairo_operator %d\n", surface, op));

    /* When the destination has no color components, we can avoid some
     * fallbacks, but we have to workaround operators which behave
     * differently in Quartz. */
    if (surface->base.content == CAIRO_CONTENT_ALPHA) {
	assert (op != CAIRO_OPERATOR_ATOP); /* filtered by surface layer */

	if (op == CAIRO_OPERATOR_SOURCE ||
	    op == CAIRO_OPERATOR_IN ||
	    op == CAIRO_OPERATOR_OUT ||
	    op == CAIRO_OPERATOR_DEST_IN ||
	    op == CAIRO_OPERATOR_DEST_ATOP ||
	    op == CAIRO_OPERATOR_XOR)
	{
	    return CAIRO_INT_STATUS_UNSUPPORTED;
	}

	if (op == CAIRO_OPERATOR_DEST_OVER)
	    op = CAIRO_OPERATOR_OVER;
	else if (op == CAIRO_OPERATOR_SATURATE)
	    op = CAIRO_OPERATOR_ADD;
	else if (op == CAIRO_OPERATOR_COLOR_DODGE)
	    op = CAIRO_OPERATOR_OVER;
	else if (op == CAIRO_OPERATOR_COLOR_BURN)
	    op = CAIRO_OPERATOR_OVER;
    }

    return _cairo_cgcontext_set_cairo_operator (surface->cgContext, op);
}

static inline CGLineCap
_cairo_quartz_cairo_line_cap_to_quartz (cairo_line_cap_t ccap)
{
    switch (ccap) {
    default:
	ASSERT_NOT_REACHED;

    case CAIRO_LINE_CAP_BUTT:
	return kCGLineCapButt;

    case CAIRO_LINE_CAP_ROUND:
	return kCGLineCapRound;

    case CAIRO_LINE_CAP_SQUARE:
	return kCGLineCapSquare;
    }
}

static inline CGLineJoin
_cairo_quartz_cairo_line_join_to_quartz (cairo_line_join_t cjoin)
{
    switch (cjoin) {
    default:
	ASSERT_NOT_REACHED;

    case CAIRO_LINE_JOIN_MITER:
	return kCGLineJoinMiter;

    case CAIRO_LINE_JOIN_ROUND:
	return kCGLineJoinRound;

    case CAIRO_LINE_JOIN_BEVEL:
	return kCGLineJoinBevel;
    }
}

static inline CGInterpolationQuality
_cairo_quartz_filter_to_quartz (cairo_filter_t filter)
{
    /* The CGInterpolationQuality enumeration values seem to have the
     * following meaning:
     *  - kCGInterpolationNone: nearest neighbor
     *  - kCGInterpolationLow: bilinear
     *  - kCGInterpolationHigh: bicubic? Lanczos?
     */

    switch (filter) {
    case CAIRO_FILTER_NEAREST:
    case CAIRO_FILTER_FAST:
	return kCGInterpolationNone;

    case CAIRO_FILTER_BEST:
	return kCGInterpolationHigh;

    case CAIRO_FILTER_GOOD:
    case CAIRO_FILTER_BILINEAR:
	return kCGInterpolationLow;

    case CAIRO_FILTER_GAUSSIAN:
	return kCGInterpolationDefault;

    default:
	ASSERT_NOT_REACHED;
	return kCGInterpolationDefault;
    }
}

static inline void
_cairo_quartz_cairo_matrix_to_quartz (const cairo_matrix_t *src,
				      CGAffineTransform *dst)
{
    dst->a = src->xx;
    dst->b = src->yx;
    dst->c = src->xy;
    dst->d = src->yy;
    dst->tx = src->x0;
    dst->ty = src->y0;
}


/*
 * Source -> Quartz setup and finish functions
 */

static void
ComputeGradientValue (void *info,
                      const cairo_quartz_float_t *in,
                      cairo_quartz_float_t *out)
{
    double fdist = *in;
    const cairo_gradient_pattern_t *grad = (cairo_gradient_pattern_t*) info;
    unsigned int i;

    /* Put fdist back in the 0.0..1.0 range if we're doing
     * REPEAT/REFLECT
     */
    if (grad->base.extend == CAIRO_EXTEND_REPEAT) {
	fdist = fdist - floor (fdist);
    } else if (grad->base.extend == CAIRO_EXTEND_REFLECT) {
	fdist = fmod (fabs (fdist), 2.0);
	if (fdist > 1.0)
	    fdist = 2.0 - fdist;
    }

    for (i = 0; i < grad->n_stops; i++)
	if (grad->stops[i].offset > fdist)
	    break;

    if (i == 0 || i == grad->n_stops) {
	if (i == grad->n_stops)
	    --i;
	out[0] = grad->stops[i].color.red;
	out[1] = grad->stops[i].color.green;
	out[2] = grad->stops[i].color.blue;
	out[3] = grad->stops[i].color.alpha;
    } else {
	cairo_quartz_float_t ax = grad->stops[i-1].offset;
	cairo_quartz_float_t bx = grad->stops[i].offset - ax;
	cairo_quartz_float_t bp = (fdist - ax)/bx;
	cairo_quartz_float_t ap = 1.0 - bp;

	out[0] =
	    grad->stops[i-1].color.red * ap +
	    grad->stops[i].color.red * bp;
	out[1] =
	    grad->stops[i-1].color.green * ap +
	    grad->stops[i].color.green * bp;
	out[2] =
	    grad->stops[i-1].color.blue * ap +
	    grad->stops[i].color.blue * bp;
	out[3] =
	    grad->stops[i-1].color.alpha * ap +
	    grad->stops[i].color.alpha * bp;
    }
}

static const cairo_quartz_float_t gradient_output_value_ranges[8] = {
    0.f, 1.f, 0.f, 1.f, 0.f, 1.f, 0.f, 1.f
};
static const CGFunctionCallbacks gradient_callbacks = {
    0, ComputeGradientValue, (CGFunctionReleaseInfoCallback) cairo_pattern_destroy
};

/* Quartz computes a small number of samples of the gradient color
 * function. On MacOS X 10.5 it apparently computes only 1024
 * samples. */
#define MAX_GRADIENT_RANGE 1024

static CGFunctionRef
CairoQuartzCreateGradientFunction (const cairo_gradient_pattern_t *gradient,
				   const cairo_rectangle_int_t    *extents,
				   cairo_circle_double_t          *start,
				   cairo_circle_double_t          *end)
{
    cairo_pattern_t *pat;
    cairo_quartz_float_t input_value_range[2];

    if (gradient->base.extend != CAIRO_EXTEND_NONE) {
	double bounds_x1, bounds_x2, bounds_y1, bounds_y2;
	double t[2], tolerance;

	tolerance = fabs (_cairo_matrix_compute_determinant (&gradient->base.matrix));
	tolerance /= _cairo_matrix_transformed_circle_major_axis (&gradient->base.matrix, 1);

	bounds_x1 = extents->x;
	bounds_y1 = extents->y;
	bounds_x2 = extents->x + extents->width;
	bounds_y2 = extents->y + extents->height;
	_cairo_matrix_transform_bounding_box (&gradient->base.matrix,
					      &bounds_x1, &bounds_y1,
					      &bounds_x2, &bounds_y2,
					      NULL);

	_cairo_gradient_pattern_box_to_parameter (gradient,
						  bounds_x1, bounds_y1,
						  bounds_x2, bounds_y2,
						  tolerance,
						  t);

	if (gradient->base.extend == CAIRO_EXTEND_PAD) {
	    t[0] = MAX (t[0], -0.5);
	    t[1] = MIN (t[1],  1.5);
	} else if (t[1] - t[0] > MAX_GRADIENT_RANGE)
	    return NULL;

	/* set the input range for the function -- the function knows how
	   to map values outside of 0.0 .. 1.0 to the correct color */
	input_value_range[0] = t[0];
	input_value_range[1] = t[1];
    } else {
	input_value_range[0] = 0;
	input_value_range[1] = 1;
    }

    _cairo_gradient_pattern_interpolate (gradient, input_value_range[0], start);
    _cairo_gradient_pattern_interpolate (gradient, input_value_range[1], end);

    if (_cairo_pattern_create_copy (&pat, &gradient->base))
	return NULL;

    return CGFunctionCreate (pat,
			     1,
			     input_value_range,
			     4,
			     gradient_output_value_ranges,
			     &gradient_callbacks);
}

static void
DataProviderReleaseCallback (void *info, const void *data, size_t size)
{
    free (info);
}

static cairo_status_t
_cairo_surface_to_cgimage (cairo_surface_t       *source,
			   cairo_rectangle_int_t *extents,
			   cairo_format_t         format,
			   cairo_matrix_t        *matrix,
			   const cairo_clip_t    *clip,
			   CGImageRef            *image_out)
{
    cairo_status_t status;
    cairo_image_surface_t *image_surface;
    void *image_data, *image_extra;
    cairo_bool_t acquired = FALSE;

    if (source->backend && source->backend->type == CAIRO_SURFACE_TYPE_QUARTZ_IMAGE) {
	cairo_quartz_image_surface_t *surface = (cairo_quartz_image_surface_t *) source;
	*image_out = CGImageRetain (surface->image);
	return CAIRO_STATUS_SUCCESS;
    }

    if (_cairo_surface_is_quartz (source)) {
	cairo_quartz_surface_t *surface = (cairo_quartz_surface_t *) source;
	if (IS_EMPTY (surface)) {
	    *image_out = NULL;
	    return CAIRO_INT_STATUS_NOTHING_TO_DO;
	}

	if (_cairo_quartz_is_cgcontext_bitmap_context (surface->cgContext)) {
	    *image_out = _cairo_quartz_surface_snapshot_get_image (surface);
	    return CAIRO_STATUS_SUCCESS;
	}

	*image_out = NULL;
	return CAIRO_STATUS_SURFACE_TYPE_MISMATCH;
    }

    if (source->type == CAIRO_SURFACE_TYPE_RECORDING) {
	image_surface = (cairo_image_surface_t *)
	    cairo_image_surface_create (format, extents->width, extents->height);
	if (unlikely (image_surface->base.status)) {
	    status = image_surface->base.status;
	    cairo_surface_destroy (&image_surface->base);
	    return status;
	}

	status = _cairo_recording_surface_replay_with_clip (source,
							    matrix,
							    &image_surface->base,
							    NULL,
							    FALSE);
	if (unlikely (status)) {
	    cairo_surface_destroy (&image_surface->base);
	    return status;
	}

	cairo_matrix_init_identity (matrix);
    }
    else {
	status = _cairo_surface_acquire_source_image (source, &image_surface,
						      &image_extra);
	if (unlikely (status))
	    return status;
	acquired = TRUE;
    }

    if (image_surface->width == 0 || image_surface->height == 0) {
	*image_out = NULL;
	if (acquired)
	    _cairo_surface_release_source_image (source, image_surface, image_extra);
	else
	    cairo_surface_destroy (&image_surface->base);

	return status;
    }

    image_data = _cairo_malloc_ab (image_surface->height, image_surface->stride);
    if (unlikely (!image_data))
    {
	if (acquired)
	    _cairo_surface_release_source_image (source, image_surface, image_extra);
	else
	    cairo_surface_destroy (&image_surface->base);

	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    // The last row of data may have less than stride bytes so make sure we
    // only copy the minimum amount required from that row.
    memcpy (image_data, image_surface->data,
	    (image_surface->height - 1) * image_surface->stride +
	    cairo_format_stride_for_width (image_surface->format,
					   image_surface->width));
    *image_out = CairoQuartzCreateCGImage (image_surface->format,
					   image_surface->width,
					   image_surface->height,
					   image_surface->stride,
					   image_data,
					   TRUE,
					   NULL,
					   DataProviderReleaseCallback,
					   image_data);

    /* TODO: differentiate memory error and unsupported surface type */
    if (unlikely (*image_out == NULL))
	status = CAIRO_INT_STATUS_UNSUPPORTED;

    if (acquired)
	_cairo_surface_release_source_image (source, image_surface, image_extra);
    else
	cairo_surface_destroy (&image_surface->base);

    return status;
}

/* Generic #cairo_pattern_t -> CGPattern function */

typedef struct {
    CGImageRef image;
    CGRect imageBounds;
    cairo_bool_t do_reflect;
} SurfacePatternDrawInfo;

static void
SurfacePatternDrawFunc (void *ainfo, CGContextRef context)
{
    SurfacePatternDrawInfo *info = (SurfacePatternDrawInfo*) ainfo;

    CGContextTranslateCTM (context, 0, info->imageBounds.size.height);
    CGContextScaleCTM (context, 1, -1);

    CGContextDrawImage (context, info->imageBounds, info->image);
    if (info->do_reflect) {
	/* draw 3 more copies of the image, flipped.
	 * DrawImage draws the image according to the current Y-direction into the rectangle given
	 * (imageBounds); at the time of the first DrawImage above, the origin is at the bottom left
	 * of the base image position, and the Y axis is extending upwards.
	 */

	/* Make the y axis extend downwards, and draw a flipped image below */
	CGContextScaleCTM (context, 1, -1);
	CGContextDrawImage (context, info->imageBounds, info->image);

	/* Shift over to the right, and flip vertically (translation is 2x,
	 * since we'll be flipping and thus rendering the rectangle "backwards"
	 */
	CGContextTranslateCTM (context, 2 * info->imageBounds.size.width, 0);
	CGContextScaleCTM (context, -1, 1);
	CGContextDrawImage (context, info->imageBounds, info->image);

	/* Then unflip the Y-axis again, and draw the image above the point. */
	CGContextScaleCTM (context, 1, -1);
	CGContextDrawImage (context, info->imageBounds, info->image);
    }
}

static void
SurfacePatternReleaseInfoFunc (void *ainfo)
{
    SurfacePatternDrawInfo *info = (SurfacePatternDrawInfo*) ainfo;

    CGImageRelease (info->image);
    free (info);
}

static cairo_int_status_t
_cairo_quartz_cairo_repeating_surface_pattern_to_quartz (cairo_quartz_surface_t *surface,
							 const cairo_pattern_t *source,
							 const cairo_clip_t *clip,
							 CGPatternRef *cgpat)
{
    cairo_surface_pattern_t *spattern = (cairo_surface_pattern_t *) source;
    cairo_surface_t *pat_surf = spattern->surface;
    cairo_rectangle_int_t extents;
    cairo_format_t format = _cairo_format_from_content (surface->base.content);

    CGImageRef image;
    CGRect pbounds;
    CGAffineTransform ptransform, stransform;
    CGPatternCallbacks cb = { 0,
			      SurfacePatternDrawFunc,
			      SurfacePatternReleaseInfoFunc };
    SurfacePatternDrawInfo *info;
    cairo_quartz_float_t rw, rh;
    cairo_status_t status;
    cairo_bool_t is_bounded = _cairo_surface_get_extents (pat_surf, &extents);
    cairo_matrix_t m = spattern->base.matrix;

    /* SURFACE is the only type we'll handle here */
    assert (source->type == CAIRO_PATTERN_TYPE_SURFACE);

    if (pat_surf->type != CAIRO_SURFACE_TYPE_RECORDING)
	assert (is_bounded);

    status = _cairo_surface_to_cgimage (pat_surf, &extents, format,
					&m, clip, &image);

    if (unlikely (status))
	return status;

    info = _cairo_malloc (sizeof (SurfacePatternDrawInfo));
    if (unlikely (!info))
    {
	CGImageRelease (image);
	return CAIRO_STATUS_NO_MEMORY;
    }

    info->image = image;
    info->imageBounds = CGRectMake (0, 0, extents.width, extents.height);
    info->do_reflect = FALSE;

    pbounds.origin.x = 0;
    pbounds.origin.y = 0;

    switch (spattern->base.extend) {
    case CAIRO_EXTEND_NONE:
    case CAIRO_EXTEND_PAD:
    case CAIRO_EXTEND_REPEAT:
	pbounds.size.width = extents.width;
	pbounds.size.height = extents.height;
	break;
    case CAIRO_EXTEND_REFLECT:
	pbounds.size.width = 2.0 * extents.width;
	pbounds.size.height = 2.0 * extents.height;
	info->do_reflect = TRUE;
	break;
    }
    rw = pbounds.size.width;
    rh = pbounds.size.height;

    cairo_matrix_invert (&m);
    _cairo_quartz_cairo_matrix_to_quartz (&m, &stransform);

    /* The pattern matrix is relative to the bottom left, again; the
     * incoming cairo pattern matrix is relative to the upper left.
     * So we take the pattern matrix and the original context matrix,
     * which gives us the correct base translation/y flip.
     */
    ptransform = CGAffineTransformConcat (stransform, surface->cgContextBaseCTM);

#ifdef QUARTZ_DEBUG
    ND ((stderr, "  pbounds: %f %f %f %f\n", pbounds.origin.x, pbounds.origin.y, pbounds.size.width, pbounds.size.height));
    ND ((stderr, "  pattern xform: t: %f %f xx: %f xy: %f yx: %f yy: %f\n", ptransform.tx, ptransform.ty, ptransform.a, ptransform.b, ptransform.c, ptransform.d));
    CGAffineTransform xform = CGContextGetCTM (surface->cgContext);
    ND ((stderr, "  context xform: t: %f %f xx: %f xy: %f yx: %f yy: %f\n", xform.tx, xform.ty, xform.a, xform.b, xform.c, xform.d));
#endif

    *cgpat = CGPatternCreate (info,
			      pbounds,
			      ptransform,
			      rw, rh,
			      kCGPatternTilingConstantSpacing, /* kCGPatternTilingNoDistortion, */
			      TRUE,
			      &cb);

    return CAIRO_STATUS_SUCCESS;
}

/* State used during a drawing operation. */
typedef struct {
    /* The destination of the mask */
    CGContextRef cgMaskContext;

    /* The destination of the drawing of the source */
    CGContextRef cgDrawContext;

    /* The filter to be used when drawing the source */
    CGInterpolationQuality filter;

    /* Action type */
    cairo_quartz_action_t action;

    /* Destination rect */
    CGRect rect;

    /* Used with DO_SHADING, DO_IMAGE */
    CGAffineTransform transform;

    /* Used with DO_IMAGE */
    CGImageRef image;

    /* Used with DO_SHADING */
    CGShadingRef shading;

    /* Temporary destination for unbounded operations */
    CGLayerRef layer;
    CGRect clipRect;
} cairo_quartz_drawing_state_t;

static cairo_int_status_t
_cairo_quartz_setup_pattern_source (cairo_quartz_drawing_state_t *state,
				    const cairo_pattern_t *source,
				    cairo_quartz_surface_t *surface,
				    const cairo_clip_t *clip,
				    cairo_operator_t op)
{
    const cairo_surface_pattern_t *spat = (const cairo_surface_pattern_t *) source;
    cairo_surface_t *pat_surf = spat->surface;
    cairo_matrix_t m = spat->base.matrix;
    cairo_format_t format = _cairo_format_from_content (surface->base.content);
    cairo_rectangle_int_t extents, pattern_extents;
    CGImageRef img;

    cairo_quartz_float_t patternAlpha = 1.0f;
    CGColorSpaceRef patternSpace;
    CGPatternRef cgpat = NULL;
    cairo_int_status_t status;

    _cairo_surface_get_extents (&surface->base, &extents);

    if (pat_surf->backend->type == CAIRO_SURFACE_TYPE_QUARTZ) {
	cairo_quartz_surface_t *quartz_surf = (cairo_quartz_surface_t *) pat_surf;

	if (quartz_surf->cgLayer && source->extend == CAIRO_EXTEND_NONE) {
	    cairo_matrix_invert (&m);
	    _cairo_quartz_cairo_matrix_to_quartz (&m, &state->transform);
	    state->rect = CGRectMake (0, 0, quartz_surf->extents.width, quartz_surf->extents.height);
	    state->layer = quartz_surf->cgLayer;
	    state->action = DO_LAYER;
	    return CAIRO_STATUS_SUCCESS;
	}
    }

    status = _cairo_surface_to_cgimage (pat_surf, &extents, format,
					&m, clip, &img); // Note that only pat_surf will get used!
    if (unlikely (status))
	return status;

    state->image = img;

   if (state->filter == kCGInterpolationNone && _cairo_matrix_is_translation (&m)) {
	m.x0 = -ceil (m.x0 - 0.5);
	m.y0 = -ceil (m.y0 - 0.5);
    } else {
	cairo_matrix_invert (&m);
    }

    _cairo_quartz_cairo_matrix_to_quartz (&m, &state->transform);

    if (pat_surf->type != CAIRO_SURFACE_TYPE_RECORDING) {
	cairo_bool_t is_bounded = _cairo_surface_get_extents (pat_surf, &pattern_extents);
	assert (is_bounded);
    } else {
	_cairo_surface_get_extents (&surface->base, &pattern_extents);
    }

    if (source->extend == CAIRO_EXTEND_NONE) {
	int x, y;

	if (op == CAIRO_OPERATOR_SOURCE &&
	    (pat_surf->content == CAIRO_CONTENT_ALPHA ||
	     ! _cairo_matrix_is_integer_translation (&m, &x, &y)))
	{
	    state->layer = CGLayerCreateWithContext (surface->cgContext,
						     state->clipRect.size,
						     NULL);
	    state->cgDrawContext = CGLayerGetContext (state->layer);
	    CGContextTranslateCTM (state->cgDrawContext,
				   -state->clipRect.origin.x,
				   -state->clipRect.origin.y);
	}

	CGContextSetRGBFillColor (state->cgDrawContext, 0, 0, 0, 1);

	state->rect = CGRectMake (0, 0, pattern_extents.width, pattern_extents.height);
	state->action = DO_IMAGE;
	return CAIRO_STATUS_SUCCESS;
    }

    if (source->extend == CAIRO_EXTEND_REPEAT)
    {
	CGAffineTransform xform = CGAffineTransformConcat (CGContextGetCTM (state->cgDrawContext),
							   state->transform);
	CGRect srcRect = CGRectMake (0, 0, extents.width, extents.height);
	srcRect = CGRectApplyAffineTransform (srcRect, xform);
	xform = CGAffineTransformInvert (xform);
	srcRect = CGRectApplyAffineTransform (srcRect, xform);
	state->rect = srcRect;
    }

    status = _cairo_quartz_cairo_repeating_surface_pattern_to_quartz (surface, source, clip, &cgpat);
    if (unlikely (status))
	return status;

    patternSpace = CGColorSpaceCreatePattern (NULL);
    /* To pass pthread-same-source. */
    if (source->extend == CAIRO_EXTEND_REPEAT)
	CGContextSetInterpolationQuality(state->cgDrawContext, state->filter);
    CGContextSetFillColorSpace (state->cgDrawContext, patternSpace);
    CGContextSetFillPattern (state->cgDrawContext, cgpat, &patternAlpha);
    CGContextSetStrokeColorSpace (state->cgDrawContext, patternSpace);
    CGContextSetStrokePattern (state->cgDrawContext, cgpat, &patternAlpha);
    CGColorSpaceRelease (patternSpace);

    /* Quartz likes to munge the pattern phase (as yet unexplained
     * why); force it to 0,0 as we've already baked in the correct
     * pattern translation into the pattern matrix
     */
    CGContextSetPatternPhase (state->cgDrawContext, CGSizeMake (0, 0));

    CGPatternRelease (cgpat);

    state->action = DO_DIRECT;
    return CAIRO_STATUS_SUCCESS;
}


/*
Quartz does not support repeating gradients. We handle repeating gradients
by manually extending the gradient and repeating color stops. We need to
minimize the number of repetitions since Quartz seems to sample our color
function across the entire range, even if part of that range is not needed
for the visible area of the gradient, and it samples with some fixed resolution,
so if the gradient range is too large it samples with very low resolution and
the gradient is very coarse. _cairo_quartz_create_gradient_function computes
the number of repetitions needed based on the extents.
*/
static cairo_int_status_t
_cairo_quartz_setup_gradient_source (cairo_quartz_drawing_state_t *state,
				     const cairo_gradient_pattern_t *gradient,
				     const cairo_rectangle_int_t *extents)
{
    cairo_matrix_t mat;
    cairo_circle_double_t start, end;
    CGFunctionRef gradFunc;
    CGColorSpaceRef rgb;
    bool extend = gradient->base.extend != CAIRO_EXTEND_NONE;

    assert (gradient->n_stops > 0);

    mat = gradient->base.matrix;
    cairo_matrix_invert (&mat);
    _cairo_quartz_cairo_matrix_to_quartz (&mat, &state->transform);

    gradFunc = CairoQuartzCreateGradientFunction (gradient, extents,
						  &start, &end);

    if (unlikely (gradFunc == NULL))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    rgb = CGColorSpaceCreateDeviceRGB ();

    if (gradient->base.type == CAIRO_PATTERN_TYPE_LINEAR) {
	state->shading = CGShadingCreateAxial (rgb,
					       CGPointMake (start.center.x,
							    start.center.y),
					       CGPointMake (end.center.x,
							    end.center.y),
					       gradFunc,
					       extend, extend);
    } else {
	state->shading = CGShadingCreateRadial (rgb,
						CGPointMake (start.center.x,
							     start.center.y),
						MAX (start.radius, 0),
						CGPointMake (end.center.x,
							     end.center.y),
						MAX (end.radius, 0),
						gradFunc,
						extend, extend);
    }

    CGColorSpaceRelease (rgb);
    CGFunctionRelease (gradFunc);

    state->action = DO_SHADING;
    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_quartz_setup_state (cairo_quartz_drawing_state_t *state,
			   cairo_composite_rectangles_t *composite)
{
    cairo_quartz_surface_t       *surface = (cairo_quartz_surface_t *) composite->surface;
    cairo_operator_t              op = composite->op;
    const cairo_pattern_t        *source = &composite->source_pattern.base;
    const cairo_clip_t           *clip = composite->clip;
    cairo_bool_t needs_temp;
    cairo_status_t status;

    state->layer = NULL;
    state->image = NULL;
    state->shading = NULL;
    state->cgDrawContext = NULL;
    state->cgMaskContext = NULL;
    state->layer = NULL;

    status = _cairo_surface_clipper_set_clip (&surface->clipper, clip);
    if (unlikely (status))
	return status;

    status = _cairo_quartz_surface_set_cairo_operator (surface, op);
    if (unlikely (status))
	return status;

    /* Save before we change the pattern, colorspace, etc. so that
     * we can restore and make sure that quartz releases our
     * pattern (which may be stack allocated)
     */

    CGContextSaveGState (surface->cgContext);
    state->clipRect = CGContextGetClipBoundingBox (surface->cgContext);
    state->clipRect = CGRectIntegral (state->clipRect);
    state->rect = state->clipRect;

    state->cgMaskContext = surface->cgContext;
    state->cgDrawContext = state->cgMaskContext;

    state->filter = _cairo_quartz_filter_to_quartz (source->filter);

    if (op == CAIRO_OPERATOR_CLEAR) {
	CGContextSetRGBFillColor (state->cgDrawContext, 0, 0, 0, 1);

	state->action = DO_DIRECT;
	return CAIRO_STATUS_SUCCESS;
    }

    /*
     * To implement mask unbounded operations Quartz needs a temporary
     * surface which will be composited entirely (ignoring the mask).
     * To implement source unbounded operations Quartz needs a
     * temporary surface which allows extending the source to a size
     * covering the whole mask, but there are some optimization
     * opportunities:
     *
     * - CLEAR completely ignores the source, thus we can just use a
     *   solid color fill.
     *
     * - SOURCE can be implemented by drawing the source and clearing
     *   outside of the source as long as the two regions have no
     *   intersection. This happens when the source is a pixel-aligned
     *   rectangle. If the source is at least as big as the
     *   intersection between the clip rectangle and the mask
     *   rectangle, no clear operation is needed.
     */
    needs_temp = ! _cairo_operator_bounded_by_mask (op);

    if (needs_temp) {
	state->layer = CGLayerCreateWithContext (surface->cgContext,
						 state->clipRect.size,
						 NULL);
	state->cgDrawContext = CGLayerGetContext (state->layer);
	state->cgMaskContext = state->cgDrawContext;
	CGContextTranslateCTM (state->cgDrawContext,
			       -state->clipRect.origin.x,
			       -state->clipRect.origin.y);
    }

    if (source->type == CAIRO_PATTERN_TYPE_SOLID) {
	cairo_solid_pattern_t *solid = (cairo_solid_pattern_t *) source;

	CGContextSetRGBStrokeColor (state->cgDrawContext,
				    solid->color.red,
				    solid->color.green,
				    solid->color.blue,
				    solid->color.alpha);
	CGContextSetRGBFillColor (state->cgDrawContext,
				  solid->color.red,
				  solid->color.green,
				  solid->color.blue,
				  solid->color.alpha);

	state->action = DO_DIRECT;
	return CAIRO_STATUS_SUCCESS;
    }

    if (source->type == CAIRO_PATTERN_TYPE_LINEAR ||
	source->type == CAIRO_PATTERN_TYPE_RADIAL)
    {
	const cairo_gradient_pattern_t *gpat = (const cairo_gradient_pattern_t *)source;
	cairo_rectangle_int_t extents;

	extents = surface->virtual_extents;
	extents.x -= surface->base.device_transform.x0;
	extents.y -= surface->base.device_transform.y0;
	_cairo_rectangle_union (&extents, &surface->extents);

	return _cairo_quartz_setup_gradient_source (state, gpat, &extents);
    }


    if (source->type == CAIRO_PATTERN_TYPE_SURFACE)
	return _cairo_quartz_setup_pattern_source (state, source, surface, clip, op);

    return CAIRO_INT_STATUS_UNSUPPORTED;
}

static void
_cairo_quartz_teardown_state (cairo_quartz_drawing_state_t *state,
			      cairo_composite_rectangles_t *extents)
{
    cairo_quartz_surface_t *surface = (cairo_quartz_surface_t *) extents->surface;

    if (state->layer) {
	if (state->action != DO_LAYER)
	    CGContextDrawLayerInRect (surface->cgContext,
				      state->clipRect,
				      state->layer);
	CGLayerRelease (state->layer);
    }

    if (state->cgMaskContext)
	CGContextRestoreGState (surface->cgContext);

    if (state->image)
	CGImageRelease (state->image);

    if (state->shading)
	CGShadingRelease (state->shading);
}

static inline void
_cairo_quartz_draw_cgcontext (cairo_quartz_drawing_state_t *state,
			      cairo_operator_t op)
{
    if (! (op == CAIRO_OPERATOR_SOURCE &&
	   state->cgDrawContext == state->cgMaskContext))
	return;

    CGContextBeginPath (state->cgDrawContext);
    CGContextAddRect (state->cgDrawContext, state->rect);

    CGContextTranslateCTM (state->cgDrawContext, 0, state->rect.size.height);
    CGContextScaleCTM (state->cgDrawContext, 1, -1);
    CGContextConcatCTM (state->cgDrawContext,
			CGAffineTransformInvert (state->transform));

    CGContextAddRect (state->cgDrawContext, state->clipRect);

    CGContextSetRGBFillColor (state->cgDrawContext, 0, 0, 0, 0);
    CGContextEOFillPath (state->cgDrawContext);
}


static void
_cairo_quartz_draw_source (cairo_quartz_drawing_state_t *state,
			   cairo_operator_t              op)
{
    CGContextSetShouldAntialias (state->cgDrawContext, state->filter != kCGInterpolationNone);
    CGContextSetInterpolationQuality(state->cgDrawContext, state->filter);

    if (state->action == DO_DIRECT) {
	CGContextFillRect (state->cgDrawContext, state->rect);
	return;
    }

    CGContextConcatCTM (state->cgDrawContext, state->transform);

    if (state->action == DO_SHADING) {
	CGContextDrawShading (state->cgDrawContext, state->shading);
	return;
    }

    CGContextTranslateCTM (state->cgDrawContext, 0, state->rect.size.height);
    CGContextScaleCTM (state->cgDrawContext, 1, -1);

    if (state->action == DO_IMAGE) {
	CGContextDrawImage (state->cgDrawContext, state->rect, state->image);
	_cairo_quartz_draw_cgcontext (state, op);
	return;
    }

    if (state->action == DO_LAYER) {
	CGContextDrawLayerInRect (state->cgDrawContext, state->rect, state->layer);
	_cairo_quartz_draw_cgcontext (state, op);
	return;
    }
    assert (FALSE); // Unreachable
}

static cairo_image_surface_t *
_cairo_quartz_surface_map_to_image (void *abstract_surface,
				    const cairo_rectangle_int_t *extents)
{
    cairo_quartz_surface_t *surface = (cairo_quartz_surface_t *) abstract_surface;
    cairo_surface_t *return_surface = NULL;
    unsigned int stride, bitinfo, bpp, color_comps;
    CGColorSpaceRef colorspace;
    unsigned char *imageData;
    cairo_format_t format;

    if (IS_EMPTY (surface))
	return (cairo_image_surface_t *) cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 0, 0);

    if (! _cairo_quartz_is_cgcontext_bitmap_context (surface->cgContext))
	return _cairo_image_surface_create_in_error (_cairo_error (CAIRO_STATUS_SURFACE_TYPE_MISMATCH));

    bitinfo = CGBitmapContextGetBitmapInfo (surface->cgContext);
    bpp = CGBitmapContextGetBitsPerPixel (surface->cgContext);

    // let's hope they don't add YUV under us
    colorspace = CGBitmapContextGetColorSpace (surface->cgContext);
    color_comps = CGColorSpaceGetNumberOfComponents (colorspace);

    /* XXX TODO: We can handle many more data formats by
     * converting to pixman_format_t */

    if (bpp == 32 && color_comps == 3 &&
	(bitinfo & kCGBitmapAlphaInfoMask) == kCGImageAlphaPremultipliedFirst &&
	(bitinfo & kCGBitmapByteOrderMask) == kCGBitmapByteOrder32Host)
    {
	format = CAIRO_FORMAT_ARGB32;
    }
    else if (bpp == 32 && color_comps == 3 &&
	     (bitinfo & kCGBitmapAlphaInfoMask) == kCGImageAlphaNoneSkipFirst &&
	     (bitinfo & kCGBitmapByteOrderMask) == kCGBitmapByteOrder32Host)
    {
	format = CAIRO_FORMAT_RGB24;
    }
    else if (bpp == 8 && color_comps == 0)
    {
	format = CAIRO_FORMAT_A8;
    }
    else
    {
	return _cairo_image_surface_create_in_error (_cairo_error (CAIRO_STATUS_INVALID_FORMAT));
    }

    imageData = CGBitmapContextGetData (surface->cgContext);
    stride = CGBitmapContextGetBytesPerRow (surface->cgContext);

    imageData += extents->y * stride + extents->x * bpp / 8;
    return_surface = cairo_image_surface_create_for_data (imageData,
							  format,
							  extents->width,
							  extents->height,
							  stride);

    return (cairo_image_surface_t *) return_surface;
}

static cairo_int_status_t
_cairo_quartz_surface_unmap_image (void *abstract_surface,
				   cairo_image_surface_t *image)
{
    cairo_surface_finish (&image->base);
    cairo_surface_destroy (&image->base);

    return CAIRO_STATUS_SUCCESS;
}


/*
 * Cairo surface backend implementations
 */

static cairo_status_t
_cairo_quartz_surface_finish (void *abstract_surface)
{
    cairo_quartz_surface_t *surface = (cairo_quartz_surface_t *) abstract_surface;

    ND ((stderr, "_cairo_quartz_surface_finish[%p] cgc: %p\n", surface, surface->cgContext));

    if (IS_EMPTY (surface))
	return CAIRO_STATUS_SUCCESS;

    /* Restore our saved gstate that we use to reset clipping */
    CGContextRestoreGState (surface->cgContext);
    _cairo_surface_clipper_reset (&surface->clipper);

    CGContextRelease (surface->cgContext);

    surface->cgContext = NULL;

    if (surface->cgLayer)
    {
	CGLayerRelease (surface->cgLayer);
	surface->cgLayer = NULL;
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
_cairo_quartz_surface_acquire_source_image (void *abstract_surface,
					     cairo_image_surface_t **image_out,
					     void **image_extra)
{
    cairo_quartz_surface_t *surface = (cairo_quartz_surface_t *) abstract_surface;

    //ND ((stderr, "%p _cairo_quartz_surface_acquire_source_image\n", surface));

    *image_extra = NULL;

    *image_out = _cairo_quartz_surface_map_to_image (surface, &surface->extents);
    if (unlikely (cairo_surface_status(&(*image_out)->base))) {
	cairo_surface_destroy (&(*image_out)->base);
	*image_out = NULL;
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_quartz_surface_release_source_image (void *abstract_surface,
					    cairo_image_surface_t *image,
					    void *image_extra)
{
    _cairo_quartz_surface_unmap_image (abstract_surface, image);
}

static cairo_surface_t*
_cairo_quartz_surface_create_with_cglayer (cairo_quartz_surface_t *surface,
					    cairo_content_t content,
					    int width, int height)
{
    CGAffineTransform xform;
    CGContextRef context;
    CGLayerRef layer;
    cairo_quartz_surface_t* new_surface;

    if (surface->cgContext == NULL || surface->cgLayer != NULL)
	return NULL;

    if (width <= 0 || height <= 0)
	return NULL;

    xform = CGContextGetUserSpaceToDeviceSpaceTransform (surface->cgContext);
    layer = CGLayerCreateWithContext (surface->cgContext,
					     CGSizeMake (width * xform.a,
							 height * xform.d),
					     NULL);

    context = CGLayerGetContext (layer);
    CGContextTranslateCTM (context, 0.0, height);
    CGContextScaleCTM (context, xform.a, -xform.d);
    new_surface = _cairo_quartz_surface_create_internal (context, content,
							 width, height);
    if (unlikely (new_surface->base.status))
    {
	CGContextRelease (context);
	CGLayerRelease (layer);
	return &new_surface->base;
    }
    new_surface->cgLayer = CGLayerRetain(layer);
    CGContextRetain(context);
    new_surface->virtual_extents = surface->virtual_extents;

    return &new_surface->base;
}

static cairo_surface_t *
_cairo_quartz_surface_create_similar (void *abstract_surface,
				      cairo_content_t content,
				      int width,
				      int height)
{
    cairo_quartz_surface_t *surface, *similar_quartz;
    cairo_surface_t *similar;
    cairo_format_t format;

    // verify width and height of surface
    if (!_cairo_quartz_verify_surface_size (width, height)) {
	return _cairo_surface_create_in_error (_cairo_error
					       (CAIRO_STATUS_INVALID_SIZE));
    }

    surface = (cairo_quartz_surface_t *) abstract_surface;
    if (surface->cgContext && !surface->cgLayer && !(width && height))
	_cairo_quartz_surface_create_with_cglayer (surface, content,
						   width, height);

    if (content == CAIRO_CONTENT_COLOR_ALPHA)
	format = CAIRO_FORMAT_ARGB32;
    else if (content == CAIRO_CONTENT_COLOR)
	format = CAIRO_FORMAT_RGB24;
    else if (content == CAIRO_CONTENT_ALPHA)
	format = CAIRO_FORMAT_A8;
    else
	return NULL;

    similar = cairo_quartz_surface_create (format, width, height);
    if (unlikely (similar->status))
	return similar;

    similar_quartz = (cairo_quartz_surface_t *) similar;
    similar_quartz->virtual_extents = surface->virtual_extents;

    return similar;
}

static cairo_bool_t
_cairo_quartz_surface_get_extents (void *abstract_surface,
				   cairo_rectangle_int_t *extents)
{
    cairo_quartz_surface_t *surface = (cairo_quartz_surface_t *) abstract_surface;

    *extents = surface->extents;
    return TRUE;
}

static cairo_int_status_t
_cairo_quartz_cg_paint (const cairo_compositor_t *compositor,
			cairo_composite_rectangles_t *extents)
{
    cairo_quartz_drawing_state_t state;
    cairo_int_status_t rv;

    ND ((stderr, "%p _cairo_quartz_surface_paint op %d source->type %d\n",
	 extents->surface, extents->op, extents->source_pattern.base.type));

    rv = _cairo_quartz_setup_state (&state, extents);
    if (unlikely (rv))
	goto BAIL;

    _cairo_quartz_draw_source (&state, extents->op);

BAIL:
    _cairo_quartz_teardown_state (&state, extents);

    ND ((stderr, "-- paint\n"));
    return rv;
}

static cairo_int_status_t
_cairo_quartz_cg_mask_with_surface (cairo_composite_rectangles_t *extents,
				    cairo_surface_t              *mask_surf,
				    const cairo_matrix_t         *mask_mat,
				    CGInterpolationQuality        filter)
{
    CGRect rect;
    CGImageRef img;
    cairo_status_t status;
    CGAffineTransform mask_matrix;
    cairo_quartz_drawing_state_t state;
    cairo_format_t format = _cairo_format_from_content (extents->surface->content);
    cairo_rectangle_int_t dest_extents;
    cairo_matrix_t m = *mask_mat;

    _cairo_surface_get_extents (extents->surface, &dest_extents);
    status = _cairo_surface_to_cgimage (mask_surf, &dest_extents, format,
					&m, extents->clip, &img);
    if (unlikely (status))
	return status;

    status = _cairo_quartz_setup_state (&state, extents);
    if (unlikely (status))
	goto BAIL;

    rect = CGRectMake (0.0, 0.0, CGImageGetWidth (img), CGImageGetHeight (img));
    _cairo_quartz_cairo_matrix_to_quartz (&m, &mask_matrix);

    /* ClipToMask is essentially drawing an image, so we need to flip the CTM
     * to get the image to appear oriented the right way */
    CGContextConcatCTM (state.cgMaskContext, CGAffineTransformInvert (mask_matrix));
    CGContextTranslateCTM (state.cgMaskContext, 0.0, rect.size.height);
    CGContextScaleCTM (state.cgMaskContext, 1.0, -1.0);

    state.filter = filter;

    CGContextSetInterpolationQuality (state.cgMaskContext, filter);
    CGContextSetShouldAntialias (state.cgMaskContext, filter != kCGInterpolationNone);

    CGContextClipToMask (state.cgMaskContext, rect, img);

    CGContextScaleCTM (state.cgMaskContext, 1.0, -1.0);
    CGContextTranslateCTM (state.cgMaskContext, 0.0, -rect.size.height);
    CGContextConcatCTM (state.cgMaskContext, mask_matrix);

    _cairo_quartz_draw_source (&state, extents->op);

BAIL:
    _cairo_quartz_teardown_state (&state, extents);

    CGImageRelease (img);

    return status;
}

static cairo_int_status_t
_cairo_quartz_cg_mask_with_solid (cairo_quartz_surface_t *surface,
				  cairo_composite_rectangles_t *extents)
{
    cairo_quartz_drawing_state_t state;
    double alpha = extents->mask_pattern.solid.color.alpha;
    cairo_status_t status;

    status = _cairo_quartz_setup_state (&state, extents);
    if (unlikely (status))
	return status;

    CGContextSetAlpha (surface->cgContext, alpha);
    _cairo_quartz_draw_source (&state, extents->op);

    _cairo_quartz_teardown_state (&state, extents);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_quartz_cg_mask (const cairo_compositor_t *compositor,
		       cairo_composite_rectangles_t *extents)
{
    cairo_quartz_surface_t *surface = (cairo_quartz_surface_t *)extents->surface;
    const cairo_pattern_t *source = &extents->source_pattern.base;
    const cairo_pattern_t *mask = &extents->mask_pattern.base;
    cairo_surface_t *mask_surf;
    cairo_matrix_t matrix;
    cairo_status_t status;
    cairo_bool_t need_temp;
    CGInterpolationQuality filter;

    ND ((stderr, "%p _cairo_quartz_surface_mask op %d source->type %d mask->type %d\n",
	 extents->surface, extents->op, extents->source_pattern.base.type,
	 extents->mask_pattern.base.type));

    if (mask->type == CAIRO_PATTERN_TYPE_SOLID)
	return _cairo_quartz_cg_mask_with_solid (surface, extents);

    need_temp = (mask->type   != CAIRO_PATTERN_TYPE_SURFACE ||
		 mask->extend != CAIRO_EXTEND_NONE);

    filter = _cairo_quartz_filter_to_quartz (source->filter);

    if (! need_temp) {
	mask_surf = extents->mask_pattern.surface.surface;

	/* When an opaque surface used as a mask in Quartz, its
	 * luminosity is used as the alpha value, so we can only use
	 * surfaces with alpha without creating a temporary mask. */
	need_temp = ! (mask_surf->content & CAIRO_CONTENT_ALPHA);
    }

    if (! need_temp) {
	CGInterpolationQuality mask_filter;
	cairo_bool_t simple_transform;

	matrix = mask->matrix;

	mask_filter = _cairo_quartz_filter_to_quartz (mask->filter);
	if (mask_filter == kCGInterpolationNone) {
	    simple_transform = _cairo_matrix_is_translation (&matrix);
	    if (simple_transform) {
		matrix.x0 = ceil (matrix.x0 - 0.5);
		matrix.y0 = ceil (matrix.y0 - 0.5);
	    }
	} else {
	    simple_transform = _cairo_matrix_is_integer_translation (&matrix,
								     NULL,
								     NULL);
	}

	/* Quartz only allows one interpolation to be set for mask and
	 * source, so we can skip the temp surface only if the source
	 * filtering makes the mask look correct. */
	if (source->type == CAIRO_PATTERN_TYPE_SURFACE)
	    need_temp = ! (simple_transform || filter == mask_filter);
	else
	    filter = mask_filter;
    }

    if (need_temp) {
	/* Render the mask to a surface */
	mask_surf = _cairo_quartz_surface_create_similar (surface,
							  CAIRO_CONTENT_ALPHA,
							  surface->extents.width,
							  surface->extents.height);
	status = mask_surf->status;
	if (unlikely (status))
	    goto BAIL;

	/* mask_surf is clear, so use OVER instead of SOURCE to avoid a
	 * temporary layer or fallback to cairo-image. */
	status = _cairo_surface_paint (mask_surf, CAIRO_OPERATOR_OVER, mask, NULL);
	if (unlikely (status))
	    goto BAIL;

	cairo_matrix_init_identity (&matrix);
    }

    status = _cairo_quartz_cg_mask_with_surface (extents,
						 mask_surf, &matrix, filter);

BAIL:

    if (need_temp)
	cairo_surface_destroy (mask_surf);

    return status;
}

static cairo_int_status_t
_cairo_quartz_cg_fill (const cairo_compositor_t *compositor,
		       cairo_composite_rectangles_t *extents,
		       const cairo_path_fixed_t *path,
		       cairo_fill_rule_t fill_rule,
		       double tolerance,
		       cairo_antialias_t antialias)
{
    cairo_quartz_drawing_state_t state;
    cairo_int_status_t rv = CAIRO_STATUS_SUCCESS;

    ND ((stderr, "%p _cairo_quartz_surface_fill op %d source->type %d\n",
	 extents->surface, extents->op, extents->source_pattern.base.type));

    rv = _cairo_quartz_setup_state (&state, extents);
    if (unlikely (rv))
	goto BAIL;

    CGContextSetShouldAntialias (state.cgMaskContext, (antialias != CAIRO_ANTIALIAS_NONE));

    _cairo_quartz_cairo_path_to_quartz_context (path, state.cgMaskContext);

    if (state.action == DO_DIRECT) {
	assert (state.cgDrawContext == state.cgMaskContext);
	if (fill_rule == CAIRO_FILL_RULE_WINDING)
	    CGContextFillPath (state.cgMaskContext);
	else
	    CGContextEOFillPath (state.cgMaskContext);
    } else {
	if (fill_rule == CAIRO_FILL_RULE_WINDING)
	    CGContextClip (state.cgMaskContext);
	else
	    CGContextEOClip (state.cgMaskContext);

	_cairo_quartz_draw_source (&state, extents->op);
    }

BAIL:
    _cairo_quartz_teardown_state (&state, extents);

    ND ((stderr, "-- fill\n"));
    return rv;
}

static cairo_int_status_t
_cairo_quartz_cg_stroke (const cairo_compositor_t *compositor,
			 cairo_composite_rectangles_t *extents,
			 const cairo_path_fixed_t *path,
			 const cairo_stroke_style_t *style,
			 const cairo_matrix_t *ctm,
			 const cairo_matrix_t *ctm_inverse,
			 double tolerance,
			 cairo_antialias_t antialias)
{
    cairo_quartz_drawing_state_t state;
    cairo_int_status_t rv = CAIRO_STATUS_SUCCESS;
    CGAffineTransform strokeTransform, invStrokeTransform;

    ND ((stderr, "%p _cairo_quartz_surface_stroke op %d source->type %d\n",
	 extents->surface, extents->op, extents->source_pattern.base.type));

    rv = _cairo_quartz_setup_state (&state, extents);
    if (unlikely (rv))
	goto BAIL;

    // Turning antialiasing off used to cause misrendering with
    // single-pixel lines (e.g. 20,10.5 -> 21,10.5 end up being rendered as 2 pixels).
    // That's been since fixed in at least 10.5, and in the latest 10.4 dot releases.
    CGContextSetShouldAntialias (state.cgMaskContext, (antialias != CAIRO_ANTIALIAS_NONE));
    CGContextSetLineWidth (state.cgMaskContext, style->line_width);
    CGContextSetLineCap (state.cgMaskContext, _cairo_quartz_cairo_line_cap_to_quartz (style->line_cap));
    CGContextSetLineJoin (state.cgMaskContext, _cairo_quartz_cairo_line_join_to_quartz (style->line_join));
    CGContextSetMiterLimit (state.cgMaskContext, style->miter_limit);

    if (style->dash && style->num_dashes) {
	cairo_quartz_float_t sdash[CAIRO_STACK_ARRAY_LENGTH (cairo_quartz_float_t)];
	cairo_quartz_float_t *fdash = sdash;
	unsigned int max_dashes = style->num_dashes;
	unsigned int k;

	if (style->num_dashes%2)
	    max_dashes *= 2;
	if (max_dashes > ARRAY_LENGTH (sdash))
	    fdash = _cairo_malloc_ab (max_dashes, sizeof (cairo_quartz_float_t));
	if (unlikely (fdash == NULL)) {
	    rv = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    goto BAIL;
	}

	for (k = 0; k < max_dashes; k++)
	    fdash[k] = (cairo_quartz_float_t) style->dash[k % style->num_dashes];

	CGContextSetLineDash (state.cgMaskContext, style->dash_offset, fdash, max_dashes);
	if (fdash != sdash)
	    free (fdash);
    } else
	CGContextSetLineDash (state.cgMaskContext, 0, NULL, 0);

    _cairo_quartz_cairo_path_to_quartz_context (path, state.cgMaskContext);

    _cairo_quartz_cairo_matrix_to_quartz (ctm, &strokeTransform);
    CGContextConcatCTM (state.cgMaskContext, strokeTransform);

    if (state.action == DO_DIRECT) {
	assert (state.cgDrawContext == state.cgMaskContext);
	CGContextStrokePath (state.cgMaskContext);
    } else {
	CGContextReplacePathWithStrokedPath (state.cgMaskContext);
	CGContextClip (state.cgMaskContext);

	_cairo_quartz_cairo_matrix_to_quartz (ctm_inverse, &invStrokeTransform);
	CGContextConcatCTM (state.cgMaskContext, invStrokeTransform);

	_cairo_quartz_draw_source (&state, extents->op);
    }

BAIL:
    _cairo_quartz_teardown_state (&state, extents);

    ND ((stderr, "-- stroke\n"));
    return rv;
}

#if CAIRO_HAS_QUARTZ_FONT
static cairo_int_status_t
_cairo_quartz_cg_glyphs (const cairo_compositor_t *compositor,
			 cairo_composite_rectangles_t *extents,
			 cairo_scaled_font_t *scaled_font,
			 cairo_glyph_t *glyphs,
			 int num_glyphs,
			 cairo_bool_t overlap)
{
    CGAffineTransform textTransform, invTextTransform;
    CGGlyph glyphs_static[CAIRO_STACK_ARRAY_LENGTH (CGPoint)];
    CGPoint cg_positions_static[CAIRO_STACK_ARRAY_LENGTH (CGPoint)];
    CGGlyph *cg_glyphs = &glyphs_static[0];
    CGPoint *cg_positions = &cg_positions_static[0];

    cairo_quartz_drawing_state_t state;
    cairo_int_status_t rv = CAIRO_INT_STATUS_UNSUPPORTED;
    CGPoint origin;
    CTFontRef ctFont = NULL;

    cairo_bool_t didForceFontSmoothing = FALSE;

    if (cairo_scaled_font_get_type (scaled_font) != CAIRO_FONT_TYPE_QUARTZ)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    rv = _cairo_quartz_setup_state (&state, extents);
    if (unlikely (rv))
	goto BAIL;

    if (state.action == DO_DIRECT) {
	assert (state.cgDrawContext == state.cgMaskContext);
	CGContextSetTextDrawingMode (state.cgMaskContext, kCGTextFill);
    } else {
	CGContextSetTextDrawingMode (state.cgMaskContext, kCGTextClip);
    }

    /* this doesn't addref */
    ctFont = _cairo_quartz_scaled_font_get_ct_font (scaled_font);
    _cairo_quartz_set_antialiasing (state.cgMaskContext, scaled_font->options.antialias);

    if (num_glyphs > ARRAY_LENGTH (glyphs_static)) {
	cg_glyphs = (CGGlyph*) _cairo_malloc_ab (num_glyphs, sizeof (CGGlyph) + sizeof (CGPoint));
	if (unlikely (cg_glyphs == NULL)) {
	    rv = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    goto BAIL;
	}

	cg_positions = (CGPoint*) (cg_glyphs + num_glyphs);
    }

    /* scale(1,-1) * scaled_font->scale */
    textTransform = CGAffineTransformMake (scaled_font->scale.xx,
					   scaled_font->scale.yx,
					   -scaled_font->scale.xy,
					   -scaled_font->scale.yy,
					   0.0, 0.0);

    /* scaled_font->scale_inverse * scale(1,-1) */
    invTextTransform = CGAffineTransformMake (scaled_font->scale_inverse.xx,
					      -scaled_font->scale_inverse.yx,
					      scaled_font->scale_inverse.xy,
					      -scaled_font->scale_inverse.yy,
					      0.0, 0.0);


    origin = CGPointMake (glyphs[0].x, glyphs[0].y);
    for (int i = 0; i < num_glyphs; ++i)
    {
	cg_glyphs[i] = glyphs[i].index;
	cg_positions[i] = CGPointMake (glyphs[i].x - origin.x, glyphs[i].y - origin.y);
	cg_positions[i] = CGPointApplyAffineTransform (cg_positions[i], invTextTransform);
    }

    /* Translate to the first glyph's position before drawing */
    CGContextTranslateCTM (state.cgMaskContext, origin.x, origin.y);
    CGContextConcatCTM (state.cgMaskContext, textTransform);

    CTFontDrawGlyphs (ctFont, cg_glyphs, cg_positions, num_glyphs, state.cgMaskContext);

    CGContextConcatCTM (state.cgMaskContext, invTextTransform);
    CGContextTranslateCTM (state.cgMaskContext, -origin.x, -origin.y);

    if (state.action != DO_DIRECT)
	_cairo_quartz_draw_source (&state, extents->op);

BAIL:
    if (didForceFontSmoothing)
	CGContextSetAllowsFontSmoothing (state.cgMaskContext, FALSE);

    _cairo_quartz_teardown_state (&state, extents);

    if (cg_glyphs != glyphs_static)
	free (cg_glyphs);

    return rv;
}
#endif /* CAIRO_HAS_QUARTZ_FONT */

static const cairo_compositor_t _cairo_quartz_cg_compositor = {
    &_cairo_fallback_compositor,

    _cairo_quartz_cg_paint,
    _cairo_quartz_cg_mask,
    _cairo_quartz_cg_stroke,
    _cairo_quartz_cg_fill,
#if CAIRO_HAS_QUARTZ_FONT
    _cairo_quartz_cg_glyphs,
#else
    NULL,
#endif
};

static cairo_int_status_t
_cairo_quartz_surface_paint (void *surface,
			     cairo_operator_t op,
			     const cairo_pattern_t *source,
			     const cairo_clip_t *clip)
{
    return _cairo_compositor_paint (&_cairo_quartz_cg_compositor,
				    surface, op, source, clip);
}

static cairo_int_status_t
_cairo_quartz_surface_mask (void *surface,
			    cairo_operator_t op,
			    const cairo_pattern_t *source,
			    const cairo_pattern_t *mask,
			    const cairo_clip_t *clip)
{
    return _cairo_compositor_mask (&_cairo_quartz_cg_compositor,
				   surface, op, source, mask,
				   clip);
}

static cairo_int_status_t
_cairo_quartz_surface_fill (void *surface,
			    cairo_operator_t op,
			    const cairo_pattern_t *source,
			    const cairo_path_fixed_t *path,
			    cairo_fill_rule_t fill_rule,
			    double tolerance,
			    cairo_antialias_t antialias,
			    const cairo_clip_t *clip)
{
    return _cairo_compositor_fill (&_cairo_quartz_cg_compositor,
				   surface, op, source, path,
				   fill_rule, tolerance, antialias,
				   clip);
}

static cairo_int_status_t
_cairo_quartz_surface_stroke (void *surface,
			      cairo_operator_t op,
			      const cairo_pattern_t *source,
			      const cairo_path_fixed_t *path,
			      const cairo_stroke_style_t *style,
			      const cairo_matrix_t *ctm,
			      const cairo_matrix_t *ctm_inverse,
			      double tolerance,
			      cairo_antialias_t antialias,
			      const cairo_clip_t *clip)
{
    return _cairo_compositor_stroke (&_cairo_quartz_cg_compositor,
				     surface, op, source, path,
				     style, ctm,ctm_inverse,
				     tolerance, antialias, clip);
}

static cairo_int_status_t
_cairo_quartz_surface_glyphs (void *surface,
			      cairo_operator_t op,
			      const cairo_pattern_t *source,
			      cairo_glyph_t *glyphs,
			      int num_glyphs,
			      cairo_scaled_font_t *scaled_font,
			      const cairo_clip_t *clip)
{
    return _cairo_compositor_glyphs (&_cairo_quartz_cg_compositor,
				     surface, op, source,
				     glyphs, num_glyphs, scaled_font,
				     clip);
}

static cairo_status_t
_cairo_quartz_surface_clipper_intersect_clip_path (cairo_surface_clipper_t *clipper,
						   cairo_path_fixed_t *path,
						   cairo_fill_rule_t fill_rule,
						   double tolerance,
						   cairo_antialias_t antialias)
{
    cairo_quartz_surface_t *surface =
	cairo_container_of (clipper, cairo_quartz_surface_t, clipper);

    ND ((stderr, "%p _cairo_quartz_surface_intersect_clip_path path: %p\n", surface, path));

    if (IS_EMPTY (surface))
	return CAIRO_STATUS_SUCCESS;

    if (path == NULL) {
	/* If we're being asked to reset the clip, we can only do it
	 * by restoring the gstate to our previous saved one, and
	 * saving it again.
	 *
	 * Note that this assumes that ALL quartz surface creation
	 * functions will do a SaveGState first; we do this in create_internal.
	 */
	CGContextRestoreGState (surface->cgContext);
	CGContextSaveGState (surface->cgContext);
    } else {
	CGContextSetShouldAntialias (surface->cgContext, (antialias != CAIRO_ANTIALIAS_NONE));

	_cairo_quartz_cairo_path_to_quartz_context (path, surface->cgContext);

	if (fill_rule == CAIRO_FILL_RULE_WINDING)
	    CGContextClip (surface->cgContext);
	else
	    CGContextEOClip (surface->cgContext);
    }

    ND ((stderr, "-- intersect_clip_path\n"));

    return CAIRO_STATUS_SUCCESS;
}

// XXXtodo implement show_page; need to figure out how to handle begin/end

static const cairo_surface_backend_t cairo_quartz_surface_backend = {
    CAIRO_SURFACE_TYPE_QUARTZ,
    _cairo_quartz_surface_finish,

    _cairo_default_context_create,

    _cairo_quartz_surface_create_similar,
    NULL, /* similar image */
    _cairo_quartz_surface_map_to_image,
    _cairo_quartz_surface_unmap_image,

    _cairo_surface_default_source,
    _cairo_quartz_surface_acquire_source_image,
    _cairo_quartz_surface_release_source_image,
    NULL, /* snapshot */

    NULL, /* copy_page */
    NULL, /* show_page */

    _cairo_quartz_surface_get_extents,
    NULL, /* get_font_options */

    NULL, /* flush */
    NULL, /* mark_dirty_rectangle */

    _cairo_quartz_surface_paint,
    _cairo_quartz_surface_mask,
    _cairo_quartz_surface_stroke,
    _cairo_quartz_surface_fill,
    NULL,  /* fill-stroke */
    _cairo_quartz_surface_glyphs,
    NULL, /* has_show_text_glyphs */
    NULL, /* show_text_glyphs */
    NULL, /* get_supported_mime_types */
    NULL, /* tag */
};

cairo_quartz_surface_t *
_cairo_quartz_surface_create_internal (CGContextRef cgContext,
				       cairo_content_t content,
				       unsigned int width,
				       unsigned int height)
{
    cairo_quartz_surface_t *surface;

    /* Init the base surface */
    surface = _cairo_malloc (sizeof (cairo_quartz_surface_t));
    if (unlikely (surface == NULL))
	return (cairo_quartz_surface_t*) _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    memset (surface, 0, sizeof (cairo_quartz_surface_t));

    _cairo_surface_init (&surface->base,
			 &cairo_quartz_surface_backend,
			 NULL, /* device */
			 content,
			 FALSE); /* is_vector */

    _cairo_surface_clipper_init (&surface->clipper,
				 _cairo_quartz_surface_clipper_intersect_clip_path);

    /* Save our extents */
    surface->extents.x = surface->extents.y = 0;
    surface->extents.width = width;
    surface->extents.height = height;
    surface->virtual_extents = surface->extents;

    if (IS_EMPTY (surface)) {
	surface->cgContext = NULL;
	surface->cgContextBaseCTM = CGAffineTransformIdentity;
	surface->base.is_clear = TRUE;
	return surface;
    }

    /* Save so we can always get back to a known-good CGContext -- this is
     * required for proper behaviour of intersect_clip_path(NULL)
     */
    CGContextSaveGState (cgContext);

    surface->cgContext = cgContext;
    surface->cgContextBaseCTM = CGContextGetCTM (cgContext);

    return surface;
}

/**
 * cairo_quartz_surface_create_for_cg_context:
 * @cgContext: the existing CGContext for which to create the surface
 * @width: width of the surface, in pixels
 * @height: height of the surface, in pixels
 *
 * Creates a Quartz surface that wraps the given CGContext.  The
 * CGContext is assumed to be in the standard Cairo coordinate space
 * (that is, with the origin at the upper left and the Y axis
 * increasing downward).  If the CGContext is in the Quartz coordinate
 * space (with the origin at the bottom left), then it should be
 * flipped before this function is called.  The flip can be accomplished
 * using a translate and a scale; for example:
 *
 * <informalexample><programlisting>
 * CGContextTranslateCTM (cgContext, 0.0, height);
 * CGContextScaleCTM (cgContext, 1.0, -1.0);
 * </programlisting></informalexample>
 *
 * All Cairo operations are implemented in terms of Quartz operations,
 * as long as Quartz-compatible elements are used (such as Quartz fonts).
 *
 * Return value: the newly created Cairo surface.
 *
 * Since: 1.6
 **/

cairo_surface_t *
cairo_quartz_surface_create_for_cg_context (CGContextRef cgContext,
					    unsigned int width,
					    unsigned int height)
{
    cairo_quartz_surface_t *surf;

    surf = _cairo_quartz_surface_create_internal (cgContext, CAIRO_CONTENT_COLOR_ALPHA,
						  width, height);
    if (likely (!surf->base.status))
	CGContextRetain (cgContext);

    return &surf->base;
}

/**
 * cairo_quartz_surface_create:
 * @format: format of pixels in the surface to create
 * @width: width of the surface, in pixels
 * @height: height of the surface, in pixels
 *
 * Creates a Quartz surface backed by a CGBitmap.  The surface is
 * created using the Device RGB (or Device Gray, for A8) color space.
 * All Cairo operations, including those that require software
 * rendering, will succeed on this surface.
 *
 * Return value: the newly created surface.
 *
 * Since: 1.6
 **/
cairo_surface_t *
cairo_quartz_surface_create (cairo_format_t format,
			     unsigned int width,
			     unsigned int height)
{
    cairo_quartz_surface_t *surf;
    CGContextRef cgc;
    CGColorSpaceRef cgColorspace;
    CGBitmapInfo bitinfo;
    void *imageData = NULL;
    int stride;
    int bitsPerComponent;

    if (!_cairo_quartz_verify_surface_size (width, height))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_INVALID_SIZE));

    if (width == 0 || height == 0) {
	return &_cairo_quartz_surface_create_internal (NULL, _cairo_content_from_format (format),
						       width, height)->base;
    }

    if (format == CAIRO_FORMAT_ARGB32 ||
	format == CAIRO_FORMAT_RGB24)
    {
	cgColorspace = CGColorSpaceCreateDeviceRGB ();
	bitinfo = kCGBitmapByteOrder32Host;
	if (format == CAIRO_FORMAT_ARGB32)
	    bitinfo |= kCGImageAlphaPremultipliedFirst;
	else
	    bitinfo |= kCGImageAlphaNoneSkipFirst;
	bitsPerComponent = 8;
	stride = width * 4;
    } else if (format == CAIRO_FORMAT_A8) {
	cgColorspace = NULL;
	stride = width;
	bitinfo = kCGImageAlphaOnly;
	bitsPerComponent = 8;
    } else if (format == CAIRO_FORMAT_A1) {
	/* I don't think we can usefully support this, as defined by
	 * cairo_format_t -- these are 1-bit pixels stored in 32-bit
	 * quantities.
	 */
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_INVALID_FORMAT));
    } else {
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_INVALID_FORMAT));
    }

    /* The Apple docs say that for best performance, the stride and the data
     * pointer should be 16-byte aligned.  malloc already aligns to 16-bytes,
     * so we don't have to anything special on allocation.
     */
    stride = (stride + 15) & ~15;
    cgc = CGBitmapContextCreate (imageData,
				 width,
				 height,
				 bitsPerComponent,
				 stride,
				 cgColorspace,
				 bitinfo);
    CGColorSpaceRelease (cgColorspace);

    if (!cgc) {
	if (imageData)
	    free (imageData);

	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));
    }

    /* flip the Y axis */
    CGContextTranslateCTM (cgc, 0.0, height);
    CGContextScaleCTM (cgc, 1.0, -1.0);

    surf = _cairo_quartz_surface_create_internal (cgc, _cairo_content_from_format (format),
						  width, height);
    if (surf->base.status) {
	CGContextRelease (cgc);

	if (imageData)
	    free (imageData);

	// create_internal will have set an error
	return &surf->base;
    }

    surf->base.is_clear = TRUE;

    return &surf->base;
}

/**
 * cairo_quartz_surface_get_cg_context:
 * @surface: the Cairo Quartz surface
 *
 * Returns the CGContextRef that the given Quartz surface is backed
 * by.
 *
 * A call to cairo_surface_flush() is required before using the
 * CGContextRef to ensure that all pending drawing operations are
 * finished and to restore any temporary modification cairo has made
 * to its state. A call to cairo_surface_mark_dirty() is required
 * after the state or the content of the CGContextRef has been
 * modified.
 *
 * Return value: the CGContextRef for the given surface.
 *
 * Since: 1.6
 **/
CGContextRef
cairo_quartz_surface_get_cg_context (cairo_surface_t *surface)
{
    if (surface && _cairo_surface_is_quartz (surface)) {
	cairo_quartz_surface_t *quartz = (cairo_quartz_surface_t *) surface;
	return quartz->cgContext;
    } else
	return NULL;
}

/**
 * _cairo_surface_is_quartz:
 * @surface: a #cairo_surface_t
 *
 * Checks if a surface is a #cairo_quartz_surface_t
 *
 * Return value: True if the surface is an quartz surface
 **/
cairo_bool_t
_cairo_surface_is_quartz (const cairo_surface_t *surface)
{
    return surface->backend == &cairo_quartz_surface_backend;
}

cairo_surface_t*
_cairo_quartz_snapshot_create (cairo_quartz_surface_t *surface)
{
    cairo_quartz_snapshot_t *snapshot = NULL;

    if (!surface || !_cairo_surface_is_quartz (&surface->base) || IS_EMPTY (surface) ||
	! _cairo_quartz_is_cgcontext_bitmap_context (surface->cgContext))
	return NULL;

    snapshot = _cairo_malloc (sizeof (cairo_quartz_snapshot_t));

    if (unlikely (surface == NULL))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    memset (snapshot, 0, sizeof (cairo_quartz_snapshot_t));
    _cairo_surface_init (&snapshot->base,
			 &cairo_quartz_snapshot_backend,
			 NULL, CAIRO_CONTENT_COLOR_ALPHA, FALSE);
    snapshot->image = CGBitmapContextCreateImage (surface->cgContext);

    return &snapshot->base;
}

cairo_status_t
_cairo_quartz_snapshot_finish (void *surface)
{
    cairo_quartz_snapshot_t *snapshot = (cairo_quartz_snapshot_t *)surface;
   if (snapshot->image)
	CGImageRelease (snapshot->image);
    return CAIRO_STATUS_SUCCESS;
}

CGImageRef
_cairo_quartz_surface_snapshot_get_image (cairo_quartz_surface_t *surface)
{
    cairo_surface_t *snapshot =
	_cairo_surface_has_snapshot (&surface->base, &cairo_quartz_snapshot_backend);

    if (unlikely (!snapshot))
    {
	snapshot = _cairo_quartz_snapshot_create (surface);
	if (unlikely (!snapshot || cairo_surface_status (snapshot)))
	    return NULL;
	_cairo_surface_attach_snapshot (&surface->base, snapshot, NULL);
	cairo_surface_destroy (snapshot);
    }

    return CGImageRetain (((cairo_quartz_snapshot_t*)snapshot)->image);
}

/* Debug stuff */

#ifdef QUARTZ_DEBUG

void
quartz_image_to_png (CGImageRef image, const char *dest)
{
    static int sctr = 0;
    const char* image_name = "quartz-image";
    char pathbuf[100];

    CFStringRef png_utti = CFSTR("public.png");
    CFStringRef path;
    CFURLRef url;
    CGImageDestinationRef image_dest;

    memset (pathbuf, 0, sizeof (pathbuf));
    dest = dest ? dest : image_name;
    snprintf (pathbuf, sizeof (pathbuf), "%s/Desktop/%s%d.png",getenv ("HOME"), dest, sctr++);
    path = CFStringCreateWithCString (NULL, pathbuf, kCFStringEncodingUTF8);
    url = CFURLCreateWithFileSystemPath (NULL, path, kCFURLPOSIXPathStyle, FALSE);
    image_dest = CGImageDestinationCreateWithURL (url, png_utti, 1, NULL);

    CGImageDestinationAddImage (image_dest, image, NULL);
    CGImageDestinationFinalize (image_dest);

    CFRelease (url);
    CFRelease (path);
 }

void
quartz_surface_to_png (cairo_quartz_surface_t *nq, const char *dest)
{
    CGImageRef image;

    if (nq->base.type != CAIRO_SURFACE_TYPE_QUARTZ) {
	fprintf (stderr, "** quartz_surface_to_png: surface %p isn't quartz!\n", nq);
	return;
    }

    image = CGBitmapContextCreateImage (nq->cgContext);
    quartz_image_to_png (image, dest);

    CGImageRelease (image);
}

#endif /* QUARTZ_DEBUG */
