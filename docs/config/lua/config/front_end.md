# `front_end = "OpenGL"`

Specifies which render front-end to use.  This option used to have
more scope in earlier versions of wezterm, but today it allows two
possible values:

* `OpenGL` - use GPU accelerated rasterization (this is the default)
* `Software` - use CPU-based rasterization.

You may wish (or need!) to select `Software` if there are issues with your
GPU/OpenGL drivers.

WezTerm will automatically select `Software` if it detects that it is
being started in a Remote Desktop environment on Windows.
