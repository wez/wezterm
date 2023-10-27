/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2022 Adrian Johnson
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

#include "cairoint.h"
#include "cairo-array-private.h"
#include "cairo-ft-private.h"
#include "cairo-pattern-private.h"
#include "cairo-scaled-font-subsets-private.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#if HAVE_FT_SVG_DOCUMENT

#include <ft2build.h>
#include FT_COLOR_H

/* #define SVG_RENDER_PRINT_FUNCTIONS 1 */

#define WHITE_SPACE_CHARS " \n\r\t\v\f"

typedef struct {
    const char *name;
    int red;
    int green;
    int blue;
} color_name_t;

/* Must be sorted */
static color_name_t color_names[] = {
    { "aliceblue", 240, 248, 255 },
    { "antiquewhite", 250, 235, 215 },
    { "aqua",  0, 255, 255 },
    { "aquamarine", 127, 255, 212 },
    { "azure", 240, 255, 255 },
    { "beige", 245, 245, 220 },
    { "bisque", 255, 228, 196 },
    { "black",  0, 0, 0 },
    { "blanchedalmond", 255, 235, 205 },
    { "blue",  0, 0, 255 },
    { "blueviolet", 138, 43, 226 },
    { "brown", 165, 42, 42 },
    { "burlywood", 222, 184, 135 },
    { "cadetblue",  95, 158, 160 },
    { "chartreuse", 127, 255, 0 },
    { "chocolate", 210, 105, 30 },
    { "coral", 255, 127, 80 },
    { "cornflowerblue", 100, 149, 237 },
    { "cornsilk", 255, 248, 220 },
    { "crimson", 220, 20, 60 },
    { "cyan",  0, 255, 255 },
    { "darkblue",  0, 0, 139 },
    { "darkcyan",  0, 139, 139 },
    { "darkgoldenrod", 184, 134, 11 },
    { "darkgray", 169, 169, 169 },
    { "darkgreen",  0, 100, 0 },
    { "darkgrey", 169, 169, 169 },
    { "darkkhaki", 189, 183, 107 },
    { "darkmagenta", 139, 0, 139 },
    { "darkolivegreen",  85, 107, 47 },
    { "darkorange", 255, 140, 0 },
    { "darkorchid", 153, 50, 204 },
    { "darkred", 139, 0, 0 },
    { "darksalmon", 233, 150, 122 },
    { "darkseagreen", 143, 188, 143 },
    { "darkslateblue",  72, 61, 139 },
    { "darkslategray",  47, 79, 79 },
    { "darkslategrey",  47, 79, 79 },
    { "darkturquoise",  0, 206, 209 },
    { "darkviolet", 148, 0, 211 },
    { "deeppink", 255, 20, 147 },
    { "deepskyblue",  0, 191, 255 },
    { "dimgray", 105, 105, 105 },
    { "dimgrey", 105, 105, 105 },
    { "dodgerblue",  30, 144, 255 },
    { "firebrick", 178, 34, 34 },
    { "floralwhite", 255, 250, 240 },
    { "forestgreen",  34, 139, 34 },
    { "fuchsia", 255, 0, 255 },
    { "gainsboro", 220, 220, 220 },
    { "ghostwhite", 248, 248, 255 },
    { "gold", 255, 215, 0 },
    { "goldenrod", 218, 165, 32 },
    { "gray", 128, 128, 128 },
    { "green",  0, 128, 0 },
    { "greenyellow", 173, 255, 47 },
    { "grey", 128, 128, 128 },
    { "honeydew", 240, 255, 240 },
    { "hotpink", 255, 105, 180 },
    { "indianred", 205, 92, 92 },
    { "indigo",  75, 0, 130 },
    { "ivory", 255, 255, 240 },
    { "khaki", 240, 230, 140 },
    { "lavender", 230, 230, 250 },
    { "lavenderblush", 255, 240, 245 },
    { "lawngreen", 124, 252, 0 },
    { "lemonchiffon", 255, 250, 205 },
    { "lightblue", 173, 216, 230 },
    { "lightcoral", 240, 128, 128 },
    { "lightcyan", 224, 255, 255 },
    { "lightgoldenrodyellow", 250, 250, 210 },
    { "lightgray", 211, 211, 211 },
    { "lightgreen", 144, 238, 144 },
    { "lightgrey", 211, 211, 211 },
    { "lightpink", 255, 182, 193 },
    { "lightsalmon", 255, 160, 122 },
    { "lightseagreen",  32, 178, 170 },
    { "lightskyblue", 135, 206, 250 },
    { "lightslategray", 119, 136, 153 },
    { "lightslategrey", 119, 136, 153 },
    { "lightsteelblue", 176, 196, 222 },
    { "lightyellow", 255, 255, 224 },
    { "lime",  0, 255, 0 },
    { "limegreen",  50, 205, 50 },
    { "linen", 250, 240, 230 },
    { "magenta", 255, 0, 255 },
    { "maroon", 128, 0, 0 },
    { "mediumaquamarine", 102, 205, 170 },
    { "mediumblue",  0, 0, 205 },
    { "mediumorchid", 186, 85, 211 },
    { "mediumpurple", 147, 112, 219 },
    { "mediumseagreen",  60, 179, 113 },
    { "mediumslateblue", 123, 104, 238 },
    { "mediumspringgreen",  0, 250, 154 },
    { "mediumturquoise",  72, 209, 204 },
    { "mediumvioletred", 199, 21, 133 },
    { "midnightblue",  25, 25, 112 },
    { "mintcream", 245, 255, 250 },
    { "mistyrose", 255, 228, 225 },
    { "moccasin", 255, 228, 181 },
    { "navajowhite", 255, 222, 173 },
    { "navy",  0, 0, 128 },
    { "oldlace", 253, 245, 230 },
    { "olive", 128, 128, 0 },
    { "olivedrab", 107, 142, 35 },
    { "orange", 255, 165, 0 },
    { "orangered", 255, 69, 0 },
    { "orchid", 218, 112, 214 },
    { "palegoldenrod", 238, 232, 170 },
    { "palegreen", 152, 251, 152 },
    { "paleturquoise", 175, 238, 238 },
    { "palevioletred", 219, 112, 147 },
    { "papayawhip", 255, 239, 213 },
    { "peachpuff", 255, 218, 185 },
    { "peru", 205, 133, 63 },
    { "pink", 255, 192, 203 },
    { "plum", 221, 160, 221 },
    { "powderblue", 176, 224, 230 },
    { "purple", 128, 0, 128 },
    { "red", 255, 0, 0 },
    { "rosybrown", 188, 143, 143 },
    { "royalblue",  65, 105, 225 },
    { "saddlebrown", 139, 69, 19 },
    { "salmon", 250, 128, 114 },
    { "sandybrown", 244, 164, 96 },
    { "seagreen",  46, 139, 87 },
    { "seashell", 255, 245, 238 },
    { "sienna", 160, 82, 45 },
    { "silver", 192, 192, 192 },
    { "skyblue", 135, 206, 235 },
    { "slateblue", 106, 90, 205 },
    { "slategray", 112, 128, 144 },
    { "slategrey", 112, 128, 144 },
    { "snow", 255, 250, 250 },
    { "springgreen",  0, 255, 127 },
    { "steelblue",  70, 130, 180 },
    { "tan", 210, 180, 140 },
    { "teal",  0, 128, 128 },
    { "thistle", 216, 191, 216 },
    { "tomato", 255, 99, 71 },
    { "turquoise",  64, 224, 208 },
    { "violet", 238, 130, 238 },
    { "wheat", 245, 222, 179 },
    { "white", 255, 255, 255 },
    { "whitesmoke", 245, 245, 245 },
    { "yellow", 255, 255, 0 },
    { "yellowgreen", 154, 205, 50 }
};

typedef struct {
    char *name;
    char *value;
} svg_attribute_t;

typedef enum {
    CONTAINER_ELEMENT,
    EMPTY_ELEMENT,
    PROCESSING_INSTRUCTION,
    DOCTYPE,
    CDATA,
    COMMENT
} tag_type_t;

#define TOP_ELEMENT_TAG "_top"

typedef struct _cairo_svg_element {
    cairo_hash_entry_t base;
    tag_type_t type;
    char *tag;
    char *id;
    cairo_array_t attributes; /* svg_attribute_t */
    cairo_array_t children; /* cairo_svg_element_t* */
    cairo_array_t  content; /* char */
    cairo_pattern_t *pattern; /* defined if a paint server */
    struct _cairo_svg_element *next; /* next on element stack */
} cairo_svg_element_t;

typedef struct _cairo_svg_color {
    enum { RGB, FOREGROUND } type;
    double red;
    double green;
    double blue;
} cairo_svg_color_t;

typedef struct _cairo_svg_paint {
    enum { PAINT_COLOR, PAINT_SERVER, PAINT_NONE } type;
    cairo_svg_color_t color;
    cairo_svg_element_t *paint_server;
} cairo_svg_paint_t;

typedef enum {
    GS_RENDER,
    GS_NO_RENDER,
    GS_COMPUTE_BBOX,
    GS_CLIP
} gs_mode_t;

typedef struct _cairo_svg_graphics_state {
    cairo_svg_paint_t fill;
    cairo_svg_paint_t stroke;
    cairo_svg_color_t color;
    double fill_opacity;
    double stroke_opacity;
    double opacity;
    cairo_fill_rule_t fill_rule;
    cairo_fill_rule_t clip_rule;
    cairo_path_t *clip_path;
    char *dash_array;
    double dash_offset;
    gs_mode_t mode;
    struct {
        double x;
        double y;
        double width;
        double height;
    } bbox;
    struct _cairo_svg_graphics_state *next;
} cairo_svg_graphics_state_t;

typedef enum {
    BUILD_PATTERN_NONE,
    BUILD_PATTERN_LINEAR,
    BUILD_PATTERN_RADIAL
} build_pattern_t;

typedef struct _cairo_svg_glyph_render {
    cairo_svg_element_t *tree;
    cairo_hash_table_t *ids;
    cairo_svg_graphics_state_t *graphics_state;
    cairo_t *cr;
    double units_per_em;
    struct {
        cairo_svg_element_t *paint_server;
        cairo_pattern_t *pattern;
        build_pattern_t type;
    } build_pattern;
    int render_element_tree_depth;
    int num_palette_entries;
    FT_Color* palette;

    /* Viewport */
    double width;
    double height;
    cairo_bool_t view_port_set;

    cairo_pattern_t *foreground_marker;
    cairo_pattern_t *foreground_source;
    cairo_bool_t foreground_source_used;

    int debug; /* 0 = quiet, 1 = errors, 2 = warnings, 3 = info */
} cairo_svg_glyph_render_t;


#define SVG_RENDER_ERROR 1
#define SVG_RENDER_WARNING 2
#define SVG_RENDER_INFO 3

#define print_error(render, ...) cairo_svg_glyph_render_printf(render, SVG_RENDER_ERROR, ##__VA_ARGS__)
#define print_warning(render, ...) cairo_svg_glyph_render_printf(render, SVG_RENDER_WARNING, ##__VA_ARGS__)
#define print_info(render, ...) cairo_svg_glyph_render_printf(render, SVG_RENDER_INFO, ##__VA_ARGS__)

static void
cairo_svg_glyph_render_printf (cairo_svg_glyph_render_t *svg_render,
                               int level,
                               const char *fmt, ...) CAIRO_PRINTF_FORMAT (3, 4);

static void
cairo_svg_glyph_render_printf (cairo_svg_glyph_render_t *svg_render,
                               int level,
                               const char *fmt, ...)
{
    va_list ap;

    if (svg_render->debug >= level ) {
        switch (level) {
            case SVG_RENDER_ERROR:
                printf("ERROR: ");
                break;
            case SVG_RENDER_WARNING:
                printf("WARNING: ");
                break;
        }
	va_start (ap, fmt);
	vprintf (fmt, ap);
	va_end (ap);
	printf ("\n");
    }
}

static cairo_bool_t
string_equal (const char *s1, const char *s2)
{
    if (s1 && s2)
        return strcmp (s1, s2) == 0;

    if (!s1 && !s2)
        return TRUE;

    return FALSE;
}

static cairo_bool_t
string_match (const char **p, const char *str)
{
    if (*p && strncmp (*p, str, strlen (str)) == 0) {
        *p += strlen (str);
        return TRUE;
    }
    return FALSE;
}

static const char *
skip_space (const char *p)
{
    while (*p && _cairo_isspace (*p))
        p++;

    return p;
}

/* Skip over character c and and whitespace before or after. Returns
 * NULL if c not found. */
static const char *
skip_char (const char *p, char c)
{
    while (_cairo_isspace (*p))
        p++;

    if (*p != c)
        return NULL;

    p++;

    while (_cairo_isspace (*p))
        p++;

    return p;
}

static int
_color_name_compare (const void *a, const void *b)
{
    const color_name_t *a_color = a;
    const color_name_t *b_color = b;

    return strcmp (a_color->name, b_color->name);
}

static void
init_element_id_key (cairo_svg_element_t *element)
{
    element->base.hash = _cairo_hash_string (element->id);
}

static cairo_bool_t
_element_id_equal (const void *key_a, const void *key_b)
{
    const cairo_svg_element_t *a = key_a;
    const cairo_svg_element_t *b = key_b;

    return string_equal (a->id, b->id);
}

/* Find element with the "id" attribute matching id. id may have the
 * '#' prefix. It will be stripped before searching.
 */
static cairo_svg_element_t *
lookup_element (cairo_svg_glyph_render_t *svg_render, const char *id)
{
    cairo_svg_element_t key;

    if (!id || strlen (id) < 1)
        return NULL;

    key.id = (char *)(id[0] == '#' ? id + 1 : id);
    init_element_id_key (&key);
    return _cairo_hash_table_lookup (svg_render->ids, &key.base);
}

/* Find element with the "id" attribute matching url where url is of
 * the form "url(#id)".
 */
static cairo_svg_element_t *
lookup_url_element (cairo_svg_glyph_render_t *svg_render, const char *url)
{
    const char *p = url;
    cairo_svg_element_t *element = NULL;

    if (p && string_match (&p, "url")) {
        p = skip_char (p, '(');
        if (!p)
            return NULL;

        const char *end = strpbrk(p, WHITE_SPACE_CHARS ")");
        if (end) {
            char *id = _cairo_strndup (p, end - p);
            element = lookup_element (svg_render, id);
            free (id);
        }
    }
    return element;
}

static const char *
get_attribute (const cairo_svg_element_t *element, const char *name)
{
    svg_attribute_t attr;
    int num_elems, i;

    num_elems = _cairo_array_num_elements (&element->attributes);
    for (i = 0; i < num_elems; i++) {
	_cairo_array_copy_element (&element->attributes, i, &attr);
        if (string_equal (attr.name, name))
            return attr.value;
    }
    return NULL;
}

static const char *
get_href_attribute (const cairo_svg_element_t *element)
{
    svg_attribute_t attr;
    int num_elems, i, len;

    /* SVG2 requires the href attribute to be "href". Older versions
     * used "xlink:href". I have seen at least one font that used an
     * alternative name space eg "ns1:href". To keep things simple we
     * search for an attribute named "href" or ending in ":href".
     */
    num_elems = _cairo_array_num_elements (&element->attributes);
    for (i = 0; i < num_elems; i++) {
	_cairo_array_copy_element (&element->attributes, i, &attr);
        if (string_equal (attr.name, "href"))
            return attr.value;

        len = strlen (attr.name);
        if (len > 4 && string_equal (attr.name + len - 5, ":href"))
            return attr.value;
    }
    return NULL;
}

/* Get a float attribute or float percentage. If attribute is a
 * percentage, the returned value is percentage * scale.  Does not
 * modify value if it returns FALSE. This allows value to be set to a
 * default before calling get_float_attribute(), then used without
 * checking the return value of this function.
 */
static cairo_bool_t
get_float_or_percent_attribute (const cairo_svg_element_t *element,
                                const char *name,
                                double scale,
                                double *value)
{
    const char *p;
    char *end;
    double v;

    p = get_attribute (element, name);
    if (p) {
        v = _cairo_strtod (p, &end);
        if (end != p) {
            *value = v;
            if (*end == '%')
                *value *= scale / 100.0;
            return TRUE;
        }
    }
    return FALSE;
}

/* Does not modify value if it returns FALSE. This allows value to be
 * set to a default before calling get_float_attribute(), then used
 * without checking the return value of this function.
 */
static cairo_bool_t
get_float_attribute (const cairo_svg_element_t *element, const char *name, double *value)
{
    const char *p;
    char *end;
    double v;

    p = get_attribute (element, name);
    if (p) {
        v = _cairo_strtod (p, &end);
        if (end != p) {
            *value = v;
            return TRUE;
        }
    }
    return FALSE;
}

static cairo_fill_rule_t
get_fill_rule_attribute (const cairo_svg_element_t *element, const char *name, cairo_fill_rule_t default_value)
{
    const char *p;

    p = get_attribute (element, name);
    if (string_equal (p, "nonzero"))
        return CAIRO_FILL_RULE_WINDING;
    else if (string_equal (p, "evenodd"))
        return CAIRO_FILL_RULE_EVEN_ODD;
    else
        return default_value;
}

static void
free_elements (cairo_svg_glyph_render_t *svg_render,
              cairo_svg_element_t      *element)
{
    int num_elems;

    num_elems = _cairo_array_num_elements (&element->children);
    for (int i = 0; i < num_elems; i++) {
	cairo_svg_element_t *child;
        _cairo_array_copy_element (&element->children, i, &child);
	free_elements (svg_render, child);
    }
    _cairo_array_fini (&element->children);

    num_elems = _cairo_array_num_elements (&element->attributes);
    for (int i = 0; i < num_elems; i++) {
	svg_attribute_t *attr = _cairo_array_index (&element->attributes, i);
	free (attr->name);
	free (attr->value);
    }
    _cairo_array_fini (&element->attributes);
    _cairo_array_fini (&element->content);

    free (element->tag);

    if (element->id) {
        _cairo_hash_table_remove (svg_render->ids, &element->base);
        free (element->id);
    }

    if (element->pattern)
        cairo_pattern_destroy (element->pattern);

    free (element);
}

#if SVG_RENDER_PRINT_FUNCTIONS

static void indent(int level)
{
    for (int i = 1; i < level; i++)
        printf("  ");
}

static void
print_element (cairo_svg_element_t *element, cairo_bool_t recurse, int level)
{
    char *content = strndup (_cairo_array_index_const (&element->content, 0),
                             _cairo_array_num_elements (&element->content));

    indent(level);
    if (element->type == COMMENT) {
        printf("<!--%s-->\n", content);
    } else if (element->type == CDATA) {
        printf("<![CDATA[%s]]>\n", content);
    } else if (element->type == DOCTYPE) {
        printf("<!DOCTYPE%s>\n", content);
    } else if (element->type == PROCESSING_INSTRUCTION) {
        printf("<?%s?>\n", content);
    } else {
        cairo_bool_t top_element = string_equal (element->tag, TOP_ELEMENT_TAG);

        if (!top_element) {
            printf("<%s", element->tag);
            int num_elems = _cairo_array_num_elements (&element->attributes);
            for (int i = 0; i < num_elems; i++) {
                svg_attribute_t *attr = _cairo_array_index (&element->attributes, i);
                printf(" %s=\"%s\"", attr->name, attr->value);
            }
            if (num_elems > 0)
                printf(" ");

            if (element->type == EMPTY_ELEMENT)
                printf("/>\n");
            else
                printf(">\n");
        }

        if (element->type == CONTAINER_ELEMENT) {
            if (recurse) {
                int num_elems = _cairo_array_num_elements (&element->children);
                for (int i = 0; i < num_elems; i++) {
                    cairo_svg_element_t *child;
                    _cairo_array_copy_element (&element->children, i, &child);
                    print_element (child, TRUE, level + 1);
                }
            }
            if (!top_element)
                printf("</%s>\n", element->tag);
        }
    }
    free (content);
}
#endif

static const char *
parse_list_of_floats (const char *p,
                      int num_required,
                      int num_optional,
                      cairo_bool_t *have_optional,
                      va_list ap)
{
    double d;
    double *dp;
    char *end;
    const char *q = NULL;
    int num_found = 0;

    for (int i = 0; i < num_required + num_optional; i++) {
        while (p && (*p == ',' || _cairo_isspace (*p)))
            p++;

        if (!p)
            break;

        d = _cairo_strtod (p, &end);
        if (end == p) {
            p = NULL;
            break;
        }
        p = end;
        dp = va_arg (ap, double *);
        *dp = d;
        num_found++;
        if (num_found == num_required)
            q = p;
    }

    if (num_optional > 0) {
        if (num_found == num_required + num_optional) {
            *have_optional = TRUE;
        } else {
            *have_optional = FALSE;
            /* restore pointer to end of required floats */
            p = q;
        }
    }

    return p;
}

static const char *
get_floats (const char *p,
            int num_required,
            int num_optional,
            cairo_bool_t *have_optional,
            ...)
{
    va_list ap;

    va_start (ap, have_optional);
    p = parse_list_of_floats (p, num_required, num_optional, have_optional, ap);
    va_end (ap);
    return p;
}

static const char *
get_path_params (const char *p, int num_params, ...)
{
    va_list ap;

    va_start (ap, num_params);
    p = parse_list_of_floats (p, num_params, 0, NULL, ap);
    va_end (ap);
    return p;
}

static cairo_bool_t
get_color (cairo_svg_glyph_render_t *svg_render,
           const char               *s,
           cairo_svg_color_t        *color)
{
    int len, matched;
    unsigned r = 0, g = 0, b = 0;

    if (!s)
        return FALSE;

    len = strlen(s);

    if (string_equal (s, "inherit")) {
	return FALSE;
    } else if (string_equal (s, "currentColor") ||
	       string_equal (s, "context-fill") ||
	       string_equal (s, "context-stroke"))
    {
	*color = svg_render->graphics_state->color;
        return TRUE;
    } else if (len > 0 && s[0] == '#') {
        if (len == 4) {
            matched = sscanf (s + 1, "%1x%1x%1x", &r, &g, &b);
            if (matched == 3) {
                /* Each digit is repeated to convert to 6 digits. eg 0x123 -> 0x112233 */
                color->type = RGB;
                color->red = 0x11*r/255.0;
                color->green = 0x11*g/255.0;
                color->blue = 0x11*b/255.0;
                return TRUE;
            }
        } else if (len == 7) {
            matched = sscanf (s + 1, "%2x%2x%2x", &r, &g, &b);
            if (matched == 3) {
                color->type = RGB;
                color->red = r/255.0;
                color->green = g/255.0;
                color->blue = b/255.0;
                return TRUE;
            }
        }
    } else if (strncmp (s, "rgb", 3) == 0) {
        matched = sscanf (s, "rgb ( %u , %u , %u )", &r, &g, &b);
        if (matched == 3) {
            color->type = RGB;
            color->red = r/255.0;
            color->green = g/255.0;
            color->blue = b/255.0;
            return TRUE;
        }
    } else if (strncmp (s, "var", 3) == 0) {
        /* CPAL palettes colors. eg "var(--color0, yellow)" */
        s += 3;
        s = skip_char (s, '(');
        if (!string_match (&s, "--color"))
            return FALSE;

        char *end;
        int entry = strtol (s, &end, 10);
        if (end == s)
            return FALSE;

        if (svg_render->palette && entry >= 0 && entry < svg_render->num_palette_entries) {
            FT_Color *palette_color = &svg_render->palette[entry];
            color->type = RGB;
            color->red = palette_color->red / 255.0;
            color->green = palette_color->green/ 255.0;
            color->blue = palette_color->blue / 255.0;
            return TRUE;
        } else {
            /* Fallback color */
            s = skip_char (end, ',');
            if (!s)
            return FALSE;

            end = strpbrk(s, WHITE_SPACE_CHARS ")");
            if (!end || end == s)
		return FALSE;

            char *fallback = _cairo_strndup (s, end - s);
            cairo_bool_t success = get_color (svg_render, fallback, color);
            free (fallback);
            return success;
        }
    } else {
        const color_name_t *color_name;
        color_name_t color_name_key;

        color_name_key.name = (char *) s;
        color_name = bsearch (&color_name_key,
                              color_names,
                              ARRAY_LENGTH (color_names),
                              sizeof (color_name_t),
                             _color_name_compare);
        if (color_name) {
            color->type = RGB;
            color->red = color_name->red/255.0;
            color->green = color_name->green/255.0;
            color->blue = color_name->blue/255.0;
            return TRUE;
        }
    }
    return FALSE;
}

static void
get_paint (cairo_svg_glyph_render_t *svg_render,
           const char *p,
           cairo_svg_paint_t *paint)
{
    cairo_svg_element_t *element;

    if (string_match (&p, "none")) {
        paint->type = PAINT_NONE;
        paint->paint_server = NULL;
    } else if (p && strncmp (p, "url", 3) == 0) {
        element = lookup_url_element (svg_render, p);
        if (element) {
            paint->type = PAINT_SERVER;
            paint->paint_server = element;
        }
    } else {
        if (get_color (svg_render, p, &paint->color)) {
            paint->type = PAINT_COLOR;
            paint->paint_server = NULL;
        }
    }
}

#ifdef SVG_RENDER_PRINT_FUNCTIONS

static void
print_color (cairo_svg_color_t *color)
{
    switch (color->type) {
        case FOREGROUND_COLOR:
            printf("foreground");
            break;
        case RGB:
            printf("#%02x%02x%02x",
                   (int)(color->red*255),
                   (int)(color->red*255),
                   (int)(color->red*255));
            break;
    }
}

static void
print_paint (cairo_svg_paint_t *paint)
{
    printf("Paint: ");
    switch (paint->type) {
        case PAINT_COLOR:
            printf("color: ");
            print_color (&paint->color);
            break;
        case PAINT_SERVER:
            printf("server: %s", paint->paint_server->tag);
            break;
        case PAINT_NONE:
            printf("none");
            break;
    }
    printf("\n");
}

#endif

static void
parse_error (cairo_svg_glyph_render_t *svg_render,
             const char *string,
             const char *location,
             const char *fmt,
             ...) CAIRO_PRINTF_FORMAT (4, 5);

static void
parse_error (cairo_svg_glyph_render_t *svg_render,
             const char *string,
             const char *location,
             const char *fmt,
             ...)
{
    va_list ap;
    const int context = 40;
    const char *start;
    const char *end;

    if (svg_render->debug >= SVG_RENDER_ERROR) {
        printf("ERROR: ");
	va_start (ap, fmt);
	vprintf (fmt, ap);
	va_end (ap);
        putchar ('\n');
        start = location - context;
        if (start < string)
            start = string;

        end = location + strlen (location);
        if (end - location > context)
            end = location + context;

        for (const char *p = start; p < end; p++) {
            if (_cairo_isspace (*p))
                putchar (' ');
            else
                putchar (*p);
        }
        putchar ('\n');

        for (int i = 0; i < location - start; i++)
            putchar(' ');
        putchar ('^');
        putchar ('\n');
	printf (" at position %td\n", location - string);
    }
}

static cairo_bool_t
append_attribute (cairo_svg_element_t *element, svg_attribute_t *attribute)
{
    const char *p;
    const char *end;
    svg_attribute_t attr;

    memset (&attr, 0, sizeof (attr));
    if (string_equal (attribute->name, "style")) {
        /* split style into individual attributes */
        p = attribute->value;
        while (*p) {
            end = strchr (p, ':');
            if (!end || end == p)
                break;
            attr.name = _cairo_strndup (p, end - p);
            p = end + 1;
            p = skip_space(p);
            end = strchr (p, ';');
            if (!end)
                end = strchr (p, 0);
            if (end == p)
                goto split_style_fail;

            attr.value = _cairo_strndup (p, end - p);
            if (*end)
                p = end + 1;

            if (_cairo_array_append (&element->attributes, &attr))
                goto split_style_fail;

            memset (&attr, 0, sizeof (attr));
            p = skip_space (p);
        }
    }

    if (_cairo_array_append (&element->attributes, attribute))
        return FALSE;

    return TRUE;

  split_style_fail:
    free (attr.name);
    free (attr.value);
    return FALSE;
}

static cairo_bool_t
add_child_element (cairo_svg_glyph_render_t *svg_render,
                   cairo_svg_element_t *parent,
                   cairo_svg_element_t *child)
{
    cairo_status_t status;
    const char* id;

    id = get_attribute (child, "id");
    if (id) {
        child->id = strdup (id);
        init_element_id_key (child);
	status = _cairo_hash_table_insert (svg_render->ids, &child->base);
	if (unlikely (status))
            return FALSE;
    }

    status = _cairo_array_append (&parent->children, &child);
    return status == CAIRO_STATUS_SUCCESS;
}

static cairo_svg_element_t *
create_element (tag_type_t type, char *tag)
{
    cairo_svg_element_t *elem;
    cairo_status_t status;

    elem = _cairo_malloc (sizeof (cairo_svg_element_t));
    if (unlikely (elem == NULL)) {
	status = _cairo_error (CAIRO_STATUS_NO_MEMORY);
        return NULL;
    }

    elem->type = type;
    elem->tag = tag;
    elem->id = NULL;
    _cairo_array_init (&elem->attributes, sizeof(svg_attribute_t));
    _cairo_array_init (&elem->children, sizeof(cairo_svg_element_t *));
    _cairo_array_init (&elem->content, sizeof(char));
    elem->pattern = NULL;
    elem->next = NULL;

    return elem;
}

static const char *
parse_attributes (cairo_svg_glyph_render_t *svg_render,
                  const char               *attributes,
                  cairo_svg_element_t      *element)
{
    svg_attribute_t attr;
    char quote_char;
    const char *p;
    const char *end;

    p = attributes;
    memset (&attr, 0, sizeof (svg_attribute_t));
    p = skip_space (p);
    while (*p && *p != '/' && *p != '>' && *p != '?') {
        end = strpbrk(p, WHITE_SPACE_CHARS "=");
        if (!end) {
            parse_error (svg_render, attributes, p, "Could not find '='");
            goto fail;
        }

        if (end == p) {
            parse_error (svg_render, attributes, p, "Missing attribute name");
            goto fail;
        }

        attr.name = _cairo_strndup (p, end - p);
        p = end;

        p = skip_space (p);
        if (*p != '=') {
            parse_error (svg_render, attributes, p, "Expected '='");
            goto fail;
        }

        p++;
        p = skip_space (p);
        if (*p == '\"' || *p == '\'') {
            quote_char = *p;
        } else {
            parse_error (svg_render, attributes, p, "Could not find '\"' or '''");
            goto fail;
        }

        p++;
        end = strchr (p, quote_char);
        if (!end) {
            parse_error (svg_render, attributes, p, "Could not find '%c'", quote_char);
            goto fail;
        }

        attr.value = _cairo_strndup (p, end - p);
        p = end + 1;

        if (!append_attribute (element, &attr))
            goto fail;

        memset (&attr, 0, sizeof (svg_attribute_t));

        p = skip_space (p);
    }

    return p;

  fail:
    free (attr.name);
    free (attr.value);
    return NULL;
}

static cairo_bool_t
parse_svg (cairo_svg_glyph_render_t *svg_render,
           const char               *svg_document)
{
    const char *p = svg_document;
    const char *end;
    int nesting; /* when > 0 we parse content */
    cairo_svg_element_t *open_elem; /* Stack of open elements */
    cairo_svg_element_t *new_elem = NULL;
    char *name;
    cairo_status_t status;

    /* Create top level element to use as a container for all top
     * level elements in the document and push it on the stack. */
    open_elem = create_element (CONTAINER_ELEMENT, strdup(TOP_ELEMENT_TAG));

    /* We don't want to add content to the top level container. There
     * should only be whitesapce between tags. */
    nesting = 0;

    while (*p) {
        if (nesting > 0) {
            /* In an open element. Anything before the next '<' is content */
            end = strchr (p, '<');
            if (!end) {
                parse_error (svg_render, svg_document, p, "Could not find '<'");
                goto fail;
            }
            status = _cairo_array_append_multiple (&open_elem->content, p, end - p);
            p = end;

        } else {
            p = skip_space (p);
            if (*p == 0)
                break; /* end of document */
        }

        /* We should now be at the start of a tag */
        if (*p != '<') {
            parse_error (svg_render, svg_document, p, "Could not find '<'");
            goto fail;
        }

        p++;
        if (*p == '!') {
            p++;
            if (string_match (&p, "[CDATA[")) {
                new_elem = create_element (CDATA, NULL);
                end = strstr (p, "]]>");
                if (!end) {
                    parse_error (svg_render, svg_document, p, "Could not find ']]>'");
                    goto fail;
                }

                status = _cairo_array_append_multiple (&new_elem->content, p, end - p);
                p = end + 3;
            } else if (string_match (&p, "--")) {
                new_elem = create_element (COMMENT, NULL);
                end = strstr (p, "-->");
                if (!end) {
                    parse_error (svg_render, svg_document, p, "Could not find '-->'");
                    goto fail;
                }

                status = _cairo_array_append_multiple (&new_elem->content, p, end - p);
                p = end + 3;
            } else if (string_match (&p, "DOCTYPE")) {
                new_elem = create_element (DOCTYPE, NULL);
                end = strchr (p, '>');
                if (!end) {
                    parse_error (svg_render, svg_document, p, "Could not find '>'");
                    goto fail;
                }

                status = _cairo_array_append_multiple (&new_elem->content, p, end - p);
                p = end + 1;
            } else {
                parse_error (svg_render, svg_document, p, "Invalid");
                goto fail;
            }

            if (!add_child_element (svg_render, open_elem, new_elem))
                goto fail;

            new_elem = NULL;
            continue;
        }

        if (*p == '?') {
            p++;
            new_elem = create_element (PROCESSING_INSTRUCTION, NULL);
            end = strstr (p, "?>");
            if (!end) {
                parse_error (svg_render, svg_document, p, "Could not find '?>'");
                goto fail;
            }

            status = _cairo_array_append_multiple (&new_elem->content, p, end - p);
            p = end + 2;

            if (!add_child_element (svg_render, open_elem, new_elem))
                goto fail;

            new_elem = NULL;
            continue;
        }

        if (*p == '/') {
            /* Closing tag */
            p++;

            /* find end of tag name */
            end = strpbrk(p, WHITE_SPACE_CHARS ">");
            if (!end) {
                parse_error (svg_render, svg_document, p, "Could not find '>'");
                goto fail;
            }

            name = _cairo_strndup (p, end - p);
            p = end;
            p = skip_space (p);
            if (*p != '>') {
                parse_error (svg_render, svg_document, p, "Could not find '>'");
                free (name);
                goto fail;
            }

            p++;
            if (nesting == 0) {
                parse_error (svg_render, svg_document, p, "parse_elements: parsed </%s> but no matching start tag", name);
                free (name);
                goto fail;
            }
            if (!string_equal (name, open_elem->tag)) {
                parse_error (svg_render, svg_document, p,
                             "parse_elements: found </%s> but current open tag is <%s>",
                             name, open_elem->tag);
                free (name);
                goto fail;
            }

            /* pop top element on open elements stack into new_elem */
            new_elem = open_elem;
            open_elem = open_elem->next;
            new_elem->next = NULL;
            nesting--;

            free (name);
            if (!add_child_element (svg_render, open_elem, new_elem))
                goto fail;

            new_elem = NULL;
            continue;
        }

        /* We should now be in a start or empty element tag */

        /* find end of tag name */
        end = strpbrk(p, WHITE_SPACE_CHARS "/>");
        if (!end) {
            parse_error (svg_render, svg_document, p, "Could not find '>'");
            goto fail;
        }

        name = _cairo_strndup (p, end - p);
        p = end;

        new_elem = create_element (CONTAINER_ELEMENT, name);
        p = parse_attributes (svg_render, p, new_elem);
        if (!p)
            goto fail;

        p = skip_space (p);
        if (*p == '/') {
            new_elem->type = EMPTY_ELEMENT;
            p++;
        }

        if (!p || *p != '>') {
            print_error (svg_render, "Could not find '>'");
            goto fail;
        }

        p++;
        if (new_elem->type == EMPTY_ELEMENT) {
            if (!add_child_element (svg_render, open_elem, new_elem))
                goto fail;

            new_elem = NULL;
        } else {
            /* push new elem onto open elements stack */
            new_elem->next = open_elem;
            open_elem = new_elem;
            new_elem = NULL;
            nesting++;
        }
    }

    if (nesting != 0) {
        parse_error (svg_render, svg_document, p, "Missing closing tag for <%s>", open_elem->tag);
        goto fail;
    }

    svg_render->tree = open_elem;
    return TRUE;

  fail:
    if (new_elem)
        free_elements (svg_render, new_elem);

    while (open_elem) {
        cairo_svg_element_t *elem = open_elem;
        open_elem = open_elem->next;
        free_elements (svg_render, elem);
    }

    return FALSE;
}

static cairo_bool_t
parse_transform (const char *p, cairo_matrix_t *matrix)
{
    cairo_matrix_t m;
    double x, y, a;
    cairo_bool_t have_optional;

    cairo_matrix_init_identity (matrix);
    while (p) {
        while (p && (*p == ',' || _cairo_isspace (*p)))
            p++;

        if (!p || *p == 0)
            break;

        if (string_match (&p, "matrix")) {
            p = skip_char (p, '(');
            if (!p)
                break;

            p = get_floats (p, 6, 0, NULL, &m.xx, &m.yx, &m.xy, &m.yy, &m.x0, &m.y0);
            if (!p)
                break;

            p = skip_char (p, ')');
            if (!p)
                break;

            cairo_matrix_multiply (matrix, &m, matrix);

        } else if (string_match (&p, "translate")) {
            p = skip_char (p, '(');
            if (!p)
                break;

            p = get_floats (p, 1, 1, &have_optional, &x, &y);
            if (!p)
                break;

            p = skip_char (p, ')');
            if (!p)
                break;

            if (!have_optional)
                y = 0;

            cairo_matrix_translate (matrix, x, y);

        } else if (string_match (&p, "scale")) {
            p = skip_char (p, '(');
            if (!p)
                break;

            p = get_floats (p, 1, 1, &have_optional, &x, &y);
            if (!p)
                break;

            p = skip_char (p, ')');
            if (!p)
                break;

            if (!have_optional)
                y = x;

            cairo_matrix_scale (matrix, x, y);

        } else if (string_match (&p, "rotate")) {
            p = skip_char (p, '(');
            if (!p)
                break;

            p = get_floats (p, 1, 2, &have_optional, &a, &x, &y);
            if (!p)
                break;

            p = skip_char (p, ')');
            if (!p)
                break;

            if (!have_optional) {
                x = 0;
                y = 0;
            }

            a *= M_PI/180.0;
            cairo_matrix_translate (matrix, x, y);
            cairo_matrix_rotate (matrix, a);
            cairo_matrix_translate (matrix, -x, -y);

        } else if (string_match (&p, "skewX")) {
            p = skip_char (p, '(');
            if (!p)
                break;

            p = get_floats (p, 1, 0, NULL, &a);
            if (!p)
                break;

            p = skip_char (p, ')');
            if (!p)
                break;

            a *= M_PI/180.0;
            cairo_matrix_init_identity (&m);
            m.xy = tan (a);
            cairo_matrix_multiply (matrix, &m, matrix);

        } else if (string_match (&p, "skewY")) {
            p = skip_char (p, '(');
            if (!p)
                break;

            p = get_floats (p, 1, 0, NULL, &a);
            if (!p)
                break;

            p = skip_char (p, ')');
            if (!p)
                break;

            a *= M_PI/180.0;
            cairo_matrix_init_identity (&m);
            m.yx = tan (a);
            cairo_matrix_multiply (matrix, &m, matrix);

        } else {
            break;
        }
    }
    return p != NULL;
}

static void
render_element_tree (cairo_svg_glyph_render_t *svg_render,
                     cairo_svg_element_t      *element,
                     cairo_svg_element_t      *display_element,
                     cairo_bool_t              children_only);

static cairo_pattern_t *
create_pattern (cairo_svg_glyph_render_t *svg_render,
                cairo_svg_element_t      *paint_server)
{
    cairo_pattern_t *pattern = NULL;

    if (paint_server) {
        svg_render->build_pattern.paint_server = paint_server;
        render_element_tree (svg_render, paint_server, NULL, FALSE);
        pattern = svg_render->build_pattern.pattern;
        svg_render->build_pattern.pattern = NULL;
        svg_render->build_pattern.paint_server = NULL;
        svg_render->build_pattern.type = BUILD_PATTERN_NONE;
    }

    if (!pattern)
        pattern = cairo_pattern_create_rgb (0, 0, 0);

    return pattern;
}

static cairo_bool_t
render_element_svg (cairo_svg_glyph_render_t *svg_render,
                    cairo_svg_element_t      *element,
                    cairo_bool_t              end_tag)
{
    double width, height;
    double vb_x, vb_y, vb_height, vb_width;
    const char *p;
    const char *end;

    if (end_tag)
        return FALSE;

    /* Default viewport width, height is EM square */
    if (!get_float_or_percent_attribute (element, "width", svg_render->units_per_em, &width))
        width = svg_render->units_per_em;

    if (!get_float_or_percent_attribute (element, "height", svg_render->units_per_em, &height))
        height = svg_render->units_per_em;

    /* Transform viewport to unit square, centering it if width != height. */
    if (width > height) {
        cairo_scale (svg_render->cr, 1.0/width, 1.0/width);
        cairo_translate (svg_render->cr, 0, (width - height)/2.0);
    } else {
        cairo_scale (svg_render->cr, 1.0/height, 1.0/height);
        cairo_translate (svg_render->cr, (height - width)/2.0, 0);
    }

    svg_render->width = width;
    svg_render->height = height;

    p = get_attribute (element, "viewBox");
    if (p) {
        /* Transform viewport to viewbox */
        end = get_path_params (p, 4, &vb_x, &vb_y, &vb_width, &vb_height);
        if (!end) {
            print_warning (svg_render, "viewBox expected 4 numbers: %s", p);
            return FALSE;
        }
        cairo_translate (svg_render->cr, -vb_x * width/vb_width, -vb_y * width/vb_width);
        cairo_scale (svg_render->cr, width/vb_width, height/vb_height);
        svg_render->width = vb_width;
        svg_render->height = vb_height;
    }

    svg_render->view_port_set = TRUE;
    return TRUE;
}

static cairo_bool_t
render_element_clip_path (cairo_svg_glyph_render_t *svg_render,
                          cairo_svg_element_t      *element,
                          cairo_bool_t              end_tag)
{
    cairo_svg_graphics_state_t *gs = svg_render->graphics_state;
    const char *p;

    if (end_tag || gs->mode != GS_CLIP || svg_render->build_pattern.type != BUILD_PATTERN_NONE) {
        return FALSE;
    }

    p = get_attribute (element, "clipPathUnits");
    if (string_equal (p, "objectBoundingBox")) {
        cairo_translate (svg_render->cr,
                                svg_render->graphics_state->bbox.x,
                                svg_render->graphics_state->bbox.y);
        cairo_scale (svg_render->cr,
                     svg_render->graphics_state->bbox.width,
                     svg_render->graphics_state->bbox.height);
    }

    return TRUE;
}

static void
apply_gradient_attributes (cairo_svg_glyph_render_t *svg_render,
                           cairo_svg_element_t      *element)
{
    cairo_pattern_t *pattern = svg_render->build_pattern.pattern;
    cairo_bool_t object_bbox = TRUE;
    cairo_matrix_t transform;
    cairo_matrix_t mat;
    const char *p;

    if (!pattern)
        return;

    p = get_attribute (element, "gradientUnits");
    if (string_equal (p, "userSpaceOnUse"))
        object_bbox = FALSE;

    cairo_matrix_init_identity (&mat);
    if (object_bbox) {
        cairo_matrix_translate (&mat,
                                svg_render->graphics_state->bbox.x,
                                svg_render->graphics_state->bbox.y);
        cairo_matrix_scale (&mat,
                            svg_render->graphics_state->bbox.width,
                            svg_render->graphics_state->bbox.height);
    }

    p = get_attribute (element, "gradientTransform");
     if (parse_transform (p, &transform))
         cairo_matrix_multiply (&mat, &transform, &mat);

    if (cairo_matrix_invert (&mat) == CAIRO_STATUS_SUCCESS)
        cairo_pattern_set_matrix (pattern, &mat);

    p = get_attribute (element, "spreadMethod");
    if (string_equal (p, "reflect"))
        cairo_pattern_set_extend (pattern, CAIRO_EXTEND_REFLECT);
    else if (string_equal (p, "repeat"))
        cairo_pattern_set_extend (pattern, CAIRO_EXTEND_REPEAT);
}

static cairo_bool_t
render_element_linear_gradient (cairo_svg_glyph_render_t *svg_render,
                                cairo_svg_element_t      *element,
                                cairo_bool_t              end_tag)
{
    double x1, y1, x2, y2;

    if (svg_render->build_pattern.paint_server != element ||
        end_tag ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    /* FIXME default value for userSpaceOnUse? */
    double width = 1.0;
    double height = 1.0;

    if (!get_float_or_percent_attribute (element, "x1", width, &x1))
        x1 = 0.0;

    if (!get_float_or_percent_attribute (element, "y1", height, &y1))
        y1 = 0.0;

    if (!get_float_or_percent_attribute (element, "x2", width, &x2))
        x2 = width;

    if (!get_float_or_percent_attribute (element, "y2", height, &y2))
        y2 = 0.0;

    if (svg_render->build_pattern.pattern)
        abort();

    svg_render->build_pattern.pattern = cairo_pattern_create_linear (x1, y1, x2, y2);
    svg_render->build_pattern.type = BUILD_PATTERN_LINEAR;
    apply_gradient_attributes (svg_render, element);
    return TRUE;
}

static cairo_bool_t
render_element_radial_gradient (cairo_svg_glyph_render_t *svg_render,
                                cairo_svg_element_t      *element,
                                cairo_bool_t              end_tag)
{
    double cx, cy, r, fx, fy;

    if (svg_render->build_pattern.paint_server != element ||
        end_tag ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    /* FIXME default value for userSpaceOnUse? */
    double width = 1.0;
    double height = 1.0;

    if (!get_float_or_percent_attribute (element, "cx", width, &cx))
        cx = 0.5 * width;

    if (!get_float_or_percent_attribute (element, "cy", height, &cy))
        cy = 0.5 * height;

    if (!get_float_or_percent_attribute (element, "r", width, &r))
        r = 0.5 * width;

    if (!get_float_or_percent_attribute (element, "fx", width, &fx))
        fx = cx;

    if (!get_float_or_percent_attribute (element, "fy", height, &fy))
        fy = cy;

    svg_render->build_pattern.pattern = cairo_pattern_create_radial (fx, fy, 0, cx, cy, r);
    svg_render->build_pattern.type = BUILD_PATTERN_RADIAL;
    apply_gradient_attributes (svg_render, element);
    return TRUE;
}

static cairo_bool_t
render_element_stop (cairo_svg_glyph_render_t *svg_render,
                     cairo_svg_element_t      *element,
                     cairo_bool_t              end_tag)
{
    double offset, opacity;
    cairo_pattern_t *pattern = svg_render->build_pattern.pattern;

    if (!pattern)
        return FALSE;

    if (cairo_pattern_get_type (pattern) != CAIRO_PATTERN_TYPE_LINEAR &&
        cairo_pattern_get_type (pattern) != CAIRO_PATTERN_TYPE_RADIAL)
        return FALSE;

    if (!get_float_or_percent_attribute (element, "offset", 1.0, &offset))
        return FALSE;

    if (!get_float_attribute (element, "stop-opacity", &opacity))
        opacity = 1.0;

    cairo_svg_color_t color;
    get_color (svg_render, "black", &color);
    get_color (svg_render, get_attribute(element, "stop-color"), &color);
    if (color.type == RGB) {
        cairo_pattern_add_color_stop_rgba (pattern,
                                           offset,
                                           color.red,
                                           color.green,
                                           color.blue,
                                           opacity);
    } else { /* color.type == FOREGROUND */
        double red, green, blue, alpha;
        if (cairo_pattern_get_rgba (svg_render->foreground_source, &red, &green, &blue, &alpha) == CAIRO_STATUS_SUCCESS) {
	    svg_render->foreground_source_used = TRUE;
	} else {
            red = green = blue = 0;
            alpha = 1;
        }
        cairo_pattern_add_color_stop_rgba (pattern, offset, red, green, blue, alpha);
    }
    return TRUE;
}

static cairo_bool_t
render_element_g (cairo_svg_glyph_render_t *svg_render,
                  cairo_svg_element_t      *element,
                  cairo_bool_t              end_tag)
{
    if (svg_render->graphics_state->mode == GS_NO_RENDER ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    if (!end_tag) {
        cairo_push_group (svg_render->cr);
    } else {
        cairo_pop_group_to_source (svg_render->cr);
        cairo_paint_with_alpha (svg_render->cr, svg_render->graphics_state->opacity);
    }
    return TRUE;
}

typedef struct {
    const char *data; /* current position in base64 data */
    char buf[3]; /* decode buffer */
    int buf_pos; /* current position in buf_pos. */
} base64_decode_t;

static cairo_status_t
_read_png_from_base64 (void *closure, unsigned char *data, unsigned int length)
{
    base64_decode_t *decode = closure;
    int n, c;
    unsigned val;

    while (length) {
        if (decode->buf_pos >= 0) {
            *data++ = decode->buf[decode->buf_pos++];
            length--;
            if (decode->buf_pos == 3)
                decode->buf_pos = -1;
        }
        if (length > 0 && decode->buf_pos < 0) {
            n = 0;
            while (*decode->data && n < 4) {
                c = *decode->data++;
                if (c >='A' && c <='Z') {
                    val = (val << 6) | (c -'A');
                    n++;
                } else if (c >='a' && c <='z') {
                    val = (val << 6) | (c -'a' + 26);
                    n++;
                } else if (c >='0' && c <='9') {
                    val = (val << 6) | (c -'0' + 52);
                    n++;
                } else if (c =='+') {
                    val = (val << 6) | 62;
                    n++;
                } else if (c =='/') {
                    val = (val << 6) | 63;
                    n++;
                } else if (c == '=') {
                    val = (val << 6);
                    n++;
                }
            }
            if (n < 4)
                return CAIRO_STATUS_READ_ERROR;

            decode->buf[0] = val >> 16;
            decode->buf[1] = val >> 8;
            decode->buf[2] = val >> 0;
            decode->buf_pos = 0;
        }
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_bool_t
render_element_image (cairo_svg_glyph_render_t *svg_render,
                      cairo_svg_element_t      *element,
                      cairo_bool_t              end_tag)
{
    double x, y, width, height;
    int w, h;
    const char *data;
    cairo_surface_t *surface;
    base64_decode_t decode;

    if (svg_render->graphics_state->mode == GS_NO_RENDER ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    if (!get_float_attribute (element, "x", &x))
        x = 0;

    if (!get_float_attribute (element, "y", &y))
        y = 0;

    if (!get_float_attribute (element, "width", &width))
        return FALSE;

    if (!get_float_attribute (element, "height", &height))
        return FALSE;

    data = get_href_attribute (element);
    if (!data)
        return FALSE;

    if (!string_match (&data, "data:image/png;base64,"))
        return FALSE;

    decode.data = data;
    decode.buf_pos = -1;
    surface = cairo_image_surface_create_from_png_stream (_read_png_from_base64, &decode);
    if (cairo_surface_status (surface)) {
        print_warning (svg_render, "Unable to decode PNG");
        cairo_surface_destroy (surface);
        return FALSE;
    }

    w = cairo_image_surface_get_width (surface);
    h = cairo_image_surface_get_height (surface);

    if (w > 0 && h > 0) {
        cairo_translate (svg_render->cr, x, y);
        cairo_scale (svg_render->cr, width/w, height/h);
        cairo_set_source_surface (svg_render->cr, surface, 0, 0);
        cairo_paint (svg_render->cr);
    }

    cairo_surface_destroy (surface);

    return FALSE;
}

static cairo_bool_t
render_element_use (cairo_svg_glyph_render_t *svg_render,
                    cairo_svg_element_t      *element,
                    cairo_bool_t              end_tag)
{
    double x = 0;
    double y = 0;
    const char *id;

    if (end_tag || svg_render->graphics_state->mode == GS_NO_RENDER ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    get_float_attribute (element, "x", &x);
    get_float_attribute (element, "y", &y);

    id = get_href_attribute (element);
    if (!id)
        return FALSE;

    cairo_svg_element_t *use_element = lookup_element (svg_render, id);
    cairo_translate (svg_render->cr, x, y);
    render_element_tree (svg_render, use_element, NULL, FALSE);
    return TRUE;
}

static cairo_bool_t
draw_path (cairo_svg_glyph_render_t *svg_render)
{
    cairo_svg_graphics_state_t *gs = svg_render->graphics_state;
    cairo_pattern_t *pattern;
    cairo_bool_t opacity_group = FALSE;

    if (gs->mode == GS_COMPUTE_BBOX) {
        cairo_set_source_rgb (svg_render->cr, 0, 0, 0);
        cairo_set_fill_rule (svg_render->cr, gs->fill_rule);
        cairo_fill (svg_render->cr);
        return FALSE;
    } else if (gs->mode == GS_CLIP) {
        return FALSE;
    }

    if (gs->opacity < 1.0) {
        cairo_push_group (svg_render->cr);
        opacity_group = TRUE;
    }

    cairo_path_t *path = cairo_copy_path (svg_render->cr);
    cairo_new_path (svg_render->cr);

    if (gs->fill.type != PAINT_NONE) {
        cairo_bool_t group = FALSE;
        if (gs->fill.type == PAINT_COLOR) {
            if (gs->fill.color.type == RGB) {
                cairo_set_source_rgba (svg_render->cr,
                                       gs->fill.color.red,
                                       gs->fill.color.green,
                                       gs->fill.color.blue,
                                       gs->fill_opacity);
            } else if (gs->fill.color.type == FOREGROUND) {
		cairo_set_source (svg_render->cr, svg_render->foreground_marker);
		if (gs->fill_opacity < 1.0)
		    group = TRUE;
            }
        } else if (gs->fill.type == PAINT_SERVER) {
            pattern = create_pattern (svg_render, gs->fill.paint_server);
            cairo_set_source (svg_render->cr, pattern);
            cairo_pattern_destroy (pattern);
            if (gs->fill_opacity < 1.0)
                group = TRUE;
        }

        if (group)
            cairo_push_group (svg_render->cr);

        cairo_append_path (svg_render->cr, path);
        cairo_set_fill_rule (svg_render->cr, gs->fill_rule);
        cairo_fill (svg_render->cr);
        if (group) {
            cairo_pop_group_to_source (svg_render->cr);
            cairo_paint_with_alpha (svg_render->cr, gs->fill_opacity);
        }
    }

    if (gs->stroke.type != PAINT_NONE) {
        cairo_bool_t group = FALSE;
        if (gs->stroke.type == PAINT_COLOR) {
            if (gs->stroke.color.type == RGB) {
                cairo_set_source_rgba (svg_render->cr,
                                       gs->stroke.color.red,
                                       gs->stroke.color.green,
                                       gs->stroke.color.blue,
                                       gs->stroke_opacity);
            } else if (gs->fill.color.type == FOREGROUND) {
		cairo_set_source (svg_render->cr, svg_render->foreground_marker);
		if (gs->fill_opacity < 1.0)
		    group = TRUE;
            }
        } else if (gs->stroke.type == PAINT_SERVER) {
            pattern = create_pattern (svg_render, gs->stroke.paint_server);
            cairo_set_source (svg_render->cr, pattern);
            cairo_pattern_destroy (pattern);
            if (gs->stroke_opacity < 1.0)
                group = TRUE;
        }

        if (group)
            cairo_push_group (svg_render->cr);

        cairo_append_path (svg_render->cr, path);
        cairo_stroke (svg_render->cr);

        if (group) {
            cairo_pop_group_to_source (svg_render->cr);
            cairo_paint_with_alpha (svg_render->cr, gs->stroke_opacity);
        }
    }

    cairo_path_destroy (path);

    if (opacity_group) {
        cairo_pop_group_to_source (svg_render->cr);
        cairo_paint_with_alpha (svg_render->cr, gs->opacity);
    }
    return TRUE;
}

static void
elliptical_arc (cairo_svg_glyph_render_t *svg_render,
                double                    cx,
                double                    cy,
                double                    rx,
                double                    ry,
                double                    angle1,
                double                    angle2)
{
    cairo_save (svg_render->cr);
    cairo_translate (svg_render->cr, cx, cy);
    cairo_scale (svg_render->cr, rx, ry);
    cairo_arc (svg_render->cr, 0, 0, 1, angle1, angle2);
    cairo_restore (svg_render->cr);
}

static cairo_bool_t
render_element_rect (cairo_svg_glyph_render_t *svg_render,
                     cairo_svg_element_t      *element,
                     cairo_bool_t              end_tag)
{
    double x = 0;
    double y = 0;
    double width = svg_render->width;
    double height = svg_render->height;
    double rx = 0;
    double ry = 0;

    if (end_tag ||
        svg_render->graphics_state->mode == GS_NO_RENDER ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    get_float_or_percent_attribute (element, "x", svg_render->width, &x);
    get_float_or_percent_attribute (element, "y", svg_render->height, &y);
    get_float_or_percent_attribute (element, "width", svg_render->width, &width);
    get_float_or_percent_attribute (element, "height", svg_render->height, &height);
    get_float_or_percent_attribute (element, "rx", svg_render->width, &rx);
    get_float_or_percent_attribute (element, "ry", svg_render->height, &ry);

    if (rx == 0 && ry == 0) {
        cairo_rectangle (svg_render->cr, x, y, width, height);
    } else {
        cairo_move_to (svg_render->cr, x + rx, y);
        cairo_line_to (svg_render->cr, x + width - rx, y);
        elliptical_arc (svg_render,    x + width - rx, y + ry, rx, ry, -M_PI/2, 0);
        cairo_line_to (svg_render->cr, x + width, y + height - ry);
        elliptical_arc (svg_render,    x + width - rx, y + height - ry, rx, ry, 0, M_PI/2);
        cairo_line_to (svg_render->cr, x + rx, y + height);
        elliptical_arc (svg_render,    x + rx, y + height - ry, rx, ry, M_PI/2, M_PI);
        cairo_line_to (svg_render->cr, x, y + ry);
        elliptical_arc (svg_render,    x + rx, y + ry, rx, ry, M_PI, -M_PI/2);
    }

    draw_path (svg_render);
    return TRUE;
}

static cairo_bool_t
render_element_circle (cairo_svg_glyph_render_t *svg_render,
                       cairo_svg_element_t      *element,
                       cairo_bool_t              end_tag)
{
    double cx = 0;
    double cy = 0;
    double r = 0;

    if (end_tag ||
        svg_render->graphics_state->mode == GS_NO_RENDER ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    get_float_or_percent_attribute (element, "cx", svg_render->width, &cx);
    get_float_or_percent_attribute (element, "cy", svg_render->height, &cy);
    get_float_or_percent_attribute (element, "r", svg_render->width, &r);

    cairo_arc (svg_render->cr, cx, cy, r, 0, 2*M_PI);

    draw_path (svg_render);
    return TRUE;
}

static cairo_bool_t
render_element_ellipse (cairo_svg_glyph_render_t *svg_render,
                        cairo_svg_element_t      *element,
                        cairo_bool_t              end_tag)
{
    double cx = 0;
    double cy = 0;
    double rx = 0;
    double ry = 0;

    if (end_tag ||
        svg_render->graphics_state->mode == GS_NO_RENDER ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    get_float_or_percent_attribute (element, "cx", svg_render->width, &cx);
    get_float_or_percent_attribute (element, "cy", svg_render->height, &cy);
    get_float_or_percent_attribute (element, "rx", svg_render->width, &rx);
    get_float_or_percent_attribute (element, "ry", svg_render->height, &ry);

    elliptical_arc (svg_render, cx, cy, rx, ry, 0, 2*M_PI);
    draw_path (svg_render);
    return TRUE;
}

static cairo_bool_t
render_element_line (cairo_svg_glyph_render_t *svg_render,
                     cairo_svg_element_t      *element,
                     cairo_bool_t              end_tag)
{
    double x1 = 0;
    double y1 = 0;
    double x2 = 0;
    double y2 = 0;

    if (end_tag ||
        svg_render->graphics_state->mode == GS_NO_RENDER ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    get_float_or_percent_attribute (element, "x1", svg_render->width, &x1);
    get_float_or_percent_attribute (element, "y1", svg_render->height, &y1);
    get_float_or_percent_attribute (element, "x2", svg_render->width, &x2);
    get_float_or_percent_attribute (element, "y2", svg_render->height, &y2);

    cairo_move_to (svg_render->cr, x1, y1);
    cairo_line_to (svg_render->cr, x2, y2);

    draw_path (svg_render);
    return TRUE;
}

static cairo_bool_t
render_element_polyline (cairo_svg_glyph_render_t *svg_render,
                         cairo_svg_element_t      *element,
                         cairo_bool_t              end_tag)
{
    const char *p;
    const char *end;
    double x, y;
    cairo_bool_t have_move = FALSE;

    if (end_tag ||
        svg_render->graphics_state->mode == GS_NO_RENDER ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    p = get_attribute (element, "points");
    do {
        end = get_path_params (p, 2, &x, &y);
        if (!end) {
            print_warning (svg_render, "points expected 2 numbers: %s", p);
            break;
        }
        p = end;
        if (!have_move) {
            cairo_move_to (svg_render->cr, x, y);
            have_move = TRUE;
        } else {
            cairo_line_to (svg_render->cr, x, y);
        }
        p = skip_space (p);
    } while (p && *p);

    if (string_equal (element->tag, "polygon"))
        cairo_close_path (svg_render->cr);

    draw_path (svg_render);
    return TRUE;
}

static double
angle_between_vectors (double ux,
                       double uy,
                       double vx,
                       double vy)
{
    double dot = ux*vx + uy*vy;
    double umag = sqrt (ux*ux + uy*uy);
    double vmag = sqrt (vx*vx + vy*vy);
    double c = dot/(umag*vmag);
    if (c > 1.0)
        c = 1.0;

    if (c < -1.0)
        c = -1.0;

    double a = acos (c);
    if (ux * vy - uy * vx < 0.0)
        a = -a;

    return a;
}

static void
arc_path (cairo_t *cr,
          double x1, double y1,
          double x2, double y2,
          double rx, double ry,
          double rotate,
          cairo_bool_t large_flag,
          cairo_bool_t sweep_flag)
{
    double x1_, y1_, cx_, cy_;
    double xm, ym, cx, cy;
    double a, b, d;
    double ux, uy, vx, vy;
    double theta, delta_theta;
    double epsilon;
    cairo_matrix_t ctm;

    cairo_get_matrix (cr, &ctm);
    epsilon = _cairo_matrix_transformed_circle_major_axis (&ctm, cairo_get_tolerance (cr));

    rotate *= M_PI/180.0;

    /* Convert endpoint to center parameterization.
     * See SVG 1.1 Appendix F.6. Step numbers are the steps in the appendix.
     */

    rx = fabs (rx);
    ry = fabs (ry);
    if (rx < epsilon || ry < epsilon) {
        cairo_line_to (cr, x2, y2);
        return;
    }

    if (fabs(x1 - x2) < epsilon && fabs(y1 - y2) < epsilon) {
        cairo_line_to (cr, x2, y2);
        return;
    }

    /* Step 1 */
    xm = (x1 - x2)/2;
    ym = (y1 - y2)/2;
    x1_ = xm * cos (rotate) + ym * sin (rotate);
    y1_ = xm * -sin (rotate) + ym * cos (rotate);

    d = (x1_*x1_)/(rx*rx) + (y1_*y1_)/(ry*ry);
    if (d > 1.0) {
        d = sqrt (d);
        rx *= d;
        ry *= d;
    }

    /* Step 2 */
    a = (rx*rx * y1_*y1_) + (ry*ry * x1_*x1_);
    if (a == 0.0)
        return;

    b = (rx*rx * ry*ry) / a - 1.0;
    if (b < 0)
        b = 0.0;

    d = sqrt(b);
    if (large_flag == sweep_flag)
        d = -d;

    cx_ = d * rx*y1_/ry;
    cy_ = d * -ry*x1_/rx;

    /* Step 3 */
    cx = cx_ * cos (rotate) - cy_ * sin (rotate) + (x1 + x2)/2;
    cy = cx_ * sin (rotate) + cy_ * cos (rotate) + (y1 + y2)/2;

    /* Step 4 */
    ux = (x1_ - cx_)/rx;
    uy = (y1_ - cy_)/ry;
    vx = (-x1_ - cx_)/rx;
    vy = (-y1_ - cy_)/ry;
    theta = angle_between_vectors (1.0, 0, ux, uy);
    delta_theta = angle_between_vectors (ux, uy, vx, vy);

    if (!sweep_flag && delta_theta > 0)
        delta_theta -= 2 * M_PI;
    else if (sweep_flag && delta_theta < 0)
        delta_theta += 2 * M_PI;

    /* Now we can call cairo_arc() */
    cairo_save (cr);
    cairo_translate (cr, cx, cy);
    cairo_scale (cr, rx, ry);
    cairo_rotate (cr, theta);
    if (delta_theta >= 0.0)
        cairo_arc (cr, 0, 0, 1, 0, delta_theta);
    else
        cairo_arc_negative (cr, 0, 0, 1, 0, delta_theta);
    cairo_restore (cr);
}

static void
get_current_point (cairo_svg_glyph_render_t *svg_render, double *x, double *y)
{
    if (cairo_has_current_point (svg_render->cr)) {
        cairo_get_current_point (svg_render->cr, x, y);
    } else {
        *x = 0;
        *y = 0;
    }
}

static void
reflect_point (double origin_x, double origin_y, double *x, double *y)
{
    *x = 2*origin_x - *x;
    *y = 2*origin_y - *y;
}

static cairo_bool_t
render_element_path (cairo_svg_glyph_render_t *svg_render,
                     cairo_svg_element_t      *element,
                     cairo_bool_t              end_tag)
{
    double cur_x, cur_y;
    double last_cp_x, last_cp_y;
    double x, y, x1, y1, x2, y2;
    double qx1, qy1, qx2, qy2;
    double rx, ry, rotate, large_flag, sweep_flag;
    cairo_bool_t rel, have_move;
    enum { CUBIC, QUADRATIC, OTHER } last_op;

    if (end_tag ||
        svg_render->graphics_state->mode == GS_NO_RENDER ||
        svg_render->build_pattern.type != BUILD_PATTERN_NONE)
        return FALSE;

    last_op = OTHER;
    const char *p = get_attribute (element, "d");
    const char *end;
    int op;

    while (p) {
        while (p && _cairo_isspace (*p))
            p++;

        if (!p || *p == 0)
            break;

        op = *p;
        switch (op) {
            case 'M':
            case 'm':
                rel = op == 'm';
                p++;
                have_move = FALSE;
                do {
                    end = get_path_params (p, 2, &x, &y);
                    if (!end) {
                        print_warning (svg_render, "path %c expected 2 numbers: %s", op, p);
                        break;
                    }
                    p = end;
                    if (rel) {
                        get_current_point (svg_render, &cur_x, &cur_y);
                        x += cur_x;
                        y += cur_y;
                    }
                    if (!have_move) {
                        cairo_move_to (svg_render->cr, x, y);
                        have_move = TRUE;
                    } else {
                        cairo_line_to (svg_render->cr, x, y);
                    }
                    p = skip_space (p);
                } while (p && *p && !_cairo_isalpha(*p));
                last_op = OTHER;
                break;
            case 'Z':
            case 'z':
                p++;
                cairo_close_path (svg_render->cr);
                last_op = OTHER;
                break;
            case 'L':
            case 'l':
                rel = op == 'l';
                p++;
                do {
                    end = get_path_params (p, 2, &x, &y);
                    if (!end) {
                        print_warning (svg_render, "path %c expected 2 numbers: %s", op, p);
                        break;
                    }
                    p = end;
                    if (rel) {
                        get_current_point (svg_render, &cur_x, &cur_y);
                        x += cur_x;
                        y += cur_y;
                    }
                    cairo_line_to (svg_render->cr, x, y);
                    p = skip_space (p);
                } while (p && *p && !_cairo_isalpha(*p));
                last_op = OTHER;
                break;
            case 'H':
            case 'h':
                rel = op == 'h';
                p++;
                do {
                    end = get_path_params (p, 1, &x1);
                    if (!end) {
                        print_warning (svg_render, "path %c expected a number: %s", op, p);
                        break;
                    }
                    p = end;
                    get_current_point (svg_render, &cur_x, &cur_y);
                    if (rel) {
                        x1 += cur_x;
                    }
                    cairo_line_to (svg_render->cr, x1, cur_y);
                    p = skip_space (p);
                } while (p && *p && !_cairo_isalpha(*p));
                last_op = OTHER;
                break;
            case 'V':
            case 'v':
                rel = op == 'v';
                p++;
                do {
                    end = get_path_params (p, 1, &y1);
                    if (!end) {
                        print_warning (svg_render, "path %c expected a number: %s", op, p);
                        break;
                    }
                    p = end;
                    get_current_point (svg_render, &cur_x, &cur_y);
                    if (rel) {
                        y1 += cur_y;
                    }
                    cairo_line_to (svg_render->cr, cur_x, y1);
                    p = skip_space (p);
                } while (p && *p && !_cairo_isalpha(*p));
                last_op = OTHER;
                break;
            case 'C':
            case 'c':
                rel = op == 'c';
                p++;
                do {
                    end = get_path_params (p, 6, &x1, &y1, &x2, &y2, &x, &y);
                    if (!end) {
                        print_warning (svg_render, "path %c expected 6 numbers: %s", op, p);
                        break;
                    }
                    p = end;
                    if (rel) {
                        get_current_point (svg_render, &cur_x, &cur_y);
                        x1 += cur_x;
                        y1 += cur_y;
                        x2 += cur_x;
                        y2 += cur_y;
                        x += cur_x;
                        y += cur_y;
                    }
                    cairo_curve_to (svg_render->cr, x1, y1, x2, y2, x, y);
                    p = skip_space (p);
                } while (p && *p && !_cairo_isalpha(*p));
                last_op = CUBIC;
                last_cp_x = x2;
                last_cp_y = y2;
                break;
            case 'S':
            case 's':
                rel = op == 's';
                p++;
                do {
                    end = get_path_params (p, 4, &x2, &y2, &x, &y);
                    if (!end) {
                        print_warning (svg_render, "path %c expected 4 numbers: %s", op, p);
                        break;
                    }
                    p = end;
                    get_current_point (svg_render, &cur_x, &cur_y);
                    if (rel) {
                        x2 += cur_x;
                        y2 += cur_y;
                        x += cur_x;
                        y += cur_y;
                    }
                    if (last_op == CUBIC) {
                        x1 = last_cp_x;
                        y1 = last_cp_y;
                        reflect_point (cur_x, cur_y, &x1, &y1);
                    } else {
                        x1 = cur_x;
                        y1 = cur_y;
                    }
                    cairo_curve_to (svg_render->cr, x1, y1, x2, y2, x, y);
                    last_op = CUBIC;
                    last_cp_x = x2;
                    last_cp_y = y2;
                    p = skip_space (p);
                } while (p && *p && !_cairo_isalpha(*p));
                break;
            case 'Q':
            case 'q':
                rel = op == 'q';
                p++;
                do {
                    end = get_path_params (p, 4, &x1, &y1, &x, &y);
                    if (!end) {
                        print_warning (svg_render, "path %c expected 4 numbers: %s", op, p);
                        break;
                    }
                    p = end;
                    get_current_point (svg_render, &cur_x, &cur_y);
                    if (rel) {
                        x1 += cur_x;
                        y1 += cur_y;
                        x += cur_x;
                        y += cur_y;
                    }
                    qx1 = cur_x + (x1 - cur_x)*2/3;
                    qy1 = cur_y + (y1 - cur_y)*2/3;
                    qx2 = x + (x1 - x)*2/3;
                    qy2 = y + (y1 - y)*2/3;
                    cairo_curve_to (svg_render->cr, qx1, qy1, qx2, qy2, x, y);
                    p = skip_space (p);
                } while (p && *p && !_cairo_isalpha(*p));
                last_op = QUADRATIC;
                last_cp_x = x1;
                last_cp_y = y1;
                break;
            case 'T':
            case 't':
                rel = op == 't';
                p++;
                do {
                    end = get_path_params (p, 2, &x, &y);
                    if (!end) {
                        print_warning (svg_render, "path %c expected 2 numbers: %s", op, p);
                        break;
                    }
                    p = end;
                    get_current_point (svg_render, &cur_x, &cur_y);
                    if (rel) {
                        x += cur_x;
                        y += cur_y;
                    }
                    if (last_op == QUADRATIC) {
                        x1 = last_cp_x;
                        y1 = last_cp_y;
                        reflect_point (cur_x, cur_y, &x1, &y1);
                    } else {
                        x1 = cur_x;
                        y1 = cur_y;
                    }
                    qx1 = cur_x + (x1 - cur_x)*2/3;
                    qy1 = cur_y + (y1 - cur_y)*2/3;
                    qx2 = x + (x1 - x)*2/3;
                    qy2 = y + (y1 - y)*2/3;
                    cairo_curve_to (svg_render->cr, qx1, qy1, qx2, qy2, x, y);
                    last_op = QUADRATIC;
                    last_cp_x = x1;
                    last_cp_y = y1;
                    p = skip_space (p);
                } while (p && *p && *p && !_cairo_isalpha(*p));
                break;
            case 'A':
            case 'a':
                rel = op == 'a';
                p++;
                do {
                    end = get_path_params (p, 7, &rx, &ry, &rotate, &large_flag, &sweep_flag, &x, &y);
                    if (!end) {
                        print_warning (svg_render, "path %c expected 7 numbers: %s", op, p);
                        break;
                    }
                    p = end;
                    get_current_point (svg_render, &cur_x, &cur_y);
                    if (rel) {
                        x += cur_x;
                        y += cur_y;
                    }
                    arc_path (svg_render->cr,
                              cur_x, cur_y,
                              x, y,
                              rx, ry,
                              rotate,
                              large_flag > 0.5,
                              sweep_flag > 0.5);
                    p = skip_space (p);
                } while (p && *p && !_cairo_isalpha(*p));
                last_op = OTHER;
                break;
            default:
                p = NULL;
                break;
        }
    }

    draw_path (svg_render);
    return TRUE;
}

static void
init_graphics_state (cairo_svg_glyph_render_t *svg_render)
{
    cairo_svg_graphics_state_t *gs;

    gs = _cairo_malloc (sizeof (cairo_svg_graphics_state_t));
    get_paint (svg_render, "black", &gs->fill);
    get_paint (svg_render, "none", &gs->stroke);
    gs->color.type = FOREGROUND;
    gs->fill_opacity = 1.0;
    gs->stroke_opacity = 1.0;
    gs->opacity = 1.0;
    gs->fill_rule = CAIRO_FILL_RULE_WINDING;
    gs->clip_rule = CAIRO_FILL_RULE_WINDING;
    gs->clip_path = NULL;
    gs->dash_array = NULL;
    gs->dash_offset = 0.0;
    gs->mode = GS_RENDER;
    gs->bbox.x = 0;
    gs->bbox.y = 0;
    gs->bbox.width = 0;
    gs->bbox.height = 0;
    gs->next = NULL;

    svg_render->graphics_state = gs;

    cairo_save (svg_render->cr);
    cairo_set_source_rgb (svg_render->cr, 0, 0, 0);
    cairo_set_line_width (svg_render->cr, 1.0);
    cairo_set_line_cap (svg_render->cr, CAIRO_LINE_CAP_BUTT);
    cairo_set_line_join (svg_render->cr, CAIRO_LINE_JOIN_MITER);
    cairo_set_miter_limit (svg_render->cr, 4.0);
}

#define MAX_DASHES 100
static void update_dash (cairo_svg_glyph_render_t *svg_render,
                         cairo_svg_element_t      *element)
{
    cairo_svg_graphics_state_t *gs = svg_render->graphics_state;
    const char *p;
    char *end;
    double value;
    double dash_array[MAX_DASHES];
    int num_dashes = 0;
    cairo_bool_t not_zero = FALSE;
    
    if (gs->dash_array == NULL || string_equal (gs->dash_array, "none")) {
        cairo_set_dash (svg_render->cr, NULL, 0, 0);
        return;
    }

    p = gs->dash_array;
    while (*p && num_dashes < MAX_DASHES) {
        while (*p && (*p == ',' || _cairo_isspace (*p)))
            p++;

        if (*p == 0)
            break;

        value = _cairo_strtod (p, &end);
        if (end == p)
            break;

        p = end;
        if (*p == '%') {
            value *= svg_render->width / 100.0;
            p++;
        }

        if (value < 0.0)
            return;

        if (value > 0.0)
            not_zero = TRUE;

        dash_array[num_dashes++] = value;
    }

    if (not_zero)
        cairo_set_dash (svg_render->cr, dash_array, num_dashes, gs->dash_offset);
}

static cairo_bool_t
pattern_requires_bbox (cairo_svg_glyph_render_t *svg_render,
                       cairo_svg_element_t      *paint_server)
{
    const char *p;

    if (string_equal (paint_server->tag, "linearGradient") ||
        string_equal (paint_server->tag, "radialGradient"))
    {
        p = get_attribute (paint_server, "gradientUnits");
        if (string_equal (p, "userSpaceOnUse"))
            return FALSE;

        return TRUE;
    }
    return FALSE;
}

static cairo_bool_t
clip_requires_bbox (cairo_svg_glyph_render_t *svg_render,
                    const char               *clip_path)
{
    cairo_svg_element_t *element;
    const char *p;

    if (clip_path && strncmp (clip_path, "url", 3) == 0) {
        element = lookup_url_element (svg_render, clip_path);
        if (element) {
            p = get_attribute (element, "clipPathUnits");
            if (string_equal (p, "objectBoundingBox"))
                return TRUE;
        }
    }
    return FALSE;
}

static cairo_bool_t
need_bbox (cairo_svg_glyph_render_t *svg_render,
           cairo_svg_element_t      *element)
{
    cairo_svg_graphics_state_t *gs = svg_render->graphics_state;
    cairo_bool_t fill_needs_bbox = FALSE;
    cairo_bool_t stroke_needs_bbox = FALSE;
    cairo_bool_t clip_needs_bbox = FALSE;
    
    if (gs->mode != GS_RENDER)
        return FALSE;

    if (gs->fill.type == PAINT_SERVER && pattern_requires_bbox (svg_render, gs->fill.paint_server))
        fill_needs_bbox = TRUE;

    if (gs->stroke.type == PAINT_SERVER && pattern_requires_bbox (svg_render, gs->stroke.paint_server))
        stroke_needs_bbox = TRUE;

    if (clip_requires_bbox (svg_render, get_attribute (element, "clip-path")))
        clip_needs_bbox = TRUE;

    if (string_equal (element->tag, "circle") ||
        string_equal (element->tag, "ellipse") ||
        string_equal (element->tag, "path") ||
        string_equal (element->tag, "polygon") ||
        string_equal (element->tag, "rect"))
    {
        return fill_needs_bbox || stroke_needs_bbox || clip_needs_bbox;
    }

    if (string_equal (element->tag, "line") ||
        string_equal (element->tag, "polyline"))
    {
        return stroke_needs_bbox || clip_needs_bbox;
    }

    if (string_equal (element->tag, "g") ||
        string_equal (element->tag, "image") ||
        string_equal (element->tag, "use"))
    {
        return clip_needs_bbox;
    }
    
    return FALSE;
}

static cairo_bool_t
call_element (cairo_svg_glyph_render_t *svg_render,
              cairo_svg_element_t      *element,
              cairo_bool_t              end_tag);

static void
update_graphics_state (cairo_svg_glyph_render_t *svg_render,
                       cairo_svg_element_t      *element)
{
    double value;
    const char *p;
    cairo_svg_graphics_state_t *gs = svg_render->graphics_state;

    p = get_attribute (element, "transform");
    if (p) {
        cairo_matrix_t m;
        if (parse_transform (p, &m))
            cairo_transform (svg_render->cr, &m);
    }

    /* The transform is all we need for bbox computation. The SVG spec
     * excludes clipping and stroke-width from the bbox. */
    if (gs->mode == GS_COMPUTE_BBOX)
        return;
    
    p = get_attribute (element, "color");
    if (p)
        get_color (svg_render, p, &gs->color);

    if (!get_float_attribute (element, "opacity", &gs->opacity))
        gs->opacity = 1.0;

    p = get_attribute (element, "fill");
    if (p) {
        get_paint (svg_render, p, &gs->fill);
    }

    get_float_attribute (element, "fill-opacity", &gs->fill_opacity);

    gs->fill_rule = get_fill_rule_attribute (element, "fill-rule", gs->fill_rule);

    gs->clip_rule = get_fill_rule_attribute (element, "fill-rule", gs->clip_rule);

    p = get_attribute (element, "stroke");
    if (p)
        get_paint (svg_render, p, &gs->stroke);

    if (get_float_or_percent_attribute (element, "stroke-width", svg_render->width, &value))
        cairo_set_line_width (svg_render->cr, value);

    p = get_attribute (element, "stroke-linecap");
    if (string_equal (p, "butt"))
        cairo_set_line_cap (svg_render->cr, CAIRO_LINE_CAP_BUTT);
    else if (string_equal (p, "round"))
        cairo_set_line_cap (svg_render->cr, CAIRO_LINE_CAP_ROUND);
    else if (string_equal (p, "square"))
        cairo_set_line_cap (svg_render->cr, CAIRO_LINE_CAP_SQUARE);

    p = get_attribute (element, "stroke-linejoin");
    if (string_equal (p, "miter"))
        cairo_set_line_join (svg_render->cr, CAIRO_LINE_JOIN_MITER);
    else if (string_equal (p, "round"))
        cairo_set_line_join (svg_render->cr, CAIRO_LINE_JOIN_ROUND);
    else if (string_equal (p, "bevel"))
        cairo_set_line_join (svg_render->cr, CAIRO_LINE_JOIN_BEVEL);

    if (get_float_attribute (element, "stroke-miterlimit", &value))
        cairo_set_miter_limit (svg_render->cr, value);

    p = get_attribute (element, "stroke-dasharray");
    if (p) {
        free (gs->dash_array);
        gs->dash_array = strdup (p);
    }

    get_float_or_percent_attribute (element, "stroke-dashoffset", svg_render->width, &gs->dash_offset);
    update_dash (svg_render, element);

    /* Some elements may need the bounding box of the element thay are
     * applied to.  As this recursively calls render_element on the
     * same element while we are in render_element and setting up the
     * graphics state, we check gs->mode to avoid re-entering the
     * compute bbox code. The GS_COMPUTE_MODE flag is also used by
     * render functions to ignore patterns and strokes (SVG spec
     * ignores stroke with in bbox calculations) and just use a solid
     * color.
     */
    if (gs->mode == GS_RENDER && need_bbox (svg_render, element)) {
        cairo_surface_t *recording = cairo_recording_surface_create (CAIRO_CONTENT_COLOR_ALPHA, NULL);
        cairo_t *old_cr = svg_render->cr;
        svg_render->cr = cairo_create (recording);
        gs_mode_t old_mode = gs->mode;
        gs->mode = GS_COMPUTE_BBOX;
        /* To avoid recursing back into this function, we call the
         * element directory then use render_element_tree to render
         * the children */
        call_element (svg_render, element, FALSE);
        render_element_tree (svg_render, element, NULL, TRUE);
        if (element->type == CONTAINER_ELEMENT)
            call_element (svg_render, element, TRUE);
        gs->mode = old_mode;
        cairo_destroy (svg_render->cr);
        svg_render->cr = old_cr;
        cairo_recording_surface_ink_extents (recording,
                                             &gs->bbox.x,
                                             &gs->bbox.y,
                                             &gs->bbox.width,
                                             &gs->bbox.height);
        cairo_surface_destroy (recording);
    }

    /* clip-path may require bbox */
    p = get_attribute (element, "clip-path");
    if (p && strncmp (p, "url", 3) == 0) {
        element = lookup_url_element (svg_render, p);
        if (element) {
            gs_mode_t old_mode = gs->mode;
            gs->mode = GS_CLIP;
            render_element_tree (svg_render, element, NULL, FALSE);
            cairo_set_fill_rule (svg_render->cr, gs->clip_rule);
            cairo_clip (svg_render->cr);
            gs->mode = old_mode;
        }
    }
}

static void
save_graphics_state (cairo_svg_glyph_render_t *svg_render)
{
    cairo_svg_graphics_state_t *gs;

    cairo_save (svg_render->cr);

    gs = _cairo_malloc (sizeof (cairo_svg_graphics_state_t));
    gs->fill           = svg_render->graphics_state->fill;
    gs->stroke         = svg_render->graphics_state->stroke;
    gs->color          = svg_render->graphics_state->color;
    gs->fill_opacity   = svg_render->graphics_state->fill_opacity;
    gs->stroke_opacity = svg_render->graphics_state->stroke_opacity;
    gs->opacity        = svg_render->graphics_state->opacity;
    gs->fill_rule      = svg_render->graphics_state->fill_rule;
    gs->clip_rule      = svg_render->graphics_state->clip_rule;
    gs->clip_path      = NULL;
    gs->dash_array     = NULL;
    if (svg_render->graphics_state->dash_array)
        gs->dash_array = strdup (svg_render->graphics_state->dash_array);
    gs->dash_offset    = svg_render->graphics_state->dash_offset;
    gs->mode           = svg_render->graphics_state->mode;
    gs->bbox           = svg_render->graphics_state->bbox;
    gs->next           = svg_render->graphics_state;
    svg_render->graphics_state = gs;
}

static void
restore_graphics_state (cairo_svg_glyph_render_t *svg_render)
{
    cairo_svg_graphics_state_t *gs;

    gs = svg_render->graphics_state;
    svg_render->graphics_state = gs->next;
    if (gs->clip_path)
        cairo_path_destroy (gs->clip_path);
    free (gs->dash_array);
    free (gs);

    cairo_restore (svg_render->cr);
}

/* render function returns TRUE if render_element_tree() is to render
 * the child nodes, FALSE if render_element_tree() is to skip the
 * child nodes.
 */
struct render_func {
    const char *tag;
    cairo_bool_t (*render) (cairo_svg_glyph_render_t *, cairo_svg_element_t *, cairo_bool_t);
};

/* Must be sorted */
static const struct render_func render_funcs[] = {
    { "circle", render_element_circle },
    { "clipPath", render_element_clip_path },
    { "defs", NULL },
    { "desc", NULL },
    { "ellipse", render_element_ellipse },
    { "g", render_element_g },
    { "image", render_element_image },
    { "line", render_element_line },
    { "linearGradient", render_element_linear_gradient },
    { "metadata", NULL },
    { "path", render_element_path },
    { "polygon", render_element_polyline },
    { "polyline", render_element_polyline },
    { "radialGradient", render_element_radial_gradient },
    { "rect", render_element_rect },
    { "stop", render_element_stop },
    { "svg", render_element_svg },
    { "title", NULL },
    { "use", render_element_use },
};

static int
_render_func_compare (const void *a, const void *b)
{
    const struct render_func *render_func_a = a;
    const struct render_func *render_func_b = b;

    return strcmp (render_func_a->tag, render_func_b->tag);
}

static cairo_bool_t
call_element (cairo_svg_glyph_render_t *svg_render,
              cairo_svg_element_t      *element,
              cairo_bool_t              end_tag)
{
    const struct render_func *func;
    struct render_func key;
    cairo_bool_t recurse = FALSE;

    key.tag = element->tag;
    key.render = NULL;
    func = bsearch (&key,
                    render_funcs,
                    ARRAY_LENGTH (render_funcs),
                    sizeof (struct render_func),
                    _render_func_compare);
    if (func) {
        if (func->render) {
            recurse = func->render (svg_render, element, end_tag);
        }
    } else {
        print_warning (svg_render, "Unsupported element: %s", element->tag);
    }

    return recurse;
}

static cairo_bool_t
render_element (cairo_svg_glyph_render_t *svg_render,
                cairo_svg_element_t      *element,
                cairo_bool_t              end_tag,
                cairo_svg_element_t      *display_element)
{
    cairo_bool_t recurse = FALSE;
    cairo_svg_graphics_state_t *gs;

    /* Ignore elements if we have not seen "<svg>". Ignore
      * "<svg>" if we have seen it */
    if (svg_render->view_port_set) {
        if (string_equal (element->tag, "svg"))
            return FALSE;
    } else {
        if (!string_equal (element->tag, "svg"))
            return FALSE;
    }

    if (element->type == EMPTY_ELEMENT ||
        (element->type == CONTAINER_ELEMENT && !end_tag))
    {
        save_graphics_state (svg_render);
        update_graphics_state (svg_render, element);
    }

    gs = svg_render->graphics_state;
    if (gs->mode == GS_NO_RENDER && element == display_element)
        gs->mode = GS_RENDER;

    recurse = call_element (svg_render, element, end_tag);

    if (element->type == EMPTY_ELEMENT ||
        (element->type == CONTAINER_ELEMENT && end_tag))
    {
        restore_graphics_state (svg_render);
    }

    return recurse;
}

#define MAX_DEPTH 100

static void
render_element_tree (cairo_svg_glyph_render_t *svg_render,
                     cairo_svg_element_t      *element,
                     cairo_svg_element_t      *display_element,
                     cairo_bool_t              children_only)
{
    if (!element)
        return;

    /* Avoid circular references by limiting the number of recursive
     * calls to this function. */
    if (svg_render->render_element_tree_depth > MAX_DEPTH)
        return;

    svg_render->render_element_tree_depth++;
    if (element->type == EMPTY_ELEMENT && !children_only) {
        render_element (svg_render, element, FALSE, display_element);

    } else if (element->type == CONTAINER_ELEMENT) {
        int num_elems;
        cairo_bool_t recurse = TRUE;;

        if (!children_only)
            recurse = render_element (svg_render, element, FALSE, display_element);

        /* We only render the children if the parent returned
         * success. This is how we avoid rendering non display
         * elements like gradients, <defs>, and anything not
         * implemented. */
        if (recurse) {
            num_elems = _cairo_array_num_elements (&element->children);
            for (int i = 0; i < num_elems; i++) {
                cairo_svg_element_t *child;
                _cairo_array_copy_element (&element->children, i, &child);
                render_element_tree (svg_render, child, display_element, FALSE);
            }
        }

        if (!children_only)
            render_element (svg_render, element, TRUE, display_element);
    }
    svg_render->render_element_tree_depth--;
}

static void
render_element_tree_id (cairo_svg_glyph_render_t *svg_render,
                        const char               *element_id)
{
    cairo_svg_element_t *glyph_element = NULL;

    if (element_id)
        glyph_element = lookup_element (svg_render, element_id);

    if (glyph_element)
        svg_render->graphics_state->mode = GS_NO_RENDER;
    else
        svg_render->graphics_state->mode = GS_RENDER;

    render_element_tree (svg_render, svg_render->tree, glyph_element, TRUE);
}

cairo_status_t
_cairo_render_svg_glyph (const char           *svg_document,
                         unsigned long         first_glyph,
                         unsigned long         last_glyph,
                         unsigned long         glyph,
                         double                units_per_em,
                         FT_Color             *palette,
                         int                   num_palette_entries,
                         cairo_t              *cr,
                         cairo_pattern_t      *foreground_source,
			 cairo_bool_t         *foreground_source_used)
{
    cairo_status_t status = CAIRO_STATUS_SUCCESS;

    cairo_svg_glyph_render_t *svg_render = _cairo_malloc (sizeof (cairo_svg_glyph_render_t));
    if (unlikely (svg_render == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    svg_render->tree = NULL;
    svg_render->ids = _cairo_hash_table_create (_element_id_equal);
    if (unlikely (svg_render->ids == NULL)) {
        free (svg_render);
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);
    }

    svg_render->debug = 0;
    const char *s = getenv ("CAIRO_DEBUG_SVG_RENDER");
    if (s) {
        if (strlen (s) > 0)
            svg_render->debug = atoi (s);
        else
            svg_render->debug = SVG_RENDER_ERROR;
    }

    svg_render->cr = cr;
    svg_render->units_per_em = units_per_em;
    svg_render->build_pattern.paint_server = NULL;
    svg_render->build_pattern.pattern = NULL;
    svg_render->build_pattern.type = BUILD_PATTERN_NONE;
    svg_render->render_element_tree_depth = 0;
    svg_render->view_port_set = FALSE;
    svg_render->num_palette_entries = num_palette_entries;
    svg_render->palette = palette;

    svg_render->foreground_marker = _cairo_pattern_create_foreground_marker ();
    svg_render->foreground_source = cairo_pattern_reference (foreground_source);;
    svg_render->foreground_source_used = FALSE;

    init_graphics_state (svg_render);

    print_info (svg_render, "Glyph ID: %ld", glyph);
    print_info (svg_render, "Palette Entries: %d", num_palette_entries);
    print_info (svg_render, "Units per EM: %f", units_per_em);
    print_info (svg_render, "SVG Document:\n%s\n", svg_document);

    /* First parse elements into a tree and populate ids hash table */
    if (!parse_svg (svg_render, svg_document)) {
        print_error (svg_render, "Parse SVG document failed");
        status = CAIRO_STATUS_SVG_FONT_ERROR;
        goto cleanup;
    }

#if SVG_RENDER_PRINT_FUNCTIONS
    printf("\nTREE\n");
    if (svg_render->tree) {
        print_element (svg_render->tree, TRUE, 0);
        printf("\n");
    }
#endif

    /* Next, render glyph */
    if (first_glyph == last_glyph) {
        /* Render whole document */
        render_element_tree_id (svg_render, NULL);
    } else {
        /* Render element with id "glyphID" where ID is glyph number. */

        char glyph_id[30];
        snprintf(glyph_id, sizeof(glyph_id), "#glyph%ld", glyph);
        render_element_tree_id (svg_render, glyph_id);
    }

  cleanup:
    if (svg_render->build_pattern.pattern)
        cairo_pattern_destroy (svg_render->build_pattern.pattern);

    if (svg_render->tree)
        free_elements (svg_render, svg_render->tree);

    while (svg_render->graphics_state)
        restore_graphics_state (svg_render);

    cairo_pattern_destroy (svg_render->foreground_marker);
    cairo_pattern_destroy (svg_render->foreground_source);
    *foreground_source_used = svg_render->foreground_source_used;

    /* The hash entry for each element with an id is removed by
     * free_elements() */
    _cairo_hash_table_destroy (svg_render->ids);

    free (svg_render);

    return status;
}

#ifdef DEBUG_SVG_RENDER

/**
 * _cairo_debug_svg_render:
 *
 * Debug function for cairo-svg-glyph-render.c. Allows invoking the renderer from outside
 * cairo to test with SVG documents, and to facilitate comparison with librsvg rendering.
 * The viewport is .
 *
 * @cr: render target
 * @svg_document: SVG Document
 * @element: element within svg_document to render (eg "#glyph8"), or NULL to render entire document.
 * @debug_level: 0 - quiet, 1 - print errors, 2 - print warnings, 3 - info
 * @return TRUE on success, ie no errors, FALSE if error
 **/
cairo_bool_t
_cairo_debug_svg_render (cairo_t       *cr,
                         const char    *svg_document,
                         const char    *element,
                         double         units_per_em,
                         int            debug_level);

cairo_bool_t
_cairo_debug_svg_render (cairo_t       *cr,
                         const char    *svg_document,
                         const char    *element,
                         double         units_per_em,
                         int            debug_level)
{
    return _cairo_render_svg_glyph (svg_document,
                                    1, 1, 1,
                                    units_per_em,
                                    NULL, 0,
                                    cr) == CAIRO_STATUS_SUCCESS;
}

#endif /* DEBUG_SVG_RENDER */

#endif /* HAVE_FT_SVG_DOCUMENT */
