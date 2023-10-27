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

#ifndef CAIRO_ARRAY_PRIVATE_H
#define CAIRO_ARRAY_PRIVATE_H

#include "cairo-compiler-private.h"
#include "cairo-types-private.h"

CAIRO_BEGIN_DECLS

/* cairo-array.c structures and functions */

cairo_private void
_cairo_array_init (cairo_array_t *array, unsigned int element_size);

cairo_private void
_cairo_array_fini (cairo_array_t *array);

cairo_private cairo_status_t
_cairo_array_grow_by (cairo_array_t *array, unsigned int additional);

cairo_private void
_cairo_array_truncate (cairo_array_t *array, unsigned int num_elements);

cairo_private cairo_status_t
_cairo_array_append (cairo_array_t *array, const void *element);

cairo_private cairo_status_t
_cairo_array_append_multiple (cairo_array_t	*array,
			      const void	*elements,
			      unsigned int	 num_elements);

cairo_private cairo_status_t
_cairo_array_allocate (cairo_array_t	 *array,
		       unsigned int	  num_elements,
		       void		**elements);

cairo_private void *
_cairo_array_index (cairo_array_t *array, unsigned int index);

cairo_private const void *
_cairo_array_index_const (const cairo_array_t *array, unsigned int index);

cairo_private void
_cairo_array_copy_element (const cairo_array_t *array, unsigned int index, void *dst);

cairo_private unsigned int
_cairo_array_num_elements (const cairo_array_t *array);

cairo_private unsigned int
_cairo_array_size (const cairo_array_t *array);

cairo_private void
_cairo_array_sort (const cairo_array_t *array, int (*compar)(const void *, const void *));

CAIRO_END_DECLS

#endif /* CAIRO_ARRAY_PRIVATE_H */
