#include "config.h"

#include <cairo-xml.h>
#include <cairo-script-interpreter.h>

#include <stdio.h>
#include <string.h>

static cairo_surface_t *
_surface_create (void *_closure,
		 cairo_content_t content,
		 double width, double height,
		 long uid)
{
    cairo_surface_t **closure = _closure;
    cairo_surface_t *surface;
    cairo_rectangle_t extents;

    extents.x = extents.y = 0;
    extents.width  = width;
    extents.height = height;
    surface = cairo_recording_surface_create (content, &extents);
    if (*closure == NULL)
	*closure = cairo_surface_reference (surface);

    return surface;
}

static cairo_status_t
stdio_write (void *closure, const unsigned char *data, unsigned len)
{
    if (fwrite (data, len, 1, closure) == 1)
	return CAIRO_STATUS_SUCCESS;
    else
	return CAIRO_STATUS_WRITE_ERROR;
}

int
main (int argc, char **argv)
{
    cairo_surface_t *surface = NULL;
    const cairo_script_interpreter_hooks_t hooks = {
	.closure = &surface,
	.surface_create = _surface_create,
    };
    cairo_script_interpreter_t *csi;
    FILE *in = stdin, *out = stdout;

    if (argc >= 2 && strcmp (argv[1], "-"))
	in = fopen (argv[1], "r");
    if (argc >= 3 && strcmp (argv[2], "-"))
	out = fopen (argv[2], "w");

    csi = cairo_script_interpreter_create ();
    cairo_script_interpreter_install_hooks (csi, &hooks);
    cairo_script_interpreter_feed_stream (csi, in);
    cairo_script_interpreter_finish (csi);
    cairo_script_interpreter_destroy (csi);

    if (surface != NULL) {
	cairo_device_t *xml;

	xml = cairo_xml_create_for_stream (stdio_write, out);
	cairo_xml_for_recording_surface (xml, surface);
	cairo_device_destroy (xml);

	cairo_surface_destroy (surface);
    }

    if (in != stdin)
	fclose (in);
    if (out != stdout)
	fclose (out);

    return 0;
}
