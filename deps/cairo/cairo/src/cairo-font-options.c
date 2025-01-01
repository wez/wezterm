/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2005 Red Hat Inc.
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
 *      Owen Taylor <otaylor@redhat.com>
 */

#include "cairoint.h"
#include "cairo-error-private.h"

/**
 * SECTION:cairo-font-options
 * @Title: cairo_font_options_t
 * @Short_Description: How a font should be rendered
 * @See_Also: #cairo_scaled_font_t
 *
 * The font options specify how fonts should be rendered.  Most of the 
 * time the font options implied by a surface are just right and do not 
 * need any changes, but for pixel-based targets tweaking font options 
 * may result in superior output on a particular display.
 **/

static const cairo_font_options_t _cairo_font_options_nil = {
    CAIRO_ANTIALIAS_DEFAULT,
    CAIRO_SUBPIXEL_ORDER_DEFAULT,
    CAIRO_LCD_FILTER_DEFAULT,
    CAIRO_HINT_STYLE_DEFAULT,
    CAIRO_HINT_METRICS_DEFAULT,
    CAIRO_ROUND_GLYPH_POS_DEFAULT,
    NULL, /* variations */
    CAIRO_COLOR_MODE_DEFAULT,
    CAIRO_COLOR_PALETTE_DEFAULT,
    NULL, 0, /* custom palette */
};

/**
 * _cairo_font_options_init_default:
 * @options: a #cairo_font_options_t
 *
 * Initializes all fields of the font options object to default values.
 **/
void
_cairo_font_options_init_default (cairo_font_options_t *options)
{
    options->antialias = CAIRO_ANTIALIAS_DEFAULT;
    options->subpixel_order = CAIRO_SUBPIXEL_ORDER_DEFAULT;
    options->lcd_filter = CAIRO_LCD_FILTER_DEFAULT;
    options->hint_style = CAIRO_HINT_STYLE_DEFAULT;
    options->hint_metrics = CAIRO_HINT_METRICS_DEFAULT;
    options->round_glyph_positions = CAIRO_ROUND_GLYPH_POS_DEFAULT;
    options->variations = NULL;
    options->color_mode = CAIRO_COLOR_MODE_DEFAULT;
    options->palette_index = CAIRO_COLOR_PALETTE_DEFAULT;
    options->custom_palette = NULL;
    options->custom_palette_size = 0;
}

void
_cairo_font_options_init_copy (cairo_font_options_t		*options,
			       const cairo_font_options_t	*other)
{
    options->antialias = other->antialias;
    options->subpixel_order = other->subpixel_order;
    options->lcd_filter = other->lcd_filter;
    options->hint_style = other->hint_style;
    options->hint_metrics = other->hint_metrics;
    options->round_glyph_positions = other->round_glyph_positions;
    options->variations = other->variations ? strdup (other->variations) : NULL;
    options->color_mode = other->color_mode;
    options->palette_index = other->palette_index;
    options->custom_palette_size = other->custom_palette_size;
    options->custom_palette = NULL;
    if (other->custom_palette) {
        options->custom_palette = (cairo_palette_color_t *) malloc (sizeof (cairo_palette_color_t) * options->custom_palette_size);
        memcpy (options->custom_palette, other->custom_palette, sizeof (cairo_palette_color_t) * options->custom_palette_size);
    }
}

/**
 * cairo_font_options_create:
 *
 * Allocates a new font options object with all options initialized
 *  to default values.
 *
 * Return value: a newly allocated #cairo_font_options_t. Free with
 *   cairo_font_options_destroy(). This function always returns a
 *   valid pointer; if memory cannot be allocated, then a special
 *   error object is returned where all operations on the object do nothing.
 *   You can check for this with cairo_font_options_status().
 *
 * Since: 1.0
 **/
cairo_font_options_t *
cairo_font_options_create (void)
{
    cairo_font_options_t *options;

    options = _cairo_malloc (sizeof (cairo_font_options_t));
    if (!options) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_font_options_t *) &_cairo_font_options_nil;
    }

    _cairo_font_options_init_default (options);

    return options;
}

/**
 * cairo_font_options_copy:
 * @original: a #cairo_font_options_t
 *
 * Allocates a new font options object copying the option values from
 *  @original.
 *
 * Return value: a newly allocated #cairo_font_options_t. Free with
 *   cairo_font_options_destroy(). This function always returns a
 *   valid pointer; if memory cannot be allocated, then a special
 *   error object is returned where all operations on the object do nothing.
 *   You can check for this with cairo_font_options_status().
 *
 * Since: 1.0
 **/
cairo_font_options_t *
cairo_font_options_copy (const cairo_font_options_t *original)
{
    cairo_font_options_t *options;

    if (cairo_font_options_status ((cairo_font_options_t *) original))
	return (cairo_font_options_t *) &_cairo_font_options_nil;

    options = _cairo_malloc (sizeof (cairo_font_options_t));
    if (!options) {
	_cairo_error_throw (CAIRO_STATUS_NO_MEMORY);
	return (cairo_font_options_t *) &_cairo_font_options_nil;
    }

    _cairo_font_options_init_copy (options, original);

    return options;
}

void
_cairo_font_options_fini (cairo_font_options_t *options)
{
    free (options->variations);
    free (options->custom_palette);
}

/**
 * cairo_font_options_destroy:
 * @options: a #cairo_font_options_t
 *
 * Destroys a #cairo_font_options_t object created with
 * cairo_font_options_create() or cairo_font_options_copy().
 *
 * Since: 1.0
 **/
void
cairo_font_options_destroy (cairo_font_options_t *options)
{
    if (cairo_font_options_status (options))
	return;

    _cairo_font_options_fini (options);
    free (options);
}

/**
 * cairo_font_options_status:
 * @options: a #cairo_font_options_t
 *
 * Checks whether an error has previously occurred for this
 * font options object
 *
 * Return value: %CAIRO_STATUS_SUCCESS, %CAIRO_STATUS_NO_MEMORY, or
 *	%CAIRO_STATUS_NULL_POINTER.
 *
 * Since: 1.0
 **/
cairo_status_t
cairo_font_options_status (cairo_font_options_t *options)
{
    if (options == NULL)
	return CAIRO_STATUS_NULL_POINTER;
    else if (options == (cairo_font_options_t *) &_cairo_font_options_nil)
	return CAIRO_STATUS_NO_MEMORY;
    else
	return CAIRO_STATUS_SUCCESS;
}
slim_hidden_def (cairo_font_options_status);

/**
 * cairo_font_options_merge:
 * @options: a #cairo_font_options_t
 * @other: another #cairo_font_options_t
 *
 * Merges non-default options from @other into @options, replacing
 * existing values. This operation can be thought of as somewhat
 * similar to compositing @other onto @options with the operation
 * of %CAIRO_OPERATOR_OVER.
 *
 * Since: 1.0
 **/
void
cairo_font_options_merge (cairo_font_options_t       *options,
			  const cairo_font_options_t *other)
{
    if (cairo_font_options_status (options))
	return;

    if (cairo_font_options_status ((cairo_font_options_t *) other))
	return;

    if (other->antialias != CAIRO_ANTIALIAS_DEFAULT)
	options->antialias = other->antialias;
    if (other->subpixel_order != CAIRO_SUBPIXEL_ORDER_DEFAULT)
	options->subpixel_order = other->subpixel_order;
    if (other->lcd_filter != CAIRO_LCD_FILTER_DEFAULT)
	options->lcd_filter = other->lcd_filter;
    if (other->hint_style != CAIRO_HINT_STYLE_DEFAULT)
	options->hint_style = other->hint_style;
    if (other->hint_metrics != CAIRO_HINT_METRICS_DEFAULT)
	options->hint_metrics = other->hint_metrics;
    if (other->round_glyph_positions != CAIRO_ROUND_GLYPH_POS_DEFAULT)
	options->round_glyph_positions = other->round_glyph_positions;

    if (other->variations) {
      if (options->variations) {
        char *p;

        /* 'merge' variations by concatenating - later entries win */
        p = malloc (strlen (other->variations) + strlen (options->variations) + 2);
        p[0] = 0;
        strcat (p, options->variations);
        strcat (p, ",");
        strcat (p, other->variations);
        free (options->variations);
        options->variations = p;
      }
      else {
        options->variations = strdup (other->variations);
      }
    }

    if (other->color_mode != CAIRO_COLOR_MODE_DEFAULT)
	options->color_mode = other->color_mode;
    if (other->palette_index != CAIRO_COLOR_PALETTE_DEFAULT)
	options->palette_index = other->palette_index;
    if (other->custom_palette) {
        options->custom_palette_size = other->custom_palette_size;
        free (options->custom_palette);
        options->custom_palette = (cairo_palette_color_t *) malloc (sizeof (cairo_palette_color_t) * options->custom_palette_size);
        memcpy (options->custom_palette, other->custom_palette, sizeof (cairo_palette_color_t) * options->custom_palette_size);
    }
}
slim_hidden_def (cairo_font_options_merge);

/**
 * cairo_font_options_equal:
 * @options: a #cairo_font_options_t
 * @other: another #cairo_font_options_t
 *
 * Compares two font options objects for equality.
 *
 * Return value: %TRUE if all fields of the two font options objects match.
 *	Note that this function will return %FALSE if either object is in
 *	error.
 *
 * Since: 1.0
 **/
cairo_bool_t
cairo_font_options_equal (const cairo_font_options_t *options,
			  const cairo_font_options_t *other)
{
    if (cairo_font_options_status ((cairo_font_options_t *) options))
	return FALSE;
    if (cairo_font_options_status ((cairo_font_options_t *) other))
	return FALSE;

    if (options == other)
	return TRUE;

    return (options->antialias == other->antialias &&
	    options->subpixel_order == other->subpixel_order &&
	    options->lcd_filter == other->lcd_filter &&
	    options->hint_style == other->hint_style &&
	    options->hint_metrics == other->hint_metrics &&
	    options->round_glyph_positions == other->round_glyph_positions &&
            ((options->variations == NULL && other->variations == NULL) ||
             (options->variations != NULL && other->variations != NULL &&
              strcmp (options->variations, other->variations) == 0)) &&
	    options->color_mode == other->color_mode &&
	    options->palette_index == other->palette_index &&
            ((options->custom_palette == NULL && other->custom_palette == NULL) ||
             (options->custom_palette != NULL && other->custom_palette != NULL &&
              options->custom_palette_size == other->custom_palette_size &&
              memcmp (options->custom_palette, other->custom_palette,
                      sizeof (cairo_palette_color_t) * options->custom_palette_size) == 0)));
}
slim_hidden_def (cairo_font_options_equal);

/**
 * cairo_font_options_hash:
 * @options: a #cairo_font_options_t
 *
 * Compute a hash for the font options object; this value will
 * be useful when storing an object containing a #cairo_font_options_t
 * in a hash table.
 *
 * Return value: the hash value for the font options object.
 *   The return value can be cast to a 32-bit type if a
 *   32-bit hash value is needed.
 *
 * Since: 1.0
 **/
unsigned long
cairo_font_options_hash (const cairo_font_options_t *options)
{
    unsigned long hash = 0;

    if (cairo_font_options_status ((cairo_font_options_t *) options))
	options = &_cairo_font_options_nil; /* force default values */

    if (options->variations)
      hash = _cairo_string_hash (options->variations, strlen (options->variations));

    hash ^= options->palette_index;

    return ((options->antialias) |
	    (options->subpixel_order << 4) |
	    (options->lcd_filter << 8) |
	    (options->hint_style << 12) |
	    (options->hint_metrics << 16) |
            (options->color_mode << 20)) ^ hash;
}
slim_hidden_def (cairo_font_options_hash);

/**
 * cairo_font_options_set_antialias:
 * @options: a #cairo_font_options_t
 * @antialias: the new antialiasing mode
 *
 * Sets the antialiasing mode for the font options object. This
 * specifies the type of antialiasing to do when rendering text.
 *
 * Since: 1.0
 **/
void
cairo_font_options_set_antialias (cairo_font_options_t *options,
				  cairo_antialias_t     antialias)
{
    if (cairo_font_options_status (options))
	return;

    options->antialias = antialias;
}
slim_hidden_def (cairo_font_options_set_antialias);

/**
 * cairo_font_options_get_antialias:
 * @options: a #cairo_font_options_t
 *
 * Gets the antialiasing mode for the font options object.
 *
 * Return value: the antialiasing mode
 *
 * Since: 1.0
 **/
cairo_antialias_t
cairo_font_options_get_antialias (const cairo_font_options_t *options)
{
    if (cairo_font_options_status ((cairo_font_options_t *) options))
	return CAIRO_ANTIALIAS_DEFAULT;

    return options->antialias;
}

/**
 * cairo_font_options_set_subpixel_order:
 * @options: a #cairo_font_options_t
 * @subpixel_order: the new subpixel order
 *
 * Sets the subpixel order for the font options object. The subpixel
 * order specifies the order of color elements within each pixel on
 * the display device when rendering with an antialiasing mode of
 * %CAIRO_ANTIALIAS_SUBPIXEL. See the documentation for
 * #cairo_subpixel_order_t for full details.
 *
 * Since: 1.0
 **/
void
cairo_font_options_set_subpixel_order (cairo_font_options_t   *options,
				       cairo_subpixel_order_t  subpixel_order)
{
    if (cairo_font_options_status (options))
	return;

    options->subpixel_order = subpixel_order;
}
slim_hidden_def (cairo_font_options_set_subpixel_order);

/**
 * cairo_font_options_get_subpixel_order:
 * @options: a #cairo_font_options_t
 *
 * Gets the subpixel order for the font options object.
 * See the documentation for #cairo_subpixel_order_t for full details.
 *
 * Return value: the subpixel order for the font options object
 *
 * Since: 1.0
 **/
cairo_subpixel_order_t
cairo_font_options_get_subpixel_order (const cairo_font_options_t *options)
{
    if (cairo_font_options_status ((cairo_font_options_t *) options))
	return CAIRO_SUBPIXEL_ORDER_DEFAULT;

    return options->subpixel_order;
}

/**
 * _cairo_font_options_set_lcd_filter:
 * @options: a #cairo_font_options_t
 * @lcd_filter: the new LCD filter
 *
 * Sets the LCD filter for the font options object. The LCD filter
 * specifies how pixels are filtered when rendered with an antialiasing
 * mode of %CAIRO_ANTIALIAS_SUBPIXEL. See the documentation for
 * #cairo_lcd_filter_t for full details.
 **/
void
_cairo_font_options_set_lcd_filter (cairo_font_options_t *options,
				    cairo_lcd_filter_t    lcd_filter)
{
    if (cairo_font_options_status (options))
	return;

    options->lcd_filter = lcd_filter;
}

/**
 * _cairo_font_options_get_lcd_filter:
 * @options: a #cairo_font_options_t
 *
 * Gets the LCD filter for the font options object.
 * See the documentation for #cairo_lcd_filter_t for full details.
 *
 * Return value: the LCD filter for the font options object
 **/
cairo_lcd_filter_t
_cairo_font_options_get_lcd_filter (const cairo_font_options_t *options)
{
    if (cairo_font_options_status ((cairo_font_options_t *) options))
	return CAIRO_LCD_FILTER_DEFAULT;

    return options->lcd_filter;
}

/**
 * _cairo_font_options_set_round_glyph_positions:
 * @options: a #cairo_font_options_t
 * @round: the new rounding value
 *
 * Sets the rounding options for the font options object. If rounding is set, a
 * glyph's position will be rounded to integer values.
 **/
void
_cairo_font_options_set_round_glyph_positions (cairo_font_options_t *options,
					       cairo_round_glyph_positions_t  round)
{
    if (cairo_font_options_status (options))
	return;

    options->round_glyph_positions = round;
}

/**
 * _cairo_font_options_get_round_glyph_positions:
 * @options: a #cairo_font_options_t
 *
 * Gets the glyph position rounding option for the font options object.
 *
 * Return value: The round glyph posistions flag for the font options object.
 **/
cairo_round_glyph_positions_t
_cairo_font_options_get_round_glyph_positions (const cairo_font_options_t *options)
{
    if (cairo_font_options_status ((cairo_font_options_t *) options))
	return CAIRO_ROUND_GLYPH_POS_DEFAULT;

    return options->round_glyph_positions;
}

/**
 * cairo_font_options_set_hint_style:
 * @options: a #cairo_font_options_t
 * @hint_style: the new hint style
 *
 * Sets the hint style for font outlines for the font options object.
 * This controls whether to fit font outlines to the pixel grid,
 * and if so, whether to optimize for fidelity or contrast.
 * See the documentation for #cairo_hint_style_t for full details.
 *
 * Since: 1.0
 **/
void
cairo_font_options_set_hint_style (cairo_font_options_t *options,
				   cairo_hint_style_t    hint_style)
{
    if (cairo_font_options_status (options))
	return;

    options->hint_style = hint_style;
}
slim_hidden_def (cairo_font_options_set_hint_style);

/**
 * cairo_font_options_get_hint_style:
 * @options: a #cairo_font_options_t
 *
 * Gets the hint style for font outlines for the font options object.
 * See the documentation for #cairo_hint_style_t for full details.
 *
 * Return value: the hint style for the font options object
 *
 * Since: 1.0
 **/
cairo_hint_style_t
cairo_font_options_get_hint_style (const cairo_font_options_t *options)
{
    if (cairo_font_options_status ((cairo_font_options_t *) options))
	return CAIRO_HINT_STYLE_DEFAULT;

    return options->hint_style;
}

/**
 * cairo_font_options_set_hint_metrics:
 * @options: a #cairo_font_options_t
 * @hint_metrics: the new metrics hinting mode
 *
 * Sets the metrics hinting mode for the font options object. This
 * controls whether metrics are quantized to integer values in
 * device units.
 * See the documentation for #cairo_hint_metrics_t for full details.
 *
 * Since: 1.0
 **/
void
cairo_font_options_set_hint_metrics (cairo_font_options_t *options,
				     cairo_hint_metrics_t  hint_metrics)
{
    if (cairo_font_options_status (options))
	return;

    options->hint_metrics = hint_metrics;
}
slim_hidden_def (cairo_font_options_set_hint_metrics);

/**
 * cairo_font_options_get_hint_metrics:
 * @options: a #cairo_font_options_t
 *
 * Gets the metrics hinting mode for the font options object.
 * See the documentation for #cairo_hint_metrics_t for full details.
 *
 * Return value: the metrics hinting mode for the font options object
 *
 * Since: 1.0
 **/
cairo_hint_metrics_t
cairo_font_options_get_hint_metrics (const cairo_font_options_t *options)
{
    if (cairo_font_options_status ((cairo_font_options_t *) options))
	return CAIRO_HINT_METRICS_DEFAULT;

    return options->hint_metrics;
}

/**
 * cairo_font_options_set_variations:
 * @options: a #cairo_font_options_t
 * @variations: the new font variations, or %NULL
 *
 * Sets the OpenType font variations for the font options object.
 * Font variations are specified as a string with a format that
 * is similar to the CSS font-variation-settings. The string contains
 * a comma-separated list of axis assignments, which each assignment
 * consists of a 4-character axis name and a value, separated by
 * whitespace and optional equals sign.
 *
 * Examples:
 *
 * wght=200,wdth=140.5
 *
 * wght 200 , wdth 140.5
 *
 * Since: 1.16
 **/
void
cairo_font_options_set_variations (cairo_font_options_t *options,
                                   const char           *variations)
{
  char *tmp = variations ? strdup (variations) : NULL;
  free (options->variations);
  options->variations = tmp;
}

/**
 * cairo_font_options_get_variations:
 * @options: a #cairo_font_options_t
 *
 * Gets the OpenType font variations for the font options object.
 * See cairo_font_options_set_variations() for details about the
 * string format.
 *
 * Return value: the font variations for the font options object. The
 *   returned string belongs to the @options and must not be modified.
 *   It is valid until either the font options object is destroyed or
 *   the font variations in this object is modified with
 *   cairo_font_options_set_variations().
 *
 * Since: 1.16
 **/
const char *
cairo_font_options_get_variations (cairo_font_options_t *options)
{
  return options->variations;
}

/**
 * cairo_font_options_set_color_mode:
 * @options: a #cairo_font_options_t
 * @font_color: the new color mode
 *
 * Sets the color mode for the font options object. This controls
 * whether color fonts are to be rendered in color or as outlines.
 * See the documentation for #cairo_color_mode_t for full details.
 *
 * Since: 1.18
 **/
cairo_public void
cairo_font_options_set_color_mode (cairo_font_options_t *options,
                                   cairo_color_mode_t    color_mode)
{
    if (cairo_font_options_status (options))
	return;

    options->color_mode = color_mode;
}

/**
 * cairo_font_options_get_color_mode:
 * @options: a #cairo_font_options_t
 *
 * Gets the color mode for the font options object.
 * See the documentation for #cairo_color_mode_t for full details.
 *
 * Return value: the color mode for the font options object
 *
 * Since: 1.18
 **/
cairo_public cairo_color_mode_t
cairo_font_options_get_color_mode (const cairo_font_options_t *options)
{
    if (cairo_font_options_status ((cairo_font_options_t *) options))
	return CAIRO_COLOR_MODE_DEFAULT;

    return options->color_mode;
}

/**
 * CAIRO_COLOR_PALETTE_DEFAULT:
 *
 * The default color palette index.
 *
 * Since: 1.18
 **/

/**
 * cairo_font_options_set_color_palette:
 * @options: a #cairo_font_options_t
 * @palette_index: the palette index in the CPAL table
 *
 * Sets the OpenType font color palette for the font options
 * object. OpenType color fonts with a CPAL table may contain multiple
 * palettes. The default color palette index is %CAIRO_COLOR_PALETTE_DEFAULT.
 *
 * If @palette_index is invalid, the default palette is used.
 *
 * Individual colors within the palette may be overriden with
 * cairo_font_options_set_custom_palette_color().
 *
 * Since: 1.18
 **/
void
cairo_font_options_set_color_palette (cairo_font_options_t *options,
                                      unsigned int          palette_index)
{
    if (cairo_font_options_status (options))
	return;

    options->palette_index = palette_index;
}

/**
 * cairo_font_options_get_color_palette:
 * @options: a #cairo_font_options_t
 *
 * Gets the current OpenType color font palette for the font options object.
 *
 * Return value: the palette index
 *
 * Since: 1.18
 **/
unsigned int
cairo_font_options_get_color_palette (const cairo_font_options_t *options)
{
    if (cairo_font_options_status ((cairo_font_options_t *) options))
	return CAIRO_COLOR_PALETTE_DEFAULT;

    return options->palette_index;
}

/**
 * cairo_font_options_set_custom_palette_color:
 * @options: a #cairo_font_options_t
 * @index: the index of the color to set
 * @red: red component of color
 * @green: green component of color
 * @blue: blue component of color
 * @alpha: alpha component of color
 *
 * Sets a custom palette color for the font options object. This
 * overrides the palette color at the specified color index. This override is
 * independent of the selected palette index and will remain in place
 * even if cairo_font_options_set_color_palette() is called to change
 * the palette index.
 *
 * It is only possible to override color indexes already in the font
 * palette.
 *
 * Since: 1.18
 */
void
cairo_font_options_set_custom_palette_color (cairo_font_options_t *options,
                                             unsigned int index,
                                             double red, double green,
                                             double blue, double alpha)
{
    unsigned int idx;

    for (idx = 0; idx < options->custom_palette_size; idx++) {
        if (options->custom_palette[idx].index == index) {
            break;
        }
    }

    if (idx == options->custom_palette_size) {
        options->custom_palette_size++;
        options->custom_palette = (cairo_palette_color_t *)
            _cairo_realloc_ab (options->custom_palette,
                               sizeof (cairo_palette_color_t),
                               options->custom_palette_size);
    }

    /* beware of holes */
    memset (&options->custom_palette[idx], 0, sizeof (cairo_palette_color_t));

    options->custom_palette[idx].index = index;
    options->custom_palette[idx].red = red;
    options->custom_palette[idx].green = green;
    options->custom_palette[idx].blue = blue;
    options->custom_palette[idx].alpha = alpha;
}

/**
 * cairo_font_options_get_custom_palette_color:
 * @options: a #cairo_font_options_t
 * @index: the index of the color to get
 * @red: return location for red component of color
 * @green: return location for green component of color
 * @blue: return location for blue component of color
 * @alpha: return location for alpha component of color
 *
 * Gets the custom palette color for the color index for the font options object.
 *
 * Returns: `CAIRO_STATUS_SUCCESS` if a custom palette color is
 * returned, `CAIRO_STATUS_INVALID_INDEX` if no custom color exists
 * for the color index.
 *
 * Since: 1.18
 */
cairo_status_t
cairo_font_options_get_custom_palette_color (cairo_font_options_t *options,
                                             unsigned int index,
                                             double *red, double *green,
                                             double *blue, double *alpha)
{
    unsigned int idx;

    for (idx = 0; idx < options->custom_palette_size; idx++) {
        if (options->custom_palette[idx].index == index) {
            *red = options->custom_palette[idx].red;
            *green = options->custom_palette[idx].green;
            *blue = options->custom_palette[idx].blue;
            *alpha = options->custom_palette[idx].alpha;
            return CAIRO_STATUS_SUCCESS;
        }
    }

    return CAIRO_STATUS_INVALID_INDEX;
}
slim_hidden_def (cairo_font_options_get_custom_palette_color);
