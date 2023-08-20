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

#ifndef CAIRO_LIST_INLINE_H
#define CAIRO_LIST_INLINE_H

#include "cairo-list-private.h"

#define cairo_list_entry(ptr, type, member) \
	cairo_container_of(ptr, type, member)

#define cairo_list_first_entry(ptr, type, member) \
	cairo_list_entry((ptr)->next, type, member)

#define cairo_list_last_entry(ptr, type, member) \
	cairo_list_entry((ptr)->prev, type, member)

#define cairo_list_foreach(pos, head)			\
	for (pos = (head)->next; pos != (head);	pos = pos->next)

#define cairo_list_foreach_entry(pos, type, head, member)		\
	for (pos = cairo_list_entry((head)->next, type, member);\
	     &pos->member != (head);					\
	     pos = cairo_list_entry(pos->member.next, type, member))

#define cairo_list_foreach_entry_safe(pos, n, type, head, member)	\
	for (pos = cairo_list_entry ((head)->next, type, member),\
	     n = cairo_list_entry (pos->member.next, type, member);\
	     &pos->member != (head);					\
	     pos = n, n = cairo_list_entry (n->member.next, type, member))

#define cairo_list_foreach_entry_reverse(pos, type, head, member)	\
	for (pos = cairo_list_entry((head)->prev, type, member);\
	     &pos->member != (head);					\
	     pos = cairo_list_entry(pos->member.prev, type, member))

#define cairo_list_foreach_entry_reverse_safe(pos, n, type, head, member)	\
	for (pos = cairo_list_entry((head)->prev, type, member),\
	     n = cairo_list_entry (pos->member.prev, type, member);\
	     &pos->member != (head);					\
	     pos = n, n = cairo_list_entry (n->member.prev, type, member))

#ifdef CAIRO_LIST_DEBUG
static inline void
_cairo_list_validate (const cairo_list_t *link)
{
    assert (link->next->prev == link);
    assert (link->prev->next == link);
}
static inline void
cairo_list_validate (const cairo_list_t *head)
{
    cairo_list_t *link;

    cairo_list_foreach (link, head)
	_cairo_list_validate (link);
}
static inline cairo_bool_t
cairo_list_is_empty (const cairo_list_t *head);
static inline void
cairo_list_validate_is_empty (const cairo_list_t *head)
{
    assert (head->next == NULL || (cairo_list_is_empty (head) && head->next == head->prev));
}
#else
#define _cairo_list_validate(link)
#define cairo_list_validate(head)
#define cairo_list_validate_is_empty(head)
#endif

static inline void
cairo_list_init (cairo_list_t *entry)
{
    entry->next = entry;
    entry->prev = entry;
}

static inline void
__cairo_list_add (cairo_list_t *entry,
	          cairo_list_t *prev,
		  cairo_list_t *next)
{
    next->prev = entry;
    entry->next = next;
    entry->prev = prev;
    prev->next = entry;
}

static inline void
cairo_list_add (cairo_list_t *entry, cairo_list_t *head)
{
    cairo_list_validate (head);
    cairo_list_validate_is_empty (entry);
    __cairo_list_add (entry, head, head->next);
    cairo_list_validate (head);
}

static inline void
cairo_list_add_tail (cairo_list_t *entry, cairo_list_t *head)
{
    cairo_list_validate (head);
    cairo_list_validate_is_empty (entry);
    __cairo_list_add (entry, head->prev, head);
    cairo_list_validate (head);
}

static inline void
__cairo_list_del (cairo_list_t *prev, cairo_list_t *next)
{
    next->prev = prev;
    prev->next = next;
}

static inline void
_cairo_list_del (cairo_list_t *entry)
{
    __cairo_list_del (entry->prev, entry->next);
}

static inline void
cairo_list_del (cairo_list_t *entry)
{
    _cairo_list_del (entry);
    cairo_list_init (entry);
}

static inline void
cairo_list_move (cairo_list_t *entry, cairo_list_t *head)
{
    cairo_list_validate (head);
    __cairo_list_del (entry->prev, entry->next);
    __cairo_list_add (entry, head, head->next);
    cairo_list_validate (head);
}

static inline void
cairo_list_move_tail (cairo_list_t *entry, cairo_list_t *head)
{
    cairo_list_validate (head);
    __cairo_list_del (entry->prev, entry->next);
    __cairo_list_add (entry, head->prev, head);
    cairo_list_validate (head);
}

static inline void
cairo_list_swap (cairo_list_t *entry, cairo_list_t *other)
{
    __cairo_list_add (entry, other->prev, other->next);
    cairo_list_init (other);
}

static inline cairo_bool_t
cairo_list_is_first (const cairo_list_t *entry,
	             const cairo_list_t *head)
{
    cairo_list_validate (head);
    return entry->prev == head;
}

static inline cairo_bool_t
cairo_list_is_last (const cairo_list_t *entry,
	            const cairo_list_t *head)
{
    cairo_list_validate (head);
    return entry->next == head;
}

static inline cairo_bool_t
cairo_list_is_empty (const cairo_list_t *head)
{
    cairo_list_validate (head);
    return head->next == head;
}

static inline cairo_bool_t
cairo_list_is_singular (const cairo_list_t *head)
{
    cairo_list_validate (head);
    return head->next == head || head->next == head->prev;
}

#endif /* CAIRO_LIST_INLINE_H */
