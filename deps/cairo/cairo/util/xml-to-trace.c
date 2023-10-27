#include <stdio.h>
#include <string.h>
#include <expat.h>
#include <assert.h>

struct trace {
    FILE *stream;
    char tail_buf[80];
    const char *tail;
    int surface_depth;
};

static void
start_element (void *closure,
	       const char *element,
	       const char **attr)
{
    struct trace *trace = closure;

    if (strcmp (element, "surface") == 0) {
	const char *content = "COLOR_ALPHA";
	const char *width = NULL;
	const char *height = NULL;

	while (*attr) {
	    if (strcmp (*attr, "content") == 0) {
		content = *++attr;
	    } else if (strcmp (*attr, "width") == 0) {
		width = *++attr;
	    } else if (strcmp (*attr, "height") == 0) {
		height = *++attr;
	    } else {
		fprintf (stderr, "unknown surface attribute '%s'\n", *attr);
		attr++;
	    }
	    attr++;
	}

	fprintf (trace->stream, "<< /content //%s", content);
	if (width != NULL && height != NULL) {
	    fprintf (trace->stream,
		     " /width %s /height %s",
		     width, height);
	}
	if (trace->surface_depth++ == 0)
	    fprintf (trace->stream, " >> surface context\n");
	else
	    fprintf (trace->stream, " >> surface dup context\n");
    } else if (strcmp (element, "image") == 0) {
	const char *format = "ARGB24";
	const char *width = NULL;
	const char *height = NULL;

	while (*attr) {
	    if (strcmp (*attr, "format") == 0) {
		format = *++attr;
	    } else if (strcmp (*attr, "width") == 0) {
		width = *++attr;
	    } else if (strcmp (*attr, "height") == 0) {
		height = *++attr;
	    } else {
		fprintf (stderr, "unknown image attribute '%s'\n", *attr);
		attr++;
	    }
	    attr++;
	}

	fprintf (trace->stream,
		 "<< /format //%s /width %s /height %s /mime-type (image/png) /source <{",
		 format, width, height);
	assert (trace->tail == NULL);
	trace->tail = "}> >> image pattern\n";
    } else if (strcmp (element, "solid") == 0) {
	trace->tail = " rgba\n";
    } else if (strcmp (element, "linear") == 0) {
	const char *x1 = NULL;
	const char *x2 = NULL;
	const char *y1 = NULL;
	const char *y2 = NULL;

	while (*attr) {
	    if (strcmp (*attr, "x1") == 0) {
		x1 = *++attr;
	    } else if (strcmp (*attr, "x2") == 0) {
		x2 = *++attr;
	    } else if (strcmp (*attr, "y1") == 0) {
		y1 = *++attr;
	    } else if (strcmp (*attr, "y2") == 0) {
		y2 = *++attr;
	    } else {
		fprintf (stderr, "unknown linear attribute '%s'\n", *attr);
		attr++;
	    }
	    attr++;
	}

	fprintf (trace->stream, "%s %s %s %s linear\n", x1, y1, x2, y2);
    } else if (strcmp (element, "radial") == 0) {
	const char *x1 = NULL;
	const char *y1 = NULL;
	const char *r1 = NULL;
	const char *y2 = NULL;
	const char *x2 = NULL;
	const char *r2 = NULL;

	while (*attr) {
	    if (strcmp (*attr, "x1") == 0) {
		x1 = *++attr;
	    } else if (strcmp (*attr, "y1") == 0) {
		y1 = *++attr;
	    } else if (strcmp (*attr, "r1") == 0) {
		r1 = *++attr;
	    } else if (strcmp (*attr, "x2") == 0) {
		x2 = *++attr;
	    } else if (strcmp (*attr, "y2") == 0) {
		y2 = *++attr;
	    } else if (strcmp (*attr, "r2") == 0) {
		r2 = *++attr;
	    } else {
		fprintf (stderr, "unknown radial attribute '%s'\n", *attr);
		attr++;
	    }
	    attr++;
	}

	fprintf (trace->stream,
		 "%s %s %s %s %s %s radial\n",
		 x1, y1, r1, x2, y2, r2);
    } else if (strcmp (element, "matrix") == 0) {
	fprintf (trace->stream, "[ ");
	trace->tail = " ] set-matrix\n";
    } else if (strcmp (element, "extend") == 0) {
	trace->tail = " set-extend\n";
    } else if (strcmp (element, "filter") == 0) {
	trace->tail = " set-filter\n";
    } else if (strcmp (element, "operator") == 0) {
	trace->tail = " set-operator\n";
    } else if (strcmp (element, "tolerance") == 0) {
	trace->tail = " set-tolerance\n";
    } else if (strcmp (element, "fill-rule") == 0) {
	trace->tail = " set-fill-rule\n";
    } else if (strcmp (element, "line-cap") == 0) {
	trace->tail = " set-line-cap\n";
    } else if (strcmp (element, "line-join") == 0) {
	trace->tail = " set-line-join\n";
    } else if (strcmp (element, "line-width") == 0) {
	trace->tail = " set-line-width\n";
    } else if (strcmp (element, "miter-limit") == 0) {
	trace->tail = " set-miter-limit\n";
    } else if (strcmp (element, "antialias") == 0) {
	trace->tail = " set-antialias\n";
    } else if (strcmp (element, "color-stop") == 0) {
	trace->tail = " add-color-stop\n";
    } else if (strcmp (element, "path") == 0) {
	/* need to reset the matrix to identity before the path */
	fprintf (trace->stream, "identity set-matrix ");
	trace->tail = "\n";
    } else if (strcmp (element, "dash") == 0) {
	const char *offset = "0";

	while (*attr) {
	    if (strcmp (*attr, "offset") == 0) {
		offset = *++attr;
	    }
	    attr++;
	}

	fprintf (trace->stream, "[");
	sprintf (trace->tail_buf, "] %s set-dash\n", offset);
	trace->tail = trace->tail_buf;
    } else {
    }
}

static void
cdata (void *closure,
       const XML_Char *s,
       int len)
{
    struct trace *trace = closure;

    if (trace->tail)
	fwrite (s, len, 1, trace->stream);
}

static void
end_element (void *closure,
	     const char *element)
{
    struct trace *trace = closure;

    if (trace->tail) {
	fprintf (trace->stream, "%s", trace->tail);
	trace->tail = NULL;
    }

    if (strcmp (element, "paint") == 0) {
	fprintf (trace->stream, "paint\n");
    } else if (strcmp (element, "mask") == 0) {
	fprintf (trace->stream, "mask\n");
    } else if (strcmp (element, "stroke") == 0) {
	fprintf (trace->stream, "stroke\n");
    } else if (strcmp (element, "fill") == 0) {
	fprintf (trace->stream, "fill\n");
    } else if (strcmp (element, "glyphs") == 0) {
	fprintf (trace->stream, "show-glyphs\n");
    } else if (strcmp (element, "clip") == 0) {
	fprintf (trace->stream, "clip\n");
    } else if (strcmp (element, "source-pattern") == 0) {
	fprintf (trace->stream, "set-source\n");
    } else if (strcmp (element, "mask-pattern") == 0) {
    } else if (strcmp (element, "surface") == 0) {
	if (--trace->surface_depth == 0)
	    fprintf (trace->stream, "pop\n");
	else
	    fprintf (trace->stream, "pop pattern\n");
    }
}

int
main (int argc, char **argv)
{
    struct trace trace;
    XML_Parser p;
    char buf[8192];
    int done = 0;
    FILE *in = stdin;

    trace.stream = stdout;
    trace.tail = NULL;
    trace.surface_depth = 0;

    if (argc >= 2 && strcmp (argv[1], "-"))
	in = fopen (argv[1], "r");
    if (argc >= 3 && strcmp (argv[2], "-"))
	trace.stream = fopen (argv[2], "w");

    p = XML_ParserCreate (NULL);
    XML_SetUserData (p, &trace);
    XML_SetElementHandler (p, start_element, end_element);
    XML_SetCharacterDataHandler (p, cdata);
    do {
	int len;

	len = fread (buf, 1, sizeof (buf), in);
	done = feof (stdin);

	if (XML_Parse (p, buf, len, done) == XML_STATUS_ERROR) {
	    fprintf (stderr, "Parse error at line %ld:\n%s\n",
		     XML_GetCurrentLineNumber (p),
		     XML_ErrorString (XML_GetErrorCode (p)));
	    exit (-1);
	}
    } while (! done);
    XML_ParserFree (p);

    if (in != stdin)
	fclose (in);
    if (trace.stream != stdout)
	fclose (trace.stream);

    return 0;
}
