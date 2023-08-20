/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
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

#include "cairoint.h"
#include "cairo-error-private.h"
#include <float.h>

#define PIXMAN_MAX_INT ((pixman_fixed_1 >> 1) - pixman_fixed_e) /* need to ensure deltas also fit */

/**
 * SECTION:cairo-matrix
 * @Title: cairo_matrix_t
 * @Short_Description: Generic matrix operations
 * @See_Also: #cairo_t
 *
 * #cairo_matrix_t is used throughout cairo to convert between different
 * coordinate spaces.  A #cairo_matrix_t holds an affine transformation,
 * such as a scale, rotation, shear, or a combination of these.
 * The transformation of a point (<literal>x</literal>,<literal>y</literal>)
 * is given by:
 *
 * <programlisting>
 * x_new = xx * x + xy * y + x0;
 * y_new = yx * x + yy * y + y0;
 * </programlisting>
 *
 * The current transformation matrix of a #cairo_t, represented as a
 * #cairo_matrix_t, defines the transformation from user-space
 * coordinates to device-space coordinates. See cairo_get_matrix() and
 * cairo_set_matrix().
 **/

static void
_cairo_matrix_scalar_multiply (cairo_matrix_t *matrix, double scalar);

static void
_cairo_matrix_compute_adjoint (cairo_matrix_t *matrix);

/**
 * cairo_matrix_init_identity:
 * @matrix: a #cairo_matrix_t
 *
 * Modifies @matrix to be an identity transformation.
 *
 * Since: 1.0
 **/
void
cairo_matrix_init_identity (cairo_matrix_t *matrix)
{
    cairo_matrix_init (matrix,
		       1, 0,
		       0, 1,
		       0, 0);
}
slim_hidden_def(cairo_matrix_init_identity);

/**
 * cairo_matrix_init:
 * @matrix: a #cairo_matrix_t
 * @xx: xx component of the affine transformation
 * @yx: yx component of the affine transformation
 * @xy: xy component of the affine transformation
 * @yy: yy component of the affine transformation
 * @x0: X translation component of the affine transformation
 * @y0: Y translation component of the affine transformation
 *
 * Sets @matrix to be the affine transformation given by
 * @xx, @yx, @xy, @yy, @x0, @y0. The transformation is given
 * by:
 * <programlisting>
 *  x_new = xx * x + xy * y + x0;
 *  y_new = yx * x + yy * y + y0;
 * </programlisting>
 *
 * Since: 1.0
 **/
void
cairo_matrix_init (cairo_matrix_t *matrix,
		   double xx, double yx,

		   double xy, double yy,
		   double x0, double y0)
{
    matrix->xx = xx; matrix->yx = yx;
    matrix->xy = xy; matrix->yy = yy;
    matrix->x0 = x0; matrix->y0 = y0;
}
slim_hidden_def(cairo_matrix_init);

/**
 * _cairo_matrix_get_affine:
 * @matrix: a #cairo_matrix_t
 * @xx: location to store xx component of matrix
 * @yx: location to store yx component of matrix
 * @xy: location to store xy component of matrix
 * @yy: location to store yy component of matrix
 * @x0: location to store x0 (X-translation component) of matrix, or %NULL
 * @y0: location to store y0 (Y-translation component) of matrix, or %NULL
 *
 * Gets the matrix values for the affine transformation that @matrix represents.
 * See cairo_matrix_init().
 *
 *
 * This function is a leftover from the old public API, but is still
 * mildly useful as an internal means for getting at the matrix
 * members in a positional way. For example, when reassigning to some
 * external matrix type, or when renaming members to more meaningful
 * names (such as a,b,c,d,e,f) for particular manipulations.
 **/
void
_cairo_matrix_get_affine (const cairo_matrix_t *matrix,
			  double *xx, double *yx,
			  double *xy, double *yy,
			  double *x0, double *y0)
{
    *xx  = matrix->xx;
    *yx  = matrix->yx;

    *xy  = matrix->xy;
    *yy  = matrix->yy;

    if (x0)
	*x0 = matrix->x0;
    if (y0)
	*y0 = matrix->y0;
}

/**
 * cairo_matrix_init_translate:
 * @matrix: a #cairo_matrix_t
 * @tx: amount to translate in the X direction
 * @ty: amount to translate in the Y direction
 *
 * Initializes @matrix to a transformation that translates by @tx and
 * @ty in the X and Y dimensions, respectively.
 *
 * Since: 1.0
 **/
void
cairo_matrix_init_translate (cairo_matrix_t *matrix,
			     double tx, double ty)
{
    cairo_matrix_init (matrix,
		       1, 0,
		       0, 1,
		       tx, ty);
}
slim_hidden_def(cairo_matrix_init_translate);

/**
 * cairo_matrix_translate:
 * @matrix: a #cairo_matrix_t
 * @tx: amount to translate in the X direction
 * @ty: amount to translate in the Y direction
 *
 * Applies a translation by @tx, @ty to the transformation in
 * @matrix. The effect of the new transformation is to first translate
 * the coordinates by @tx and @ty, then apply the original transformation
 * to the coordinates.
 *
 * Since: 1.0
 **/
void
cairo_matrix_translate (cairo_matrix_t *matrix, double tx, double ty)
{
    cairo_matrix_t tmp;

    cairo_matrix_init_translate (&tmp, tx, ty);

    cairo_matrix_multiply (matrix, &tmp, matrix);
}
slim_hidden_def (cairo_matrix_translate);

/**
 * cairo_matrix_init_scale:
 * @matrix: a #cairo_matrix_t
 * @sx: scale factor in the X direction
 * @sy: scale factor in the Y direction
 *
 * Initializes @matrix to a transformation that scales by @sx and @sy
 * in the X and Y dimensions, respectively.
 *
 * Since: 1.0
 **/
void
cairo_matrix_init_scale (cairo_matrix_t *matrix,
			 double sx, double sy)
{
    cairo_matrix_init (matrix,
		       sx,  0,
		       0, sy,
		       0, 0);
}
slim_hidden_def(cairo_matrix_init_scale);

/**
 * cairo_matrix_scale:
 * @matrix: a #cairo_matrix_t
 * @sx: scale factor in the X direction
 * @sy: scale factor in the Y direction
 *
 * Applies scaling by @sx, @sy to the transformation in @matrix. The
 * effect of the new transformation is to first scale the coordinates
 * by @sx and @sy, then apply the original transformation to the coordinates.
 *
 * Since: 1.0
 **/
void
cairo_matrix_scale (cairo_matrix_t *matrix, double sx, double sy)
{
    cairo_matrix_t tmp;

    cairo_matrix_init_scale (&tmp, sx, sy);

    cairo_matrix_multiply (matrix, &tmp, matrix);
}
slim_hidden_def(cairo_matrix_scale);

/**
 * cairo_matrix_init_rotate:
 * @matrix: a #cairo_matrix_t
 * @radians: angle of rotation, in radians. The direction of rotation
 * is defined such that positive angles rotate in the direction from
 * the positive X axis toward the positive Y axis. With the default
 * axis orientation of cairo, positive angles rotate in a clockwise
 * direction.
 *
 * Initialized @matrix to a transformation that rotates by @radians.
 *
 * Since: 1.0
 **/
void
cairo_matrix_init_rotate (cairo_matrix_t *matrix,
			  double radians)
{
    double  s;
    double  c;

    s = sin (radians);
    c = cos (radians);

    cairo_matrix_init (matrix,
		       c, s,
		       -s, c,
		       0, 0);
}
slim_hidden_def(cairo_matrix_init_rotate);

/**
 * cairo_matrix_rotate:
 * @matrix: a #cairo_matrix_t
 * @radians: angle of rotation, in radians. The direction of rotation
 * is defined such that positive angles rotate in the direction from
 * the positive X axis toward the positive Y axis. With the default
 * axis orientation of cairo, positive angles rotate in a clockwise
 * direction.
 *
 * Applies rotation by @radians to the transformation in
 * @matrix. The effect of the new transformation is to first rotate the
 * coordinates by @radians, then apply the original transformation
 * to the coordinates.
 *
 * Since: 1.0
 **/
void
cairo_matrix_rotate (cairo_matrix_t *matrix, double radians)
{
    cairo_matrix_t tmp;

    cairo_matrix_init_rotate (&tmp, radians);

    cairo_matrix_multiply (matrix, &tmp, matrix);
}
slim_hidden_def (cairo_matrix_rotate);

/**
 * cairo_matrix_multiply:
 * @result: a #cairo_matrix_t in which to store the result
 * @a: a #cairo_matrix_t
 * @b: a #cairo_matrix_t
 *
 * Multiplies the affine transformations in @a and @b together
 * and stores the result in @result. The effect of the resulting
 * transformation is to first apply the transformation in @a to the
 * coordinates and then apply the transformation in @b to the
 * coordinates.
 *
 * It is allowable for @result to be identical to either @a or @b.
 *
 * Since: 1.0
 **/
/*
 * XXX: The ordering of the arguments to this function corresponds
 *      to [row_vector]*A*B. If we want to use column vectors instead,
 *      then we need to switch the two arguments and fix up all
 *      uses.
 */
void
cairo_matrix_multiply (cairo_matrix_t *result, const cairo_matrix_t *a, const cairo_matrix_t *b)
{
    cairo_matrix_t r;

    r.xx = a->xx * b->xx + a->yx * b->xy;
    r.yx = a->xx * b->yx + a->yx * b->yy;

    r.xy = a->xy * b->xx + a->yy * b->xy;
    r.yy = a->xy * b->yx + a->yy * b->yy;

    r.x0 = a->x0 * b->xx + a->y0 * b->xy + b->x0;
    r.y0 = a->x0 * b->yx + a->y0 * b->yy + b->y0;

    *result = r;
}
slim_hidden_def(cairo_matrix_multiply);

void
_cairo_matrix_multiply (cairo_matrix_t *r,
			const cairo_matrix_t *a,
			const cairo_matrix_t *b)
{
    r->xx = a->xx * b->xx + a->yx * b->xy;
    r->yx = a->xx * b->yx + a->yx * b->yy;

    r->xy = a->xy * b->xx + a->yy * b->xy;
    r->yy = a->xy * b->yx + a->yy * b->yy;

    r->x0 = a->x0 * b->xx + a->y0 * b->xy + b->x0;
    r->y0 = a->x0 * b->yx + a->y0 * b->yy + b->y0;
}

/**
 * cairo_matrix_transform_distance:
 * @matrix: a #cairo_matrix_t
 * @dx: X component of a distance vector. An in/out parameter
 * @dy: Y component of a distance vector. An in/out parameter
 *
 * Transforms the distance vector (@dx,@dy) by @matrix. This is
 * similar to cairo_matrix_transform_point() except that the translation
 * components of the transformation are ignored. The calculation of
 * the returned vector is as follows:
 *
 * <programlisting>
 * dx2 = dx1 * a + dy1 * c;
 * dy2 = dx1 * b + dy1 * d;
 * </programlisting>
 *
 * Affine transformations are position invariant, so the same vector
 * always transforms to the same vector. If (@x1,@y1) transforms
 * to (@x2,@y2) then (@x1+@dx1,@y1+@dy1) will transform to
 * (@x1+@dx2,@y1+@dy2) for all values of @x1 and @x2.
 *
 * Since: 1.0
 **/
void
cairo_matrix_transform_distance (const cairo_matrix_t *matrix, double *dx, double *dy)
{
    double new_x, new_y;

    new_x = (matrix->xx * *dx + matrix->xy * *dy);
    new_y = (matrix->yx * *dx + matrix->yy * *dy);

    *dx = new_x;
    *dy = new_y;
}
slim_hidden_def(cairo_matrix_transform_distance);

/**
 * cairo_matrix_transform_point:
 * @matrix: a #cairo_matrix_t
 * @x: X position. An in/out parameter
 * @y: Y position. An in/out parameter
 *
 * Transforms the point (@x, @y) by @matrix.
 *
 * Since: 1.0
 **/
void
cairo_matrix_transform_point (const cairo_matrix_t *matrix, double *x, double *y)
{
    cairo_matrix_transform_distance (matrix, x, y);

    *x += matrix->x0;
    *y += matrix->y0;
}
slim_hidden_def(cairo_matrix_transform_point);

void
_cairo_matrix_transform_bounding_box (const cairo_matrix_t *matrix,
				      double *x1, double *y1,
				      double *x2, double *y2,
				      cairo_bool_t *is_tight)
{
    int i;
    double quad_x[4], quad_y[4];
    double min_x, max_x;
    double min_y, max_y;

    if (matrix->xy == 0. && matrix->yx == 0.) {
	/* non-rotation/skew matrix, just map the two extreme points */

	if (matrix->xx != 1.) {
	    quad_x[0] = *x1 * matrix->xx;
	    quad_x[1] = *x2 * matrix->xx;
	    if (quad_x[0] < quad_x[1]) {
		*x1 = quad_x[0];
		*x2 = quad_x[1];
	    } else {
		*x1 = quad_x[1];
		*x2 = quad_x[0];
	    }
	}
	if (matrix->x0 != 0.) {
	    *x1 += matrix->x0;
	    *x2 += matrix->x0;
	}

	if (matrix->yy != 1.) {
	    quad_y[0] = *y1 * matrix->yy;
	    quad_y[1] = *y2 * matrix->yy;
	    if (quad_y[0] < quad_y[1]) {
		*y1 = quad_y[0];
		*y2 = quad_y[1];
	    } else {
		*y1 = quad_y[1];
		*y2 = quad_y[0];
	    }
	}
	if (matrix->y0 != 0.) {
	    *y1 += matrix->y0;
	    *y2 += matrix->y0;
	}

	if (is_tight)
	    *is_tight = TRUE;

	return;
    }

    /* general matrix */
    quad_x[0] = *x1;
    quad_y[0] = *y1;
    cairo_matrix_transform_point (matrix, &quad_x[0], &quad_y[0]);

    quad_x[1] = *x2;
    quad_y[1] = *y1;
    cairo_matrix_transform_point (matrix, &quad_x[1], &quad_y[1]);

    quad_x[2] = *x1;
    quad_y[2] = *y2;
    cairo_matrix_transform_point (matrix, &quad_x[2], &quad_y[2]);

    quad_x[3] = *x2;
    quad_y[3] = *y2;
    cairo_matrix_transform_point (matrix, &quad_x[3], &quad_y[3]);

    min_x = max_x = quad_x[0];
    min_y = max_y = quad_y[0];

    for (i=1; i < 4; i++) {
	if (quad_x[i] < min_x)
	    min_x = quad_x[i];
	if (quad_x[i] > max_x)
	    max_x = quad_x[i];

	if (quad_y[i] < min_y)
	    min_y = quad_y[i];
	if (quad_y[i] > max_y)
	    max_y = quad_y[i];
    }

    *x1 = min_x;
    *y1 = min_y;
    *x2 = max_x;
    *y2 = max_y;

    if (is_tight) {
        /* it's tight if and only if the four corner points form an axis-aligned
           rectangle.
           And that's true if and only if we can derive corners 0 and 3 from
           corners 1 and 2 in one of two straightforward ways...
           We could use a tolerance here but for now we'll fall back to FALSE in the case
           of floating point error.
        */
        *is_tight =
            (quad_x[1] == quad_x[0] && quad_y[1] == quad_y[3] &&
             quad_x[2] == quad_x[3] && quad_y[2] == quad_y[0]) ||
            (quad_x[1] == quad_x[3] && quad_y[1] == quad_y[0] &&
             quad_x[2] == quad_x[0] && quad_y[2] == quad_y[3]);
    }
}

cairo_private void
_cairo_matrix_transform_bounding_box_fixed (const cairo_matrix_t *matrix,
					    cairo_box_t          *bbox,
					    cairo_bool_t *is_tight)
{
    double x1, y1, x2, y2;

    _cairo_box_to_doubles (bbox, &x1, &y1, &x2, &y2);
    _cairo_matrix_transform_bounding_box (matrix, &x1, &y1, &x2, &y2, is_tight);
    _cairo_box_from_doubles (bbox, &x1, &y1, &x2, &y2);
}

static void
_cairo_matrix_scalar_multiply (cairo_matrix_t *matrix, double scalar)
{
    matrix->xx *= scalar;
    matrix->yx *= scalar;

    matrix->xy *= scalar;
    matrix->yy *= scalar;

    matrix->x0 *= scalar;
    matrix->y0 *= scalar;
}

/* This function isn't a correct adjoint in that the implicit 1 in the
   homogeneous result should actually be ad-bc instead. But, since this
   adjoint is only used in the computation of the inverse, which
   divides by det (A)=ad-bc anyway, everything works out in the end. */
static void
_cairo_matrix_compute_adjoint (cairo_matrix_t *matrix)
{
    /* adj (A) = transpose (C:cofactor (A,i,j)) */
    double a, b, c, d, tx, ty;

    _cairo_matrix_get_affine (matrix,
			      &a,  &b,
			      &c,  &d,
			      &tx, &ty);

    cairo_matrix_init (matrix,
		       d, -b,
		       -c, a,
		       c*ty - d*tx, b*tx - a*ty);
}

/**
 * cairo_matrix_invert:
 * @matrix: a #cairo_matrix_t
 *
 * Changes @matrix to be the inverse of its original value. Not
 * all transformation matrices have inverses; if the matrix
 * collapses points together (it is <firstterm>degenerate</firstterm>),
 * then it has no inverse and this function will fail.
 *
 * Returns: If @matrix has an inverse, modifies @matrix to
 *  be the inverse matrix and returns %CAIRO_STATUS_SUCCESS. Otherwise,
 *  returns %CAIRO_STATUS_INVALID_MATRIX.
 *
 * Since: 1.0
 **/
cairo_status_t
cairo_matrix_invert (cairo_matrix_t *matrix)
{
    double det;

    /* Simple scaling|translation matrices are quite common... */
    if (matrix->xy == 0. && matrix->yx == 0.) {
	matrix->x0 = -matrix->x0;
	matrix->y0 = -matrix->y0;

	if (matrix->xx != 1.) {
	    if (matrix->xx == 0.)
		return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

	    matrix->xx = 1. / matrix->xx;
	    matrix->x0 *= matrix->xx;
	}

	if (matrix->yy != 1.) {
	    if (matrix->yy == 0.)
		return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

	    matrix->yy = 1. / matrix->yy;
	    matrix->y0 *= matrix->yy;
	}

	return CAIRO_STATUS_SUCCESS;
    }

    /* inv (A) = 1/det (A) * adj (A) */
    det = _cairo_matrix_compute_determinant (matrix);

    if (! ISFINITE (det))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    if (det == 0)
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    _cairo_matrix_compute_adjoint (matrix);
    _cairo_matrix_scalar_multiply (matrix, 1 / det);

    return CAIRO_STATUS_SUCCESS;
}
slim_hidden_def(cairo_matrix_invert);

cairo_bool_t
_cairo_matrix_is_invertible (const cairo_matrix_t *matrix)
{
    double det;

    det = _cairo_matrix_compute_determinant (matrix);

    return ISFINITE (det) && det != 0.;
}

cairo_bool_t
_cairo_matrix_is_scale_0 (const cairo_matrix_t *matrix)
{
    return matrix->xx == 0. &&
           matrix->xy == 0. &&
           matrix->yx == 0. &&
           matrix->yy == 0.;
}

double
_cairo_matrix_compute_determinant (const cairo_matrix_t *matrix)
{
    double a, b, c, d;

    a = matrix->xx; b = matrix->yx;
    c = matrix->xy; d = matrix->yy;

    return a*d - b*c;
}

/**
 * _cairo_matrix_compute_basis_scale_factors:
 * @matrix: a matrix
 * @basis_scale: the scale factor in the direction of basis
 * @normal_scale: the scale factor in the direction normal to the basis
 * @x_basis: basis to use.  X basis if true, Y basis otherwise.
 *
 * Computes |Mv| and det(M)/|Mv| for v=[1,0] if x_basis is true, and v=[0,1]
 * otherwise, and M is @matrix.
 *
 * Return value: the scale factor of @matrix on the height of the font,
 * or 1.0 if @matrix is %NULL.
 **/
cairo_status_t
_cairo_matrix_compute_basis_scale_factors (const cairo_matrix_t *matrix,
					   double *basis_scale, double *normal_scale,
					   cairo_bool_t x_basis)
{
    double det;

    det = _cairo_matrix_compute_determinant (matrix);

    if (! ISFINITE (det))
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);

    if (det == 0)
    {
	*basis_scale = *normal_scale = 0;
    }
    else
    {
	double x = x_basis != 0;
	double y = x == 0;
	double major, minor;

	cairo_matrix_transform_distance (matrix, &x, &y);
	major = hypot (x, y);
	/*
	 * ignore mirroring
	 */
	if (det < 0)
	    det = -det;
	if (major)
	    minor = det / major;
	else
	    minor = 0.0;
	if (x_basis)
	{
	    *basis_scale = major;
	    *normal_scale = minor;
	}
	else
	{
	    *basis_scale = minor;
	    *normal_scale = major;
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

cairo_bool_t
_cairo_matrix_is_integer_translation (const cairo_matrix_t *matrix,
				      int *itx, int *ity)
{
    if (_cairo_matrix_is_translation (matrix))
    {
        cairo_fixed_t x0_fixed = _cairo_fixed_from_double (matrix->x0);
        cairo_fixed_t y0_fixed = _cairo_fixed_from_double (matrix->y0);

        if (_cairo_fixed_is_integer (x0_fixed) &&
            _cairo_fixed_is_integer (y0_fixed))
        {
            if (itx)
                *itx = _cairo_fixed_integer_part (x0_fixed);
            if (ity)
                *ity = _cairo_fixed_integer_part (y0_fixed);

            return TRUE;
        }
    }

    return FALSE;
}

#define SCALING_EPSILON _cairo_fixed_to_double(1)

/* This only returns true if the matrix is 90 degree rotations or
 * flips. It appears calling code is relying on this. It will return
 * false for other rotations even if the scale is one. Approximations
 * are allowed to handle matricies filled in using trig functions
 * such as sin(M_PI_2).
 */
cairo_bool_t
_cairo_matrix_has_unity_scale (const cairo_matrix_t *matrix)
{
    /* check that the determinant is near +/-1 */
    double det = _cairo_matrix_compute_determinant (matrix);
    if (fabs (det * det - 1.0) < SCALING_EPSILON) {
	/* check that one axis is close to zero */
	if (fabs (matrix->xy) < SCALING_EPSILON  &&
	    fabs (matrix->yx) < SCALING_EPSILON)
	    return TRUE;
	if (fabs (matrix->xx) < SCALING_EPSILON  &&
	    fabs (matrix->yy) < SCALING_EPSILON)
	    return TRUE;
	/* If rotations are allowed then it must instead test for
	 * orthogonality. This is xx*xy+yx*yy ~= 0.
	 */
    }
    return FALSE;
}

/* By pixel exact here, we mean a matrix that is composed only of
 * 90 degree rotations, flips, and integer translations and produces a 1:1
 * mapping between source and destination pixels. If we transform an image
 * with a pixel-exact matrix, filtering is not useful.
 */
cairo_bool_t
_cairo_matrix_is_pixel_exact (const cairo_matrix_t *matrix)
{
    cairo_fixed_t x0_fixed, y0_fixed;

    if (! _cairo_matrix_has_unity_scale (matrix))
	return FALSE;

    x0_fixed = _cairo_fixed_from_double (matrix->x0);
    y0_fixed = _cairo_fixed_from_double (matrix->y0);

    return _cairo_fixed_is_integer (x0_fixed) && _cairo_fixed_is_integer (y0_fixed);
}

/*
  A circle in user space is transformed into an ellipse in device space.

  The following is a derivation of a formula to calculate the length of the
  major axis for this ellipse; this is useful for error bounds calculations.

  Thanks to Walter Brisken <wbrisken@aoc.nrao.edu> for this derivation:

  1.  First some notation:

  All capital letters represent vectors in two dimensions.  A prime '
  represents a transformed coordinate.  Matrices are written in underlined
  form, ie _R_.  Lowercase letters represent scalar real values.

  2.  The question has been posed:  What is the maximum expansion factor
  achieved by the linear transformation

  X' = X _R_

  where _R_ is a real-valued 2x2 matrix with entries:

  _R_ = [a b]
        [c d]  .

  In other words, what is the maximum radius, MAX[ |X'| ], reached for any
  X on the unit circle ( |X| = 1 ) ?

  3.  Some useful formulae

  (A) through (C) below are standard double-angle formulae.  (D) is a lesser
  known result and is derived below:

  (A)  sin²(θ) = (1 - cos(2*θ))/2
  (B)  cos²(θ) = (1 + cos(2*θ))/2
  (C)  sin(θ)*cos(θ) = sin(2*θ)/2
  (D)  MAX[a*cos(θ) + b*sin(θ)] = sqrt(a² + b²)

  Proof of (D):

  find the maximum of the function by setting the derivative to zero:

       -a*sin(θ)+b*cos(θ) = 0

  From this it follows that

       tan(θ) = b/a

  and hence

       sin(θ) = b/sqrt(a² + b²)

  and

       cos(θ) = a/sqrt(a² + b²)

  Thus the maximum value is

       MAX[a*cos(θ) + b*sin(θ)] = (a² + b²)/sqrt(a² + b²)
                                   = sqrt(a² + b²)

  4.  Derivation of maximum expansion

  To find MAX[ |X'| ] we search brute force method using calculus.  The unit
  circle on which X is constrained is to be parameterized by t:

       X(θ) = (cos(θ), sin(θ))

  Thus

       X'(θ) = X(θ) * _R_ = (cos(θ), sin(θ)) * [a b]
                                               [c d]
             = (a*cos(θ) + c*sin(θ), b*cos(θ) + d*sin(θ)).

  Define

       r(θ) = |X'(θ)|

  Thus

       r²(θ) = (a*cos(θ) + c*sin(θ))² + (b*cos(θ) + d*sin(θ))²
             = (a² + b²)*cos²(θ) + (c² + d²)*sin²(θ)
                 + 2*(a*c + b*d)*cos(θ)*sin(θ)

  Now apply the double angle formulae (A) to (C) from above:

       r²(θ) = (a² + b² + c² + d²)/2
	     + (a² + b² - c² - d²)*cos(2*θ)/2
  	     + (a*c + b*d)*sin(2*θ)
             = f + g*cos(φ) + h*sin(φ)

  Where

       f = (a² + b² + c² + d²)/2
       g = (a² + b² - c² - d²)/2
       h = (a*c + d*d)
       φ = 2*θ

  It is clear that MAX[ |X'| ] = sqrt(MAX[ r² ]).  Here we determine MAX[ r² ]
  using (D) from above:

       MAX[ r² ] = f + sqrt(g² + h²)

  And finally

       MAX[ |X'| ] = sqrt( f + sqrt(g² + h²) )

  Which is the solution to this problem.

  Walter Brisken
  2004/10/08

  (Note that the minor axis length is at the minimum of the above solution,
  which is just sqrt ( f - sqrt(g² + h²) ) given the symmetry of (D)).


  For another derivation of the same result, using Singular Value Decomposition,
  see doc/tutorial/src/singular.c.
*/

/* determine the length of the major axis of a circle of the given radius
   after applying the transformation matrix. */
double
_cairo_matrix_transformed_circle_major_axis (const cairo_matrix_t *matrix,
					     double radius)
{
    double  a, b, c, d, f, g, h, i, j;

    if (_cairo_matrix_has_unity_scale (matrix))
	return radius;

    _cairo_matrix_get_affine (matrix,
                              &a, &b,
                              &c, &d,
                              NULL, NULL);

    i = a*a + b*b;
    j = c*c + d*d;

    f = 0.5 * (i + j);
    g = 0.5 * (i - j);
    h = a*c + b*d;

    return radius * sqrt (f + hypot (g, h));

    /*
     * we don't need the minor axis length, which is
     * double min = radius * sqrt (f - sqrt (g*g+h*h));
     */
}

static const pixman_transform_t pixman_identity_transform = {{
        {1 << 16,        0,       0},
        {       0, 1 << 16,       0},
        {       0,       0, 1 << 16}
    }};

static cairo_status_t
_cairo_matrix_to_pixman_matrix (const cairo_matrix_t	*matrix,
				pixman_transform_t	*pixman_transform,
				double xc,
				double yc)
{
    cairo_matrix_t inv;
    unsigned max_iterations;

    pixman_transform->matrix[0][0] = _cairo_fixed_16_16_from_double (matrix->xx);
    pixman_transform->matrix[0][1] = _cairo_fixed_16_16_from_double (matrix->xy);
    pixman_transform->matrix[0][2] = _cairo_fixed_16_16_from_double (matrix->x0);

    pixman_transform->matrix[1][0] = _cairo_fixed_16_16_from_double (matrix->yx);
    pixman_transform->matrix[1][1] = _cairo_fixed_16_16_from_double (matrix->yy);
    pixman_transform->matrix[1][2] = _cairo_fixed_16_16_from_double (matrix->y0);

    pixman_transform->matrix[2][0] = 0;
    pixman_transform->matrix[2][1] = 0;
    pixman_transform->matrix[2][2] = 1 << 16;

    /* The conversion above breaks cairo's translation invariance:
     * a translation of (a, b) in device space translates to
     * a translation of (xx * a + xy * b, yx * a + yy * b)
     * for cairo, while pixman uses rounded versions of xx ... yy.
     * This error increases as a and b get larger.
     *
     * To compensate for this, we fix the point (xc, yc) in pattern
     * space and adjust pixman's transform to agree with cairo's at
     * that point.
     */

    if (_cairo_matrix_has_unity_scale (matrix))
	return CAIRO_STATUS_SUCCESS;

    if (unlikely (fabs (matrix->xx) > PIXMAN_MAX_INT ||
		  fabs (matrix->xy) > PIXMAN_MAX_INT ||
		  fabs (matrix->x0) > PIXMAN_MAX_INT ||
		  fabs (matrix->yx) > PIXMAN_MAX_INT ||
		  fabs (matrix->yy) > PIXMAN_MAX_INT ||
		  fabs (matrix->y0) > PIXMAN_MAX_INT))
    {
	return _cairo_error (CAIRO_STATUS_INVALID_MATRIX);
    }

    /* Note: If we can't invert the transformation, skip the adjustment. */
    inv = *matrix;
    if (cairo_matrix_invert (&inv) != CAIRO_STATUS_SUCCESS)
	return CAIRO_STATUS_SUCCESS;

    /* find the pattern space coordinate that maps to (xc, yc) */
    max_iterations = 5;
    do {
	double x,y;
	pixman_vector_t vector;
	cairo_fixed_16_16_t dx, dy;

	vector.vector[0] = _cairo_fixed_16_16_from_double (xc);
	vector.vector[1] = _cairo_fixed_16_16_from_double (yc);
	vector.vector[2] = 1 << 16;

	/* If we can't transform the reference point, skip the adjustment. */
	if (! pixman_transform_point_3d (pixman_transform, &vector))
	    return CAIRO_STATUS_SUCCESS;

	x = pixman_fixed_to_double (vector.vector[0]);
	y = pixman_fixed_to_double (vector.vector[1]);
	cairo_matrix_transform_point (&inv, &x, &y);

	/* Ideally, the vector should now be (xc, yc).
	 * We can now compensate for the resulting error.
	 */
	x -= xc;
	y -= yc;
	cairo_matrix_transform_distance (matrix, &x, &y);
	dx = _cairo_fixed_16_16_from_double (x);
	dy = _cairo_fixed_16_16_from_double (y);
	pixman_transform->matrix[0][2] -= dx;
	pixman_transform->matrix[1][2] -= dy;

	if (dx == 0 && dy == 0)
	    return CAIRO_STATUS_SUCCESS;
    } while (--max_iterations);

    /* We didn't find an exact match between cairo and pixman, but
     * the matrix should be mostly correct */
    return CAIRO_STATUS_SUCCESS;
}

static inline double
_pixman_nearest_sample (double d)
{
    return ceil (d - .5);
}

/**
 * _cairo_matrix_is_pixman_translation:
 * @matrix: a matrix
 * @filter: the filter to be used on the pattern transformed by @matrix
 * @x_offset: the translation in the X direction
 * @y_offset: the translation in the Y direction
 *
 * Checks if @matrix translated by (x_offset, y_offset) can be
 * represented using just an offset (within the range pixman can
 * accept) and an identity matrix.
 *
 * Passing a non-zero value in x_offset/y_offset has the same effect
 * as applying cairo_matrix_translate(matrix, x_offset, y_offset) and
 * setting x_offset and y_offset to 0.
 *
 * Upon return x_offset and y_offset contain the translation vector if
 * the return value is %TRUE. If the return value is %FALSE, they will
 * not be modified.
 *
 * Return value: %TRUE if @matrix can be represented as a pixman
 * translation, %FALSE otherwise.
 **/
cairo_bool_t
_cairo_matrix_is_pixman_translation (const cairo_matrix_t     *matrix,
				     cairo_filter_t            filter,
				     int                      *x_offset,
				     int                      *y_offset)
{
    double tx, ty;

    if (!_cairo_matrix_is_translation (matrix))
	return FALSE;

    if (matrix->x0 == 0. && matrix->y0 == 0.)
	return TRUE;

    tx = matrix->x0 + *x_offset;
    ty = matrix->y0 + *y_offset;

    if (filter == CAIRO_FILTER_FAST || filter == CAIRO_FILTER_NEAREST) {
	tx = _pixman_nearest_sample (tx);
	ty = _pixman_nearest_sample (ty);
    } else if (tx != floor (tx) || ty != floor (ty)) {
	return FALSE;
    }

    if (fabs (tx) > PIXMAN_MAX_INT || fabs (ty) > PIXMAN_MAX_INT)
	return FALSE;

    *x_offset = _cairo_lround (tx);
    *y_offset = _cairo_lround (ty);
    return TRUE;
}

/**
 * _cairo_matrix_to_pixman_matrix_offset:
 * @matrix: a matrix
 * @filter: the filter to be used on the pattern transformed by @matrix
 * @xc: the X coordinate of the point to fix in pattern space
 * @yc: the Y coordinate of the point to fix in pattern space
 * @out_transform: the transformation which best approximates @matrix
 * @x_offset: the translation in the X direction
 * @y_offset: the translation in the Y direction
 *
 * This function tries to represent @matrix translated by (x_offset,
 * y_offset) as a %pixman_transform_t and an translation.
 *
 * Passing a non-zero value in x_offset/y_offset has the same effect
 * as applying cairo_matrix_translate(matrix, x_offset, y_offset) and
 * setting x_offset and y_offset to 0.
 *
 * If it is possible to represent the matrix with an identity
 * %pixman_transform_t and a translation within the valid range for
 * pixman, this function will set @out_transform to be the identity,
 * @x_offset and @y_offset to be the translation vector and will
 * return %CAIRO_INT_STATUS_NOTHING_TO_DO. Otherwise it will try to
 * evenly divide the translational component of @matrix between
 * @out_transform and (@x_offset, @y_offset).
 *
 * Upon return x_offset and y_offset contain the translation vector.
 *
 * Return value: %CAIRO_INT_STATUS_NOTHING_TO_DO if the out_transform
 * is the identity, %CAIRO_STATUS_INVALID_MATRIX if it was not
 * possible to represent @matrix as a pixman_transform_t without
 * overflows, %CAIRO_STATUS_SUCCESS otherwise.
 **/
cairo_status_t
_cairo_matrix_to_pixman_matrix_offset (const cairo_matrix_t	*matrix,
				       cairo_filter_t            filter,
				       double                    xc,
				       double                    yc,
				       pixman_transform_t	*out_transform,
				       int                      *x_offset,
				       int                      *y_offset)
{
    cairo_bool_t is_pixman_translation;

    is_pixman_translation = _cairo_matrix_is_pixman_translation (matrix,
								 filter,
								 x_offset,
								 y_offset);

    if (is_pixman_translation) {
	*out_transform = pixman_identity_transform;
	return CAIRO_INT_STATUS_NOTHING_TO_DO;
    } else {
	cairo_matrix_t m;

	m = *matrix;
	cairo_matrix_translate (&m, *x_offset, *y_offset);
	if (m.x0 != 0.0 || m.y0 != 0.0) {
	    double tx, ty, norm;
	    int i, j;

	    /* pixman also limits the [xy]_offset to 16 bits so evenly
	     * spread the bits between the two.
	     *
	     * To do this, find the solutions of:
	     *   |x| = |x*m.xx + y*m.xy + m.x0|
	     *   |y| = |x*m.yx + y*m.yy + m.y0|
	     *
	     * and select the one whose maximum norm is smallest.
	     */
	    tx = m.x0;
	    ty = m.y0;
	    norm = MAX (fabs (tx), fabs (ty));

	    for (i = -1; i < 2; i+=2) {
		for (j = -1; j < 2; j+=2) {
		    double x, y, den, new_norm;

		    den = (m.xx + i) * (m.yy + j) - m.xy * m.yx;
		    if (fabs (den) < DBL_EPSILON)
			continue;

		    x = m.y0 * m.xy - m.x0 * (m.yy + j);
		    y = m.x0 * m.yx - m.y0 * (m.xx + i);

		    den = 1 / den;
		    x *= den;
		    y *= den;

		    new_norm = MAX (fabs (x), fabs (y));
		    if (norm > new_norm) {
			norm = new_norm;
			tx = x;
			ty = y;
		    }
		}
	    }

	    tx = floor (tx);
	    ty = floor (ty);
	    *x_offset = -tx;
	    *y_offset = -ty;
	    cairo_matrix_translate (&m, tx, ty);
	} else {
	    *x_offset = 0;
	    *y_offset = 0;
	}

	return _cairo_matrix_to_pixman_matrix (&m, out_transform, xc, yc);
    }
}
