/* -*- Mode: c; tab-width: 8; c-basic-offset: 4; indent-tabs-mode: t; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2006 Adrian Johnson
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
 *      Eugeniy Meshcheryakov <eugen@debian.org>
 */

/*
 * Useful links:
 * http://www.adobe.com/content/dam/Adobe/en/devnet/font/pdfs/5176.CFF.pdf
 * http://www.adobe.com/content/dam/Adobe/en/devnet/font/pdfs/5177.Type2.pdf
 */

#define _DEFAULT_SOURCE /* for snprintf(), strdup() */
#include "cairoint.h"

#include "cairo-array-private.h"
#include "cairo-error-private.h"

#if CAIRO_HAS_FONT_SUBSET

#include "cairo-scaled-font-subsets-private.h"
#include "cairo-truetype-subset-private.h"
#include <string.h>
#include <locale.h>

/* CFF Dict Operators. If the high byte is 0 the command is encoded
 * with a single byte. */
#define BASEFONTNAME_OP     0x0c16
#define CIDCOUNT_OP         0x0c22
#define CHARSET_OP          0x000f
#define CHARSTRINGS_OP      0x0011
#define COPYRIGHT_OP        0x0c00
#define DEFAULTWIDTH_OP     0x0014
#define ENCODING_OP         0x0010
#define FAMILYNAME_OP       0x0003
#define FDARRAY_OP          0x0c24
#define FDSELECT_OP         0x0c25
#define FONTBBOX_OP         0x0005
#define FONTMATRIX_OP       0x0c07
#define FONTNAME_OP         0x0c26
#define FULLNAME_OP         0x0002
#define LOCAL_SUB_OP        0x0013
#define NOMINALWIDTH_OP     0x0015
#define NOTICE_OP           0x0001
#define POSTSCRIPT_OP       0x0c15
#define PRIVATE_OP          0x0012
#define ROS_OP              0x0c1e
#define UNIQUEID_OP         0x000d
#define VERSION_OP          0x0000
#define WEIGHT_OP           0x0004
#define XUID_OP             0x000e
#define BLUEVALUES_OP       0x0006
#define OTHERBLUES_OP       0x0007
#define FAMILYBLUES_OP      0x0008
#define FAMILYOTHERBLUES_OP 0x0009
#define STEMSNAPH_OP        0x0c0c
#define STEMSNAPV_OP        0x0c0d

#define NUM_STD_STRINGS 391

/* Type 2 Charstring operators */
#define TYPE2_hstem     0x0001
#define TYPE2_vstem     0x0003
#define TYPE2_callsubr  0x000a

#define TYPE2_return    0x000b
#define TYPE2_endchar   0x000e

#define TYPE2_hstemhm   0x0012
#define TYPE2_hintmask  0x0013
#define TYPE2_cntrmask  0x0014
#define TYPE2_vstemhm   0x0017
#define TYPE2_callgsubr 0x001d

#define TYPE2_rmoveto   0x0015
#define TYPE2_hmoveto   0x0016
#define TYPE2_vmoveto   0x0004


#define MAX_SUBROUTINE_NESTING 10 /* From Type2 Charstring spec */


typedef struct _cff_header {
    uint8_t major;
    uint8_t minor;
    uint8_t header_size;
    uint8_t offset_size;
} cff_header_t;

typedef struct _cff_index_element {
    cairo_bool_t   is_copy;
    unsigned char *data;
    int            length;
} cff_index_element_t;

typedef struct _cff_dict_operator {
    cairo_hash_entry_t base;

    unsigned short operator;
    unsigned char *operand;
    int            operand_length;
    int            operand_offset;
} cff_dict_operator_t;

typedef struct _cairo_cff_font {

    cairo_scaled_font_subset_t *scaled_font_subset;
    const cairo_scaled_font_backend_t *backend;

    /* Font Data */
    unsigned char       *data;
    unsigned long        data_length;
    unsigned char       *current_ptr;
    unsigned char       *data_end;
    cff_header_t        *header;
    char                *font_name;
    char                *ps_name;
    cairo_hash_table_t  *top_dict;
    cairo_hash_table_t  *private_dict;
    cairo_array_t        strings_index;
    cairo_array_t        charstrings_index;
    cairo_array_t        global_sub_index;
    cairo_array_t        local_sub_index;
    unsigned char       *charset;
    int                  num_glyphs;
    cairo_bool_t         is_cid;
    cairo_bool_t         is_opentype;
    int  		 units_per_em;
    int 		 global_sub_bias;
    int			 local_sub_bias;
    double               default_width;
    double               nominal_width;

    /* CID Font Data */
    int                 *fdselect;
    unsigned int         num_fontdicts;
    cairo_hash_table_t **fd_dict;
    cairo_hash_table_t **fd_private_dict;
    cairo_array_t       *fd_local_sub_index;
    int			*fd_local_sub_bias;
    double              *fd_default_width;
    double              *fd_nominal_width;

    /* Subsetted Font Data */
    char                *subset_font_name;
    cairo_array_t        charstrings_subset_index;
    cairo_array_t        strings_subset_index;
    int			 euro_sid;
    int                 *fdselect_subset;
    unsigned int         num_subset_fontdicts;
    int                 *fd_subset_map;
    int                 *private_dict_offset;
    cairo_bool_t         subset_subroutines;
    cairo_bool_t	*global_subs_used;
    cairo_bool_t	*local_subs_used;
    cairo_bool_t       **fd_local_subs_used;
    cairo_array_t        output;

    /* Subset Metrics */
    int                 *widths;
    int                  x_min, y_min, x_max, y_max;
    int                  ascent, descent;

    /* Type 2 charstring data */
    int 	 	 type2_stack_size;
    int 		 type2_stack_top_value;
    cairo_bool_t 	 type2_stack_top_is_int;
    int 		 type2_num_hints;
    int 		 type2_hintmask_bytes;
    int                  type2_nesting_level;
    cairo_bool_t         type2_seen_first_int;
    cairo_bool_t         type2_find_width;
    cairo_bool_t         type2_found_width;
    int                  type2_width;
    cairo_bool_t         type2_has_path;

} cairo_cff_font_t;

/* Encoded integer using maximum sized encoding. This is required for
 * operands that are later modified after encoding. */
static unsigned char *
encode_integer_max (unsigned char *p, int i)
{
    *p++ = 29;
    *p++ = i >> 24;
    *p++ = (i >> 16) & 0xff;
    *p++ = (i >> 8)  & 0xff;
    *p++ = i & 0xff;
    return p;
}

static unsigned char *
encode_integer (unsigned char *p, int i)
{
    if (i >= -107 && i <= 107) {
        *p++ = i + 139;
    } else if (i >= 108 && i <= 1131) {
        i -= 108;
        *p++ = (i >> 8)+ 247;
        *p++ = i & 0xff;
    } else if (i >= -1131 && i <= -108) {
        i = -i - 108;
        *p++ = (i >> 8)+ 251;
        *p++ = i & 0xff;
    } else if (i >= -32768 && i <= 32767) {
        *p++ = 28;
        *p++ = (i >> 8)  & 0xff;
        *p++ = i & 0xff;
    } else {
        p = encode_integer_max (p, i);
    }
    return p;
}

static unsigned char *
decode_integer (unsigned char *p, int *integer)
{
    if (*p == 28) {
        *integer = (int16_t)(p[1]<<8 | p[2]);
        p += 3;
    } else if (*p == 29) {
        *integer = (int32_t)(((uint32_t)p[1] << 24) | (p[2] << 16) | (p[3] << 8) | p[4]);
        p += 5;
    } else if (*p >= 32 && *p <= 246) {
        *integer = *p++ - 139;
    } else if (*p <= 250) {
        *integer = (p[0] - 247) * 256 + p[1] + 108;
        p += 2;
    } else if (*p <= 254) {
        *integer = -(p[0] - 251) * 256 - p[1] - 108;
        p += 2;
    } else {
        *integer = 0;
        p += 1;
    }
    return p;
}

static char *
decode_nibble (int n, char *buf)
{
    switch (n)
    {
    case 0xa:
	*buf++ = '.';
	break;
    case 0xb:
	*buf++ = 'E';
	break;
    case 0xc:
	*buf++ = 'E';
	*buf++ = '-';
	break;
    case 0xd:
	*buf++ = '-';
	break;
    case 0xe:
	*buf++ = '-';
	break;
    case 0xf:
	break;
    default:
	*buf++ = '0' + n;
	break;
    }

    return buf;
}

static unsigned char *
decode_real (unsigned char *p, double *real)
{
    char buffer[100];
    char *buf = buffer;
    char *buf_end = buffer + sizeof (buffer);
    char *end;
    int n;

    p++;
    while (buf + 2 < buf_end) {
	n = *p >> 4;
	buf = decode_nibble (n, buf);
	n = *p & 0x0f;
	buf = decode_nibble (n, buf);
        if ((*p & 0x0f) == 0x0f) {
	    p++;
            break;
	}
	p++;
    };
    *buf = 0;

    *real = _cairo_strtod (buffer, &end);

    return p;
}

static unsigned char *
decode_number (unsigned char *p, double *number)
{
    if (*p == 30) {
        p = decode_real (p, number);
    } else {
        int i;
        p = decode_integer (p, &i);
        *number = i;
    }
    return p;
}

static unsigned char *
decode_operator (unsigned char *p, unsigned short *operator)
{
    unsigned short op = 0;

    op = *p++;
    if (op == 12) {
        op <<= 8;
        op |= *p++;
    }
    *operator = op;
    return p;
}

/* return 0 if not an operand */
static int
operand_length (unsigned char *p)
{
    unsigned char *begin = p;

    if (*p == 28)
        return 3;

    if (*p == 29)
        return 5;

    if (*p >= 32 && *p <= 246)
        return 1;

    if (*p >= 247 && *p <= 254)
        return 2;

    if (*p == 30) {
        while ((*p & 0x0f) != 0x0f)
            p++;
        return p - begin + 1;
    }

    return 0;
}

static unsigned char *
encode_index_offset (unsigned char *p, int offset_size, unsigned long offset)
{
    while (--offset_size >= 0) {
        p[offset_size] = (unsigned char) (offset & 0xff);
        offset >>= 8;
    }
    return p + offset_size;
}

static size_t
decode_index_offset(unsigned char *p, int off_size)
{
    unsigned long offset = 0;

    while (off_size-- > 0)
        offset = offset*256 + *p++;
    return offset;
}

static void
cff_index_init (cairo_array_t *index)
{
    _cairo_array_init (index, sizeof (cff_index_element_t));
}

static cairo_int_status_t
cff_index_read (cairo_array_t *index, unsigned char **ptr, unsigned char *end_ptr)
{
    cff_index_element_t element;
    unsigned char *data, *p;
    cairo_status_t status;
    int offset_size, count, i;
    size_t start, end = 0;

    p = *ptr;
    if (p + 2 > end_ptr)
        return CAIRO_INT_STATUS_UNSUPPORTED;
    count = get_unaligned_be16 (p);
    p += 2;
    if (count > 0) {
        offset_size = *p++;
        if (p + (count + 1)*offset_size > end_ptr || offset_size > 4)
            return CAIRO_INT_STATUS_UNSUPPORTED;
        data = p + offset_size*(count + 1) - 1;
        start = decode_index_offset (p, offset_size);
        p += offset_size;
        for (i = 0; i < count; i++) {
            end = decode_index_offset (p, offset_size);
            p += offset_size;
            if (p > end_ptr || end < start || data + end > end_ptr)
                return CAIRO_INT_STATUS_UNSUPPORTED;
            element.length = end - start;
            element.is_copy = FALSE;
            element.data = data + start;
            status = _cairo_array_append (index, &element);
            if (unlikely (status))
                return status;
            start = end;
        }
        p = data + end;
    }
    *ptr = p;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cff_index_write (cairo_array_t *index, cairo_array_t *output)
{
    int offset_size;
    int offset;
    int num_elem;
    int i;
    cff_index_element_t *element;
    uint16_t count;
    unsigned char buf[5];
    cairo_status_t status;

    num_elem = _cairo_array_num_elements (index);
    count = cpu_to_be16 ((uint16_t) num_elem);
    status = _cairo_array_append_multiple (output, &count, 2);
    if (unlikely (status))
        return status;

    if (num_elem == 0)
        return CAIRO_STATUS_SUCCESS;

    /* Find maximum offset to determine offset size */
    offset = 1;
    for (i = 0; i < num_elem; i++) {
        element = _cairo_array_index (index, i);
        offset += element->length;
    }
    if (offset < 0x100)
        offset_size = 1;
    else if (offset < 0x10000)
        offset_size = 2;
    else if (offset < 0x1000000)
        offset_size = 3;
    else
        offset_size = 4;

    buf[0] = (unsigned char) offset_size;
    status = _cairo_array_append (output, buf);
    if (unlikely (status))
        return status;

    offset = 1;
    encode_index_offset (buf, offset_size, offset);
    status = _cairo_array_append_multiple (output, buf, offset_size);
    if (unlikely (status))
        return status;

    for (i = 0; i < num_elem; i++) {
        element = _cairo_array_index (index, i);
        offset += element->length;
        encode_index_offset (buf, offset_size, offset);
        status = _cairo_array_append_multiple (output, buf, offset_size);
        if (unlikely (status))
            return status;
    }

    for (i = 0; i < num_elem; i++) {
        element = _cairo_array_index (index, i);
        if (element->length > 0) {
            status = _cairo_array_append_multiple (output,
                                                   element->data,
                                                   element->length);
        }
        if (unlikely (status))
            return status;
    }
    return CAIRO_STATUS_SUCCESS;
}

static void
cff_index_set_object (cairo_array_t *index, int obj_index,
                      unsigned char *object , int length)
{
    cff_index_element_t *element;

    element = _cairo_array_index (index, obj_index);
    if (element->is_copy)
        free (element->data);

    element->data = object;
    element->length = length;
    element->is_copy = FALSE;
}

static cairo_status_t
cff_index_append (cairo_array_t *index, unsigned char *object , int length)
{
    cff_index_element_t element;

    element.length = length;
    element.is_copy = FALSE;
    element.data = object;

    return _cairo_array_append (index, &element);
}

static cairo_status_t
cff_index_append_copy (cairo_array_t *index,
                       const unsigned char *object,
                       unsigned int length)
{
    cff_index_element_t element;
    cairo_status_t status;

    element.length = length;
    element.is_copy = TRUE;
    element.data = _cairo_malloc (element.length);
    if (unlikely (element.data == NULL && length != 0))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    memcpy (element.data, object, element.length);

    status = _cairo_array_append (index, &element);
    if (unlikely (status)) {
	free (element.data);
	return status;
    }

    return CAIRO_STATUS_SUCCESS;
}

static void
cff_index_fini (cairo_array_t *index)
{
    cff_index_element_t *element;
    unsigned int i;

    for (i = 0; i < _cairo_array_num_elements (index); i++) {
        element = _cairo_array_index (index, i);
        if (element->is_copy && element->data)
            free (element->data);
    }
    _cairo_array_fini (index);
}

static cairo_bool_t
_cairo_cff_dict_equal (const void *key_a, const void *key_b)
{
    const cff_dict_operator_t *op_a = key_a;
    const cff_dict_operator_t *op_b = key_b;

    return op_a->operator == op_b->operator;
}

static cairo_status_t
cff_dict_init (cairo_hash_table_t **dict)
{
    *dict = _cairo_hash_table_create (_cairo_cff_dict_equal);
    if (unlikely (*dict == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    return CAIRO_STATUS_SUCCESS;
}

static void
_cairo_dict_init_key (cff_dict_operator_t *key, int operator)
{
    key->base.hash = (unsigned long) operator;
    key->operator = operator;
}

static cairo_status_t
cff_dict_create_operator (int            operator,
                          unsigned char *operand,
                          int            size,
			  cff_dict_operator_t **out)
{
    cff_dict_operator_t *op;

    op = _cairo_malloc (sizeof (cff_dict_operator_t));
    if (unlikely (op == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    _cairo_dict_init_key (op, operator);
    if (size != 0) {
	op->operand = _cairo_malloc (size);
	if (unlikely (op->operand == NULL)) {
	    free (op);
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	}
	memcpy (op->operand, operand, size);
    } else {
	op->operand = NULL;
	/* Delta-encoded arrays can be empty. */
	if (operator != BLUEVALUES_OP &&
	    operator != OTHERBLUES_OP &&
	    operator != FAMILYBLUES_OP &&
	    operator != FAMILYOTHERBLUES_OP &&
	    operator != STEMSNAPH_OP &&
	    operator != STEMSNAPV_OP) {
	    free (op);
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
	}
    }

    op->operand_length = size;
    op->operand_offset = -1;

    *out = op;
    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cff_dict_read (cairo_hash_table_t *dict, unsigned char *p, int dict_size)
{
    unsigned char *end;
    cairo_array_t operands;
    cff_dict_operator_t *op;
    unsigned short operator;
    cairo_status_t status = CAIRO_STATUS_SUCCESS;
    int size;

    end = p + dict_size;
    _cairo_array_init (&operands, 1);
    while (p < end) {
        size = operand_length (p);
        if (size != 0) {
            status = _cairo_array_append_multiple (&operands, p, size);
            if (unlikely (status))
                goto fail;

            p += size;
        } else {
            p = decode_operator (p, &operator);
            status = cff_dict_create_operator (operator,
                                          _cairo_array_index (&operands, 0),
                                          _cairo_array_num_elements (&operands),
					  &op);
            if (unlikely (status))
                goto fail;

            status = _cairo_hash_table_insert (dict, &op->base);
            if (unlikely (status))
                goto fail;

            _cairo_array_truncate (&operands, 0);
        }
    }

fail:
    _cairo_array_fini (&operands);

    return status;
}

static void
cff_dict_remove (cairo_hash_table_t *dict, unsigned short operator)
{
    cff_dict_operator_t key, *op;

    _cairo_dict_init_key (&key, operator);
    op = _cairo_hash_table_lookup (dict, &key.base);
    if (op != NULL) {
        free (op->operand);
        _cairo_hash_table_remove (dict, (cairo_hash_entry_t *) op);
        free (op);
    }
}

static unsigned char *
cff_dict_get_operands (cairo_hash_table_t *dict,
                       unsigned short      operator,
                       int                *size)
{
    cff_dict_operator_t key, *op;

    _cairo_dict_init_key (&key, operator);
    op = _cairo_hash_table_lookup (dict, &key.base);
    if (op != NULL) {
        *size = op->operand_length;
        return op->operand;
    }

    return NULL;
}

static cairo_status_t
cff_dict_set_operands (cairo_hash_table_t *dict,
                       unsigned short      operator,
                       unsigned char      *operand,
                       int                 size)
{
    cff_dict_operator_t key, *op;
    cairo_status_t status;

    _cairo_dict_init_key (&key, operator);
    op = _cairo_hash_table_lookup (dict, &key.base);
    if (op != NULL) {
        free (op->operand);
        op->operand = _cairo_malloc (size);
	if (unlikely (op->operand == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

        memcpy (op->operand, operand, size);
        op->operand_length = size;
    }
    else
    {
        status = cff_dict_create_operator (operator, operand, size, &op);
        if (unlikely (status))
	    return status;

	status = _cairo_hash_table_insert (dict, &op->base);
	if (unlikely (status))
	    return status;
    }

    return CAIRO_STATUS_SUCCESS;
}

static int
cff_dict_get_location (cairo_hash_table_t *dict,
                       unsigned short      operator,
                       int                *size)
{
    cff_dict_operator_t key, *op;

    _cairo_dict_init_key (&key, operator);
    op = _cairo_hash_table_lookup (dict, &key.base);
    if (op != NULL) {
        *size = op->operand_length;
        return op->operand_offset;
    }

    return -1;
}

typedef struct _dict_write_info {
    cairo_array_t *output;
    cairo_status_t status;
} dict_write_info_t;

static void
cairo_dict_write_operator (cff_dict_operator_t *op, dict_write_info_t *write_info)
{
    unsigned char data;

    op->operand_offset = _cairo_array_num_elements (write_info->output);
    write_info->status = _cairo_array_append_multiple (write_info->output, op->operand, op->operand_length);
    if (write_info->status)
        return;

    if (op->operator & 0xff00) {
        data = op->operator >> 8;
        write_info->status = _cairo_array_append (write_info->output, &data);
        if (write_info->status)
            return;
    }
    data = op->operator & 0xff;
    write_info->status = _cairo_array_append (write_info->output, &data);
}

static void
_cairo_dict_collect (void *entry, void *closure)
{
    dict_write_info_t   *write_info = closure;
    cff_dict_operator_t *op = entry;

    if (write_info->status)
        return;

    /* The ROS operator is handled separately in cff_dict_write() */
    if (op->operator != ROS_OP)
        cairo_dict_write_operator (op, write_info);
}

static cairo_status_t
cff_dict_write (cairo_hash_table_t *dict, cairo_array_t *output)
{
    dict_write_info_t write_info;
    cff_dict_operator_t key, *op;

    write_info.output = output;
    write_info.status = CAIRO_STATUS_SUCCESS;

    /* The CFF specification requires that the Top Dict of CID fonts
     * begin with the ROS operator. */
    _cairo_dict_init_key (&key, ROS_OP);
    op = _cairo_hash_table_lookup (dict, &key.base);
    if (op != NULL)
        cairo_dict_write_operator (op, &write_info);

    _cairo_hash_table_foreach (dict, _cairo_dict_collect, &write_info);

    return write_info.status;
}

static void
_cff_dict_entry_pluck (void *_entry, void *dict)
{
    cff_dict_operator_t *entry = _entry;

    _cairo_hash_table_remove (dict, &entry->base);
    free (entry->operand);
    free (entry);
}

static void
cff_dict_fini (cairo_hash_table_t *dict)
{
    _cairo_hash_table_foreach (dict, _cff_dict_entry_pluck, dict);
    _cairo_hash_table_destroy (dict);
}

static cairo_int_status_t
cairo_cff_font_read_header (cairo_cff_font_t *font)
{
    if (font->data_length < sizeof (cff_header_t))
        return CAIRO_INT_STATUS_UNSUPPORTED;


    font->header = (cff_header_t *) font->data;
    font->current_ptr = font->data + font->header->header_size;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
cairo_cff_font_read_name (cairo_cff_font_t *font)
{
    cairo_array_t index;
    cairo_int_status_t status;
    cff_index_element_t *element;
    unsigned char *p;
    int i, len;

    cff_index_init (&index);
    status = cff_index_read (&index, &font->current_ptr, font->data_end);
    if (status == CAIRO_INT_STATUS_SUCCESS && !font->is_opentype) {
        element = _cairo_array_index (&index, 0);
	p = element->data;
	len = element->length;

	/* If font name is prefixed with a subset tag, strip it off. */
	if (len > 7 && p[6] == '+') {
	    for (i = 0; i < 6; i++)
		if (p[i] < 'A' || p[i] > 'Z')
		    break;
	    if (i == 6) {
		p += 7;
		len -= 7;
	    }
	}

	font->ps_name = _cairo_strndup ((char*)p, len);
	if (unlikely (font->ps_name == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

        status = _cairo_escape_ps_name (&font->ps_name);
    }
    cff_index_fini (&index);

    return status;
}

static cairo_int_status_t
cairo_cff_font_read_private_dict (cairo_cff_font_t   *font,
                                  cairo_hash_table_t *private_dict,
                                  cairo_array_t      *local_sub_index,
                                  int                *local_sub_bias,
                                  cairo_bool_t      **local_subs_used,
                                  double             *default_width,
                                  double             *nominal_width,
                                  unsigned char      *ptr,
                                  int                 size)
{
    cairo_int_status_t status;
    unsigned char buf[10];
    unsigned char *end_buf;
    int offset;
    int i;
    unsigned char *operand;
    unsigned char *p;
    int num_subs;

    status = cff_dict_read (private_dict, ptr, size);
    if (unlikely (status))
	return status;

    operand = cff_dict_get_operands (private_dict, LOCAL_SUB_OP, &i);
    if (operand) {
        decode_integer (operand, &offset);
        p = ptr + offset;
        status = cff_index_read (local_sub_index, &p, font->data_end);
	if (unlikely (status))
	    return status;

	/* Use maximum sized encoding to reserve space for later modification. */
	end_buf = encode_integer_max (buf, 0);
	status = cff_dict_set_operands (private_dict, LOCAL_SUB_OP, buf, end_buf - buf);
	if (unlikely (status))
	    return status;
    }

    *default_width = 0;
    operand = cff_dict_get_operands (private_dict, DEFAULTWIDTH_OP, &i);
    if (operand)
        decode_number (operand, default_width);

    *nominal_width = 0;
    operand = cff_dict_get_operands (private_dict, NOMINALWIDTH_OP, &i);
    if (operand)
	 decode_number (operand, nominal_width);

    num_subs = _cairo_array_num_elements (local_sub_index);
    *local_subs_used = calloc (num_subs, sizeof (cairo_bool_t));
    if (unlikely (*local_subs_used == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    if (num_subs < 1240)
	*local_sub_bias = 107;
    else if (num_subs < 33900)
	*local_sub_bias = 1131;
    else
	*local_sub_bias = 32768;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
cairo_cff_font_read_fdselect (cairo_cff_font_t *font, unsigned char *p)
{
    int type, num_ranges, first, last, fd, i, j;

    font->fdselect = calloc (font->num_glyphs, sizeof (int));
    if (unlikely (font->fdselect == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    type = *p++;
    if (type == 0)
    {
        for (i = 0; i < font->num_glyphs; i++)
            font->fdselect[i] = *p++;
    } else if (type == 3) {
        num_ranges = get_unaligned_be16 (p);
        p += 2;
        for  (i = 0; i < num_ranges; i++)
        {
            first = get_unaligned_be16 (p);
            p += 2;
            fd = *p++;
            last = get_unaligned_be16 (p);
            if (last > font->num_glyphs)
                return CAIRO_INT_STATUS_UNSUPPORTED;
            for (j = first; j < last; j++)
                font->fdselect[j] = fd;
        }
    } else {
        return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
cairo_cff_font_read_cid_fontdict (cairo_cff_font_t *font, unsigned char *ptr)
{
    cairo_array_t index;
    cff_index_element_t *element;
    unsigned int i;
    int size;
    unsigned char *operand;
    int offset;
    cairo_int_status_t status;
    unsigned char buf[100];
    unsigned char *end_buf;

    cff_index_init (&index);
    status = cff_index_read (&index, &ptr, font->data_end);
    if (unlikely (status))
        goto fail;

    font->num_fontdicts = _cairo_array_num_elements (&index);

    font->fd_dict = calloc (sizeof (cairo_hash_table_t *), font->num_fontdicts);
    if (unlikely (font->fd_dict == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail;
    }

    font->fd_private_dict = calloc (sizeof (cairo_hash_table_t *), font->num_fontdicts);
    if (unlikely (font->fd_private_dict == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail;
    }

    font->fd_local_sub_index = calloc (sizeof (cairo_array_t), font->num_fontdicts);
    if (unlikely (font->fd_local_sub_index == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail;
    }

    font->fd_local_sub_bias = calloc (sizeof (int), font->num_fontdicts);
    if (unlikely (font->fd_local_sub_bias == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail;
    }

    font->fd_local_subs_used = calloc (sizeof (cairo_bool_t *), font->num_fontdicts);
    if (unlikely (font->fd_local_subs_used == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail;
    }

    font->fd_default_width = calloc (font->num_fontdicts, sizeof (double));
    if (unlikely (font->fd_default_width == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail;
    }

    font->fd_nominal_width = calloc (font->num_fontdicts, sizeof (double));
    if (unlikely (font->fd_nominal_width == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail;
    }

    for (i = 0; i < font->num_fontdicts; i++) {
        status = cff_dict_init (&font->fd_dict[i]);
        if (unlikely (status))
            goto fail;

        element = _cairo_array_index (&index, i);
        status = cff_dict_read (font->fd_dict[i], element->data, element->length);
        if (unlikely (status))
            goto fail;

        operand = cff_dict_get_operands (font->fd_dict[i], PRIVATE_OP, &size);
        if (operand == NULL) {
            status = CAIRO_INT_STATUS_UNSUPPORTED;
            goto fail;
        }
        operand = decode_integer (operand, &size);
        decode_integer (operand, &offset);
        status = cff_dict_init (&font->fd_private_dict[i]);
	if (unlikely (status))
            goto fail;

        cff_index_init (&font->fd_local_sub_index[i]);
        status = cairo_cff_font_read_private_dict (font,
                                                   font->fd_private_dict[i],
                                                   &font->fd_local_sub_index[i],
                                                   &font->fd_local_sub_bias[i],
                                                   &font->fd_local_subs_used[i],
                                                   &font->fd_default_width[i],
                                                   &font->fd_nominal_width[i],
                                                   font->data + offset,
                                                   size);
        if (unlikely (status))
            goto fail;

	/* Set integer operand to max value to use max size encoding to reserve
         * space for any value later */
        end_buf = encode_integer_max (buf, 0);
        end_buf = encode_integer_max (end_buf, 0);
        status = cff_dict_set_operands (font->fd_dict[i], PRIVATE_OP, buf, end_buf - buf);
        if (unlikely (status))
            goto fail;
    }

    status = CAIRO_STATUS_SUCCESS;

fail:
    cff_index_fini (&index);

    return status;
}

static void
cairo_cff_font_read_font_metrics (cairo_cff_font_t *font, cairo_hash_table_t  *top_dict)
{
    unsigned char *p;
    unsigned char *end;
    int size;
    double x_min, y_min, x_max, y_max;
    double xx, yx, xy, yy;

    x_min = 0.0;
    y_min = 0.0;
    x_max = 0.0;
    y_max = 0.0;
    p = cff_dict_get_operands (font->top_dict, FONTBBOX_OP, &size);
    if (p) {
        end = p + size;
        if (p < end)
            p = decode_number (p, &x_min);
        if (p < end)
            p = decode_number (p, &y_min);
        if (p < end)
            p = decode_number (p, &x_max);
        if (p < end)
            p = decode_number (p, &y_max);
    }
    font->x_min = floor (x_min);
    font->y_min = floor (y_min);
    font->x_max = floor (x_max);
    font->y_max = floor (y_max);
    font->ascent = font->y_max;
    font->descent = font->y_min;

    xx = 0.001;
    yx = 0.0;
    xy = 0.0;
    yy = 0.001;
    p = cff_dict_get_operands (font->top_dict, FONTMATRIX_OP, &size);
    if (p) {
        end = p + size;
        if (p < end)
            p = decode_number (p, &xx);
        if (p < end)
            p = decode_number (p, &yx);
        if (p < end)
            p = decode_number (p, &xy);
        if (p < end)
            p = decode_number (p, &yy);
    }
    /* Freetype uses 1/abs(yy) to get units per EM */
    font->units_per_em = _cairo_round(1.0/fabs(yy));
}

static cairo_int_status_t
cairo_cff_font_read_top_dict (cairo_cff_font_t *font)
{
    cairo_array_t index;
    cff_index_element_t *element;
    unsigned char buf[20];
    unsigned char *end_buf;
    unsigned char *operand;
    cairo_int_status_t status;
    unsigned char *p;
    int size;
    int offset;

    cff_index_init (&index);
    status = cff_index_read (&index, &font->current_ptr, font->data_end);
    if (unlikely (status))
        goto fail;

    element = _cairo_array_index (&index, 0);
    status = cff_dict_read (font->top_dict, element->data, element->length);
    if (unlikely (status))
        goto fail;

    if (cff_dict_get_operands (font->top_dict, ROS_OP, &size) != NULL)
        font->is_cid = TRUE;
    else
        font->is_cid = FALSE;

    operand = cff_dict_get_operands (font->top_dict, CHARSTRINGS_OP, &size);
    decode_integer (operand, &offset);
    p = font->data + offset;
    status = cff_index_read (&font->charstrings_index, &p, font->data_end);
    if (unlikely (status))
        goto fail;
    font->num_glyphs = _cairo_array_num_elements (&font->charstrings_index);

    if (font->is_cid) {
	 operand = cff_dict_get_operands (font->top_dict, CHARSET_OP, &size);
	 if (!operand)
	      return CAIRO_INT_STATUS_UNSUPPORTED;

	 decode_integer (operand, &offset);
	 font->charset = font->data + offset;
	 if (font->charset >= font->data_end)
	      return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    if (!font->is_opentype)
        cairo_cff_font_read_font_metrics (font, font->top_dict);

    if (font->is_cid) {
        operand = cff_dict_get_operands (font->top_dict, FDSELECT_OP, &size);
        decode_integer (operand, &offset);
        status = cairo_cff_font_read_fdselect (font, font->data + offset);
	if (unlikely (status))
	    goto fail;

        operand = cff_dict_get_operands (font->top_dict, FDARRAY_OP, &size);
        decode_integer (operand, &offset);
        status = cairo_cff_font_read_cid_fontdict (font, font->data + offset);
	if (unlikely (status))
	    goto fail;
    } else {
        operand = cff_dict_get_operands (font->top_dict, PRIVATE_OP, &size);
        operand = decode_integer (operand, &size);
        decode_integer (operand, &offset);
	status = cairo_cff_font_read_private_dict (font,
                                                   font->private_dict,
						   &font->local_sub_index,
						   &font->local_sub_bias,
						   &font->local_subs_used,
                                                   &font->default_width,
                                                   &font->nominal_width,
						   font->data + offset,
						   size);
	if (unlikely (status))
	    goto fail;
    }

    /* Use maximum sized encoding to reserve space for later modification. */
    end_buf = encode_integer_max (buf, 0);
    status = cff_dict_set_operands (font->top_dict,
	                            CHARSTRINGS_OP, buf, end_buf - buf);
    if (unlikely (status))
	goto fail;

    status = cff_dict_set_operands (font->top_dict,
	                            CHARSET_OP, buf, end_buf - buf);
    if (unlikely (status))
	goto fail;

    if (font->scaled_font_subset->is_latin) {
        status = cff_dict_set_operands (font->top_dict,
                                        ENCODING_OP, buf, end_buf - buf);
        if (unlikely (status))
            goto fail;

	/* Private has two operands - size and offset */
	end_buf = encode_integer_max (end_buf, 0);
	cff_dict_set_operands (font->top_dict, PRIVATE_OP, buf, end_buf - buf);
        if (unlikely (status))
            goto fail;

    } else {
        status = cff_dict_set_operands (font->top_dict,
                                        FDSELECT_OP, buf, end_buf - buf);
        if (unlikely (status))
            goto fail;

        status = cff_dict_set_operands (font->top_dict,
                                        FDARRAY_OP, buf, end_buf - buf);
        if (unlikely (status))
            goto fail;

        cff_dict_remove (font->top_dict, ENCODING_OP);
        cff_dict_remove (font->top_dict, PRIVATE_OP);
    }

    /* Remove the unique identifier operators as the subsetted font is
     * not the same is the original font. */
    cff_dict_remove (font->top_dict, UNIQUEID_OP);
    cff_dict_remove (font->top_dict, XUID_OP);

fail:
    cff_index_fini (&index);

    return status;
}

static cairo_int_status_t
cairo_cff_font_read_strings (cairo_cff_font_t *font)
{
    return cff_index_read (&font->strings_index, &font->current_ptr, font->data_end);
}

static cairo_int_status_t
cairo_cff_font_read_global_subroutines (cairo_cff_font_t *font)
{
    cairo_int_status_t status;
    int num_subs;

    status = cff_index_read (&font->global_sub_index, &font->current_ptr, font->data_end);
    if (unlikely (status))
	return status;

    num_subs = _cairo_array_num_elements (&font->global_sub_index);
    font->global_subs_used = calloc (num_subs, sizeof(cairo_bool_t));
    if (unlikely (font->global_subs_used == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    if (num_subs < 1240)
        font->global_sub_bias = 107;
    else if (num_subs < 33900)
        font->global_sub_bias = 1131;
    else
        font->global_sub_bias = 32768;

    return CAIRO_STATUS_SUCCESS;
}

typedef cairo_int_status_t
(*font_read_t) (cairo_cff_font_t *font);

static const font_read_t font_read_funcs[] = {
    cairo_cff_font_read_header,
    cairo_cff_font_read_name,
    cairo_cff_font_read_top_dict,
    cairo_cff_font_read_strings,
    cairo_cff_font_read_global_subroutines,
};

static cairo_int_status_t
cairo_cff_font_read_font (cairo_cff_font_t *font)
{
    cairo_int_status_t status;
    unsigned int i;

    for (i = 0; i < ARRAY_LENGTH (font_read_funcs); i++) {
        status = font_read_funcs[i] (font);
        if (unlikely (status))
            return status;
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_set_ros_strings (cairo_cff_font_t *font)
{
    cairo_status_t status;
    unsigned char buf[30];
    unsigned char *p;
    int sid1, sid2;
    const char *registry = "Adobe";
    const char *ordering = "Identity";

    sid1 = NUM_STD_STRINGS + _cairo_array_num_elements (&font->strings_subset_index);
    status = cff_index_append_copy (&font->strings_subset_index,
                                    (unsigned char *)registry,
                                    strlen(registry));
    if (unlikely (status))
	return status;

    sid2 = NUM_STD_STRINGS + _cairo_array_num_elements (&font->strings_subset_index);
    status = cff_index_append_copy (&font->strings_subset_index,
                                    (unsigned char *)ordering,
				    strlen(ordering));
    if (unlikely (status))
	return status;

    p = encode_integer (buf, sid1);
    p = encode_integer (p, sid2);
    p = encode_integer (p, 0);
    status = cff_dict_set_operands (font->top_dict, ROS_OP, buf, p - buf);
    if (unlikely (status))
	return status;

    p = encode_integer (buf, font->scaled_font_subset->num_glyphs);
    status = cff_dict_set_operands (font->top_dict, CIDCOUNT_OP, buf, p - buf);
    if (unlikely (status))
	return status;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_subset_dict_string(cairo_cff_font_t   *font,
                                  cairo_hash_table_t *dict,
                                  int                 operator)
{
    int size;
    unsigned char *p;
    int sid;
    unsigned char buf[100];
    cff_index_element_t *element;
    cairo_status_t status;

    p = cff_dict_get_operands (dict, operator, &size);
    if (!p)
        return CAIRO_STATUS_SUCCESS;

    decode_integer (p, &sid);
    if (sid < NUM_STD_STRINGS)
        return CAIRO_STATUS_SUCCESS;

    element = _cairo_array_index (&font->strings_index, sid - NUM_STD_STRINGS);
    sid = NUM_STD_STRINGS + _cairo_array_num_elements (&font->strings_subset_index);
    status = cff_index_append (&font->strings_subset_index, element->data, element->length);
    if (unlikely (status))
        return status;

    p = encode_integer (buf, sid);
    status = cff_dict_set_operands (dict, operator, buf, p - buf);
    if (unlikely (status))
	return status;

    return CAIRO_STATUS_SUCCESS;
}

static const int dict_strings[] = {
    VERSION_OP,
    NOTICE_OP,
    COPYRIGHT_OP,
    FULLNAME_OP,
    FAMILYNAME_OP,
    WEIGHT_OP,
    POSTSCRIPT_OP,
    BASEFONTNAME_OP,
    FONTNAME_OP,
};

static cairo_status_t
cairo_cff_font_subset_dict_strings (cairo_cff_font_t   *font,
                                    cairo_hash_table_t *dict)
{
    cairo_status_t status;
    unsigned int i;

    for (i = 0; i < ARRAY_LENGTH (dict_strings); i++) {
        status = cairo_cff_font_subset_dict_string (font, dict, dict_strings[i]);
        if (unlikely (status))
            return status;
    }

    return CAIRO_STATUS_SUCCESS;
}

static unsigned char *
type2_decode_integer (unsigned char *p, int *integer)
{
    if (*p == 28) {
	*integer = p[1] << 8 | p[2];
	p += 3;
    } else if (*p <= 246) {
        *integer = *p++ - 139;
    } else if (*p <= 250) {
        *integer = (p[0] - 247) * 256 + p[1] + 108;
        p += 2;
    } else if (*p <= 254) {
        *integer = -(p[0] - 251) * 256 - p[1] - 108;
        p += 2;
    } else { /* *p == 255 */
	 /* 16.16 fixed-point number. The fraction is ignored. */
	 *integer = (int16_t)((p[1] << 8) | p[2]);
        p += 5;
    }
    return p;
}

/* Type 2 charstring parser for finding calls to local or global
 * subroutines. For non Opentype CFF fonts it also gets the glyph
 * widths.
 *
 * When we find a subroutine operator, the subroutine is marked as in
 * use and recursively followed. The subroutine number is the value on
 * the top of the stack when the subroutine operator is executed. In
 * most fonts the subroutine number is encoded in an integer
 * immediately preceding the subroutine operator. However it is
 * possible for the subroutine number on the stack to be the result of
 * a computation (in which case there will be an operator preceding
 * the subroutine operator). If this occurs, subroutine subsetting is
 * disabled since we can't easily determine which subroutines are
 * used.
 *
 * The width, if present, is the first integer in the charstring. The
 * only way to confirm if the integer at the start of the charstring is
 * the width is when the first stack clearing operator is parsed,
 * check if there is an extra integer left over on the stack.
 *
 * When the first stack clearing operator is encountered
 * type2_find_width is set to FALSE and type2_found_width is set to
 * TRUE if an extra argument is found, otherwise FALSE.
 */
static cairo_status_t
cairo_cff_parse_charstring (cairo_cff_font_t *font,
                            unsigned char *charstring, int length,
                            int glyph_id,
			    cairo_bool_t need_width)
{
    unsigned char *p = charstring;
    unsigned char *end = charstring + length;
    int integer;
    int hint_bytes;
    int sub_num;
    cff_index_element_t *element;
    int fd;

    while (p < end) {
        if (*p == 28 || *p >= 32) {
            /* Integer value */
            p = type2_decode_integer (p, &integer);
            font->type2_stack_size++;
            font->type2_stack_top_value = integer;
            font->type2_stack_top_is_int = TRUE;
	    if (!font->type2_seen_first_int) {
		font->type2_width = integer;
		font->type2_seen_first_int = TRUE;
	    }
	} else if (*p == TYPE2_hstem || *p == TYPE2_vstem ||
		   *p == TYPE2_hstemhm || *p == TYPE2_vstemhm) {
            /* Hint operator. The number of hints declared by the
             * operator depends on the size of the stack. */
	    font->type2_stack_top_is_int = FALSE;
	    font->type2_num_hints += font->type2_stack_size/2;
	    if (font->type2_find_width && font->type2_stack_size % 2)
		font->type2_found_width = TRUE;

	    font->type2_stack_size = 0;
	    font->type2_find_width = FALSE;
	    p++;
	} else if (*p == TYPE2_hintmask || *p == TYPE2_cntrmask) {
	    /* Hintmask operator. These operators are followed by a
	     * variable length mask where the length depends on the
	     * number of hints declared. The first time this is called
	     * it is also an implicit vstem if there are arguments on
	     * the stack. */
	    if (font->type2_hintmask_bytes == 0) {
		font->type2_stack_top_is_int = FALSE;
		font->type2_num_hints += font->type2_stack_size/2;
		if (font->type2_find_width && font->type2_stack_size % 2)
		    font->type2_found_width = TRUE;

		font->type2_stack_size = 0;
		font->type2_find_width = FALSE;
		font->type2_hintmask_bytes = (font->type2_num_hints+7)/8;
	    }

	    hint_bytes = font->type2_hintmask_bytes;
	    p++;
	    p += hint_bytes;
	} else if (*p == TYPE2_rmoveto) {
	    if (font->type2_find_width && font->type2_stack_size > 2)
		font->type2_found_width = TRUE;

	    font->type2_stack_size = 0;
	    font->type2_find_width = FALSE;
	    font->type2_has_path = TRUE;
	    p++;
	} else if (*p == TYPE2_hmoveto || *p == TYPE2_vmoveto) {
	    if (font->type2_find_width && font->type2_stack_size > 1)
		font->type2_found_width = TRUE;

	    font->type2_stack_size = 0;
	    font->type2_find_width = FALSE;
	    font->type2_has_path = TRUE;
	    p++;
	} else if (*p == TYPE2_endchar) {
	    if (!font->type2_has_path && font->type2_stack_size > 3)
		return CAIRO_INT_STATUS_UNSUPPORTED; /* seac (Ref Appendix C of Type 2 Charstring Format */

	    if (font->type2_find_width && font->type2_stack_size > 0)
		font->type2_found_width = TRUE;

	    return CAIRO_STATUS_SUCCESS;
        } else if (*p == TYPE2_callsubr) {
            /* call to local subroutine */
	    if (! font->type2_stack_top_is_int)
		return CAIRO_INT_STATUS_UNSUPPORTED;

            if (++font->type2_nesting_level > MAX_SUBROUTINE_NESTING)
		return CAIRO_INT_STATUS_UNSUPPORTED;

            p++;
	    font->type2_stack_top_is_int = FALSE;
            font->type2_stack_size--;
	    if (font->type2_find_width && font->type2_stack_size == 0)
		font->type2_seen_first_int = FALSE;

            if (font->is_cid) {
                fd = font->fdselect[glyph_id];
		sub_num = font->type2_stack_top_value + font->fd_local_sub_bias[fd];
		if (sub_num >= (int)_cairo_array_num_elements(&font->fd_local_sub_index[fd]))
		    return CAIRO_INT_STATUS_UNSUPPORTED;
                element = _cairo_array_index (&font->fd_local_sub_index[fd], sub_num);
                if (! font->fd_local_subs_used[fd][sub_num]) {
		    font->fd_local_subs_used[fd][sub_num] = TRUE;
		    cairo_cff_parse_charstring (font, element->data, element->length, glyph_id, need_width);
		}
            } else {
		sub_num = font->type2_stack_top_value + font->local_sub_bias;
		if (sub_num >= (int)_cairo_array_num_elements(&font->local_sub_index))
		    return CAIRO_INT_STATUS_UNSUPPORTED;
                element = _cairo_array_index (&font->local_sub_index, sub_num);
                if (! font->local_subs_used[sub_num] ||
		    (need_width && !font->type2_found_width))
		{
		    font->local_subs_used[sub_num] = TRUE;
		    cairo_cff_parse_charstring (font, element->data, element->length, glyph_id, need_width);
		}
            }
            font->type2_nesting_level--;
        } else if (*p == TYPE2_callgsubr) {
            /* call to global subroutine */
	    if (! font->type2_stack_top_is_int)
		return CAIRO_INT_STATUS_UNSUPPORTED;

            if (++font->type2_nesting_level > MAX_SUBROUTINE_NESTING)
		return CAIRO_INT_STATUS_UNSUPPORTED;

            p++;
            font->type2_stack_size--;
	    font->type2_stack_top_is_int = FALSE;
	    if (font->type2_find_width && font->type2_stack_size == 0)
		font->type2_seen_first_int = FALSE;

	    sub_num = font->type2_stack_top_value + font->global_sub_bias;
	    if (sub_num >= (int)_cairo_array_num_elements(&font->global_sub_index))
		return CAIRO_INT_STATUS_UNSUPPORTED;
	    element = _cairo_array_index (&font->global_sub_index, sub_num);
            if (! font->global_subs_used[sub_num] ||
		(need_width && !font->type2_found_width))
	    {
                font->global_subs_used[sub_num] = TRUE;
                cairo_cff_parse_charstring (font, element->data, element->length, glyph_id, need_width);
            }
            font->type2_nesting_level--;
        } else if (*p == 12) {
            /* 2 byte instruction */

	    /* All the 2 byte operators are either not valid before a
	     * stack clearing operator or they are one of the
	     * arithmetic, storage, or conditional operators. */
	    if (need_width && font->type2_find_width)
		return CAIRO_INT_STATUS_UNSUPPORTED;

            p += 2;
	    font->type2_stack_top_is_int = FALSE;
	} else {
            /* 1 byte instruction */
            p++;
	    font->type2_stack_top_is_int = FALSE;
        }
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_find_width_and_subroutines_used (cairo_cff_font_t  *font,
					   unsigned char *charstring, int length,
					   int glyph_id, int subset_id)
{
    cairo_status_t status;
    int width;
    int fd;

    font->type2_stack_size = 0;
    font->type2_stack_top_value = 0;;
    font->type2_stack_top_is_int = FALSE;
    font->type2_num_hints = 0;
    font->type2_hintmask_bytes = 0;
    font->type2_nesting_level = 0;
    font->type2_seen_first_int = FALSE;
    font->type2_find_width = TRUE;
    font->type2_found_width = FALSE;
    font->type2_width = 0;
    font->type2_has_path = FALSE;

    status = cairo_cff_parse_charstring (font, charstring, length, glyph_id, TRUE);
    if (status)
	return status;

    if (!font->is_opentype) {
        if (font->is_cid) {
            fd = font->fdselect[glyph_id];
            if (font->type2_found_width)
                width = font->fd_nominal_width[fd] + font->type2_width;
            else
                width = font->fd_default_width[fd];
        } else {
            if (font->type2_found_width)
                width = font->nominal_width + font->type2_width;
            else
                width = font->default_width;
        }
        font->widths[subset_id] = width;
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
cairo_cff_font_get_gid_for_cid (cairo_cff_font_t  *font, unsigned long cid, unsigned long *gid)
{
    unsigned char *p;
    unsigned long first_gid;
    unsigned long first_cid;
    int num_left;
    unsigned long c, g;

    if (cid == 0) {
	*gid = 0;
	return CAIRO_STATUS_SUCCESS;
    }

    switch (font->charset[0]) {
	/* Format 0 */
	case 0:
	    p = font->charset + 1;
	    g = 1;
	    while (g <= (unsigned)font->num_glyphs && p < font->data_end) {
		c = get_unaligned_be16 (p);
		if (c == cid) {
		    *gid = g;
		    return CAIRO_STATUS_SUCCESS;
		}
		g++;
		p += 2;
	    }
	    break;

	/* Format 1 */
	case 1:
	    first_gid = 1;
	    p = font->charset + 1;
	    while (first_gid <= (unsigned)font->num_glyphs && p + 2 < font->data_end) {
		first_cid = get_unaligned_be16 (p);
		num_left = p[2];
		if (cid >= first_cid && cid <= first_cid + num_left) {
		    *gid = first_gid + cid - first_cid;
		    return CAIRO_STATUS_SUCCESS;
		}
		first_gid += num_left + 1;
		p += 3;
	    }
	    break;

	/* Format 2 */
	case 2:
	    first_gid = 1;
	    p = font->charset + 1;
	    while (first_gid <= (unsigned)font->num_glyphs && p + 3 < font->data_end) {
		first_cid = get_unaligned_be16 (p);
		num_left = get_unaligned_be16 (p+2);
		if (cid >= first_cid && cid <= first_cid + num_left) {
		    *gid = first_gid + cid - first_cid;
		    return CAIRO_STATUS_SUCCESS;
		}
		first_gid += num_left + 1;
		p += 4;
	    }
	    break;

	default:
	    break;
    }
    return CAIRO_INT_STATUS_UNSUPPORTED;
}

static cairo_int_status_t
cairo_cff_font_subset_charstrings_and_subroutines (cairo_cff_font_t  *font)
{
    cff_index_element_t *element;
    unsigned int i;
    cairo_int_status_t status;
    unsigned long glyph, cid;

    font->subset_subroutines = TRUE;
    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++) {
	if (font->is_cid && !font->is_opentype) {
	    cid = font->scaled_font_subset->glyphs[i];
	    status = cairo_cff_font_get_gid_for_cid (font, cid, &glyph);
	    if (unlikely (status))
		return status;
	} else {
	    glyph = font->scaled_font_subset->glyphs[i];
	}
        element = _cairo_array_index (&font->charstrings_index, glyph);
        status = cff_index_append (&font->charstrings_subset_index,
                                   element->data,
                                   element->length);
        if (unlikely (status))
            return status;

	if (font->subset_subroutines) {
	    status = cairo_cff_find_width_and_subroutines_used (font,
								element->data, element->length,
								glyph, i);
	    if (status == CAIRO_INT_STATUS_UNSUPPORTED) {
		/* If parsing the charstrings fails we embed all the
		 * subroutines. But if the font is not opentype we
		 * need to successfully parse all charstrings to get
		 * the widths. */
		font->subset_subroutines = FALSE;
		if (!font->is_opentype)
		    return status;
	    } else if (unlikely (status)) {
                return status;
	    }
        }
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_subset_fontdict (cairo_cff_font_t  *font)
{
    unsigned int i;
    int fd;
    int *reverse_map;
    unsigned long cid, gid;
    cairo_int_status_t status;

    font->fdselect_subset = calloc (font->scaled_font_subset->num_glyphs,
                                     sizeof (int));
    if (unlikely (font->fdselect_subset == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    font->fd_subset_map = calloc (font->num_fontdicts, sizeof (int));
    if (unlikely (font->fd_subset_map == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    font->private_dict_offset = calloc (font->num_fontdicts, sizeof (int));
    if (unlikely (font->private_dict_offset == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    reverse_map = calloc (font->num_fontdicts, sizeof (int));
    if (unlikely (reverse_map == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    for (i = 0; i < font->num_fontdicts; i++)
        reverse_map[i] = -1;

    font->num_subset_fontdicts = 0;
    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++) {
	if (font->is_opentype) {
	    gid = font->scaled_font_subset->glyphs[i];
	} else {
	    cid = font->scaled_font_subset->glyphs[i];
	    status = cairo_cff_font_get_gid_for_cid (font, cid, &gid);
	    if (unlikely (status)) {
		free (reverse_map);
		return status;
	    }
	}

        fd = font->fdselect[gid];
        if (reverse_map[fd] < 0) {
            font->fd_subset_map[font->num_subset_fontdicts] = fd;
            reverse_map[fd] = font->num_subset_fontdicts++;
        }
        font->fdselect_subset[i] = reverse_map[fd];
    }

    free (reverse_map);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_create_cid_fontdict (cairo_cff_font_t *font)
{
    unsigned char buf[100];
    unsigned char *end_buf;
    cairo_status_t status;

    font->num_fontdicts = 1;
    font->fd_dict = _cairo_malloc (sizeof (cairo_hash_table_t *));
    if (unlikely (font->fd_dict == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    if (cff_dict_init (&font->fd_dict[0])) {
	free (font->fd_dict);
	font->fd_dict = NULL;
	font->num_fontdicts = 0;
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    font->fd_subset_map = _cairo_malloc (sizeof (int));
    if (unlikely (font->fd_subset_map == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    font->private_dict_offset = _cairo_malloc (sizeof (int));
    if (unlikely (font->private_dict_offset == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    font->fd_subset_map[0] = 0;
    font->num_subset_fontdicts = 1;

    /* Set integer operand to max value to use max size encoding to reserve
     * space for any value later */
    end_buf = encode_integer_max (buf, 0);
    end_buf = encode_integer_max (end_buf, 0);
    status = cff_dict_set_operands (font->fd_dict[0], PRIVATE_OP, buf, end_buf - buf);
    if (unlikely (status))
	return status;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_subset_strings (cairo_cff_font_t *font)
{
    cairo_status_t status;
    unsigned int i;

    status = cairo_cff_font_subset_dict_strings (font, font->top_dict);
    if (unlikely (status))
        return status;

    if (font->is_cid) {
        for (i = 0; i < font->num_subset_fontdicts; i++) {
            status = cairo_cff_font_subset_dict_strings (font, font->fd_dict[font->fd_subset_map[i]]);
            if (unlikely (status))
                return status;

            status = cairo_cff_font_subset_dict_strings (font, font->fd_private_dict[font->fd_subset_map[i]]);
            if (unlikely (status))
                return status;
        }
    } else {
        status = cairo_cff_font_subset_dict_strings (font, font->private_dict);
    }

    return status;
}

/* The Euro is the only the only character in the winansi encoding
 * with a glyph name that is not a CFF standard string. As the strings
 * are written before the charset, we need to check during the
 * subsetting phase if the Euro glyph is required and add the
 * glyphname to the list of strings to write out.
 */
static cairo_status_t
cairo_cff_font_add_euro_charset_string (cairo_cff_font_t *font)
{
    cairo_status_t status;
    unsigned int i;
    int ch;
    const char *euro = "Euro";

    for (i = 1; i < font->scaled_font_subset->num_glyphs; i++) {
	ch = font->scaled_font_subset->to_latin_char[i];
	if (ch == 128) {
	    font->euro_sid = NUM_STD_STRINGS + _cairo_array_num_elements (&font->strings_subset_index);
	    status = cff_index_append_copy (&font->strings_subset_index,
					    (unsigned char *)euro, strlen(euro));
	    return status;
	}
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_subset_font (cairo_cff_font_t  *font)
{
    cairo_status_t status;

    if (!font->scaled_font_subset->is_latin) {
	status = cairo_cff_font_set_ros_strings (font);
	if (unlikely (status))
	    return status;
    }

    status = cairo_cff_font_subset_charstrings_and_subroutines (font);
    if (unlikely (status))
        return status;

    if (!font->scaled_font_subset->is_latin) {
	if (font->is_cid)
	    status = cairo_cff_font_subset_fontdict (font);
	else
	    status = cairo_cff_font_create_cid_fontdict (font);
	if (unlikely (status))
	    return status;
    }  else {
	font->private_dict_offset = _cairo_malloc (sizeof (int));
	if (unlikely (font->private_dict_offset == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    status = cairo_cff_font_subset_strings (font);
    if (unlikely (status))
        return status;

    if (font->scaled_font_subset->is_latin)
	status = cairo_cff_font_add_euro_charset_string (font);

    return status;
}

/* Set the operand of the specified operator in the (already written)
 * top dict to point to the current position in the output
 * array. Operands updated with this function must have previously
 * been encoded with the 5-byte (max) integer encoding. */
static void
cairo_cff_font_set_topdict_operator_to_cur_pos (cairo_cff_font_t  *font,
                                                int                operator)
{
    int cur_pos;
    int offset;
    int size;
    unsigned char buf[10];
    unsigned char *buf_end;
    unsigned char *op_ptr;

    cur_pos = _cairo_array_num_elements (&font->output);
    buf_end = encode_integer_max (buf, cur_pos);
    offset = cff_dict_get_location (font->top_dict, operator, &size);
    assert (offset > 0);
    op_ptr = _cairo_array_index (&font->output, offset);
    memcpy (op_ptr, buf, buf_end - buf);
}

static cairo_status_t
cairo_cff_font_write_header (cairo_cff_font_t *font)
{
    return _cairo_array_append_multiple (&font->output,
                                         font->header,
                                         font->header->header_size);
}

static cairo_status_t
cairo_cff_font_write_name (cairo_cff_font_t *font)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;
    cairo_array_t index;

    cff_index_init (&index);

    status = cff_index_append_copy (&index,
                                    (unsigned char *) font->ps_name,
                                    strlen(font->ps_name));
    if (unlikely (status))
	goto FAIL;

    status = cff_index_write (&index, &font->output);
    if (unlikely (status))
        goto FAIL;

FAIL:
    cff_index_fini (&index);

    return status;
}

static cairo_status_t
cairo_cff_font_write_top_dict (cairo_cff_font_t *font)
{
    uint16_t count;
    unsigned char buf[10];
    unsigned char *p;
    int offset_index;
    int dict_start, dict_size;
    int offset_size = 4;
    cairo_status_t status;

    /* Write an index containing the top dict */

    count = cpu_to_be16 (1);
    status = _cairo_array_append_multiple (&font->output, &count, 2);
    if (unlikely (status))
        return status;
    buf[0] = offset_size;
    status = _cairo_array_append (&font->output, buf);
    if (unlikely (status))
        return status;
    encode_index_offset (buf, offset_size, 1);
    status = _cairo_array_append_multiple (&font->output, buf, offset_size);
    if (unlikely (status))
        return status;

    /* Reserve space for last element of offset array and update after
     * dict is written */
    offset_index = _cairo_array_num_elements (&font->output);
    status = _cairo_array_append_multiple (&font->output, buf, offset_size);
    if (unlikely (status))
        return status;

    dict_start = _cairo_array_num_elements (&font->output);
    status = cff_dict_write (font->top_dict, &font->output);
    if (unlikely (status))
        return status;
    dict_size = _cairo_array_num_elements (&font->output) - dict_start;

    encode_index_offset (buf, offset_size, dict_size + 1);
    p = _cairo_array_index (&font->output, offset_index);
    memcpy (p, buf, offset_size);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_write_strings (cairo_cff_font_t  *font)
{
    return cff_index_write (&font->strings_subset_index, &font->output);
}

static cairo_status_t
cairo_cff_font_write_global_subrs (cairo_cff_font_t  *font)
{
    unsigned int i;
    unsigned char return_op = TYPE2_return;

    /* poppler and fontforge don't like zero length subroutines so we
     * replace unused subroutines with a 'return' instruction. */
    if (font->subset_subroutines) {
        for (i = 0; i < _cairo_array_num_elements (&font->global_sub_index); i++) {
            if (! font->global_subs_used[i])
                cff_index_set_object (&font->global_sub_index, i, &return_op, 1);
        }
    }

    return cff_index_write (&font->global_sub_index, &font->output);
}

static cairo_status_t
cairo_cff_font_write_encoding (cairo_cff_font_t  *font)
{
    unsigned char buf[2];
    cairo_status_t status;
    unsigned int i;

    cairo_cff_font_set_topdict_operator_to_cur_pos (font, ENCODING_OP);
    buf[0] = 0; /* Format 0 */
    buf[1] = font->scaled_font_subset->num_glyphs - 1;
    status = _cairo_array_append_multiple (&font->output, buf, 2);
    if (unlikely (status))
	return status;

    for (i = 1; i < font->scaled_font_subset->num_glyphs; i++) {
	unsigned char ch = font->scaled_font_subset->to_latin_char[i];
        status = _cairo_array_append (&font->output, &ch);
        if (unlikely (status))
            return status;
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_write_fdselect (cairo_cff_font_t  *font)
{
    unsigned char data;
    unsigned int i;
    cairo_int_status_t status;

    cairo_cff_font_set_topdict_operator_to_cur_pos (font, FDSELECT_OP);

    if (font->is_cid) {
        data = 0;
        status = _cairo_array_append (&font->output, &data);
        if (unlikely (status))
            return status;

        for (i = 0; i < font->scaled_font_subset->num_glyphs; i++) {
            data = font->fdselect_subset[i];
            status = _cairo_array_append (&font->output, &data);
            if (unlikely (status))
                return status;
        }
    } else {
        unsigned char byte;
        uint16_t word;

        status = _cairo_array_grow_by (&font->output, 9);
        if (unlikely (status))
            return status;

        byte = 3;
        status = _cairo_array_append (&font->output, &byte);
        assert (status == CAIRO_INT_STATUS_SUCCESS);

        word = cpu_to_be16 (1);
        status = _cairo_array_append_multiple (&font->output, &word, 2);
        assert (status == CAIRO_INT_STATUS_SUCCESS);

        word = cpu_to_be16 (0);
        status = _cairo_array_append_multiple (&font->output, &word, 2);
        assert (status == CAIRO_INT_STATUS_SUCCESS);

        byte = 0;
        status = _cairo_array_append (&font->output, &byte);
        assert (status == CAIRO_INT_STATUS_SUCCESS);

        word = cpu_to_be16 (font->scaled_font_subset->num_glyphs);
        status = _cairo_array_append_multiple (&font->output, &word, 2);
        assert (status == CAIRO_INT_STATUS_SUCCESS);
    }

    return CAIRO_STATUS_SUCCESS;
}

/* Winansi to CFF standard strings mapping for characters 128 to 255 */
static const int winansi_to_cff_std_string[] = {
	/* 128 */
      0,   0, 117, 101, 118, 121, 112, 113,
    126, 122, 192, 107, 142,   0, 199,   0,
	/* 144 */
      0,  65,   8, 105, 119, 116, 111, 137,
    127, 153, 221, 108, 148,   0, 228, 198,
	/* 160 */
      0,  96,  97,  98, 103, 100, 160, 102,
    131, 170, 139, 106, 151,   0, 165, 128,
	/* 176 */
    161, 156, 164, 169, 125, 152, 115, 114,
    133, 150, 143, 120, 158, 155, 163, 123,
	/* 192 */
    174, 171, 172, 176, 173, 175, 138, 177,
    181, 178, 179, 180, 185, 182, 183, 184,
	/* 208 */
    154, 186, 190, 187, 188, 191, 189, 168,
    141, 196, 193, 194, 195, 197, 157, 149,
	/* 224 */
    203, 200, 201, 205, 202, 204, 144, 206,
    210, 207, 208, 209, 214, 211, 212, 213,
	/* 240 */
    167, 215, 219, 216, 217, 220, 218, 159,
    147, 225, 222, 223, 224, 226, 162, 227,
};

static int
cairo_cff_font_get_sid_for_winansi_char (cairo_cff_font_t  *font, int ch)
{
    int sid;

    if (ch == 39) {
	sid = 104;

    } else if (ch == 96) {
	sid = 124;

    } else if (ch >= 32 && ch <= 126) {
	sid = ch - 31;

    } else if (ch == 128) {
	assert (font->euro_sid >= NUM_STD_STRINGS);
	sid = font->euro_sid;

    } else if (ch >= 128 && ch <= 255) {
	sid = winansi_to_cff_std_string[ch - 128];

    } else {
	sid = 0;
    }

    return sid;
}

static cairo_status_t
cairo_cff_font_write_type1_charset (cairo_cff_font_t  *font)
{
    unsigned char format = 0;
    unsigned int i;
    int ch, sid;
    cairo_status_t status;
    uint16_t sid_be16;

    cairo_cff_font_set_topdict_operator_to_cur_pos (font, CHARSET_OP);
    status = _cairo_array_append (&font->output, &format);
    if (unlikely (status))
	return status;

    for (i = 1; i < font->scaled_font_subset->num_glyphs; i++) {
	ch = font->scaled_font_subset->to_latin_char[i];
	sid = cairo_cff_font_get_sid_for_winansi_char (font, ch);
        if (unlikely (status))
	    return status;

	sid_be16 = cpu_to_be16(sid);
	status = _cairo_array_append_multiple (&font->output, &sid_be16, sizeof(sid_be16));
        if (unlikely (status))
            return status;
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_write_cid_charset (cairo_cff_font_t  *font)
{
    unsigned char byte;
    uint16_t word;
    cairo_status_t status;

    cairo_cff_font_set_topdict_operator_to_cur_pos (font, CHARSET_OP);
    status = _cairo_array_grow_by (&font->output, 5);
    if (unlikely (status))
        return status;

    byte = 2;
    status = _cairo_array_append (&font->output, &byte);
    assert (status == CAIRO_STATUS_SUCCESS);

    word = cpu_to_be16 (1);
    status = _cairo_array_append_multiple (&font->output, &word, 2);
    assert (status == CAIRO_STATUS_SUCCESS);

    word = cpu_to_be16 (font->scaled_font_subset->num_glyphs - 2);
    status = _cairo_array_append_multiple (&font->output, &word, 2);
    assert (status == CAIRO_STATUS_SUCCESS);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_write_charstrings (cairo_cff_font_t  *font)
{
    cairo_cff_font_set_topdict_operator_to_cur_pos (font, CHARSTRINGS_OP);

    return cff_index_write (&font->charstrings_subset_index, &font->output);
}

static cairo_status_t
cairo_cff_font_write_cid_fontdict (cairo_cff_font_t *font)
{
    unsigned int i;
    cairo_int_status_t status;
    unsigned int offset_array;
    unsigned char *offset_array_ptr;
    int offset_base;
    uint16_t count;
    uint8_t offset_size = 4;

    cairo_cff_font_set_topdict_operator_to_cur_pos (font, FDARRAY_OP);
    count = cpu_to_be16 (font->num_subset_fontdicts);
    status = _cairo_array_append_multiple (&font->output, &count, sizeof (uint16_t));
    if (unlikely (status))
        return status;
    status = _cairo_array_append (&font->output, &offset_size);
    if (unlikely (status))
        return status;

    offset_array = _cairo_array_num_elements (&font->output);
    status = _cairo_array_allocate (&font->output,
                                    (font->num_subset_fontdicts + 1)*offset_size,
                                    (void **) &offset_array_ptr);
    if (unlikely (status))
        return status;
    offset_base = _cairo_array_num_elements (&font->output) - 1;
    put_unaligned_be32(1, offset_array_ptr);
    offset_array += sizeof(uint32_t);
    for (i = 0; i < font->num_subset_fontdicts; i++) {
        status = cff_dict_write (font->fd_dict[font->fd_subset_map[i]],
                                 &font->output);
        if (unlikely (status))
            return status;

	offset_array_ptr = _cairo_array_index (&font->output, offset_array);
	put_unaligned_be32 (_cairo_array_num_elements (&font->output) - offset_base,
			    offset_array_ptr);
	offset_array += sizeof(uint32_t);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_write_private_dict (cairo_cff_font_t   *font,
				   int                 dict_num,
				   cairo_hash_table_t *parent_dict,
				   cairo_hash_table_t *private_dict)
{
    int offset;
    int size;
    unsigned char buf[10];
    unsigned char *buf_end;
    unsigned char *p;
    cairo_status_t status;

    /* Write private dict and update offset and size in top dict */
    font->private_dict_offset[dict_num] = _cairo_array_num_elements (&font->output);
    status = cff_dict_write (private_dict, &font->output);
    if (unlikely (status))
        return status;

    size = _cairo_array_num_elements (&font->output) - font->private_dict_offset[dict_num];
    /* private entry has two operands - size and offset */
    buf_end = encode_integer_max (buf, size);
    buf_end = encode_integer_max (buf_end, font->private_dict_offset[dict_num]);
    offset = cff_dict_get_location (parent_dict, PRIVATE_OP, &size);
    assert (offset > 0);
    p = _cairo_array_index (&font->output, offset);
    memcpy (p, buf, buf_end - buf);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_write_local_sub (cairo_cff_font_t   *font,
                                int                 dict_num,
                                cairo_hash_table_t *private_dict,
                                cairo_array_t      *local_sub_index,
                                cairo_bool_t       *local_subs_used)
{
    int offset;
    int size;
    unsigned char buf[10];
    unsigned char *buf_end;
    unsigned char *p;
    cairo_status_t status;
    unsigned int i;
    unsigned char return_op = TYPE2_return;

    if (_cairo_array_num_elements (local_sub_index) > 0) {
        /* Write local subroutines and update offset in private
         * dict. Local subroutines offset is relative to start of
         * private dict */
        offset = _cairo_array_num_elements (&font->output) - font->private_dict_offset[dict_num];
        buf_end = encode_integer_max (buf, offset);
        offset = cff_dict_get_location (private_dict, LOCAL_SUB_OP, &size);
        assert (offset > 0);
        p = _cairo_array_index (&font->output, offset);
        memcpy (p, buf, buf_end - buf);

	/* poppler and fontforge don't like zero length subroutines so
	 * we replace unused subroutines with a 'return' instruction.
	 */
        if (font->subset_subroutines) {
            for (i = 0; i < _cairo_array_num_elements (local_sub_index); i++) {
                if (! local_subs_used[i])
                    cff_index_set_object (local_sub_index, i, &return_op, 1);
            }
        }
        status = cff_index_write (local_sub_index, &font->output);
        if (unlikely (status))
            return status;
    }

    return CAIRO_STATUS_SUCCESS;
}


static cairo_status_t
cairo_cff_font_write_cid_private_dict_and_local_sub (cairo_cff_font_t  *font)
{
    unsigned int i;
    cairo_int_status_t status;

    if (font->is_cid) {
        for (i = 0; i < font->num_subset_fontdicts; i++) {
            status = cairo_cff_font_write_private_dict (
                            font,
                            i,
                            font->fd_dict[font->fd_subset_map[i]],
                            font->fd_private_dict[font->fd_subset_map[i]]);
            if (unlikely (status))
                return status;
        }

        for (i = 0; i < font->num_subset_fontdicts; i++) {
            status = cairo_cff_font_write_local_sub (
                            font,
                            i,
                            font->fd_private_dict[font->fd_subset_map[i]],
                            &font->fd_local_sub_index[font->fd_subset_map[i]],
                            font->fd_local_subs_used[font->fd_subset_map[i]]);
            if (unlikely (status))
                return status;
        }
    } else {
        status = cairo_cff_font_write_private_dict (font,
                                                    0,
                                                    font->fd_dict[0],
                                                    font->private_dict);
	if (unlikely (status))
	    return status;

        status = cairo_cff_font_write_local_sub (font,
                                                 0,
                                                 font->private_dict,
                                                 &font->local_sub_index,
                                                 font->local_subs_used);
	if (unlikely (status))
	    return status;
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
cairo_cff_font_write_type1_private_dict_and_local_sub (cairo_cff_font_t  *font)
{
    cairo_int_status_t status;

    status = cairo_cff_font_write_private_dict (font,
						0,
						font->top_dict,
						font->private_dict);
    if (unlikely (status))
	return status;

    status = cairo_cff_font_write_local_sub (font,
					     0,
					     font->private_dict,
					     &font->local_sub_index,
					     font->local_subs_used);
    if (unlikely (status))
	return status;

    return CAIRO_STATUS_SUCCESS;
}


typedef cairo_status_t
(*font_write_t) (cairo_cff_font_t *font);

static const font_write_t font_write_cid_funcs[] = {
    cairo_cff_font_write_header,
    cairo_cff_font_write_name,
    cairo_cff_font_write_top_dict,
    cairo_cff_font_write_strings,
    cairo_cff_font_write_global_subrs,
    cairo_cff_font_write_cid_charset,
    cairo_cff_font_write_fdselect,
    cairo_cff_font_write_charstrings,
    cairo_cff_font_write_cid_fontdict,
    cairo_cff_font_write_cid_private_dict_and_local_sub,
};

static const font_write_t font_write_type1_funcs[] = {
    cairo_cff_font_write_header,
    cairo_cff_font_write_name,
    cairo_cff_font_write_top_dict,
    cairo_cff_font_write_strings,
    cairo_cff_font_write_global_subrs,
    cairo_cff_font_write_encoding,
    cairo_cff_font_write_type1_charset,
    cairo_cff_font_write_charstrings,
    cairo_cff_font_write_type1_private_dict_and_local_sub,
};

static cairo_status_t
cairo_cff_font_write_subset (cairo_cff_font_t *font)
{
    cairo_int_status_t status;
    unsigned int i;

    if (font->scaled_font_subset->is_latin) {
        for (i = 0; i < ARRAY_LENGTH (font_write_type1_funcs); i++) {
            status = font_write_type1_funcs[i] (font);
            if (unlikely (status))
                return status;
        }
    } else {
        for (i = 0; i < ARRAY_LENGTH (font_write_cid_funcs); i++) {
            status = font_write_cid_funcs[i] (font);
            if (unlikely (status))
                return status;
        }
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
cairo_cff_font_generate (cairo_cff_font_t  *font,
                         const char       **data,
                         unsigned long     *length)
{
    cairo_int_status_t status;

    status = cairo_cff_font_read_font (font);
    if (unlikely (status))
        return status;

    /* If the PS name is not found, create a CairoFont-x-y name. */
    if (font->ps_name == NULL) {
        font->ps_name = _cairo_malloc (30);
        if (unlikely (font->ps_name == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);

        snprintf(font->ps_name, 30, "CairoFont-%u-%u",
                 font->scaled_font_subset->font_id,
                 font->scaled_font_subset->subset_id);
    }

    status = cairo_cff_font_subset_font (font);
    if (unlikely (status))
        return status;

    status = cairo_cff_font_write_subset (font);
    if (unlikely (status))
        return status;


    *data = _cairo_array_index (&font->output, 0);
    *length = _cairo_array_num_elements (&font->output);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
cairo_cff_font_create_set_widths (cairo_cff_font_t *font)
{
    unsigned long size;
    unsigned long long_entry_size;
    unsigned long short_entry_size;
    unsigned int i;
    tt_hhea_t hhea;
    int num_hmetrics;
    uint16_t short_entry;
    int glyph_index;
    cairo_int_status_t status;

    size = sizeof (tt_hhea_t);
    status = font->backend->load_truetype_table (font->scaled_font_subset->scaled_font,
                                                 TT_TAG_hhea, 0,
                                                 (unsigned char*) &hhea, &size);
    if (unlikely (status))
        return status;
    num_hmetrics = be16_to_cpu (hhea.num_hmetrics);

    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++) {
        glyph_index = font->scaled_font_subset->glyphs[i];
        long_entry_size = 2 * sizeof (int16_t);
        short_entry_size = sizeof (int16_t);
        if (glyph_index < num_hmetrics) {
            status = font->backend->load_truetype_table (font->scaled_font_subset->scaled_font,
                                                         TT_TAG_hmtx,
                                                         glyph_index * long_entry_size,
                                                         (unsigned char *) &short_entry,
							 &short_entry_size);
            if (unlikely (status))
                return status;
        }
        else
        {
            status = font->backend->load_truetype_table (font->scaled_font_subset->scaled_font,
                                                         TT_TAG_hmtx,
                                                         (num_hmetrics - 1) * long_entry_size,
                                                         (unsigned char *) &short_entry,
							 &short_entry_size);
            if (unlikely (status))
                return status;
        }
	font->widths[i] = be16_to_cpu (short_entry);
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
check_fontdata_is_cff (const unsigned char *data, long length)
{
    cff_header_t *header;

    if (length < (long)sizeof (cff_header_t))
        return FALSE;

    header = (cff_header_t *) data;
    if (header->major == 1 &&
	header->minor == 0 &&
	header->header_size == 4)
    {
	return TRUE;
    }

    return FALSE;
}

static cairo_int_status_t
_cairo_cff_font_load_opentype_cff (cairo_cff_font_t  *font)
{
    const cairo_scaled_font_backend_t *backend = font->backend;
    cairo_status_t status;
    tt_head_t head;
    tt_hhea_t hhea;
    unsigned long size, data_length;

    if (!backend->load_truetype_table)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    data_length = 0;
    status = backend->load_truetype_table (font->scaled_font_subset->scaled_font,
                                           TT_TAG_CFF, 0, NULL, &data_length);
    if (status)
        return status;

    size = sizeof (tt_head_t);
    status = backend->load_truetype_table (font->scaled_font_subset->scaled_font,
                                           TT_TAG_head, 0,
                                           (unsigned char *) &head, &size);
    if (unlikely (status))
        return status;

    size = sizeof (tt_hhea_t);
    status = backend->load_truetype_table (font->scaled_font_subset->scaled_font,
                                           TT_TAG_hhea, 0,
                                           (unsigned char *) &hhea, &size);
    if (unlikely (status))
        return status;

    size = 0;
    status = backend->load_truetype_table (font->scaled_font_subset->scaled_font,
                                           TT_TAG_hmtx, 0, NULL, &size);
    if (unlikely (status))
        return status;

    font->x_min = (int16_t) be16_to_cpu (head.x_min);
    font->y_min = (int16_t) be16_to_cpu (head.y_min);
    font->x_max = (int16_t) be16_to_cpu (head.x_max);
    font->y_max = (int16_t) be16_to_cpu (head.y_max);
    font->ascent = (int16_t) be16_to_cpu (hhea.ascender);
    font->descent = (int16_t) be16_to_cpu (hhea.descender);
    font->units_per_em = (int16_t) be16_to_cpu (head.units_per_em);
    if (font->units_per_em == 0)
        font->units_per_em = 1000;

    font->font_name = NULL;
    status = _cairo_truetype_read_font_name (font->scaled_font_subset->scaled_font,
					     &font->ps_name,
					     &font->font_name);
    if (_cairo_status_is_error (status))
	return status;

    font->is_opentype = TRUE;
    font->data_length = data_length;
    font->data = _cairo_malloc (data_length);
    if (unlikely (font->data == NULL))
        return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    status = font->backend->load_truetype_table (font->scaled_font_subset->scaled_font,
						 TT_TAG_CFF, 0, font->data,
						 &font->data_length);
    if (unlikely (status))
        return status;

    if (!check_fontdata_is_cff (font->data, data_length))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_cff_font_load_cff (cairo_cff_font_t  *font)
{
    const cairo_scaled_font_backend_t *backend = font->backend;
    cairo_status_t status;
    unsigned long data_length;

    if (!backend->load_type1_data)
	return CAIRO_INT_STATUS_UNSUPPORTED;

    data_length = 0;
    status = backend->load_type1_data (font->scaled_font_subset->scaled_font,
				      0, NULL, &data_length);
    if (unlikely (status))
        return status;

    font->font_name = NULL;
    font->is_opentype = FALSE;
    font->data_length = data_length;
    font->data = _cairo_malloc (data_length);
    if (unlikely (font->data == NULL))
        return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    status = font->backend->load_type1_data (font->scaled_font_subset->scaled_font,
					     0, font->data, &font->data_length);
    if (unlikely (status))
        return status;

    if (!check_fontdata_is_cff (font->data, data_length))
	return CAIRO_INT_STATUS_UNSUPPORTED;

    return CAIRO_STATUS_SUCCESS;
}

static cairo_int_status_t
_cairo_cff_font_create (cairo_scaled_font_subset_t  *scaled_font_subset,
                        cairo_cff_font_t           **font_return,
                        const char                  *subset_name)
{
    const cairo_scaled_font_backend_t *backend;
    cairo_int_status_t status;
    cairo_bool_t is_synthetic;
    cairo_cff_font_t *font;

    backend = scaled_font_subset->scaled_font->backend;

    /* We need to use a fallback font if this font differs from the CFF outlines. */
    if (backend->is_synthetic) {
	status = backend->is_synthetic (scaled_font_subset->scaled_font, &is_synthetic);
	if (unlikely (status))
	    return status;

	if (is_synthetic)
	    return CAIRO_INT_STATUS_UNSUPPORTED;
    }

    font = calloc (1, sizeof (cairo_cff_font_t));
    if (unlikely (font == NULL))
        return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    font->backend = backend;
    font->scaled_font_subset = scaled_font_subset;

    status = _cairo_cff_font_load_opentype_cff (font);
    if (status == CAIRO_INT_STATUS_UNSUPPORTED)
	status = _cairo_cff_font_load_cff (font);
    if (status)
	goto fail1;

    font->data_end = font->data + font->data_length;
    _cairo_array_init (&font->output, sizeof (char));
    status = _cairo_array_grow_by (&font->output, 4096);
    if (unlikely (status))
	goto fail2;

    font->subset_font_name = strdup (subset_name);
    if (unlikely (font->subset_font_name == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail2;
    }

    font->widths = calloc (font->scaled_font_subset->num_glyphs, sizeof (int));
    if (unlikely (font->widths == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail3;
    }

    if (font->is_opentype) {
	status = cairo_cff_font_create_set_widths (font);
	if (unlikely (status))
	    goto fail4;
    }

    status = cff_dict_init (&font->top_dict);
    if (unlikely (status))
	goto fail4;

    status = cff_dict_init (&font->private_dict);
    if (unlikely (status))
	goto fail5;

    cff_index_init (&font->strings_index);
    cff_index_init (&font->charstrings_index);
    cff_index_init (&font->global_sub_index);
    cff_index_init (&font->local_sub_index);
    cff_index_init (&font->charstrings_subset_index);
    cff_index_init (&font->strings_subset_index);
    font->euro_sid = 0;
    font->fdselect = NULL;
    font->fd_dict = NULL;
    font->fd_private_dict = NULL;
    font->fd_local_sub_index = NULL;
    font->fd_local_sub_bias = NULL;
    font->fdselect_subset = NULL;
    font->fd_subset_map = NULL;
    font->private_dict_offset = NULL;
    font->global_subs_used = NULL;
    font->local_subs_used = NULL;
    font->fd_local_subs_used = NULL;

    *font_return = font;

    return CAIRO_STATUS_SUCCESS;

fail5:
    _cairo_hash_table_destroy (font->top_dict);
fail4:
    free (font->widths);
fail3:
    free (font->subset_font_name);
fail2:
    free (font->ps_name);
    _cairo_array_fini (&font->output);
fail1:
    free (font->data);
    free (font->font_name);
    free (font);

    return status;
}

static void
cairo_cff_font_destroy (cairo_cff_font_t *font)
{
    unsigned int i;

    free (font->widths);
    free (font->font_name);
    free (font->ps_name);
    free (font->subset_font_name);
    _cairo_array_fini (&font->output);
    cff_dict_fini (font->top_dict);
    cff_dict_fini (font->private_dict);
    cff_index_fini (&font->strings_index);
    cff_index_fini (&font->charstrings_index);
    cff_index_fini (&font->global_sub_index);
    cff_index_fini (&font->local_sub_index);
    cff_index_fini (&font->charstrings_subset_index);
    cff_index_fini (&font->strings_subset_index);

    /* If we bailed out early as a result of an error some of the
     * following cairo_cff_font_t members may still be NULL */
    if (font->fd_dict) {
        for (i = 0; i < font->num_fontdicts; i++) {
            if (font->fd_dict[i])
                cff_dict_fini (font->fd_dict[i]);
        }
        free (font->fd_dict);
    }
    free (font->global_subs_used);
    free (font->local_subs_used);
    free (font->fd_subset_map);
    free (font->private_dict_offset);

    if (font->is_cid) {
	free (font->fdselect);
	free (font->fdselect_subset);
        if (font->fd_private_dict) {
            for (i = 0; i < font->num_fontdicts; i++) {
                if (font->fd_private_dict[i])
                    cff_dict_fini (font->fd_private_dict[i]);
            }
            free (font->fd_private_dict);
        }
        if (font->fd_local_sub_index) {
            for (i = 0; i < font->num_fontdicts; i++)
                cff_index_fini (&font->fd_local_sub_index[i]);
            free (font->fd_local_sub_index);
        }
	free (font->fd_local_sub_bias);
        if (font->fd_local_subs_used) {
            for (i = 0; i < font->num_fontdicts; i++) {
		free (font->fd_local_subs_used[i]);
            }
            free (font->fd_local_subs_used);
        }
	free (font->fd_default_width);
	free (font->fd_nominal_width);
    }

    free (font->data);

    free (font);
}

cairo_status_t
_cairo_cff_subset_init (cairo_cff_subset_t          *cff_subset,
                        const char		    *subset_name,
                        cairo_scaled_font_subset_t  *font_subset)
{
    cairo_cff_font_t *font = NULL; /* squelch bogus compiler warning */
    cairo_status_t status;
    const char *data = NULL; /* squelch bogus compiler warning */
    unsigned long length = 0; /* squelch bogus compiler warning */
    unsigned int i;

    status = _cairo_cff_font_create (font_subset, &font, subset_name);
    if (unlikely (status))
	return status;

    status = cairo_cff_font_generate (font, &data, &length);
    if (unlikely (status))
	goto fail1;

    cff_subset->ps_name = strdup (font->ps_name);
    if (unlikely (cff_subset->ps_name == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail1;
    }

    if (font->font_name) {
	cff_subset->family_name_utf8 = strdup (font->font_name);
	if (cff_subset->family_name_utf8 == NULL) {
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    goto fail2;
	}
    } else {
	cff_subset->family_name_utf8 = NULL;
    }

    cff_subset->widths = calloc (sizeof (double), font->scaled_font_subset->num_glyphs);
    if (unlikely (cff_subset->widths == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail3;
    }
    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++)
        cff_subset->widths[i] = (double)font->widths[i]/font->units_per_em;

    cff_subset->x_min = (double)font->x_min/font->units_per_em;
    cff_subset->y_min = (double)font->y_min/font->units_per_em;
    cff_subset->x_max = (double)font->x_max/font->units_per_em;
    cff_subset->y_max = (double)font->y_max/font->units_per_em;
    cff_subset->ascent = (double)font->ascent/font->units_per_em;
    cff_subset->descent = (double)font->descent/font->units_per_em;

    cff_subset->data = _cairo_malloc (length);
    if (unlikely (cff_subset->data == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail4;
    }

    memcpy (cff_subset->data, data, length);
    cff_subset->data_length = length;

    cairo_cff_font_destroy (font);

    return CAIRO_STATUS_SUCCESS;

 fail4:
    free (cff_subset->widths);
 fail3:
    free (cff_subset->family_name_utf8);
 fail2:
    free (cff_subset->ps_name);
 fail1:
    cairo_cff_font_destroy (font);

    return status;
}

void
_cairo_cff_subset_fini (cairo_cff_subset_t *subset)
{
    free (subset->ps_name);
    free (subset->family_name_utf8);
    free (subset->widths);
    free (subset->data);
}

cairo_bool_t
_cairo_cff_scaled_font_is_cid_cff (cairo_scaled_font_t *scaled_font)
{
    const cairo_scaled_font_backend_t *backend;
    cairo_int_status_t status;
    unsigned char *data;
    unsigned long data_length;
    unsigned char *current_ptr;
    unsigned char *data_end;
    cff_header_t  *header;
    cff_index_element_t *element;
    cairo_hash_table_t  *top_dict;
    cairo_array_t index;
    int size;
    cairo_bool_t is_cid = FALSE;

    backend = scaled_font->backend;
    data = NULL;
    data_length = 0;
    status = CAIRO_INT_STATUS_UNSUPPORTED;
    /* Try to load an OpenType/CFF font */
    if (backend->load_truetype_table &&
	(status = backend->load_truetype_table (scaled_font, TT_TAG_CFF,
						0, NULL, &data_length)) == CAIRO_INT_STATUS_SUCCESS)
    {
	data = _cairo_malloc (data_length);
	if (unlikely (data == NULL)) {
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    return FALSE;
	}

	status = backend->load_truetype_table (scaled_font, TT_TAG_CFF,
					   0, data, &data_length);
	if (unlikely (status))
	    goto fail1;
    }
    /* Try to load a CFF font */
    if (status == CAIRO_INT_STATUS_UNSUPPORTED &&
	backend->load_type1_data &&
	(status = backend->load_type1_data (scaled_font,
					    0, NULL, &data_length)) == CAIRO_INT_STATUS_SUCCESS)
    {
	data = _cairo_malloc (data_length);
	if (unlikely (data == NULL)) {
	    status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	    return FALSE;
	}

	status = backend->load_type1_data (scaled_font, 0, data, &data_length);
	if (unlikely (status))
	    goto fail1;
    }
    if (status)
	goto fail1;

    /* Check if it looks like a CFF font */
    if (!check_fontdata_is_cff (data, data_length))
	goto fail1;

    data_end = data + data_length;

    /* skip header */
    if (data_length < sizeof (cff_header_t))
        goto fail1;

    header = (cff_header_t *) data;
    current_ptr = data + header->header_size;

    /* skip name */
    cff_index_init (&index);
    status = cff_index_read (&index, &current_ptr, data_end);
    cff_index_fini (&index);

    if (status)
        goto fail1;

    /* read top dict */
    cff_index_init (&index);
    status = cff_index_read (&index, &current_ptr, data_end);
    if (unlikely (status))
        goto fail2;

    status = cff_dict_init (&top_dict);
    if (unlikely (status))
	goto fail2;

    element = _cairo_array_index (&index, 0);
    status = cff_dict_read (top_dict, element->data, element->length);
    if (unlikely (status))
        goto fail3;

    /* check for ROS operator indicating a CID font */
    if (cff_dict_get_operands (top_dict, ROS_OP, &size) != NULL)
        is_cid = TRUE;

fail3:
    cff_dict_fini (top_dict);

fail2:
    cff_index_fini (&index);

fail1:
    free (data);

    return is_cid;
}

static cairo_int_status_t
_cairo_cff_font_fallback_create (cairo_scaled_font_subset_t  *scaled_font_subset,
                                 cairo_cff_font_t           **font_return,
                                 const char                  *subset_name)
{
    cairo_status_t status;
    cairo_cff_font_t *font;

    font = _cairo_malloc (sizeof (cairo_cff_font_t));
    if (unlikely (font == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    font->backend = NULL;
    font->scaled_font_subset = scaled_font_subset;

    _cairo_array_init (&font->output, sizeof (char));
    status = _cairo_array_grow_by (&font->output, 4096);
    if (unlikely (status))
	goto fail1;

    font->subset_font_name = strdup (subset_name);
    if (unlikely (font->subset_font_name == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail1;
    }

    font->ps_name = strdup (subset_name);
    if (unlikely (font->ps_name == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail2;
    }
    font->font_name = NULL;

    font->x_min = 0;
    font->y_min = 0;
    font->x_max = 0;
    font->y_max = 0;
    font->ascent = 0;
    font->descent = 0;

    font->widths = calloc (font->scaled_font_subset->num_glyphs, sizeof (int));
    if (unlikely (font->widths == NULL)) {
        status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        goto fail3;
    }

    font->data_length = 0;
    font->data = NULL;
    font->data_end = NULL;

    status = cff_dict_init (&font->top_dict);
    if (unlikely (status))
	goto fail4;

    status = cff_dict_init (&font->private_dict);
    if (unlikely (status))
	goto fail5;

    cff_index_init (&font->strings_index);
    cff_index_init (&font->charstrings_index);
    cff_index_init (&font->global_sub_index);
    cff_index_init (&font->local_sub_index);
    cff_index_init (&font->charstrings_subset_index);
    cff_index_init (&font->strings_subset_index);
    font->global_subs_used = NULL;
    font->local_subs_used = NULL;
    font->subset_subroutines = FALSE;
    font->fdselect = NULL;
    font->fd_dict = NULL;
    font->fd_private_dict = NULL;
    font->fd_local_sub_index = NULL;
    font->fdselect_subset = NULL;
    font->fd_subset_map = NULL;
    font->private_dict_offset = NULL;

    *font_return = font;

    return CAIRO_STATUS_SUCCESS;

fail5:
    _cairo_hash_table_destroy (font->top_dict);
fail4:
    free (font->widths);
fail3:
    free (font->font_name);
    free (font->ps_name);
fail2:
    free (font->subset_font_name);
fail1:
    _cairo_array_fini (&font->output);
    free (font);
    return status;
}

static cairo_int_status_t
cairo_cff_font_fallback_generate (cairo_cff_font_t           *font,
                                  cairo_type2_charstrings_t  *type2_subset,
                                  const char                **data,
                                  unsigned long              *length)
{
    cairo_int_status_t status;
    cff_header_t header;
    cairo_array_t *charstring;
    unsigned char buf[40];
    unsigned char *end_buf, *end_buf2;
    unsigned int i;
    int sid;

    /* Create header */
    header.major = 1;
    header.minor = 0;
    header.header_size = 4;
    header.offset_size = 4;
    font->header = &header;

    /* Create Top Dict */
    font->is_cid = FALSE;

    snprintf((char*)buf, sizeof(buf), "CairoFont-%u-%u",
	     font->scaled_font_subset->font_id,
	     font->scaled_font_subset->subset_id);
    sid = NUM_STD_STRINGS + _cairo_array_num_elements (&font->strings_subset_index);
    status = cff_index_append_copy (&font->strings_subset_index,
                                    (unsigned char *)buf,
                                    strlen((char*)buf));
    if (unlikely (status))
	return status;

    end_buf = encode_integer (buf, sid);
    status = cff_dict_set_operands (font->top_dict, FULLNAME_OP,
				    buf, end_buf - buf);
    if (unlikely (status))
	return status;

    status = cff_dict_set_operands (font->top_dict, FAMILYNAME_OP,
				    buf, end_buf - buf);
    if (unlikely (status))
	return status;

    end_buf = encode_integer (buf, type2_subset->x_min);
    end_buf = encode_integer (end_buf, type2_subset->y_min);
    end_buf = encode_integer (end_buf, type2_subset->x_max);
    end_buf = encode_integer (end_buf, type2_subset->y_max);
    status = cff_dict_set_operands (font->top_dict,
	                            FONTBBOX_OP, buf, end_buf - buf);
    if (unlikely (status))
	return status;

    end_buf = encode_integer_max (buf, 0);
    status = cff_dict_set_operands (font->top_dict,
	                            CHARSTRINGS_OP, buf, end_buf - buf);
    if (unlikely (status))
	return status;


    if (font->scaled_font_subset->is_latin) {
	status = cff_dict_set_operands (font->top_dict,
                                        ENCODING_OP, buf, end_buf - buf);
        if (unlikely (status))
	    return status;

	/* Private has two operands - size and offset */
	end_buf2 = encode_integer_max (end_buf, 0);
	cff_dict_set_operands (font->top_dict, PRIVATE_OP, buf, end_buf2 - buf);
        if (unlikely (status))
	    return status;

    } else {
	status = cff_dict_set_operands (font->top_dict,
					FDSELECT_OP, buf, end_buf - buf);
	if (unlikely (status))
	    return status;

	status = cff_dict_set_operands (font->top_dict,
					FDARRAY_OP, buf, end_buf - buf);
	if (unlikely (status))
	    return status;
    }

    status = cff_dict_set_operands (font->top_dict,
	                            CHARSET_OP, buf, end_buf - buf);
    if (unlikely (status))
	return status;

    if (!font->scaled_font_subset->is_latin) {
	status = cairo_cff_font_set_ros_strings (font);
	if (unlikely (status))
	    return status;

	/* Create CID FD dictionary */
	status = cairo_cff_font_create_cid_fontdict (font);
	if (unlikely (status))
	    return status;
    } else {
	font->private_dict_offset = _cairo_malloc (sizeof (int));
	if (unlikely (font->private_dict_offset == NULL))
	    return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    /* Create charstrings */
    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++) {
        charstring = _cairo_array_index(&type2_subset->charstrings, i);

        status = cff_index_append (&font->charstrings_subset_index,
                                   _cairo_array_index (charstring, 0),
                                   _cairo_array_num_elements (charstring));

        if (unlikely (status))
            return status;
    }

    if (font->scaled_font_subset->is_latin)
	status = cairo_cff_font_add_euro_charset_string (font);

    status = cairo_cff_font_write_subset (font);
    if (unlikely (status))
        return status;

    *data = _cairo_array_index (&font->output, 0);
    *length = _cairo_array_num_elements (&font->output);

    return CAIRO_STATUS_SUCCESS;
}

cairo_status_t
_cairo_cff_fallback_init (cairo_cff_subset_t          *cff_subset,
                          const char		      *subset_name,
                          cairo_scaled_font_subset_t  *font_subset)
{
    cairo_cff_font_t *font = NULL; /* squelch bogus compiler warning */
    cairo_status_t status;
    const char *data = NULL; /* squelch bogus compiler warning */
    unsigned long length = 0; /* squelch bogus compiler warning */
    unsigned int i;
    cairo_type2_charstrings_t type2_subset;

    status = _cairo_cff_font_fallback_create (font_subset, &font, subset_name);
    if (unlikely (status))
	return status;

    status = _cairo_type2_charstrings_init (&type2_subset, font_subset);
    if (unlikely (status))
	goto fail1;

    status = cairo_cff_font_fallback_generate (font, &type2_subset, &data, &length);
    if (unlikely (status))
	goto fail2;

    cff_subset->family_name_utf8 = NULL;
    cff_subset->ps_name = strdup (font->ps_name);
    if (unlikely (cff_subset->ps_name == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail2;
    }

    cff_subset->widths = calloc (sizeof (double), font->scaled_font_subset->num_glyphs);
    if (unlikely (cff_subset->widths == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail3;
    }

    for (i = 0; i < font->scaled_font_subset->num_glyphs; i++)
        cff_subset->widths[i] = (double)type2_subset.widths[i]/1000;

    cff_subset->x_min = (double)type2_subset.x_min/1000;
    cff_subset->y_min = (double)type2_subset.y_min/1000;
    cff_subset->x_max = (double)type2_subset.x_max/1000;
    cff_subset->y_max = (double)type2_subset.y_max/1000;
    cff_subset->ascent = (double)type2_subset.y_max/1000;
    cff_subset->descent = (double)type2_subset.y_min/1000;

    cff_subset->data = _cairo_malloc (length);
    if (unlikely (cff_subset->data == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
	goto fail4;
    }

    memcpy (cff_subset->data, data, length);
    cff_subset->data_length = length;

    _cairo_type2_charstrings_fini (&type2_subset);
    cairo_cff_font_destroy (font);

    return CAIRO_STATUS_SUCCESS;

 fail4:
    free (cff_subset->widths);
 fail3:
    free (cff_subset->ps_name);
 fail2:
    _cairo_type2_charstrings_fini (&type2_subset);
 fail1:
    cairo_cff_font_destroy (font);

    return status;
}

void
_cairo_cff_fallback_fini (cairo_cff_subset_t *subset)
{
    free (subset->ps_name);
    free (subset->widths);
    free (subset->data);
}

#endif /* CAIRO_HAS_FONT_SUBSET */
