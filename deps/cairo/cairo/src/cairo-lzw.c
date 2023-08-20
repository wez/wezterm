/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2006 Red Hat, Inc.
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

typedef struct _lzw_buf {
    cairo_status_t status;

    unsigned char *data;
    int data_size;
    int num_data;
    uint32_t pending;
    unsigned int pending_bits;
} lzw_buf_t;

/* An lzw_buf_t is a simple, growable chunk of memory for holding
 * variable-size objects of up to 16 bits each.
 *
 * Initialize an lzw_buf_t to the given size in bytes.
 *
 * To store objects into the lzw_buf_t, call _lzw_buf_store_bits and
 * when finished, call _lzw_buf_store_pending, (which flushes out the
 * last few bits that hadn't yet made a complete byte yet).
 *
 * Instead of returning failure from any functions, lzw_buf_t provides
 * a status value that the caller can query, (and should query at
 * least once when done with the object). The status value will be
 * either %CAIRO_STATUS_SUCCESS or %CAIRO_STATUS_NO_MEMORY;
 */
static void
_lzw_buf_init (lzw_buf_t *buf, int size)
{
    if (size == 0)
	size = 16;

    buf->status = CAIRO_STATUS_SUCCESS;
    buf->data_size = size;
    buf->num_data = 0;
    buf->pending = 0;
    buf->pending_bits = 0;

    buf->data = _cairo_malloc (size);
    if (unlikely (buf->data == NULL)) {
	buf->data_size = 0;
	buf->status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	return;
    }
}

/* Increase the buffer size by doubling.
 *
 * Returns %CAIRO_STATUS_SUCCESS or %CAIRO_STATUS_NO_MEMORY
 */
static cairo_status_t
_lzw_buf_grow (lzw_buf_t *buf)
{
    int new_size = buf->data_size * 2;
    unsigned char *new_data;

    if (buf->status)
	return buf->status;

    new_data = NULL;
    /* check for integer overflow */
    if (new_size / 2 == buf->data_size)
	new_data = realloc (buf->data, new_size);

    if (unlikely (new_data == NULL)) {
	free (buf->data);
	buf->data_size = 0;
	buf->status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	return buf->status;
    }

    buf->data = new_data;
    buf->data_size = new_size;

    return CAIRO_STATUS_SUCCESS;
}

/* Store the lowest num_bits bits of values into buf.
 *
 * Note: The bits of value above size_in_bits must be 0, (so don't lie
 * about the size).
 *
 * See also _lzw_buf_store_pending which must be called after the last
 * call to _lzw_buf_store_bits.
 *
 * Sets buf->status to either %CAIRO_STATUS_SUCCESS or %CAIRO_STATUS_NO_MEMORY.
 */
static void
_lzw_buf_store_bits (lzw_buf_t *buf, uint16_t value, int num_bits)
{
    cairo_status_t status;

    assert (value <= (1 << num_bits) - 1);

    if (buf->status)
	return;

    buf->pending = (buf->pending << num_bits) | value;
    buf->pending_bits += num_bits;

    while (buf->pending_bits >= 8) {
	if (buf->num_data >= buf->data_size) {
	    status = _lzw_buf_grow (buf);
	    if (unlikely (status))
		return;
	}
	buf->data[buf->num_data++] = buf->pending >> (buf->pending_bits - 8);
	buf->pending_bits -= 8;
    }
}

/* Store the last remaining pending bits into the buffer.
 *
 * Note: This function must be called after the last call to
 * _lzw_buf_store_bits.
 *
 * Sets buf->status to either %CAIRO_STATUS_SUCCESS or %CAIRO_STATUS_NO_MEMORY.
 */
static void
_lzw_buf_store_pending  (lzw_buf_t *buf)
{
    cairo_status_t status;

    if (buf->status)
	return;

    if (buf->pending_bits == 0)
	return;

    assert (buf->pending_bits < 8);

    if (buf->num_data >= buf->data_size) {
	status = _lzw_buf_grow (buf);
	if (unlikely (status))
	    return;
    }

    buf->data[buf->num_data++] = buf->pending << (8 - buf->pending_bits);
    buf->pending_bits = 0;
}

/* LZW defines a few magic code values */
#define LZW_CODE_CLEAR_TABLE	256
#define LZW_CODE_EOD		257
#define LZW_CODE_FIRST		258

/* We pack three separate values into a symbol as follows:
 *
 * 12 bits (31 down to 20):	CODE: code value used to represent this symbol
 * 12 bits (19 down to  8):	PREV: previous code value in chain
 *  8 bits ( 7 down to  0):	NEXT: next byte value in chain
 */
typedef uint32_t lzw_symbol_t;

#define LZW_SYMBOL_SET(sym, prev, next)			((sym) = ((prev) << 8)|(next))
#define LZW_SYMBOL_SET_CODE(sym, code, prev, next)	((sym) = ((code << 20)|(prev) << 8)|(next))
#define LZW_SYMBOL_GET_CODE(sym)			(((sym) >> 20))
#define LZW_SYMBOL_GET_PREV(sym)			(((sym) >>  8) & 0x7ff)
#define LZW_SYMBOL_GET_BYTE(sym)			(((sym) >>  0) & 0x0ff)

/* The PREV+NEXT fields can be seen as the key used to fetch values
 * from the hash table, while the code is the value fetched.
 */
#define LZW_SYMBOL_KEY_MASK	0x000fffff

/* Since code values are only stored starting with 258 we can safely
 * use a zero value to represent free slots in the hash table. */
#define LZW_SYMBOL_FREE		0x00000000

/* These really aren't very free for modifying. First, the PostScript
 * specification sets the 9-12 bit range. Second, the encoding of
 * lzw_symbol_t above also relies on 2 of LZW_BITS_MAX plus one byte
 * fitting within 32 bits.
 *
 * But other than that, the LZW compression scheme could function with
 * more bits per code.
 */
#define LZW_BITS_MIN		9
#define LZW_BITS_MAX		12
#define LZW_BITS_BOUNDARY(bits)	((1<<(bits))-1)
#define LZW_MAX_SYMBOLS		(1<<LZW_BITS_MAX)

#define LZW_SYMBOL_TABLE_SIZE	9013
#define LZW_SYMBOL_MOD1		LZW_SYMBOL_TABLE_SIZE
#define LZW_SYMBOL_MOD2		9011

typedef struct _lzw_symbol_table {
    lzw_symbol_t table[LZW_SYMBOL_TABLE_SIZE];
} lzw_symbol_table_t;

/* Initialize the hash table to entirely empty */
static void
_lzw_symbol_table_init (lzw_symbol_table_t *table)
{
    memset (table->table, 0, LZW_SYMBOL_TABLE_SIZE * sizeof (lzw_symbol_t));
}

/* Lookup a symbol in the symbol table. The PREV and NEXT fields of
 * symbol form the key for the lookup.
 *
 * If successful, then this function returns %TRUE and slot_ret will be
 * left pointing at the result that will have the CODE field of
 * interest.
 *
 * If the lookup fails, then this function returns %FALSE and slot_ret
 * will be pointing at the location in the table to which a new CODE
 * value should be stored along with PREV and NEXT.
 */
static cairo_bool_t
_lzw_symbol_table_lookup (lzw_symbol_table_t	 *table,
			  lzw_symbol_t		  symbol,
			  lzw_symbol_t		**slot_ret)
{
    /* The algorithm here is identical to that in cairo-hash.c. We
     * copy it here to allow for a rather more efficient
     * implementation due to several circumstances that do not apply
     * to the more general case:
     *
     * 1) We have a known bound on the total number of symbols, so we
     *    have a fixed-size table without any copying when growing
     *
     * 2) We never delete any entries, so we don't need to
     *    support/check for DEAD entries during lookup.
     *
     * 3) The object fits in 32 bits so we store each object in its
     *    entirety within the table rather than storing objects
     *    externally and putting pointers in the table, (which here
     *    would just double the storage requirements and have negative
     *    impacts on memory locality).
     */
    int i, idx, step, hash = symbol & LZW_SYMBOL_KEY_MASK;
    lzw_symbol_t candidate;

    idx = hash % LZW_SYMBOL_MOD1;
    step = 0;

    *slot_ret = NULL;
    for (i = 0; i < LZW_SYMBOL_TABLE_SIZE; i++)
    {
	candidate = table->table[idx];
	if (candidate == LZW_SYMBOL_FREE)
	{
	    *slot_ret = &table->table[idx];
	    return FALSE;
	}
	else /* candidate is LIVE */
	{
	    if ((candidate & LZW_SYMBOL_KEY_MASK) ==
		(symbol & LZW_SYMBOL_KEY_MASK))
	    {
		*slot_ret = &table->table[idx];
		return TRUE;
	    }
	}

	if (step == 0) {
	    step = hash % LZW_SYMBOL_MOD2;
	    if (step == 0)
		step = 1;
	}

	idx += step;
	if (idx >= LZW_SYMBOL_TABLE_SIZE)
	    idx -= LZW_SYMBOL_TABLE_SIZE;
    }

    return FALSE;
}

/* Compress a bytestream using the LZW algorithm.
 *
 * This is an original implementation based on reading the
 * specification of the LZWDecode filter in the PostScript Language
 * Reference. The free parameters in the LZW algorithm are set to the
 * values mandated by PostScript, (symbols encoded with widths from 9
 * to 12 bits).
 *
 * This function returns a pointer to a newly allocated buffer holding
 * the compressed data, or %NULL if an out-of-memory situation
 * occurs.
 *
 * Notice that any one of the _lzw_buf functions called here could
 * trigger an out-of-memory condition. But lzw_buf_t uses cairo's
 * shutdown-on-error idiom, so it's safe to continue to call into
 * lzw_buf without having to check for errors, (until a final check at
 * the end).
 */
unsigned char *
_cairo_lzw_compress (unsigned char *data, unsigned long *size_in_out)
{
    int bytes_remaining = *size_in_out;
    lzw_buf_t buf;
    lzw_symbol_table_t table;
    lzw_symbol_t symbol, *slot = NULL; /* just to squelch a warning */
    int code_next = LZW_CODE_FIRST;
    int code_bits = LZW_BITS_MIN;
    int prev, next = 0; /* just to squelch a warning */

    if (*size_in_out == 0)
	return NULL;

    _lzw_buf_init (&buf, *size_in_out);

    _lzw_symbol_table_init (&table);

    /* The LZW header is a clear table code. */
    _lzw_buf_store_bits (&buf, LZW_CODE_CLEAR_TABLE, code_bits);

    while (1) {

	/* Find the longest existing code in the symbol table that
	 * matches the current input, if any. */
	prev = *data++;
	bytes_remaining--;
	if (bytes_remaining) {
	    do
	    {
		next = *data++;
		bytes_remaining--;
		LZW_SYMBOL_SET (symbol, prev, next);
		if (_lzw_symbol_table_lookup (&table, symbol, &slot))
		    prev = LZW_SYMBOL_GET_CODE (*slot);
	    } while (bytes_remaining && *slot != LZW_SYMBOL_FREE);
	    if (*slot == LZW_SYMBOL_FREE) {
		data--;
		bytes_remaining++;
	    }
	}

	/* Write the code into the output. This is either a byte read
	 * directly from the input, or a code from the last successful
	 * lookup. */
	_lzw_buf_store_bits (&buf, prev, code_bits);

	if (likely (slot != NULL))
	    LZW_SYMBOL_SET_CODE (*slot, code_next, prev, next);

	code_next++;

	if (code_next > LZW_BITS_BOUNDARY(code_bits))
	{
	    code_bits++;
	    if (code_bits > LZW_BITS_MAX) {
		_lzw_symbol_table_init (&table);
		_lzw_buf_store_bits (&buf, LZW_CODE_CLEAR_TABLE, code_bits - 1);
		code_bits = LZW_BITS_MIN;
		code_next = LZW_CODE_FIRST;
	    }
	}

	if (bytes_remaining == 0)
	    break;
    }

    /* The LZW footer is an end-of-data code. */
    _lzw_buf_store_bits (&buf, LZW_CODE_EOD, code_bits);

    _lzw_buf_store_pending (&buf);

    /* See if we ever ran out of memory while writing to buf. */
    if (buf.status == CAIRO_STATUS_NO_MEMORY) {
	*size_in_out = 0;
	return NULL;
    }

    assert (buf.status == CAIRO_STATUS_SUCCESS);

    *size_in_out = buf.num_data;
    return buf.data;
}
