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

#include "cairo-missing.h"

#ifndef HAVE_GETLINE
#include "cairo-malloc-private.h"

#define GETLINE_MIN_BUFFER_SIZE 128
ssize_t
getline (char	**lineptr,
	 size_t  *n,
	 FILE	 *stream)
{
    char *line, *tmpline;
    size_t len, offset;
    ssize_t ret;

    offset = 0;
    len = *n;
    line = *lineptr;
    if (len < GETLINE_MIN_BUFFER_SIZE) {
	len = GETLINE_MIN_BUFFER_SIZE;
	line = NULL;
    }

    if (line == NULL) {
	line = (char *) _cairo_malloc (len);
	if (unlikely (line == NULL))
	    return -1;
    }

    while (1) {
	if (offset + 1 == len) {
	    tmpline = (char *) _cairo_realloc_ab (line, len, 2);
	    if (unlikely (tmpline == NULL)) {
		if (line != *lineptr)
		    free (line);
		return -1;
	    }
	    len *= 2;
	    line = tmpline;
	}

	ret = getc (stream);
	if (ret == -1)
	    break;

	line[offset++] = ret;
	if (ret == '\n') {
	    ret = offset;
	    break;
	}
    }

    line[offset++] = '\0';
    *lineptr = line;
    *n = len;

    return ret;
}
#undef GETLINE_BUFFER_SIZE
#endif
