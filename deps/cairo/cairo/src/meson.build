cairo_sources = [
  'cairo-analysis-surface.c',
  'cairo-arc.c',
  'cairo-array.c',
  'cairo-atomic.c',
  'cairo-base64-stream.c',
  'cairo-base85-stream.c',
  'cairo-bentley-ottmann-rectangular.c',
  'cairo-bentley-ottmann-rectilinear.c',
  'cairo-bentley-ottmann.c',
  'cairo-botor-scan-converter.c',
  'cairo-boxes-intersect.c',
  'cairo-boxes.c',
  'cairo-cache.c',
  'cairo-clip-boxes.c',
  'cairo-clip-polygon.c',
  'cairo-clip-region.c',
  'cairo-clip-surface.c',
  'cairo-clip-tor-scan-converter.c',
  'cairo-clip.c',
  'cairo-color.c',
  'cairo-composite-rectangles.c',
  'cairo-compositor.c',
  'cairo-contour.c',
  'cairo-damage.c',
  'cairo-debug.c',
  'cairo-default-context.c',
  'cairo-device.c',
  'cairo-error.c',
  'cairo-fallback-compositor.c',
  'cairo-fixed.c',
  'cairo-font-face-twin-data.c',
  'cairo-font-face-twin.c',
  'cairo-font-face.c',
  'cairo-font-options.c',
  'cairo-freed-pool.c',
  'cairo-freelist.c',
  'cairo-gstate.c',
  'cairo-hash.c',
  'cairo-hull.c',
  'cairo-image-compositor.c',
  'cairo-image-info.c',
  'cairo-image-source.c',
  'cairo-image-surface.c',
  'cairo-line.c',
  'cairo-lzw.c',
  'cairo-mask-compositor.c',
  'cairo-matrix.c',
  'cairo-mempool.c',
  'cairo-mesh-pattern-rasterizer.c',
  'cairo-misc.c',
  'cairo-mono-scan-converter.c',
  'cairo-mutex.c',
  'cairo-no-compositor.c',
  'cairo-observer.c',
  'cairo-output-stream.c',
  'cairo-paginated-surface.c',
  'cairo-path-bounds.c',
  'cairo-path-fill.c',
  'cairo-path-fixed.c',
  'cairo-path-in-fill.c',
  'cairo-path-stroke-boxes.c',
  'cairo-path-stroke-polygon.c',
  'cairo-path-stroke-traps.c',
  'cairo-path-stroke-tristrip.c',
  'cairo-path-stroke.c',
  'cairo-path.c',
  'cairo-pattern.c',
  'cairo-pen.c',
  'cairo-polygon-intersect.c',
  'cairo-polygon-reduce.c',
  'cairo-polygon.c',
  'cairo-raster-source-pattern.c',
  'cairo-recording-surface.c',
  'cairo-rectangle.c',
  'cairo-rectangular-scan-converter.c',
  'cairo-region.c',
  'cairo-rtree.c',
  'cairo-scaled-font.c',
  'cairo-shape-mask-compositor.c',
  'cairo-slope.c',
  'cairo-spans-compositor.c',
  'cairo-spans.c',
  'cairo-spline.c',
  'cairo-stroke-dash.c',
  'cairo-stroke-style.c',
  'cairo-surface-clipper.c',
  'cairo-surface-fallback.c',
  'cairo-surface-observer.c',
  'cairo-surface-offset.c',
  'cairo-surface-snapshot.c',
  'cairo-surface-subsurface.c',
  'cairo-surface-wrapper.c',
  'cairo-surface.c',
  'cairo-time.c',
  'cairo-tor-scan-converter.c',
  'cairo-tor22-scan-converter.c',
  'cairo-toy-font-face.c',
  'cairo-traps-compositor.c',
  'cairo-traps.c',
  'cairo-tristrip.c',
  'cairo-unicode.c',
  'cairo-user-font.c',
  'cairo-version.c',
  'cairo-wideint.c',
  'cairo.c',
  'cairo-cff-subset.c',
  'cairo-scaled-font-subsets.c',
  'cairo-truetype-subset.c',
  'cairo-type1-fallback.c',
  'cairo-type1-glyph-names.c',
  'cairo-type1-subset.c',
  'cairo-type3-glyph-surface.c',
  'cairo-pdf-operators.c',
  'cairo-pdf-shading.c',
  'cairo-tag-attributes.c',
  'cairo-tag-stack.c',
  'cairo-deflate-stream.c',
]

cairo_headers = [
  'cairo.h',
  'cairo-version.h',
  'cairo-deprecated.h',
]

cairo_feature_sources = {
  'cairo-png': [
    'cairo-png.c',
  ],
  'cairo-ft': [
    'cairo-ft-font.c',
    'cairo-colr-glyph-render.c',
    'cairo-svg-glyph-render.c'
  ],

  'cairo-xlib': [
    'cairo-xlib-display.c',
    'cairo-xlib-core-compositor.c',
    'cairo-xlib-fallback-compositor.c',
    'cairo-xlib-render-compositor.c',
    'cairo-xlib-screen.c',
    'cairo-xlib-source.c',
    'cairo-xlib-surface.c',
    'cairo-xlib-surface-shm.c',
    'cairo-xlib-visual.c',
    'cairo-xlib-xcb-surface.c',
  ],
  'cairo-xcb': [
    'cairo-xcb-connection.c',
    'cairo-xcb-connection-core.c',
    'cairo-xcb-connection-render.c',
    'cairo-xcb-connection-shm.c',
    'cairo-xcb-screen.c',
    'cairo-xcb-shm.c',
    'cairo-xcb-surface.c',
    'cairo-xcb-surface-core.c',
    'cairo-xcb-surface-render.c',
    'cairo-xcb-resources.c',
  ],
  'cairo-quartz': [
    'cairo-quartz-surface.c',
  ],
  'cairo-quartz-image': [
    'cairo-quartz-image-surface.c',
  ],
  'cairo-quartz-font': [
    'cairo-quartz-font.c',
  ],
  'cairo-win32': [
    'win32/cairo-win32-debug.c',
    'win32/cairo-win32-device.c',
    'win32/cairo-win32-gdi-compositor.c',
    'win32/cairo-win32-system.c',
    'win32/cairo-win32-surface.c',
    'win32/cairo-win32-display-surface.c',
    'win32/cairo-win32-printing-surface.c',
  ],
  'cairo-win32-font': [
    'win32/cairo-win32-font.c',
  ],
  'cairo-dwrite-font': [
    'win32/cairo-dwrite-font.cpp',
  ],
  'cairo-script': [
    'cairo-script-surface.c',
  ],
  'cairo-ps': [
    'cairo-ps-surface.c',
  ],
  'cairo-pdf': [
    'cairo-pdf-surface.c',
    'cairo-pdf-interchange.c',
  ],
  'cairo-svg': [
    'cairo-svg-surface.c',
  ],
  'cairo-xml': [
    'cairo-xml-surface.c',
  ],
  'cairo-tee': [
    'cairo-tee-surface.c',
  ],
}

cairo_feature_headers = {
  'cairo-ps': ['cairo-ps.h'],
  'cairo-pdf': ['cairo-pdf.h'],
  'cairo-svg': ['cairo-svg.h'],
  'cairo-ft': ['cairo-ft.h'],
  'cairo-xlib': ['cairo-xlib.h'],
  'cairo-xlib-xrender': ['cairo-xlib-xrender.h'],
  'cairo-xcb': ['cairo-xcb.h'],
  'cairo-quartz': ['cairo-quartz.h'],
  'cairo-quartz-image': ['cairo-quartz-image.h'],
  'cairo-win32': ['cairo-win32.h'],
  'cairo-dwrite-font': ['cairo-dwrite.h'],
  'cairo-gl': ['cairo-gl.h'],
  'cairo-script': ['cairo-script.h'],
  'cairo-tee': ['cairo-tee.h'],
  'cairo-xml': ['cairo-xml.h'],
  'cairo-vg': ['cairo-vg.h'],
}

cairo_no_warn_c_args = cc.get_supported_arguments([
  '-Wno-attributes',
  '-Wno-unused-but-set-variable',
  '-Wno-missing-field-initializers',
  '-Wno-unused-parameter',
  '-Wno-long-long',
])

foreach feature: built_features
  source_key = feature.get('source-key', feature.get('name'))
  cairo_sources += cairo_feature_sources.get(source_key, [])
  cairo_headers += cairo_feature_headers.get(source_key, [])
endforeach

incsrc = include_directories('.')

cairo_static_args = []
if get_option('default_library') == 'static' and host_machine.system() == 'windows'
  cairo_static_args += ['-DCAIRO_WIN32_STATIC_BUILD']
  add_project_arguments('-DCAIRO_WIN32_STATIC_BUILD', language: 'c')
endif

libcairo = library('cairo', cairo_sources,
  dependencies: deps,
  c_args: cairo_no_warn_c_args + pthread_c_args,
  cpp_args: cairo_no_warn_c_args + pthread_c_args,
  link_args: extra_link_args,
  soversion: cairo_version_sonum,
  version: cairo_libversion,
  install: true,
  include_directories: incbase,
)

cairo_features_file = configure_file(output: 'cairo-features.h', configuration: feature_conf)
cairo_headers += [cairo_features_file]

libcairo_dep = declare_dependency(link_with: libcairo,
  dependencies: deps,
  include_directories: incsrc,
  compile_args: cairo_static_args,
)

pkgmod.generate(libcairo,
  description: 'Multi-platform 2D graphics library',
  subdirs: [meson.project_name()],
)

meson.override_dependency('cairo', libcairo_dep)

install_headers(cairo_headers, subdir: 'cairo')

shell = find_program('sh', required: false)
if shell.found()
  test_scripts = [
    'check-doc-syntax.sh',
    'check-headers.sh',
    'check-preprocessor-syntax.sh',
  ]

  foreach test_script: test_scripts
    test(test_script, shell,
      args: [test_script],
      suite: ['fast', 'lint'],
      workdir: meson.current_source_dir())
  endforeach

  env = environment()
  env.set('CAIRO_HAS_HIDDEN_SYMBOLS', '1')

  cairo_def = custom_target('make-cairo-def',
                            input : cairo_headers,
                            output : 'cairo.def',
                            command : [ shell,
                                        meson.current_source_dir()/'make-cairo-def.sh',
                                        '@OUTPUT@',
                                        cairo_features_file,
                                        '@INPUT@'
                                      ])

  test('check-def', shell,
       args: ['meson-check-def.sh', cairo_def, libcairo ],
       env: env,
       workdir: meson.current_source_dir())

  test('check-plt.sh', shell,
    args: ['check-plt.sh', libcairo ],
    env: env,
    suite: ['fast', 'plt'],
    workdir: meson.current_source_dir())

endif
