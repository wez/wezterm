/*
 * Copyright © 2008 Chris Wilson
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
 * The Initial Developer of the Original Code is Chris Wilson
 *
 * Contributor(s):
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

/* This fragment implements a comb sort (specifically combsort11) */
#ifndef _HAVE_CAIRO_COMBSORT_NEWGAP
#define _HAVE_CAIRO_COMBSORT_NEWGAP
static inline unsigned int
_cairo_combsort_newgap (unsigned int gap)
{
  gap = 10 * gap / 13;
  if (gap == 9 || gap == 10)
    gap = 11;
  if (gap < 1)
    gap = 1;
  return gap;
}
#endif

#define CAIRO_COMBSORT_DECLARE(NAME, TYPE, CMP) \
static void \
NAME (TYPE *base, unsigned int nmemb) \
{ \
  unsigned int gap = nmemb; \
  unsigned int i, j; \
  int swapped; \
  do { \
      gap = _cairo_combsort_newgap (gap); \
      swapped = gap > 1; \
      for (i = 0; i < nmemb-gap ; i++) { \
	  j = i + gap; \
	  if (CMP (base[i], base[j]) > 0 ) { \
	      TYPE tmp; \
	      tmp = base[i]; \
	      base[i] = base[j]; \
	      base[j] = tmp; \
	      swapped = 1; \
	  } \
      } \
  } while (swapped); \
}

#define CAIRO_COMBSORT_DECLARE_WITH_DATA(NAME, TYPE, CMP) \
static void \
NAME (TYPE *base, unsigned int nmemb, void *data) \
{ \
  unsigned int gap = nmemb; \
  unsigned int i, j; \
  int swapped; \
  do { \
      gap = _cairo_combsort_newgap (gap); \
      swapped = gap > 1; \
      for (i = 0; i < nmemb-gap ; i++) { \
	  j = i + gap; \
	  if (CMP (base[i], base[j], data) > 0 ) { \
	      TYPE tmp; \
	      tmp = base[i]; \
	      base[i] = base[j]; \
	      base[j] = tmp; \
	      swapped = 1; \
	  } \
      } \
  } while (swapped); \
}
