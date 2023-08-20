/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2006 Red Hat, Inc.
 * Copyright © 2011 Andrea Canciani
 *
 * Permission to use, copy, modify, distribute, and sell this software
 * and its documentation for any purpose is hereby granted without
 * fee, provided that the above copyright notice appear in all copies
 * and that both that copyright notice and this permission notice
 * appear in supporting documentation, and that the name of the
 * copyright holders not be used in advertising or publicity
 * pertaining to distribution of the software without specific,
 * written prior permission. The copyright holders make no
 * representations about the suitability of this software for any
 * purpose.  It is provided "as is" without express or implied
 * warranty.
 *
 * THE COPYRIGHT HOLDERS DISCLAIM ALL WARRANTIES WITH REGARD TO THIS
 * SOFTWARE, INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
 * FITNESS, IN NO EVENT SHALL THE COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * SPECIAL, INDIRECT OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN
 * AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING
 * OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS
 * SOFTWARE.
 *
 * Authors: Carl Worth <cworth@cworth.org>
 *	    Andrea Canciani <ranma42@gmail.com>
 */

#ifndef CAIRO_MISSING_H
#define CAIRO_MISSING_H

#include "cairo-compiler-private.h"

#include <stdio.h>
#include <string.h>
#include <sys/types.h>

#ifdef _WIN32
#include <windows.h>

#if !defined(_SSIZE_T_DEFINED) && !defined(_SSIZE_T_)
typedef SSIZE_T ssize_t;
#endif
#endif

#ifndef HAVE_GETLINE
cairo_private ssize_t
getline (char **lineptr, size_t *n, FILE *stream);
#endif

#ifndef HAVE_STRNDUP
cairo_private char *
strndup (const char *s, size_t n);
#endif

#endif
