/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2016 Adrian Johnson
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
 * The Initial Developer of the Original Code is Adrian Johnson.
 *
 * Contributor(s):
 *	Adrian Johnson <ajohnson@redneon.com>
 */

#ifndef CAIRO_TAG_STACK_PRIVATE_H
#define CAIRO_TAG_STACK_PRIVATE_H

#include "cairo-error-private.h"
#include "cairo-list-inline.h"

/* The type of a single tag */
typedef enum {
    TAG_TYPE_INVALID = 0,
    TAG_TYPE_STRUCTURE = 1,
    TAG_TYPE_LINK = 2,
    TAG_TYPE_DEST = 4,
} cairo_tag_type_t;

/* The type of the structure tree. */
typedef enum _cairo_tag_stack_structure_type {
    TAG_TREE_TYPE_TAGGED, /* compliant with Tagged PDF */
    TAG_TREE_TYPE_STRUCTURE, /* valid structure but not 'Tagged PDF' compliant */
    TAG_TREE_TYPE_LINK_ONLY, /* contains Link tags only */
    TAG_TREE_TYPE_NO_TAGS, /* no tags used */
    TAG_TREE_TYPE_INVALID, /* invalid tag structure */
} cairo_tag_stack_structure_type_t;

typedef struct _cairo_tag_stack_elem {
    char *name;
    char *attributes;
    void *data;
    cairo_list_t link;

} cairo_tag_stack_elem_t;

typedef struct _cairo_tag_stack {
    cairo_list_t list;
    cairo_tag_stack_structure_type_t type;
    int size;

} cairo_tag_stack_t;

cairo_private void
_cairo_tag_stack_init (cairo_tag_stack_t *stack);

cairo_private void
_cairo_tag_stack_fini (cairo_tag_stack_t *stack);

cairo_private cairo_tag_stack_structure_type_t
_cairo_tag_stack_get_structure_type (cairo_tag_stack_t *stack);

cairo_private cairo_int_status_t
_cairo_tag_stack_push (cairo_tag_stack_t *stack,
		       const char        *name,
		       const char        *attributes);

cairo_private void
_cairo_tag_stack_set_top_data (cairo_tag_stack_t *stack,
			       void              *data);

cairo_private cairo_int_status_t
_cairo_tag_stack_pop (cairo_tag_stack_t *stack,
		      const char *name,
		      cairo_tag_stack_elem_t **elem);

cairo_private cairo_tag_stack_elem_t *
_cairo_tag_stack_top_elem (cairo_tag_stack_t *stack);

cairo_private void
_cairo_tag_stack_free_elem (cairo_tag_stack_elem_t *elem);

cairo_private cairo_tag_type_t
_cairo_tag_get_type (const char *name);

cairo_private cairo_status_t
_cairo_tag_error (const char *fmt, ...) CAIRO_PRINTF_FORMAT (1, 2);

#endif /* CAIRO_TAG_STACK_PRIVATE_H */
