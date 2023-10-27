/*
 * Copyright © 2004 Keith Packard
 * Copyright © 2008 Red Hat, Inc.
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
 * The Initial Developer of the Original Code is Keith Packard
 *
 * Contributor(s):
 *      Keith Packard <keithp@keithp.com>
 *      Behdad Esfahbod <behdad@behdad.org>
 */

#include "cairoint.h"
#include "cairo-error-private.h"

#include <math.h>

/*
 * This file implements a user-font rendering the descendant of the Hershey
 * font coded by Keith Packard for use in the Twin window system.
 * The actual font data is in cairo-font-face-twin-data.c
 *
 * Ported to cairo user font and extended by Behdad Esfahbod.
 */



static cairo_user_data_key_t twin_properties_key;


/*
 * Face properties
 */

/* We synthesize multiple faces from the twin data.  Here is the parameters. */

/* The following tables and matching code are copied from Pango */

/* CSS weight */
typedef enum {
  TWIN_WEIGHT_THIN = 100,
  TWIN_WEIGHT_ULTRALIGHT = 200,
  TWIN_WEIGHT_LIGHT = 300,
  TWIN_WEIGHT_BOOK = 380,
  TWIN_WEIGHT_NORMAL = 400,
  TWIN_WEIGHT_MEDIUM = 500,
  TWIN_WEIGHT_SEMIBOLD = 600,
  TWIN_WEIGHT_BOLD = 700,
  TWIN_WEIGHT_ULTRABOLD = 800,
  TWIN_WEIGHT_HEAVY = 900,
  TWIN_WEIGHT_ULTRAHEAVY = 1000
} twin_face_weight_t;

/* CSS stretch */
typedef enum {
  TWIN_STRETCH_ULTRA_CONDENSED,
  TWIN_STRETCH_EXTRA_CONDENSED,
  TWIN_STRETCH_CONDENSED,
  TWIN_STRETCH_SEMI_CONDENSED,
  TWIN_STRETCH_NORMAL,
  TWIN_STRETCH_SEMI_EXPANDED,
  TWIN_STRETCH_EXPANDED,
  TWIN_STRETCH_EXTRA_EXPANDED,
  TWIN_STRETCH_ULTRA_EXPANDED
} twin_face_stretch_t;

typedef struct
{
  int value;
  const char str[16];
} FieldMap;

static const FieldMap slant_map[] = {
  { CAIRO_FONT_SLANT_NORMAL, "" },
  { CAIRO_FONT_SLANT_NORMAL, "Roman" },
  { CAIRO_FONT_SLANT_OBLIQUE, "Oblique" },
  { CAIRO_FONT_SLANT_ITALIC, "Italic" }
};

static const FieldMap smallcaps_map[] = {
  { FALSE, "" },
  { TRUE, "Small-Caps" }
};

static const FieldMap weight_map[] = {
  { TWIN_WEIGHT_THIN, "Thin" },
  { TWIN_WEIGHT_ULTRALIGHT, "Ultra-Light" },
  { TWIN_WEIGHT_ULTRALIGHT, "Extra-Light" },
  { TWIN_WEIGHT_LIGHT, "Light" },
  { TWIN_WEIGHT_BOOK, "Book" },
  { TWIN_WEIGHT_NORMAL, "" },
  { TWIN_WEIGHT_NORMAL, "Regular" },
  { TWIN_WEIGHT_MEDIUM, "Medium" },
  { TWIN_WEIGHT_SEMIBOLD, "Semi-Bold" },
  { TWIN_WEIGHT_SEMIBOLD, "Demi-Bold" },
  { TWIN_WEIGHT_BOLD, "Bold" },
  { TWIN_WEIGHT_ULTRABOLD, "Ultra-Bold" },
  { TWIN_WEIGHT_ULTRABOLD, "Extra-Bold" },
  { TWIN_WEIGHT_HEAVY, "Heavy" },
  { TWIN_WEIGHT_HEAVY, "Black" },
  { TWIN_WEIGHT_ULTRAHEAVY, "Ultra-Heavy" },
  { TWIN_WEIGHT_ULTRAHEAVY, "Extra-Heavy" },
  { TWIN_WEIGHT_ULTRAHEAVY, "Ultra-Black" },
  { TWIN_WEIGHT_ULTRAHEAVY, "Extra-Black" }
};

static const FieldMap stretch_map[] = {
  { TWIN_STRETCH_ULTRA_CONDENSED, "Ultra-Condensed" },
  { TWIN_STRETCH_EXTRA_CONDENSED, "Extra-Condensed" },
  { TWIN_STRETCH_CONDENSED,       "Condensed" },
  { TWIN_STRETCH_SEMI_CONDENSED,  "Semi-Condensed" },
  { TWIN_STRETCH_NORMAL,          "" },
  { TWIN_STRETCH_SEMI_EXPANDED,   "Semi-Expanded" },
  { TWIN_STRETCH_EXPANDED,        "Expanded" },
  { TWIN_STRETCH_EXTRA_EXPANDED,  "Extra-Expanded" },
  { TWIN_STRETCH_ULTRA_EXPANDED,  "Ultra-Expanded" }
};

static const FieldMap monospace_map[] = {
  { FALSE, "" },
  { TRUE, "Mono" },
  { TRUE, "Monospace" }
};


typedef struct _twin_face_properties {
    cairo_font_slant_t  slant;
    twin_face_weight_t  weight;
    twin_face_stretch_t stretch;

    /* lets have some fun */
    cairo_bool_t monospace;
    cairo_bool_t smallcaps;
} twin_face_properties_t;

static cairo_bool_t
field_matches (const char *s1,
               const char *s2,
               int len)
{
  int c1, c2;

  while (len && *s1 && *s2)
    {
#define TOLOWER(c) \
   (((c) >= 'A' && (c) <= 'Z') ? (c) - 'A' + 'a' : (c))

      c1 = TOLOWER (*s1);
      c2 = TOLOWER (*s2);
      if (c1 != c2) {
        if (c1 == '-') {
          s1++;
          continue;
        }
        return FALSE;
      }
      s1++; s2++;
      len--;
    }

  return len == 0 && *s1 == '\0';
}

static cairo_bool_t
parse_int (const char *word,
	   size_t      wordlen,
	   int        *out)
{
  char *end;
  long val = strtol (word, &end, 10);
  int i = val;

  if (end != word && (end == word + wordlen) && val >= 0 && val == i)
    {
      if (out)
        *out = i;

      return TRUE;
    }

  return FALSE;
}

static cairo_bool_t
find_field (const char *what,
	    const FieldMap *map,
	    int n_elements,
	    const char *str,
	    int len,
	    int *val)
{
  int i;
  cairo_bool_t had_prefix = FALSE;

  if (what)
    {
      i = strlen (what);
      if (len > i && 0 == strncmp (what, str, i) && str[i] == '=')
	{
	  str += i + 1;
	  len -= i + 1;
	  had_prefix = TRUE;
	}
    }

  for (i=0; i<n_elements; i++)
    {
      if (map[i].str[0] && field_matches (map[i].str, str, len))
	{
	  if (val)
	    *val = map[i].value;
	  return TRUE;
	}
    }

  if (!what || had_prefix)
    return parse_int (str, len, val);

  return FALSE;
}

static void
parse_field (twin_face_properties_t *props,
	     const char *str,
	     int len)
{
  if (field_matches ("Normal", str, len))
    return;

#define FIELD(NAME) \
  if (find_field (STRINGIFY (NAME), NAME##_map, ARRAY_LENGTH (NAME##_map), str, len, \
		  (int *)(void *)&props->NAME)) \
      return; \

  FIELD (weight);
  FIELD (slant);
  FIELD (stretch);
  FIELD (smallcaps);
  FIELD (monospace);

#undef FIELD
}

static void
face_props_parse (twin_face_properties_t *props,
	     const char *s)
{
    const char *start, *end;

    for (start = end = s; *end; end++) {
	if (*end != ' ' && *end != ':')
	    continue;

	if (start < end)
		parse_field (props, start, end - start);
	start = end + 1;
    }
    if (start < end)
	    parse_field (props, start, end - start);
}

static twin_face_properties_t *
twin_font_face_create_properties (cairo_font_face_t *twin_face)
{
    twin_face_properties_t *props;

    props = _cairo_malloc (sizeof (twin_face_properties_t));
    if (unlikely (props == NULL))
	return NULL;

    props->stretch  = TWIN_STRETCH_NORMAL;
    props->slant = CAIRO_FONT_SLANT_NORMAL;
    props->weight = TWIN_WEIGHT_NORMAL;
    props->monospace = FALSE;
    props->smallcaps = FALSE;

    if (unlikely (cairo_font_face_set_user_data (twin_face,
					    &twin_properties_key,
					    props, free))) {
	free (props);
	return NULL;
    }

    return props;
}

static cairo_status_t
twin_font_face_set_properties_from_toy (cairo_font_face_t *twin_face,
					cairo_toy_font_face_t *toy_face)
{
    twin_face_properties_t *props;

    props = twin_font_face_create_properties (twin_face);
    if (unlikely (props == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);

    props->slant = toy_face->slant;
    props->weight = toy_face->weight == CAIRO_FONT_WEIGHT_NORMAL ?
		    TWIN_WEIGHT_NORMAL : TWIN_WEIGHT_BOLD;
    face_props_parse (props, toy_face->family);

    return CAIRO_STATUS_SUCCESS;
}


/*
 * Scaled properties
 */

typedef struct _twin_scaled_properties {
	twin_face_properties_t *face_props;

	cairo_bool_t snap; /* hint outlines */

	double weight; /* unhinted pen width */
	double penx, peny; /* hinted pen width */
	double marginl, marginr; /* hinted side margins */

	double stretch; /* stretch factor */
} twin_scaled_properties_t;

static void
compute_hinting_scale (cairo_t *cr,
		       double x, double y,
		       double *scale, double *inv)
{
    cairo_user_to_device_distance (cr, &x, &y);
    *scale = x == 0 ? y : y == 0 ? x :sqrt (x*x + y*y);
    *inv = 1 / *scale;
}

static void
compute_hinting_scales (cairo_t *cr,
			double *x_scale, double *x_scale_inv,
			double *y_scale, double *y_scale_inv)
{
    double x, y;

    x = 1; y = 0;
    compute_hinting_scale (cr, x, y, x_scale, x_scale_inv);

    x = 0; y = 1;
    compute_hinting_scale (cr, x, y, y_scale, y_scale_inv);
}

#define SNAPXI(p)	(_cairo_round ((p) * x_scale) * x_scale_inv)
#define SNAPYI(p)	(_cairo_round ((p) * y_scale) * y_scale_inv)

/* This controls the global font size */
#define F(g)		((g) / 72.)

static void
twin_hint_pen_and_margins(cairo_t *cr,
			  double *penx, double *peny,
			  double *marginl, double *marginr)
{
    double x_scale, x_scale_inv;
    double y_scale, y_scale_inv;
    double margin;

    compute_hinting_scales (cr,
			    &x_scale, &x_scale_inv,
			    &y_scale, &y_scale_inv);

    *penx = SNAPXI (*penx);
    if (*penx < x_scale_inv)
	*penx = x_scale_inv;

    *peny = SNAPYI (*peny);
    if (*peny < y_scale_inv)
	*peny = y_scale_inv;

    margin = *marginl + *marginr;
    *marginl = SNAPXI (*marginl);
    if (*marginl < x_scale_inv)
	*marginl = x_scale_inv;

    *marginr = margin - *marginl;
    if (*marginr < 0)
	*marginr = 0;
    *marginr = SNAPXI (*marginr);
}

static cairo_status_t
twin_scaled_font_compute_properties (cairo_scaled_font_t *scaled_font,
				     cairo_t           *cr)
{
    cairo_status_t status;
    twin_scaled_properties_t *props;

    props = _cairo_malloc (sizeof (twin_scaled_properties_t));
    if (unlikely (props == NULL))
	return _cairo_error (CAIRO_STATUS_NO_MEMORY);


    props->face_props = cairo_font_face_get_user_data (cairo_scaled_font_get_font_face (scaled_font),
						       &twin_properties_key);

    props->snap = scaled_font->options.hint_style > CAIRO_HINT_STYLE_NONE;

    /* weight */
    props->weight = props->face_props->weight * (F (4) / TWIN_WEIGHT_NORMAL);

    /* pen & margins */
    props->penx = props->peny = props->weight;
    props->marginl = props->marginr = F (4);
    if (scaled_font->options.hint_style > CAIRO_HINT_STYLE_SLIGHT)
	twin_hint_pen_and_margins(cr,
				  &props->penx, &props->peny,
				  &props->marginl, &props->marginr);

    /* stretch */
    props->stretch = 1 + .1 * ((int) props->face_props->stretch - (int) TWIN_STRETCH_NORMAL);


    /* Save it */
    status = cairo_scaled_font_set_user_data (scaled_font,
					      &twin_properties_key,
					      props, free);
    if (unlikely (status))
	goto FREE_PROPS;

    return CAIRO_STATUS_SUCCESS;

FREE_PROPS:
    free (props);
    return status;
}


/*
 * User-font implementation
 */

static cairo_status_t
twin_scaled_font_init (cairo_scaled_font_t  *scaled_font,
		       cairo_t              *cr,
		       cairo_font_extents_t *metrics)
{
  metrics->ascent  = F (54);
  metrics->descent = 1 - metrics->ascent;

  return twin_scaled_font_compute_properties (scaled_font, cr);
}

#define TWIN_GLYPH_MAX_SNAP_X 4
#define TWIN_GLYPH_MAX_SNAP_Y 7

typedef struct {
    int n_snap_x;
    int8_t snap_x[TWIN_GLYPH_MAX_SNAP_X];
    double snapped_x[TWIN_GLYPH_MAX_SNAP_X];
    int n_snap_y;
    int8_t snap_y[TWIN_GLYPH_MAX_SNAP_Y];
    double snapped_y[TWIN_GLYPH_MAX_SNAP_Y];
} twin_snap_info_t;

#define twin_glyph_left(g)      ((g)[0])
#define twin_glyph_right(g)     ((g)[1])
#define twin_glyph_ascent(g)    ((g)[2])
#define twin_glyph_descent(g)   ((g)[3])

#define twin_glyph_n_snap_x(g)  ((g)[4])
#define twin_glyph_n_snap_y(g)  ((g)[5])
#define twin_glyph_snap_x(g)    (&g[6])
#define twin_glyph_snap_y(g)    (twin_glyph_snap_x(g) + twin_glyph_n_snap_x(g))
#define twin_glyph_draw(g)      (twin_glyph_snap_y(g) + twin_glyph_n_snap_y(g))

static void
twin_compute_snap (cairo_t             *cr,
		   twin_snap_info_t    *info,
		   const signed char   *b)
{
    int			s, n;
    const signed char	*snap;
    double x_scale, x_scale_inv;
    double y_scale, y_scale_inv;

    compute_hinting_scales (cr,
			    &x_scale, &x_scale_inv,
			    &y_scale, &y_scale_inv);

    snap = twin_glyph_snap_x (b);
    n = twin_glyph_n_snap_x (b);
    info->n_snap_x = n;
    assert (n <= TWIN_GLYPH_MAX_SNAP_X);
    for (s = 0; s < n; s++) {
	info->snap_x[s] = snap[s];
	info->snapped_x[s] = SNAPXI (F (snap[s]));
    }

    snap = twin_glyph_snap_y (b);
    n = twin_glyph_n_snap_y (b);
    info->n_snap_y = n;
    assert (n <= TWIN_GLYPH_MAX_SNAP_Y);
    for (s = 0; s < n; s++) {
	info->snap_y[s] = snap[s];
	info->snapped_y[s] = SNAPYI (F (snap[s]));
    }
}

static double
twin_snap (int8_t v, int n, int8_t *snap, double *snapped)
{
    int	s;

    if (!n)
	return F(v);

    if (snap[0] == v)
	return snapped[0];

    for (s = 0; s < n - 1; s++)
    {
	if (snap[s+1] == v)
	    return snapped[s+1];

	if (snap[s] <= v && v <= snap[s+1])
	{
	    int before = snap[s];
	    int after = snap[s+1];
	    int dist = after - before;
	    double snap_before = snapped[s];
	    double snap_after = snapped[s+1];
	    double dist_before = v - before;
	    return snap_before + (snap_after - snap_before) * dist_before / dist;
	}
    }
    return F(v);
}

#define SNAPX(p)	twin_snap (p, info.n_snap_x, info.snap_x, info.snapped_x)
#define SNAPY(p)	twin_snap (p, info.n_snap_y, info.snap_y, info.snapped_y)

static cairo_status_t
twin_scaled_font_render_glyph (cairo_scaled_font_t  *scaled_font,
			       unsigned long         glyph,
			       cairo_t              *cr,
			       cairo_text_extents_t *metrics)
{
    double x1, y1, x2, y2, x3, y3;
    double marginl;
    twin_scaled_properties_t *props;
    twin_snap_info_t info;
    const int8_t *b;
    const int8_t *g;
    int8_t w;
    double gw;

    props = cairo_scaled_font_get_user_data (scaled_font, &twin_properties_key);

    /* Save glyph space, we need it when stroking */
    cairo_save (cr);

    /* center the pen */
    cairo_translate (cr, props->penx * .5, -props->peny * .5);

    /* small-caps */
    if (props->face_props->smallcaps && glyph >= 'a' && glyph <= 'z') {
	glyph += 'A' - 'a';
	/* 28 and 42 are small and capital letter heights of the glyph data */
	cairo_scale (cr, 1, 28. / 42);
    }

    /* slant */
    if (props->face_props->slant != CAIRO_FONT_SLANT_NORMAL) {
	cairo_matrix_t shear = { 1, 0, -.2, 1, 0, 0};
	cairo_transform (cr, &shear);
    }

    b = _cairo_twin_outlines +
	_cairo_twin_charmap[unlikely (glyph >= ARRAY_LENGTH (_cairo_twin_charmap)) ? 0 : glyph];
    g = twin_glyph_draw(b);
    w = twin_glyph_right(b);
    gw = F(w);

    marginl = props->marginl;

    /* monospace */
    if (props->face_props->monospace) {
	double monow = F(24);
	double extra =  props->penx + props->marginl + props->marginr;
	cairo_scale (cr, (monow + extra) / (gw + extra), 1);
	gw = monow;

	/* resnap margin for new transform */
	{
	    double x, y, x_scale, x_scale_inv;
	    x = 1; y = 0;
	    compute_hinting_scale (cr, x, y, &x_scale, &x_scale_inv);
	    marginl = SNAPXI (marginl);
	}
    }

    cairo_translate (cr, marginl, 0);

    /* stretch */
    cairo_scale (cr, props->stretch, 1);

    if (props->snap)
	twin_compute_snap (cr, &info, b);
    else
	info.n_snap_x = info.n_snap_y = 0;

    /* advance width */
    metrics->x_advance = gw * props->stretch + props->penx + props->marginl + props->marginr;

    /* glyph shape */
    for (;;) {
	switch (*g++) {
	case 'M':
	    cairo_close_path (cr);
	    /* fall through */
	case 'm':
	    x1 = SNAPX(*g++);
	    y1 = SNAPY(*g++);
	    cairo_move_to (cr, x1, y1);
	    continue;
	case 'L':
	    cairo_close_path (cr);
	    /* fall through */
	case 'l':
	    x1 = SNAPX(*g++);
	    y1 = SNAPY(*g++);
	    cairo_line_to (cr, x1, y1);
	    continue;
	case 'C':
	    cairo_close_path (cr);
	    /* fall through */
	case 'c':
	    x1 = SNAPX(*g++);
	    y1 = SNAPY(*g++);
	    x2 = SNAPX(*g++);
	    y2 = SNAPY(*g++);
	    x3 = SNAPX(*g++);
	    y3 = SNAPY(*g++);
	    cairo_curve_to (cr, x1, y1, x2, y2, x3, y3);
	    continue;
	case 'E':
	    cairo_close_path (cr);
	    /* fall through */
	case 'e':
	    cairo_restore (cr); /* restore glyph space */
	    cairo_set_tolerance (cr, 0.01);
	    cairo_set_line_join (cr, CAIRO_LINE_JOIN_ROUND);
	    cairo_set_line_cap (cr, CAIRO_LINE_CAP_ROUND);
	    cairo_set_line_width (cr, 1);
	    cairo_scale (cr, props->penx, props->peny);
	    cairo_stroke (cr);
	    break;
	case 'X':
	    /* filler */
	    continue;
	}
	break;
    }

    return CAIRO_STATUS_SUCCESS;
}

static cairo_status_t
twin_scaled_font_unicode_to_glyph (cairo_scaled_font_t *scaled_font,
				   unsigned long        unicode,
				   unsigned long       *glyph)
{
    /* We use an identity charmap.  Which means we could live
     * with no unicode_to_glyph method too.  But we define this
     * to map all unknown chars to a single unknown glyph to
     * reduce pressure on cache. */

    if (likely (unicode < ARRAY_LENGTH (_cairo_twin_charmap)))
	*glyph = unicode;
    else
	*glyph = 0;

    return CAIRO_STATUS_SUCCESS;
}


/*
 * Face constructor
 */

static cairo_font_face_t *
_cairo_font_face_twin_create_internal (void)
{
    cairo_font_face_t *twin_font_face;

    twin_font_face = cairo_user_font_face_create ();
    cairo_user_font_face_set_init_func             (twin_font_face, twin_scaled_font_init);
    cairo_user_font_face_set_render_glyph_func     (twin_font_face, twin_scaled_font_render_glyph);
    cairo_user_font_face_set_unicode_to_glyph_func (twin_font_face, twin_scaled_font_unicode_to_glyph);

    return twin_font_face;
}

cairo_font_face_t *
_cairo_font_face_twin_create_fallback (void)
{
    cairo_font_face_t *twin_font_face;

    twin_font_face = _cairo_font_face_twin_create_internal ();
    if (! twin_font_face_create_properties (twin_font_face)) {
	cairo_font_face_destroy (twin_font_face);
	return (cairo_font_face_t *) &_cairo_font_face_nil;
    }

    return twin_font_face;
}

cairo_status_t
_cairo_font_face_twin_create_for_toy (cairo_toy_font_face_t   *toy_face,
				      cairo_font_face_t      **font_face)
{
    cairo_status_t status;
    cairo_font_face_t *twin_font_face;

    twin_font_face = _cairo_font_face_twin_create_internal ();
    status = twin_font_face_set_properties_from_toy (twin_font_face, toy_face);
    if (status) {
	cairo_font_face_destroy (twin_font_face);
	return status;
    }

    *font_face = twin_font_face;

    return CAIRO_STATUS_SUCCESS;
}
