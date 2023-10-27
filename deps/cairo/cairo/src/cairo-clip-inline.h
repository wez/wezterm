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
 *	Kristian Høgsberg <krh@redhat.com>
 *	Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_CLIP_INLINE_H
#define CAIRO_CLIP_INLINE_H

#include "cairo-clip-private.h"

static inline cairo_bool_t _cairo_clip_is_all_clipped(const cairo_clip_t *clip)
{
    return clip == &__cairo_clip_all;
}

static inline cairo_clip_t *
_cairo_clip_set_all_clipped (cairo_clip_t *clip)
{
    _cairo_clip_destroy (clip);
    return (cairo_clip_t *) &__cairo_clip_all;
}

static inline cairo_clip_t *
_cairo_clip_copy_intersect_rectangle (const cairo_clip_t       *clip,
				      const cairo_rectangle_int_t *r)
{
    return _cairo_clip_intersect_rectangle (_cairo_clip_copy (clip), r);
}

static inline cairo_clip_t *
_cairo_clip_copy_intersect_clip (const cairo_clip_t *clip,
				 const cairo_clip_t *other)
{
    return _cairo_clip_intersect_clip (_cairo_clip_copy (clip), other);
}

static inline void
_cairo_clip_steal_boxes (cairo_clip_t *clip, cairo_boxes_t *boxes)
{
    cairo_box_t *array = clip->boxes;

    if (array == &clip->embedded_box) {
	assert (clip->num_boxes == 1);
	boxes->boxes_embedded[0] = clip->embedded_box;
	array = &boxes->boxes_embedded[0];
    }
    _cairo_boxes_init_for_array (boxes, array, clip->num_boxes);
    clip->boxes = NULL;
    clip->num_boxes = 0;
}

static inline void
_cairo_clip_unsteal_boxes (cairo_clip_t *clip, cairo_boxes_t *boxes)
{
    if (boxes->chunks.base == &boxes->boxes_embedded[0]) {
	assert(boxes->num_boxes == 1);
	clip->embedded_box = *boxes->chunks.base;
	clip->boxes = &clip->embedded_box;
    } else {
	clip->boxes = boxes->chunks.base;
    }
    clip->num_boxes = boxes->num_boxes;
}

#endif /* CAIRO_CLIP_INLINE_H */
