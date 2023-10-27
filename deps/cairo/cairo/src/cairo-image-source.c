/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2003 University of Southern California
 * Copyright © 2009,2010,2011 Intel Corporation
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
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

/* The purpose of this file/surface is to simply translate a pattern
 * to a pixman_image_t and thence to feed it back to the general
 * compositor interface.
 */

#include "cairoint.h"

#include "cairo-image-surface-private.h"

#include "cairo-compositor-private.h"
#include "cairo-error-private.h"
#include "cairo-pattern-inline.h"
#include "cairo-paginated-private.h"
#include "cairo-recording-surface-private.h"
#include "cairo-surface-observer-private.h"
#include "cairo-surface-snapshot-inline.h"
#include "cairo-surface-subsurface-private.h"

#define PIXMAN_MAX_INT ((pixman_fixed_1 >> 1) - pixman_fixed_e) /* need to ensure deltas also fit */

#if CAIRO_NO_MUTEX
#define PIXMAN_HAS_ATOMIC_OPS 1
#endif

#if PIXMAN_HAS_ATOMIC_OPS
static pixman_image_t *__pixman_transparent_image;
static pixman_image_t *__pixman_black_image;
static pixman_image_t *__pixman_white_image;

static pixman_image_t *
_pixman_transparent_image (void)
{
    pixman_image_t *image;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    image = __pixman_transparent_image;
    if (unlikely (image == NULL)) {
	pixman_color_t color;

	color.red   = 0x00;
	color.green = 0x00;
	color.blue  = 0x00;
	color.alpha = 0x00;

	image = pixman_image_create_solid_fill (&color);
	if (unlikely (image == NULL))
	    return NULL;

	if (_cairo_atomic_ptr_cmpxchg (&__pixman_transparent_image,
				       NULL, image))
	{
	    pixman_image_ref (image);
	}
    } else {
	pixman_image_ref (image);
    }

    return image;
}

static pixman_image_t *
_pixman_black_image (void)
{
    pixman_image_t *image;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    image = __pixman_black_image;
    if (unlikely (image == NULL)) {
	pixman_color_t color;

	color.red   = 0x00;
	color.green = 0x00;
	color.blue  = 0x00;
	color.alpha = 0xffff;

	image = pixman_image_create_solid_fill (&color);
	if (unlikely (image == NULL))
	    return NULL;

	if (_cairo_atomic_ptr_cmpxchg (&__pixman_black_image,
				       NULL, image))
	{
	    pixman_image_ref (image);
	}
    } else {
	pixman_image_ref (image);
    }

    return image;
}

static pixman_image_t *
_pixman_white_image (void)
{
    pixman_image_t *image;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    image = __pixman_white_image;
    if (unlikely (image == NULL)) {
	pixman_color_t color;

	color.red   = 0xffff;
	color.green = 0xffff;
	color.blue  = 0xffff;
	color.alpha = 0xffff;

	image = pixman_image_create_solid_fill (&color);
	if (unlikely (image == NULL))
	    return NULL;

	if (_cairo_atomic_ptr_cmpxchg (&__pixman_white_image,
				       NULL, image))
	{
	    pixman_image_ref (image);
	}
    } else {
	pixman_image_ref (image);
    }

    return image;
}

static uint32_t
hars_petruska_f54_1_random (void)
{
#define rol(x,k) ((x << k) | (x >> (32-k)))
    static uint32_t x;
    return x = (x ^ rol (x, 5) ^ rol (x, 24)) + 0x37798849;
#undef rol
}

static struct {
    cairo_color_t color;
    pixman_image_t *image;
} cache[16];
static int n_cached;

#else  /* !PIXMAN_HAS_ATOMIC_OPS */
static pixman_image_t *
_pixman_transparent_image (void)
{
    TRACE ((stderr, "%s\n", __FUNCTION__));
    return _pixman_image_for_color (CAIRO_COLOR_TRANSPARENT);
}

static pixman_image_t *
_pixman_black_image (void)
{
    TRACE ((stderr, "%s\n", __FUNCTION__));
    return _pixman_image_for_color (CAIRO_COLOR_BLACK);
}

static pixman_image_t *
_pixman_white_image (void)
{
    TRACE ((stderr, "%s\n", __FUNCTION__));
    return _pixman_image_for_color (CAIRO_COLOR_WHITE);
}
#endif /* !PIXMAN_HAS_ATOMIC_OPS */


pixman_image_t *
_pixman_image_for_color (const cairo_color_t *cairo_color)
{
    pixman_color_t color;
    pixman_image_t *image;

#if PIXMAN_HAS_ATOMIC_OPS
    int i;

    if (CAIRO_COLOR_IS_CLEAR (cairo_color))
	return _pixman_transparent_image ();

    if (CAIRO_COLOR_IS_OPAQUE (cairo_color)) {
	if (cairo_color->red_short <= 0x00ff &&
	    cairo_color->green_short <= 0x00ff &&
	    cairo_color->blue_short <= 0x00ff)
	{
	    return _pixman_black_image ();
	}

	if (cairo_color->red_short >= 0xff00 &&
	    cairo_color->green_short >= 0xff00 &&
	    cairo_color->blue_short >= 0xff00)
	{
	    return _pixman_white_image ();
	}
    }

    CAIRO_MUTEX_LOCK (_cairo_image_solid_cache_mutex);
    for (i = 0; i < n_cached; i++) {
	if (_cairo_color_equal (&cache[i].color, cairo_color)) {
	    image = pixman_image_ref (cache[i].image);
	    goto UNLOCK;
	}
    }
#endif

    color.red   = cairo_color->red_short;
    color.green = cairo_color->green_short;
    color.blue  = cairo_color->blue_short;
    color.alpha = cairo_color->alpha_short;

    image = pixman_image_create_solid_fill (&color);
#if PIXMAN_HAS_ATOMIC_OPS
    if (image == NULL)
	goto UNLOCK;

    if (n_cached < ARRAY_LENGTH (cache)) {
	i = n_cached++;
    } else {
	i = hars_petruska_f54_1_random () % ARRAY_LENGTH (cache);
	pixman_image_unref (cache[i].image);
    }
    cache[i].image = pixman_image_ref (image);
    cache[i].color = *cairo_color;

UNLOCK:
    CAIRO_MUTEX_UNLOCK (_cairo_image_solid_cache_mutex);
#endif
    return image;
}


void
_cairo_image_reset_static_data (void)
{
#if PIXMAN_HAS_ATOMIC_OPS
    while (n_cached)
	pixman_image_unref (cache[--n_cached].image);

    if (__pixman_transparent_image) {
	pixman_image_unref (__pixman_transparent_image);
	__pixman_transparent_image = NULL;
    }

    if (__pixman_black_image) {
	pixman_image_unref (__pixman_black_image);
	__pixman_black_image = NULL;
    }

    if (__pixman_white_image) {
	pixman_image_unref (__pixman_white_image);
	__pixman_white_image = NULL;
    }
#endif
}

static pixman_image_t *
_pixman_image_for_gradient (const cairo_gradient_pattern_t *pattern,
			    const cairo_rectangle_int_t *extents,
			    int *ix, int *iy)
{
    pixman_image_t	  *pixman_image;
    pixman_gradient_stop_t pixman_stops_static[2];
    pixman_gradient_stop_t *pixman_stops = pixman_stops_static;
    pixman_transform_t      pixman_transform;
    cairo_matrix_t matrix;
    cairo_circle_double_t extremes[2];
    pixman_point_fixed_t p1, p2;
    unsigned int i;
    cairo_int_status_t status;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (pattern->n_stops > ARRAY_LENGTH(pixman_stops_static)) {
	pixman_stops = _cairo_malloc_ab (pattern->n_stops,
					 sizeof(pixman_gradient_stop_t));
	if (unlikely (pixman_stops == NULL))
	    return NULL;
    }

    for (i = 0; i < pattern->n_stops; i++) {
	pixman_stops[i].x = _cairo_fixed_16_16_from_double (pattern->stops[i].offset);
	pixman_stops[i].color.red   = pattern->stops[i].color.red_short;
	pixman_stops[i].color.green = pattern->stops[i].color.green_short;
	pixman_stops[i].color.blue  = pattern->stops[i].color.blue_short;
	pixman_stops[i].color.alpha = pattern->stops[i].color.alpha_short;
    }

    _cairo_gradient_pattern_fit_to_range (pattern, PIXMAN_MAX_INT >> 1, &matrix, extremes);

    p1.x = _cairo_fixed_16_16_from_double (extremes[0].center.x);
    p1.y = _cairo_fixed_16_16_from_double (extremes[0].center.y);
    p2.x = _cairo_fixed_16_16_from_double (extremes[1].center.x);
    p2.y = _cairo_fixed_16_16_from_double (extremes[1].center.y);

    if (pattern->base.type == CAIRO_PATTERN_TYPE_LINEAR) {
	pixman_image = pixman_image_create_linear_gradient (&p1, &p2,
							    pixman_stops,
							    pattern->n_stops);
    } else {
	pixman_fixed_t r1, r2;

	r1   = _cairo_fixed_16_16_from_double (extremes[0].radius);
	r2   = _cairo_fixed_16_16_from_double (extremes[1].radius);

	pixman_image = pixman_image_create_radial_gradient (&p1, &p2, r1, r2,
							    pixman_stops,
							    pattern->n_stops);
    }

    if (pixman_stops != pixman_stops_static)
	free (pixman_stops);

    if (unlikely (pixman_image == NULL))
	return NULL;

    *ix = *iy = 0;
    status = _cairo_matrix_to_pixman_matrix_offset (&matrix, pattern->base.filter,
						    extents->x + extents->width/2.,
						    extents->y + extents->height/2.,
						    &pixman_transform, ix, iy);
    if (status != CAIRO_INT_STATUS_NOTHING_TO_DO) {
	if (unlikely (status != CAIRO_INT_STATUS_SUCCESS) ||
	    ! pixman_image_set_transform (pixman_image, &pixman_transform))
	{
	    pixman_image_unref (pixman_image);
	    return NULL;
	}
    }

    {
	pixman_repeat_t pixman_repeat;

	switch (pattern->base.extend) {
	default:
	case CAIRO_EXTEND_NONE:
	    pixman_repeat = PIXMAN_REPEAT_NONE;
	    break;
	case CAIRO_EXTEND_REPEAT:
	    pixman_repeat = PIXMAN_REPEAT_NORMAL;
	    break;
	case CAIRO_EXTEND_REFLECT:
	    pixman_repeat = PIXMAN_REPEAT_REFLECT;
	    break;
	case CAIRO_EXTEND_PAD:
	    pixman_repeat = PIXMAN_REPEAT_PAD;
	    break;
	}

	pixman_image_set_repeat (pixman_image, pixman_repeat);
    }

    return pixman_image;
}

static pixman_image_t *
_pixman_image_for_mesh (const cairo_mesh_pattern_t *pattern,
			const cairo_rectangle_int_t *extents,
			int *tx, int *ty)
{
    pixman_image_t *image;
    int width, height;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    *tx = -extents->x;
    *ty = -extents->y;
    width = extents->width;
    height = extents->height;

    image = pixman_image_create_bits (PIXMAN_a8r8g8b8, width, height, NULL, 0);
    if (unlikely (image == NULL))
	return NULL;

    _cairo_mesh_pattern_rasterize (pattern,
				   pixman_image_get_data (image),
				   width, height,
				   pixman_image_get_stride (image),
				   *tx, *ty);
    return image;
}

struct acquire_source_cleanup {
    cairo_surface_t *surface;
    cairo_image_surface_t *image;
    void *image_extra;
};

static void
_acquire_source_cleanup (pixman_image_t *pixman_image,
			 void *closure)
{
    struct acquire_source_cleanup *data = closure;

    _cairo_surface_release_source_image (data->surface,
					 data->image,
					 data->image_extra);
    free (data);
}

static void
_defer_free_cleanup (pixman_image_t *pixman_image,
		     void *closure)
{
    cairo_surface_destroy (closure);
}

static uint16_t
expand_channel (uint16_t v, uint32_t bits)
{
    int offset = 16 - bits;
    while (offset > 0) {
	v |= v >> bits;
	offset -= bits;
	bits += bits;
    }
    return v;
}

static pixman_image_t *
_pixel_to_solid (cairo_image_surface_t *image, int x, int y)
{
    uint32_t pixel;
    float *rgba;
    pixman_color_t color;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    switch (image->format) {
    default:
    case CAIRO_FORMAT_INVALID:
	ASSERT_NOT_REACHED;
	return NULL;

    case CAIRO_FORMAT_A1:
	pixel = *(uint8_t *) (image->data + y * image->stride + x/8);
	return pixel & (1 << (x&7)) ? _pixman_black_image () : _pixman_transparent_image ();

    case CAIRO_FORMAT_A8:
	color.alpha = *(uint8_t *) (image->data + y * image->stride + x);
	color.alpha |= color.alpha << 8;
	if (color.alpha == 0)
	    return _pixman_transparent_image ();
	if (color.alpha == 0xffff)
	    return _pixman_black_image ();

	color.red = color.green = color.blue = 0;
	return pixman_image_create_solid_fill (&color);

    case CAIRO_FORMAT_RGB16_565:
	pixel = *(uint16_t *) (image->data + y * image->stride + 2 * x);
	if (pixel == 0)
	    return _pixman_black_image ();
	if (pixel == 0xffff)
	    return _pixman_white_image ();

	color.alpha = 0xffff;
	color.red = expand_channel ((pixel >> 11 & 0x1f) << 11, 5);
	color.green = expand_channel ((pixel >> 5 & 0x3f) << 10, 6);
	color.blue = expand_channel ((pixel & 0x1f) << 11, 5);
	return pixman_image_create_solid_fill (&color);

    case CAIRO_FORMAT_RGB30:
	pixel = *(uint32_t *) (image->data + y * image->stride + 4 * x);
	pixel &= 0x3fffffff; /* ignore alpha bits */
	if (pixel == 0)
	    return _pixman_black_image ();
	if (pixel == 0x3fffffff)
	    return _pixman_white_image ();

	/* convert 10bpc to 16bpc */
	color.alpha = 0xffff;
	color.red = expand_channel((pixel >> 20) & 0x3fff, 10);
	color.green = expand_channel((pixel >> 10) & 0x3fff, 10);
	color.blue = expand_channel(pixel & 0x3fff, 10);
	return pixman_image_create_solid_fill (&color);

    case CAIRO_FORMAT_ARGB32:
    case CAIRO_FORMAT_RGB24:
	pixel = *(uint32_t *) (image->data + y * image->stride + 4 * x);
	color.alpha = image->format == CAIRO_FORMAT_ARGB32 ? (pixel >> 24) | (pixel >> 16 & 0xff00) : 0xffff;
	if (color.alpha == 0)
	    return _pixman_transparent_image ();
	if (pixel == 0xffffffff)
	    return _pixman_white_image ();
	if (color.alpha == 0xffff && (pixel & 0xffffff) == 0)
	    return _pixman_black_image ();

	color.red = (pixel >> 16 & 0xff) | (pixel >> 8 & 0xff00);
	color.green = (pixel >> 8 & 0xff) | (pixel & 0xff00);
	color.blue = (pixel & 0xff) | (pixel << 8 & 0xff00);
	return pixman_image_create_solid_fill (&color);

    case CAIRO_FORMAT_RGB96F:
    case CAIRO_FORMAT_RGBA128F:
	if (image->format == CAIRO_FORMAT_RGBA128F)
	{
	    rgba = (float *)&image->data[y * image->stride + 16 * x];
	    color.alpha = 65535.f * rgba[3];

	    if (color.alpha == 0)
		return _pixman_transparent_image ();
	}
	else
	{
	    rgba = (float *)&image->data[y * image->stride + 12 * x];
	    color.alpha = 0xffff;
	}

	if (color.alpha == 0xffff && rgba[0] == 0.f && rgba[1] == 0.f && rgba[2] == 0.f)
	    return _pixman_black_image ();
	if (color.alpha == 0xffff && rgba[0] == 1.f && rgba[1] == 1.f && rgba[2] == 1.f)
	    return _pixman_white_image ();

	color.red = rgba[0] * 65535.f;
	color.green = rgba[1] * 65535.f;
	color.blue = rgba[2] * 65535.f;
	return pixman_image_create_solid_fill (&color);
    }
}

/* ========================================================================== */

/* Index into filter table */
typedef enum
{
    KERNEL_IMPULSE,
    KERNEL_BOX,
    KERNEL_LINEAR,
    KERNEL_MITCHELL,
    KERNEL_NOTCH,
    KERNEL_CATMULL_ROM,
    KERNEL_LANCZOS3,
    KERNEL_LANCZOS3_STRETCHED,
    KERNEL_TENT
} kernel_t;

/* Produce contribution of a filter of size r for pixel centered on x.
   For a typical low-pass function this evaluates the function at x/r.
   If the frequency is higher than 1/2, such as when r is less than 1,
   this may need to integrate several samples, see cubic for examples.
*/
typedef double (* kernel_func_t) (double x, double r);

/* Return maximum number of pixels that will be non-zero. Except for
   impluse this is the maximum of 2 and the width of the non-zero part
   of the filter rounded up to the next integer.
*/
typedef int (* kernel_width_func_t) (double r);

/* Table of filters */
typedef struct
{
    kernel_t		kernel;
    kernel_func_t	func;
    kernel_width_func_t	width;
} filter_info_t;

/* PIXMAN_KERNEL_IMPULSE: Returns pixel nearest the center.  This
   matches PIXMAN_FILTER_NEAREST. This is useful if you wish to
   combine the result of nearest in one direction with another filter
   in the other.
*/

static double
impulse_kernel (double x, double r)
{
    return 1;
}

static int
impulse_width (double r)
{
    return 1;
}

/* PIXMAN_KERNEL_BOX: Intersection of a box of width r with square
   pixels. This is the smallest possible filter such that the output
   image contains an equal contribution from all the input
   pixels. Lots of software uses this. The function is a trapazoid of
   width r+1, not a box.

   When r == 1.0, PIXMAN_KERNEL_BOX, PIXMAN_KERNEL_LINEAR, and
   PIXMAN_KERNEL_TENT all produce the same filter, allowing
   them to be exchanged at this point.
*/

static double
box_kernel (double x, double r)
{
    return MAX (0.0, MIN (MIN (r, 1.0),
			  MIN ((r + 1) / 2 - x, (r + 1) / 2 + x)));
}

static int
box_width (double r)
{
    return r < 1.0 ? 2 : ceil(r + 1);
}

/* PIXMAN_KERNEL_LINEAR: Weighted sum of the two pixels nearest the
   center, or a triangle of width 2. This matches
   PIXMAN_FILTER_BILINEAR. This is useful if you wish to combine the
   result of bilinear in one direction with another filter in the
   other.  This is not a good filter if r > 1. You may actually want
   PIXMAN_FILTER_TENT.

   When r == 1.0, PIXMAN_KERNEL_BOX, PIXMAN_KERNEL_LINEAR, and
   PIXMAN_KERNEL_TENT all produce the same filter, allowing
   them to be exchanged at this point.
*/

static double
linear_kernel (double x, double r)
{
    return MAX (1.0 - fabs(x), 0.0);
}

static int
linear_width (double r)
{
    return 2;
}

/* Cubic functions described in the Mitchell-Netravali paper.
   http://mentallandscape.com/Papers_siggraph88.pdf. This describes
   all possible cubic functions that can be used for sampling.
*/

static double
general_cubic (double x, double r, double B, double C)
{
    double ax;
    if (r < 1.0)
	return
	    general_cubic(x * 2 - .5, r * 2, B, C) +
	    general_cubic(x * 2 + .5, r * 2, B, C);

    ax = fabs (x / r);

    if (ax < 1)
    {
	return (((12 - 9 * B - 6 * C) * ax +
		 (-18 + 12 * B + 6 * C)) * ax * ax +
		(6 - 2 * B)) / 6;
    }
    else if (ax < 2)
    {
	return ((((-B - 6 * C) * ax +
		 (6 * B + 30 * C)) * ax +
		(-12 * B - 48 * C)) * ax +
		(8 * B + 24 * C)) / 6;
    }
    else
    {
	return 0.0;
    }
}

static int
cubic_width (double r)
{
    return MAX (2, ceil (r * 4));
}

/* PIXMAN_KERNEL_CATMULL_ROM: Catmull-Rom interpolation. Often called
   "cubic interpolation", "b-spline", or just "cubic" by other
   software. This filter has negative values so it can produce ringing
   and output pixels outside the range of input pixels. This is very
   close to lanczos2 so there is no reason to supply that as well.
*/

static double
cubic_kernel (double x, double r)
{
    return general_cubic (x, r, 0.0, 0.5);
}

/* PIXMAN_KERNEL_MITCHELL: Cubic recommended by the Mitchell-Netravali
   paper.  This has negative values and because the values at +/-1 are
   not zero it does not interpolate the pixels, meaning it will change
   an image even if there is no translation.
*/

static double
mitchell_kernel (double x, double r)
{
    return general_cubic (x, r, 1/3.0, 1/3.0);
}

/* PIXMAN_KERNEL_NOTCH: Cubic recommended by the Mitchell-Netravali
   paper to remove postaliasing artifacts. This does not remove
   aliasing already present in the source image, though it may appear
   to due to it's excessive blurriness. In any case this is more
   useful than gaussian for image reconstruction.
*/

static double
notch_kernel (double x, double r)
{
    return general_cubic (x, r, 1.5, -0.25);
}

/* PIXMAN_KERNEL_LANCZOS3: lanczos windowed sinc function from -3 to
   +3. Very popular with high-end software though I think any
   advantage over cubics is hidden by quantization and programming
   mistakes. You will see LANCZOS5 or even 7 sometimes.
*/

static double
sinc (double x)
{
    return x ? sin (M_PI * x) / (M_PI * x) : 1.0;
}

static double
lanczos (double x, double n)
{
    return fabs (x) < n ? sinc (x) * sinc (x * (1.0 / n)) : 0.0;
}

static double
lanczos3_kernel (double x, double r)
{
    if (r < 1.0)
	return
	    lanczos3_kernel (x * 2 - .5, r * 2) +
	    lanczos3_kernel (x * 2 + .5, r * 2);
    else
	return lanczos (x / r, 3.0);
}

static int
lanczos3_width (double r)
{
    return MAX (2, ceil (r * 6));
}

/* PIXMAN_KERNEL_LANCZOS3_STRETCHED - The LANCZOS3 kernel widened by
   4/3.  Recommended by Jim Blinn
   http://graphics.cs.cmu.edu/nsp/course/15-462/Fall07/462/papers/jaggy.pdf
*/

static double
nice_kernel (double x, double r)
{
    return lanczos3_kernel (x, r * (4.0/3));
}

static int
nice_width (double r)
{
    return MAX (2.0, ceil (r * 8));
}

/* PIXMAN_KERNEL_TENT: Triangle of width 2r. Lots of software uses
   this as a "better" filter, twice the size of a box but smaller than
   a cubic.

   When r == 1.0, PIXMAN_KERNEL_BOX, PIXMAN_KERNEL_LINEAR, and
   PIXMAN_KERNEL_TENT all produce the same filter, allowing
   them to be exchanged at this point.
*/

static double
tent_kernel (double x, double r)
{
    if (r < 1.0)
	return box_kernel(x, r);
    else
	return MAX (1.0 - fabs(x / r), 0.0);
}

static int
tent_width (double r)
{
    return r < 1.0 ? 2 : ceil(2 * r);
}


static const filter_info_t filters[] =
{
    { KERNEL_IMPULSE,		impulse_kernel,   impulse_width },
    { KERNEL_BOX,		box_kernel,       box_width },
    { KERNEL_LINEAR,		linear_kernel,    linear_width },
    { KERNEL_MITCHELL,		mitchell_kernel,  cubic_width },
    { KERNEL_NOTCH,		notch_kernel,     cubic_width },
    { KERNEL_CATMULL_ROM,	cubic_kernel,     cubic_width },
    { KERNEL_LANCZOS3,		lanczos3_kernel,  lanczos3_width },
    { KERNEL_LANCZOS3_STRETCHED,nice_kernel,      nice_width },
    { KERNEL_TENT,		tent_kernel,	  tent_width }
};

/* Fills in one dimension of the filter array */
static void get_filter(kernel_t filter, double r,
		       int width, int subsample,
		       pixman_fixed_t* out)
{
    int i;
    pixman_fixed_t *p = out;
    int n_phases = 1 << subsample;
    double step = 1.0 / n_phases;
    kernel_func_t func = filters[filter].func;

    /* special-case the impulse filter: */
    if (width <= 1)
    {
	for (i = 0; i < n_phases; ++i)
	    *p++ = pixman_fixed_1;
	return;
    }

    for (i = 0; i < n_phases; ++i)
    {
	double frac = (i + .5) * step;
	/* Center of left-most pixel: */
	double x1 = ceil (frac - width / 2.0 - 0.5) - frac + 0.5;
	double total = 0;
	pixman_fixed_t new_total = 0;
	int j;

	for (j = 0; j < width; ++j)
	{
	    double v = func(x1 + j, r);
	    total += v;
	    p[j] = pixman_double_to_fixed (v);
	}

	/* Normalize */
        total = 1 / total;
	for (j = 0; j < width; ++j)
	    new_total += (p[j] *= total);

	/* Put any error on center pixel */
	p[width / 2] += (pixman_fixed_1 - new_total);

	p += width;
    }
}


/* Create the parameter list for a SEPARABLE_CONVOLUTION filter
 * with the given kernels and scale parameters. 
 */
static pixman_fixed_t *
create_separable_convolution (int *n_values,
			      kernel_t xfilter,
			      double sx,
			      kernel_t yfilter,
			      double sy)
{
    int xwidth, xsubsample, ywidth, ysubsample, size_x, size_y;
    pixman_fixed_t *params;

    xwidth = filters[xfilter].width(sx);
    xsubsample = 0;
    if (xwidth > 1)
	while (sx * (1 << xsubsample) <= 128.0) xsubsample++;
    size_x = (1 << xsubsample) * xwidth;

    ywidth = filters[yfilter].width(sy);
    ysubsample = 0;
    if (ywidth > 1)
	while (sy * (1 << ysubsample) <= 128.0) ysubsample++;
    size_y = (1 << ysubsample) * ywidth;

    *n_values = 4 + size_x + size_y;
    params = _cairo_malloc (*n_values * sizeof (pixman_fixed_t));
    if (!params) return 0;

    params[0] = pixman_int_to_fixed (xwidth);
    params[1] = pixman_int_to_fixed (ywidth);
    params[2] = pixman_int_to_fixed (xsubsample);
    params[3] = pixman_int_to_fixed (ysubsample);

    get_filter(xfilter, sx, xwidth, xsubsample, params + 4);
    get_filter(yfilter, sy, ywidth, ysubsample, params + 4 + size_x);

    return params;
}

/* ========================================================================== */

static cairo_bool_t
_pixman_image_set_properties (pixman_image_t *pixman_image,
			      const cairo_pattern_t *pattern,
			      const cairo_rectangle_int_t *extents,
			      int *ix,int *iy)
{
    pixman_transform_t pixman_transform;
    cairo_int_status_t status;

    status = _cairo_matrix_to_pixman_matrix_offset (&pattern->matrix,
						    pattern->filter,
						    extents->x + extents->width/2.,
						    extents->y + extents->height/2.,
						    &pixman_transform, ix, iy);
    if (status == CAIRO_INT_STATUS_NOTHING_TO_DO)
    {
	/* If the transform is an identity, we don't need to set it
	 * and we can use any filtering, so choose the fastest one. */
	pixman_image_set_filter (pixman_image, PIXMAN_FILTER_NEAREST, NULL, 0);
    }
    else if (unlikely (status != CAIRO_INT_STATUS_SUCCESS ||
		       ! pixman_image_set_transform (pixman_image,
						     &pixman_transform)))
    {
	return FALSE;
    }
    else
    {
	pixman_filter_t pixman_filter;
	kernel_t kernel;
	double dx, dy;

	/* Compute scale factors from the pattern matrix. These scale
	 * factors are from user to pattern space, and as such they
	 * are greater than 1.0 for downscaling and less than 1.0 for
	 * upscaling. The factors are the size of an axis-aligned
	 * rectangle with the same area as the parallelgram a 1x1
	 * square transforms to.
	 */
	dx = hypot (pattern->matrix.xx, pattern->matrix.xy);
	dy = hypot (pattern->matrix.yx, pattern->matrix.yy);

	/* Clip at maximum pixman_fixed number. Besides making it
	 * passable to pixman, this avoids errors from inf and nan.
	 */
	if (! (dx < 0x7FFF)) dx = 0x7FFF;
	if (! (dy < 0x7FFF)) dy = 0x7FFF;

	switch (pattern->filter) {
	case CAIRO_FILTER_FAST:
	    pixman_filter = PIXMAN_FILTER_FAST;
	    break;
	case CAIRO_FILTER_GOOD:
	    pixman_filter = PIXMAN_FILTER_SEPARABLE_CONVOLUTION;
	    kernel = KERNEL_BOX;
	    /* Clip the filter size to prevent extreme slowness. This
	       value could be raised if 2-pass filtering is done */
	    if (dx > 16.0) dx = 16.0;
	    if (dy > 16.0) dy = 16.0;
	    /* Match the bilinear filter for scales > .75: */
	    if (dx < 1.0/0.75) dx = 1.0;
	    if (dy < 1.0/0.75) dy = 1.0;
	    break;
	case CAIRO_FILTER_BEST:
	    pixman_filter = PIXMAN_FILTER_SEPARABLE_CONVOLUTION;
	    kernel = KERNEL_CATMULL_ROM; /* LANCZOS3 is better but not much */
	    /* Clip the filter size to prevent extreme slowness. This
	       value could be raised if 2-pass filtering is done */
	    if (dx > 16.0) { dx = 16.0; kernel = KERNEL_BOX; }
	    /* blur up to 2x scale, then blend to square pixels for larger: */
	    else if (dx < 1.0) {
		if (dx < 1.0/128) dx = 1.0/127;
		else if (dx < 0.5) dx = 1.0 / (1.0 / dx - 1.0);
		else dx = 1.0;
	    }
	    if (dy > 16.0) { dy = 16.0; kernel = KERNEL_BOX; }
	    else if (dy < 1.0) {
		if (dy < 1.0/128) dy = 1.0/127;
		else if (dy < 0.5) dy = 1.0 / (1.0 / dy - 1.0);
		else dy = 1.0;
	    }
	    break;
	case CAIRO_FILTER_NEAREST:
	    pixman_filter = PIXMAN_FILTER_NEAREST;
	    break;
	case CAIRO_FILTER_BILINEAR:
	    pixman_filter = PIXMAN_FILTER_BILINEAR;
	    break;
	case CAIRO_FILTER_GAUSSIAN:
	    /* XXX: The GAUSSIAN value has no implementation in cairo
	     * whatsoever, so it was really a mistake to have it in the
	     * API. We could fix this by officially deprecating it, or
	     * else inventing semantics and providing an actual
	     * implementation for it. */
	default:
	    pixman_filter = PIXMAN_FILTER_BEST;
	}

	if (pixman_filter == PIXMAN_FILTER_SEPARABLE_CONVOLUTION) {
	    int n_params;
	    pixman_fixed_t *params;
	    params = create_separable_convolution
		(&n_params, kernel, dx, kernel, dy);
	    pixman_image_set_filter (pixman_image, pixman_filter,
				     params, n_params);
	    free (params);
	} else {
	    pixman_image_set_filter (pixman_image, pixman_filter, NULL, 0);
	}
    }

    {
	pixman_repeat_t pixman_repeat;

	switch (pattern->extend) {
	default:
	case CAIRO_EXTEND_NONE:
	    pixman_repeat = PIXMAN_REPEAT_NONE;
	    break;
	case CAIRO_EXTEND_REPEAT:
	    pixman_repeat = PIXMAN_REPEAT_NORMAL;
	    break;
	case CAIRO_EXTEND_REFLECT:
	    pixman_repeat = PIXMAN_REPEAT_REFLECT;
	    break;
	case CAIRO_EXTEND_PAD:
	    pixman_repeat = PIXMAN_REPEAT_PAD;
	    break;
	}

	pixman_image_set_repeat (pixman_image, pixman_repeat);
    }

    if (pattern->has_component_alpha)
	pixman_image_set_component_alpha (pixman_image, TRUE);

    return TRUE;
}

struct proxy {
    cairo_surface_t base;
    cairo_surface_t *image;
};

static cairo_status_t
proxy_acquire_source_image (void			 *abstract_surface,
			    cairo_image_surface_t	**image_out,
			    void			**image_extra)
{
    struct proxy *proxy = abstract_surface;
    return _cairo_surface_acquire_source_image (proxy->image, image_out, image_extra);
}

static void
proxy_release_source_image (void			*abstract_surface,
			    cairo_image_surface_t	*image,
			    void			*image_extra)
{
    struct proxy *proxy = abstract_surface;
    _cairo_surface_release_source_image (proxy->image, image, image_extra);
}

static cairo_status_t
proxy_finish (void *abstract_surface)
{
    return CAIRO_STATUS_SUCCESS;
}

static const cairo_surface_backend_t proxy_backend  = {
    CAIRO_INTERNAL_SURFACE_TYPE_NULL,
    proxy_finish,
    NULL,

    NULL, /* create similar */
    NULL, /* create similar image */
    NULL, /* map to image */
    NULL, /* unmap image */

    _cairo_surface_default_source,
    proxy_acquire_source_image,
    proxy_release_source_image,
};

static cairo_surface_t *
attach_proxy (cairo_surface_t *source,
	      cairo_surface_t *image)
{
    struct proxy *proxy;

    proxy = _cairo_malloc (sizeof (*proxy));
    if (unlikely (proxy == NULL))
	return _cairo_surface_create_in_error (CAIRO_STATUS_NO_MEMORY);

    _cairo_surface_init (&proxy->base, &proxy_backend, NULL, image->content, FALSE);

    proxy->image = image;
    _cairo_surface_attach_snapshot (source, &proxy->base, NULL);

    return &proxy->base;
}

static void
detach_proxy (cairo_surface_t *source,
	      cairo_surface_t *proxy)
{
    cairo_surface_finish (proxy);
    cairo_surface_destroy (proxy);
}

static cairo_surface_t *
get_proxy (cairo_surface_t *proxy)
{
    return ((struct proxy *)proxy)->image;
}

static pixman_image_t *
_pixman_image_for_recording (cairo_image_surface_t *dst,
			     const cairo_surface_pattern_t *pattern,
			     cairo_bool_t is_mask,
			     const cairo_rectangle_int_t *extents,
			     const cairo_rectangle_int_t *sample,
			     int *ix, int *iy)
{
    cairo_surface_t *source, *clone, *proxy;
    cairo_rectangle_int_t limit;
    cairo_rectangle_int_t src_limit;
    pixman_image_t *pixman_image;
    cairo_status_t status;
    cairo_extend_t extend;
    cairo_matrix_t *m, matrix;
    double sx = 1.0, sy = 1.0;
    int tx = 0, ty = 0;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    *ix = *iy = 0;

    source = _cairo_pattern_get_source (pattern, &limit);
    src_limit = limit;

    extend = pattern->base.extend;
    if (_cairo_rectangle_contains_rectangle (&limit, sample))
	extend = CAIRO_EXTEND_NONE;

    if (extend == CAIRO_EXTEND_NONE) {
	if (! _cairo_rectangle_intersect (&limit, sample))
	    return _pixman_transparent_image ();
    }

    if (! _cairo_matrix_is_identity (&pattern->base.matrix)) {
	double x1, y1, x2, y2;

	matrix = pattern->base.matrix;
	status = cairo_matrix_invert (&matrix);
	assert (status == CAIRO_STATUS_SUCCESS);

	x1 = limit.x;
	y1 = limit.y;
	x2 = limit.x + limit.width;
	y2 = limit.y + limit.height;

	_cairo_matrix_transform_bounding_box (&matrix,
					      &x1, &y1, &x2, &y2, NULL);

	limit.x = floor (x1);
	limit.y = floor (y1);
	limit.width  = ceil (x2) - limit.x;
	limit.height = ceil (y2) - limit.y;
	sx = (double)src_limit.width / limit.width;
	sy = (double)src_limit.height / limit.height;
    }
    tx = limit.x;
    ty = limit.y;

    /* XXX transformations! */
    proxy = _cairo_surface_has_snapshot (source, &proxy_backend);
    if (proxy != NULL) {
	clone = cairo_surface_reference (get_proxy (proxy));
	goto done;
    }

    if (is_mask) {
	    clone = cairo_image_surface_create (CAIRO_FORMAT_A8,
						limit.width, limit.height);
    } else {
	if (dst->base.content == source->content)
	    clone = cairo_image_surface_create (dst->format,
						limit.width, limit.height);
	else
	    clone = _cairo_image_surface_create_with_content (source->content,
							      limit.width,
							      limit.height);
	if (dst->base.foreground_source)
	    clone->foreground_source = cairo_pattern_reference (dst->base.foreground_source);
    }

    m = NULL;
    if (extend == CAIRO_EXTEND_NONE) {
	matrix = pattern->base.matrix;
	if (tx | ty)
	    cairo_matrix_translate (&matrix, tx, ty);
	m = &matrix;
    } else {
	cairo_matrix_init_scale (&matrix, sx, sy);
	cairo_matrix_translate (&matrix, src_limit.x/sx, src_limit.y/sy);
	m = &matrix;
    }

    /* Handle recursion by returning future reads from the current image */
    proxy = attach_proxy (source, clone);
    status = _cairo_recording_surface_replay_with_clip (source, m, clone, NULL, FALSE);
    if (clone->foreground_used)
	dst->base.foreground_used = clone->foreground_used;
    detach_proxy (source, proxy);
    if (unlikely (status)) {
	cairo_surface_destroy (clone);
	return NULL;
    }

done:
    pixman_image = pixman_image_ref (((cairo_image_surface_t *)clone)->pixman_image);
    cairo_surface_destroy (clone);

    if (extend == CAIRO_EXTEND_NONE) {
	*ix = -limit.x;
	*iy = -limit.y;
    } else {
	cairo_pattern_union_t tmp_pattern;
	_cairo_pattern_init_static_copy (&tmp_pattern.base, &pattern->base);
	matrix = pattern->base.matrix;
	status = cairo_matrix_invert(&matrix);
	assert (status == CAIRO_STATUS_SUCCESS);
	cairo_matrix_translate (&matrix, src_limit.x, src_limit.y);
	cairo_matrix_scale (&matrix, sx, sy);
	status = cairo_matrix_invert(&matrix);
	assert (status == CAIRO_STATUS_SUCCESS);
	cairo_pattern_set_matrix (&tmp_pattern.base, &matrix);
	if (! _pixman_image_set_properties (pixman_image,
					    &tmp_pattern.base, extents,
					    ix, iy)) {
	    pixman_image_unref (pixman_image);
	    pixman_image= NULL;
	}
    }

    return pixman_image;
}

static pixman_image_t *
_pixman_image_for_surface (cairo_image_surface_t *dst,
			   const cairo_surface_pattern_t *pattern,
			   cairo_bool_t is_mask,
			   const cairo_rectangle_int_t *extents,
			   const cairo_rectangle_int_t *sample,
			   int *ix, int *iy)
{
    cairo_extend_t extend = pattern->base.extend;
    pixman_image_t *pixman_image;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    *ix = *iy = 0;
    pixman_image = NULL;
    if (pattern->surface->type == CAIRO_SURFACE_TYPE_RECORDING)
	return _pixman_image_for_recording(dst, pattern,
					   is_mask, extents, sample,
					   ix, iy);

    if (pattern->surface->type == CAIRO_SURFACE_TYPE_IMAGE &&
	(! is_mask || ! pattern->base.has_component_alpha ||
	 (pattern->surface->content & CAIRO_CONTENT_COLOR) == 0))
    {
	cairo_surface_t *defer_free = NULL;
	cairo_image_surface_t *source = (cairo_image_surface_t *) pattern->surface;
	cairo_surface_type_t type;

	if (_cairo_surface_is_snapshot (&source->base)) {
	    defer_free = _cairo_surface_snapshot_get_target (&source->base);
	    source = (cairo_image_surface_t *) defer_free;
	}

	type = source->base.backend->type;
	if (type == CAIRO_SURFACE_TYPE_IMAGE) {
	    if (extend != CAIRO_EXTEND_NONE &&
		sample->x >= 0 &&
		sample->y >= 0 &&
		sample->x + sample->width  <= source->width &&
		sample->y + sample->height <= source->height)
	    {
		extend = CAIRO_EXTEND_NONE;
	    }

	    if (sample->width == 1 && sample->height == 1) {
		if (sample->x < 0 ||
		    sample->y < 0 ||
		    sample->x >= source->width ||
		    sample->y >= source->height)
		{
		    if (extend == CAIRO_EXTEND_NONE) {
			cairo_surface_destroy (defer_free);
			return _pixman_transparent_image ();
		    }
		}
		else
		{
		    pixman_image = _pixel_to_solid (source,
						    sample->x, sample->y);
                    if (pixman_image) {
			cairo_surface_destroy (defer_free);
                        return pixman_image;
		    }
		}
	    }

#if PIXMAN_HAS_ATOMIC_OPS
	    /* avoid allocating a 'pattern' image if we can reuse the original */
	    if (extend == CAIRO_EXTEND_NONE &&
		_cairo_matrix_is_pixman_translation (&pattern->base.matrix,
						     pattern->base.filter,
						     ix, iy))
	    {
		cairo_surface_destroy (defer_free);
		return pixman_image_ref (source->pixman_image);
	    }
#endif

	    pixman_image = pixman_image_create_bits (source->pixman_format,
						     source->width,
						     source->height,
						     (uint32_t *) source->data,
						     source->stride);
	    if (unlikely (pixman_image == NULL)) {
		cairo_surface_destroy (defer_free);
		return NULL;
	    }

	    if (defer_free) {
		pixman_image_set_destroy_function (pixman_image,
						   _defer_free_cleanup,
						   defer_free);
	    }
	} else if (type == CAIRO_SURFACE_TYPE_SUBSURFACE) {
	    cairo_surface_subsurface_t *sub;
	    cairo_bool_t is_contained = FALSE;

	    sub = (cairo_surface_subsurface_t *) source;
	    source = (cairo_image_surface_t *) sub->target;

	    if (sample->x >= 0 &&
		sample->y >= 0 &&
		sample->x + sample->width  <= sub->extents.width &&
		sample->y + sample->height <= sub->extents.height)
	    {
		is_contained = TRUE;
	    }

	    if (sample->width == 1 && sample->height == 1) {
		if (is_contained) {
		    pixman_image = _pixel_to_solid (source,
                                                    sub->extents.x + sample->x,
                                                    sub->extents.y + sample->y);
                    if (pixman_image)
                        return pixman_image;
		} else {
		    if (extend == CAIRO_EXTEND_NONE)
			return _pixman_transparent_image ();
		}
	    }

#if PIXMAN_HAS_ATOMIC_OPS
	    *ix = sub->extents.x;
	    *iy = sub->extents.y;
	    if (is_contained &&
		_cairo_matrix_is_pixman_translation (&pattern->base.matrix,
						     pattern->base.filter,
						     ix, iy))
	    {
		return pixman_image_ref (source->pixman_image);
	    }
#endif

	    /* Avoid sub-byte offsets, force a copy in that case. */
	    if (PIXMAN_FORMAT_BPP (source->pixman_format) >= 8) {
		if (is_contained) {
		    void *data = source->data
			+ sub->extents.x * PIXMAN_FORMAT_BPP(source->pixman_format)/8
			+ sub->extents.y * source->stride;
		    pixman_image = pixman_image_create_bits (source->pixman_format,
							     sub->extents.width,
							     sub->extents.height,
							     data,
							     source->stride);
		    if (unlikely (pixman_image == NULL))
			return NULL;
		} else {
		    /* XXX for a simple translation and EXTEND_NONE we can
		     * fix up the pattern matrix instead.
		     */
		}
	    }
	}
    }

    if (pixman_image == NULL) {
	struct acquire_source_cleanup *cleanup;
	cairo_image_surface_t *image;
	void *extra;
	cairo_status_t status;

	status = _cairo_surface_acquire_source_image (pattern->surface, &image, &extra);
	if (unlikely (status))
	    return NULL;

	pixman_image = pixman_image_create_bits (image->pixman_format,
						 image->width,
						 image->height,
						 (uint32_t *) image->data,
						 image->stride);
	if (unlikely (pixman_image == NULL)) {
	    _cairo_surface_release_source_image (pattern->surface, image, extra);
	    return NULL;
	}

	cleanup = _cairo_malloc (sizeof (*cleanup));
	if (unlikely (cleanup == NULL)) {
	    _cairo_surface_release_source_image (pattern->surface, image, extra);
	    pixman_image_unref (pixman_image);
	    return NULL;
	}

	cleanup->surface = pattern->surface;
	cleanup->image = image;
	cleanup->image_extra = extra;
	pixman_image_set_destroy_function (pixman_image,
					   _acquire_source_cleanup, cleanup);
    }

    if (! _pixman_image_set_properties (pixman_image,
					&pattern->base, extents,
					ix, iy)) {
	pixman_image_unref (pixman_image);
	pixman_image= NULL;
    }

    return pixman_image;
}

struct raster_source_cleanup {
    const cairo_pattern_t *pattern;
    cairo_surface_t *surface;
    cairo_image_surface_t *image;
    void *image_extra;
};

static void
_raster_source_cleanup (pixman_image_t *pixman_image,
			void *closure)
{
    struct raster_source_cleanup *data = closure;

    _cairo_surface_release_source_image (data->surface,
					 data->image,
					 data->image_extra);

    _cairo_raster_source_pattern_release (data->pattern,
					  data->surface);

    free (data);
}

static pixman_image_t *
_pixman_image_for_raster (cairo_image_surface_t *dst,
			  const cairo_raster_source_pattern_t *pattern,
			  cairo_bool_t is_mask,
			  const cairo_rectangle_int_t *extents,
			  const cairo_rectangle_int_t *sample,
			  int *ix, int *iy)
{
    pixman_image_t *pixman_image;
    struct raster_source_cleanup *cleanup;
    cairo_image_surface_t *image;
    void *extra;
    cairo_status_t status;
    cairo_surface_t *surface;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    *ix = *iy = 0;

    surface = _cairo_raster_source_pattern_acquire (&pattern->base,
						    &dst->base, NULL);
    if (unlikely (surface == NULL || surface->status))
	return NULL;

    status = _cairo_surface_acquire_source_image (surface, &image, &extra);
    if (unlikely (status)) {
	_cairo_raster_source_pattern_release (&pattern->base, surface);
	return NULL;
    }

    assert (image->width == pattern->extents.width);
    assert (image->height == pattern->extents.height);

    pixman_image = pixman_image_create_bits (image->pixman_format,
					     image->width,
					     image->height,
					     (uint32_t *) image->data,
					     image->stride);
    if (unlikely (pixman_image == NULL)) {
	_cairo_surface_release_source_image (surface, image, extra);
	_cairo_raster_source_pattern_release (&pattern->base, surface);
	return NULL;
    }

    cleanup = _cairo_malloc (sizeof (*cleanup));
    if (unlikely (cleanup == NULL)) {
	pixman_image_unref (pixman_image);
	_cairo_surface_release_source_image (surface, image, extra);
	_cairo_raster_source_pattern_release (&pattern->base, surface);
	return NULL;
    }

    cleanup->pattern = &pattern->base;
    cleanup->surface = surface;
    cleanup->image = image;
    cleanup->image_extra = extra;
    pixman_image_set_destroy_function (pixman_image,
				       _raster_source_cleanup, cleanup);

    if (! _pixman_image_set_properties (pixman_image,
					&pattern->base, extents,
					ix, iy)) {
	pixman_image_unref (pixman_image);
	pixman_image= NULL;
    }

    return pixman_image;
}

pixman_image_t *
_pixman_image_for_pattern (cairo_image_surface_t *dst,
			   const cairo_pattern_t *pattern,
			   cairo_bool_t is_mask,
			   const cairo_rectangle_int_t *extents,
			   const cairo_rectangle_int_t *sample,
			   int *tx, int *ty)
{
    *tx = *ty = 0;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    if (pattern == NULL)
	return _pixman_white_image ();

    switch (pattern->type) {
    default:
	ASSERT_NOT_REACHED;
    case CAIRO_PATTERN_TYPE_SOLID:
	return _pixman_image_for_color (&((const cairo_solid_pattern_t *) pattern)->color);

    case CAIRO_PATTERN_TYPE_RADIAL:
    case CAIRO_PATTERN_TYPE_LINEAR:
	return _pixman_image_for_gradient ((const cairo_gradient_pattern_t *) pattern,
					   extents, tx, ty);

    case CAIRO_PATTERN_TYPE_MESH:
	return _pixman_image_for_mesh ((const cairo_mesh_pattern_t *) pattern,
					   extents, tx, ty);

    case CAIRO_PATTERN_TYPE_SURFACE:
	return _pixman_image_for_surface (dst,
					  (const cairo_surface_pattern_t *) pattern,
					  is_mask, extents, sample,
					  tx, ty);

    case CAIRO_PATTERN_TYPE_RASTER_SOURCE:
	return _pixman_image_for_raster (dst,
					 (const cairo_raster_source_pattern_t *) pattern,
					 is_mask, extents, sample,
					 tx, ty);
    }
}

static cairo_status_t
_cairo_image_source_finish (void *abstract_surface)
{
    cairo_image_source_t *source = abstract_surface;

    pixman_image_unref (source->pixman_image);
    return CAIRO_STATUS_SUCCESS;
}

const cairo_surface_backend_t _cairo_image_source_backend = {
    CAIRO_SURFACE_TYPE_IMAGE,
    _cairo_image_source_finish,
    NULL, /* read-only wrapper */
};

cairo_surface_t *
_cairo_image_source_create_for_pattern (cairo_surface_t *dst,
					 const cairo_pattern_t *pattern,
					 cairo_bool_t is_mask,
					 const cairo_rectangle_int_t *extents,
					 const cairo_rectangle_int_t *sample,
					 int *src_x, int *src_y)
{
    cairo_image_source_t *source;

    TRACE ((stderr, "%s\n", __FUNCTION__));

    source = _cairo_malloc (sizeof (cairo_image_source_t));
    if (unlikely (source == NULL))
	return _cairo_surface_create_in_error (_cairo_error (CAIRO_STATUS_NO_MEMORY));

    source->pixman_image =
	_pixman_image_for_pattern ((cairo_image_surface_t *)dst,
				   pattern, is_mask,
				   extents, sample,
				   src_x, src_y);
    if (unlikely (source->pixman_image == NULL)) {
	free (source);
	return _cairo_surface_create_in_error (CAIRO_STATUS_NO_MEMORY);
    }

    _cairo_surface_init (&source->base,
			 &_cairo_image_source_backend,
			 NULL, /* device */
			 CAIRO_CONTENT_COLOR_ALPHA,
			 FALSE); /* is_vector */

    source->is_opaque_solid =
	pattern == NULL || _cairo_pattern_is_opaque_solid (pattern);

    return &source->base;
}
