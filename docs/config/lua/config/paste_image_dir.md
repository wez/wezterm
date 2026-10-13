---
tags:
  - clipboard
---
# `paste_image_dir = "/path/to/dir"`

{{since('nightly')}}

Specifies the directory into which image data that was copied to the
clipboard will be written when you paste it into a pane.

When the clipboard holds an image rather than text, the image is decoded,
saved into this directory, and the path of the saved file is pasted into
the terminal.  This is useful for command line tools that accept the path
to an image file.

The file name is derived from a name suggested by the source of the image
when one is available; otherwise a name is generated automatically.

If a relative path is specified, it is interpreted relative to the
directory containing your configuration file.

If not specified, the `pasted` subdirectory of the wezterm cache directory is
used. Images are saved as PNG files and are not deleted automatically.
