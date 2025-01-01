cairo_missing_sources = [
  'strndup.c',
  'getline.c',
]

libcairomissing = static_library('cairo-missing', cairo_missing_sources,
  include_directories: [incbase, incsrc],
  install: false,
  build_by_default: false,
)

libcairomissing_dep = declare_dependency(
  link_with: libcairomissing,
  include_directories: include_directories('.'),
)
