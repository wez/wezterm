/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2009 Chris Wilson
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
 * The Initial Developer of the Original Code is Chris Wilson.
 *
 * Contributor(s):
 *      Chris Wilson <chris@chris-wilson.co.uk>
 *
 */

#ifndef CAIRO_RTREE_PRIVATE_H
#define CAIRO_RTREE_PRIVATE_H

#include "cairo-compiler-private.h"
#include "cairo-error-private.h"
#include "cairo-types-private.h"

#include "cairo-freelist-private.h"
#include "cairo-list-inline.h"

enum {
    CAIRO_RTREE_NODE_AVAILABLE,
    CAIRO_RTREE_NODE_DIVIDED,
    CAIRO_RTREE_NODE_OCCUPIED,
};

typedef struct _cairo_rtree_node {
    struct _cairo_rtree_node *children[4], *parent;
    cairo_list_t link;
    uint16_t pinned;
    uint16_t state;
    uint16_t x, y;
    uint16_t width, height;
} cairo_rtree_node_t;

typedef struct _cairo_rtree {
    cairo_rtree_node_t root;
    int min_size;
    cairo_list_t pinned;
    cairo_list_t available;
    cairo_list_t evictable;
    void (*destroy) (cairo_rtree_node_t *);
    cairo_freepool_t node_freepool;
} cairo_rtree_t;

cairo_private cairo_rtree_node_t *
_cairo_rtree_node_create (cairo_rtree_t		 *rtree,
		          cairo_rtree_node_t	 *parent,
			  int			  x,
			  int			  y,
			  int			  width,
			  int			  height);

cairo_private cairo_status_t
_cairo_rtree_node_insert (cairo_rtree_t *rtree,
	                  cairo_rtree_node_t *node,
			  int width,
			  int height,
			  cairo_rtree_node_t **out);

cairo_private void
_cairo_rtree_node_collapse (cairo_rtree_t *rtree, cairo_rtree_node_t *node);

cairo_private void
_cairo_rtree_node_remove (cairo_rtree_t *rtree, cairo_rtree_node_t *node);

cairo_private void
_cairo_rtree_node_destroy (cairo_rtree_t *rtree, cairo_rtree_node_t *node);

cairo_private void
_cairo_rtree_init (cairo_rtree_t	*rtree,
	           int			 width,
		   int			 height,
		   int			 min_size,
		   int			 node_size,
		   void (*destroy)(cairo_rtree_node_t *));

cairo_private cairo_int_status_t
_cairo_rtree_insert (cairo_rtree_t	     *rtree,
		     int		      width,
	             int		      height,
	             cairo_rtree_node_t	    **out);

cairo_private cairo_int_status_t
_cairo_rtree_evict_random (cairo_rtree_t	 *rtree,
		           int			  width,
		           int			  height,
		           cairo_rtree_node_t	**out);

cairo_private void
_cairo_rtree_foreach (cairo_rtree_t *rtree,
		      void (*func)(cairo_rtree_node_t *, void *data),
		      void *data);

static inline void *
_cairo_rtree_pin (cairo_rtree_t *rtree, cairo_rtree_node_t *node)
{
    assert (node->state == CAIRO_RTREE_NODE_OCCUPIED);
    if (! node->pinned) {
	cairo_list_move (&node->link, &rtree->pinned);
	node->pinned = 1;
    }

    return node;
}

cairo_private void
_cairo_rtree_unpin (cairo_rtree_t *rtree);

cairo_private void
_cairo_rtree_reset (cairo_rtree_t *rtree);

cairo_private void
_cairo_rtree_fini (cairo_rtree_t *rtree);

#endif /* CAIRO_RTREE_PRIVATE_H */
