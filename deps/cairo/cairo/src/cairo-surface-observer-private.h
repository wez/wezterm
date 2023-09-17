/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2011 Intel Corporation
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
 * The Initial Developer of the Original Code is Intel Corporation.
 *
 * Contributor(s):
 *      Chris Wilson <chris@chris-wilson.co.uk>
 */

#ifndef CAIRO_SURFACE_OBSERVER_PRIVATE_H
#define CAIRO_SURFACE_OBSERVER_PRIVATE_H

#include "cairoint.h"

#include "cairo-device-private.h"
#include "cairo-list-private.h"
#include "cairo-recording-surface-private.h"
#include "cairo-surface-private.h"
#include "cairo-surface-backend-private.h"
#include "cairo-time-private.h"

struct stat {
    double min, max, sum, sum_sq;
    unsigned count;
};

#define NUM_OPERATORS (CAIRO_OPERATOR_HSL_LUMINOSITY+1)
#define NUM_CAPS (CAIRO_LINE_CAP_SQUARE+1)
#define NUM_JOINS (CAIRO_LINE_JOIN_BEVEL+1)
#define NUM_ANTIALIAS (CAIRO_ANTIALIAS_BEST+1)
#define NUM_FILL_RULE (CAIRO_FILL_RULE_EVEN_ODD+1)

struct extents {
    struct stat area;
    unsigned int bounded, unbounded;
};

struct pattern {
    unsigned int type[8]; /* native/record/other surface/gradients */
};

struct path {
    unsigned int type[5]; /* empty/pixel/rectilinear/straight/curved */
};

struct clip {
    unsigned int type[6]; /* none, region, boxes, single path, polygon, general */
};

typedef struct _cairo_observation cairo_observation_t;
typedef struct _cairo_observation_record cairo_observation_record_t;
typedef struct _cairo_device_observer cairo_device_observer_t;

struct _cairo_observation_record {
    cairo_content_t target_content;
    int target_width;
    int target_height;

    int index;
    cairo_operator_t op;
    int source;
    int mask;
    int num_glyphs;
    int path;
    int fill_rule;
    double tolerance;
    int antialias;
    int clip;
    cairo_time_t elapsed;
};

struct _cairo_observation {
    int num_surfaces;
    int num_contexts;
    int num_sources_acquired;

    /* XXX put interesting stats here! */

    struct paint {
	cairo_time_t elapsed;
	unsigned int count;
	struct extents extents;
	unsigned int operators[NUM_OPERATORS];
	struct pattern source;
	struct clip clip;
	unsigned int noop;

	cairo_observation_record_t slowest;
    } paint;

    struct mask {
	cairo_time_t elapsed;
	unsigned int count;
	struct extents extents;
	unsigned int operators[NUM_OPERATORS];
	struct pattern source;
	struct pattern mask;
	struct clip clip;
	unsigned int noop;

	cairo_observation_record_t slowest;
    } mask;

    struct fill {
	cairo_time_t elapsed;
	unsigned int count;
	struct extents extents;
	unsigned int operators[NUM_OPERATORS];
	struct pattern source;
	struct path path;
	unsigned int antialias[NUM_ANTIALIAS];
	unsigned int fill_rule[NUM_FILL_RULE];
	struct clip clip;
	unsigned int noop;

	cairo_observation_record_t slowest;
    } fill;

    struct stroke {
	cairo_time_t elapsed;
	unsigned int count;
	struct extents extents;
	unsigned int operators[NUM_OPERATORS];
	unsigned int caps[NUM_CAPS];
	unsigned int joins[NUM_CAPS];
	unsigned int antialias[NUM_ANTIALIAS];
	struct pattern source;
	struct path path;
	struct stat line_width;
	struct clip clip;
	unsigned int noop;

	cairo_observation_record_t slowest;
    } stroke;

    struct glyphs {
	cairo_time_t elapsed;
	unsigned int count;
	struct extents extents;
	unsigned int operators[NUM_OPERATORS];
	struct pattern source;
	struct clip clip;
	unsigned int noop;

	cairo_observation_record_t slowest;
    } glyphs;

    cairo_array_t timings;
    cairo_recording_surface_t *record;
};

struct _cairo_device_observer {
    cairo_device_t base;
    cairo_device_t *target;

    cairo_observation_t log;
};

struct callback_list {
    cairo_list_t link;

    cairo_surface_observer_callback_t func;
    void *data;
};

struct _cairo_surface_observer {
    cairo_surface_t base;
    cairo_surface_t *target;

    cairo_observation_t log;

    cairo_list_t paint_callbacks;
    cairo_list_t mask_callbacks;
    cairo_list_t fill_callbacks;
    cairo_list_t stroke_callbacks;
    cairo_list_t glyphs_callbacks;

    cairo_list_t flush_callbacks;
    cairo_list_t finish_callbacks;
};

#endif /* CAIRO_SURFACE_OBSERVER_PRIVATE_H */
