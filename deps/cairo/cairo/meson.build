project('cairo', 'c', 'cpp',
  meson_version: '>= 0.56.0',
  version: run_command(find_program('version.py'), check: true).stdout().strip(),
  default_options: ['warning_level=2'],
)

# Keep in sync with configure.ac!
freetype_required_version = '>= 9.7.3'
fontconfig_required_version = '>= 2.2.95'
xrender_required_version = '>= 0.6'
xcb_required_version = '>= 1.6'
xcb_render_required_version = '>= 1.6'
libudev_required_version = '>= 136'
glib_required_version = '>= 2.14'

# library versioning
version_arr = meson.project_version().split('.')
cairo_version_major = version_arr[0].to_int()
cairo_version_minor = version_arr[1].to_int()
cairo_version_micro = version_arr[2].to_int()

# The libtool shared library version stuff.
# Try and maintain compatibility with the previous library versioning.
cairo_version_sonum = cairo_version_major + 1
cairo_version =  cairo_version_major * 10000 + cairo_version_minor * 100 + cairo_version_micro

if cairo_version_minor % 2 == 1
  # unstable release
  cairo_libversion = '@0@.@1@.0'.format(cairo_version_sonum, cairo_version)
else
  # stable release
  cairo_libversion = '@0@.@1@.@2@'.format(cairo_version_sonum, cairo_version, cairo_version_micro)
endif

conf = configuration_data()

cc = meson.get_compiler('c')

# Compiler flags
cflags = []
if cc.get_id() != 'msvc'
  cflags += [
    '-Wmissing-declarations',
    '-Werror-implicit-function-declaration',
    '-Wpointer-arith',
    '-Wwrite-strings',
    '-Wsign-compare',
    '-Wpacked',
    '-Wswitch-enum',
    '-Wmissing-format-attribute',
    '-Wvolatile-register-var',
    '-Wstrict-aliasing=2',
    '-Winit-self',
    '-Wunsafe-loop-optimizations',
    '-Wno-missing-field-initializers',
    '-Wno-unused-parameter',
    '-Wno-attributes',
    '-Wno-long-long',
    '-Winline'
  ]

  cflags += ['-Wno-unused-but-set-variable',
             '-Wno-enum-conversion'
  ]

  cflags += [
    '-fno-strict-aliasing',
    '-fno-common'
  ]

  if get_option('optimization') in ['1', '2', '3']
    cflags += '-Wp,-D_FORTIFY_SOURCE=2'
  endif

  supported_cflags = cc.get_supported_arguments(cflags)
  add_project_arguments(supported_cflags, language: 'c')

  # We only wish to enable attribute(warn_unused_result) if we can prevent
  # gcc from generating thousands of warnings about the misapplication of the
  # attribute to void functions and variables.
  warn_unused_result = ''
  if supported_cflags.contains('-Wno-attributes')
    if cc.compiles(files('meson-cc-tests/check-unused-result.c'), args : ['-Wno-attributes', '-Werror'])
      warn_unused_result = '__attribute__((__warn_unused_result__))'
    endif
  endif
  conf.set('WARN_UNUSED_RESULT', warn_unused_result)
endif

if cc.get_id() == 'msvc'
  # Basic usage in the cairo type system that causes spammy and useless warnings
  add_project_arguments('/wd4244', '/wd4146',
                        # Don't warn about double -> float truncation
                        '/wd4305',
                        language : 'c')
endif

add_project_arguments('-D_GNU_SOURCE', language: 'c')

# Make sure source directory hasn't been configured with autotools
fs = import('fs')
if fs.exists('config.h') or fs.exists('src/cairo-features.h') or fs.exists('src/cairo-supported-features.h')
  error('''
        The source directory '@0@' appears to contain
        autotools configuration artifacts. This can cause difficult to
        debug build problems. Please clean it up and then re-run meson.
        '''.format(meson.source_root()))
endif

pkgmod = import('pkgconfig')
python3 = import('python').find_installation()

check_sizeofs = [
  ['void *', {'conf-name': 'SIZEOF_VOID_P'}],
  ['int'],
  ['long'],
  ['long long'],
  ['size_t'],
]

check_headers = [
  ['stdint.h'],
  ['inttypes.h'],
  ['sys/int_types.h'],
  ['fcntl.h'],
  ['unistd.h'],
  ['signal.h'],
  ['sys/stat.h'],
  ['sys/socket.h'],
  ['poll.h'],
  ['sys/poll.h'],
  ['sys/un.h'],
  ['sched.h', {'check-funcs': ['sched_getaffinity']}],
  ['sys/mman.h', {'check-funcs': ['mmap']}],
  ['time.h', {'check-funcs': ['clock_gettime']}],
  ['libgen.h'],
  ['byteswap.h'],
  ['signal.h'],
  ['setjmp.h'],
  ['fenv.h'],
  ['sys/wait.h'],
  ['sys/stat.h'],
  ['io.h'],
  ['fenv.h', {'check-funcs': ['feenableexcept', 'fedisableexcept', 'feclearexcept']}],
  ['xlocale.h'],
  ['sys/ioctl.h'],
  ['intsafe.h'],
]

check_types = [
  ['uint64_t', {'headers': ['stdint.h']}],
  ['uint128_t', {'headers': ['stdint.h']}],
  ['__uint128_t']
]

check_funcs = [
  'alarm',
  'ctime_r',
  'localtime_r',
  'gmtime_r',
  'drand48',
  'flockfile',
  'funlockfile',
  'getline',
  'link',
  'fork',
  'waitpid',
  'raise',
  'newlocale',
  'strtod_l',
]

check_thread_flags = [
  [['-D_REENTRANT'], ['-lpthread']],
  [['-pthread'], []],
  [['-D_REENTRANT'], [], {'real': false}],
]

m_dep = cc.find_library('m', required: false)
# Used in util
gtk_dep = dependency('gtk+-2.0', required: get_option('gtk2-utils'))

deps = [m_dep]
test_deps = []
internal_deps = []
extra_link_args = []

if host_machine.endian() == 'big'
  conf.set('WORDS_BIGENDIAN', 1)
endif

float_order = cc.get_define('__FLOAT_WORD_ORDER__')
if float_order != ''
  if float_order == cc.get_define('__ORDER_BIG_ENDIAN__')
    conf.set('FLOAT_WORDS_BIGENDIAN', 1)
  endif
else
  # Assume same as platform endian
  if host_machine.endian() == 'big'
    conf.set('FLOAT_WORDS_BIGENDIAN', 1)
  endif
endif

lzo_dep = dependency('lzo2', required: false)
if lzo_dep.found()
  conf.set('HAVE_LZO', 1)
endif

dl_dep = cc.find_library('dl', required: false)
if dl_dep.found() and cc.has_function('dlsym', dependencies: [dl_dep])
  deps += [dl_dep]
  conf.set('CAIRO_HAS_DLSYM', 1)
elif cc.has_function('dlsym')
  conf.set('CAIRO_HAS_DLSYM', 1)
elif cc.has_function('dlsym', prefix: '#include <dlfcn.h>')
  conf.set('CAIRO_HAS_DLSYM', 1)
endif

feature_conf = configuration_data()

# Array of dictionaries, used to generate per-feature pc files
# Mandatory keys: name, description
# Optional keys: requires, libs
built_features = []

zlib_dep = dependency('zlib',
  required: get_option('zlib'),
  fallback : ['zlib', 'zlib_dep'],
)
if zlib_dep.found()
  if zlib_dep.type_name() == 'internal'
    internal_deps += [zlib_dep]
  else
    deps += [zlib_dep]
  endif
  conf.set('HAVE_ZLIB', 1)
endif

png_dep = dependency('libpng',
  required: get_option('png'),
  fallback: ['libpng', 'libpng_dep']
)
if png_dep.found()
  feature_conf.set('CAIRO_HAS_SVG_SURFACE', 1)
  feature_conf.set('CAIRO_HAS_PNG_FUNCTIONS', 1)
  built_features += [
    {
      'name': 'cairo-png',
      'description': 'PNG functions',
      'deps': [png_dep],
    },
    {
      'name': 'cairo-svg',
      'description': 'SVG surface backend',
      'deps': [png_dep],
    }
  ]

  if png_dep.type_name() == 'internal'
    internal_deps += [png_dep]
  else
    deps += [png_dep]
  endif
endif

# Don't build fontconfig as a subproject on Windows unless
# explicitly requested
fontconfig_option = get_option('fontconfig')
if host_machine.system() == 'windows' and not fontconfig_option.enabled()
  fontconfig_option = false
endif

fontconfig_dep = dependency('fontconfig',
  required: fontconfig_option,
  version: fontconfig_required_version,
  fallback: ['fontconfig', 'fontconfig_dep'],
)
if fontconfig_dep.found()
  fc_check_funcs = [
    'FcInit',
    'FcFini'
  ]

  if fontconfig_dep.type_name() == 'internal'
    foreach func : fc_check_funcs
      conf.set('HAVE_@0@'.format(func.to_upper()), 1)
    endforeach
    internal_deps += [fontconfig_dep]
  else
    check_funcs += fc_check_funcs
    deps += [fontconfig_dep]
  endif

  feature_conf.set('CAIRO_HAS_FC_FONT', 1)
  built_features += [{
    'name': 'cairo-fc',
    'description': 'Fontconfig font backend',
    'deps': [fontconfig_dep],
  }]
endif

ttx = find_program('ttx', required: false)

freetype_dep = dependency('freetype2',
  required: get_option('freetype'),
  version: freetype_required_version,
  fallback: ['freetype2', 'freetype_dep'],
)
if freetype_dep.found()
  feature_conf.set('CAIRO_HAS_FT_FONT', 1)
  built_features += [{
    'name': 'cairo-ft',
    'description': 'Freetype font backend',
    'deps': [freetype_dep],
    # cairo-ft.h includes fontconfig.h so it needs its cflags
    'compile-deps': [fontconfig_dep.partial_dependency(compile_args: true, includes: true)],
  }]

  ft_check_funcs = [
    'FT_Get_X11_Font_Format',
    'FT_GlyphSlot_Embolden',
    'FT_GlyphSlot_Oblique',
    'FT_Load_Sfnt_Table',
    'FT_Library_SetLcdFilter',
    'FT_Get_Var_Design_Coordinates',
    'FT_Done_MM_Var',
    'FT_Palette_Select',
  ]

  if freetype_dep.type_name() == 'internal'
    foreach func : ft_check_funcs
      conf.set('HAVE_@0@'.format(func.to_upper()), 1)
    endforeach
    internal_deps += [freetype_dep]
  else
    if not cc.links(files('meson-cc-tests/ft_has_color.c'), dependencies: freetype_dep, name: 'FT has color')
      conf.set('FT_HAS_COLOR', '(0)')
    endif
    if cc.has_type('FT_SVG_Document', dependencies: freetype_dep, prefix: '#include <freetype/otsvg.h>')
      conf.set('HAVE_FT_SVG_DOCUMENT', 1)
      if ttx.found()
        conf.set('CAIRO_CAN_TEST_TTX_FONT', 1)
      endif
    endif
    # When FT COLRv1 API is no longer experimental we can change this to a
    # version check for the stable COLRv1 API.
    if cc.get_define('DEBUG_ENABLE_COLR_V1') != ''
      conf.set('HAVE_FT_COLR_V1', 1)
    endif
    check_funcs += ft_check_funcs
    deps += [freetype_dep]
  endif
endif

x11_dep = dependency('x11', required: get_option('xlib'))
xext_dep = dependency('xext', required: get_option('xlib'))
if x11_dep.found() and xext_dep.found()
  feature_conf.set('CAIRO_HAS_XLIB_SURFACE', 1)
  built_features += [{
    'name': 'cairo-xlib',
    'description': 'Xlib surface backend',
    'deps': [x11_dep, xext_dep],
  }]

  extra_headers = ['X11/Xlibint.h', 'X11/Xproto.h']
  check_headers += [
    ['X11/extensions/XShm.h', {'extra-headers': extra_headers}],
    ['X11/extensions/shmproto.h', {'extra-headers': extra_headers}],
    ['X11/extensions/shmstr.h', {'extra-headers': extra_headers}],
  ]
  deps += [x11_dep, xext_dep]

  # Can skip the run check by providing the result in a cross file or
  # native file as bool property value.
  prop = meson.get_external_property('ipc_rmid_deferred_release', 'auto')
  # We don't know the type of prop (bool, string) but need to differentiate
  # between a set value (bool) or the fallback value (string), so convert to
  # a string and check the string value.
  prop_str = '@0@'.format(prop)
  if prop_str in ['true', 'false']
    ipc_rmid_deferred_release = (prop_str == 'true')
    message('IPC_RMID_DEFERRED_RELEASE:', ipc_rmid_deferred_release)
  elif prop_str == 'auto'
    res = cc.run(files('meson-cc-tests/ipc_rmid_deferred_release.c'),
      dependencies: [x11_dep, xext_dep],
      name: 'shmctl IPC_RMID allowes subsequent attaches')

    ipc_rmid_deferred_release = (res.returncode() == 0)
  else
    error('Unexpected value for external property ipc_rmid_deferred_release: @0@'.format(prop_str))
  endif

  conf.set10('IPC_RMID_DEFERRED_RELEASE', ipc_rmid_deferred_release)
endif

if feature_conf.get('CAIRO_HAS_XLIB_SURFACE', 0) == 1
  xrender_dep = dependency('xrender', required: get_option('xlib'),
                           version: xrender_required_version)

  if xrender_dep.found()
    check_funcs += [
      'XRenderCreateSolidFill',
      'XRenderCreateLinearGradient',
      'XRenderCreateRadialGradient',
      'XRenderCreateConicalGradient',
    ]

    deps += [xrender_dep]

    built_features += [{
      'name': 'cairo-xlib-xrender',
      'description': 'Xlib Xrender surface backend',
      'deps': [xrender_dep],
    }]
    feature_conf.set('CAIRO_HAS_XLIB_XRENDER_SURFACE', 1)
  endif
endif

xcb_dep = dependency('xcb', required: get_option('xcb'),
                     version: xcb_required_version)
xcb_render_dep = dependency('xcb-render', required: get_option('xcb'),
                            version: xcb_render_required_version)
if xcb_dep.found() and xcb_render_dep.found()
  feature_conf.set('CAIRO_HAS_XCB_SURFACE', 1)
  built_features += [{
    'name': 'cairo-xcb',
    'description': 'XCB surface backend',
    'deps': [xcb_dep, xcb_render_dep],
  }]

  deps += [xcb_dep, xcb_render_dep]
endif

if feature_conf.get('CAIRO_HAS_XCB_SURFACE', 0) == 1 and feature_conf.get('CAIRO_HAS_XLIB_SURFACE', 0) == 1
  x11xcb_dep = dependency('x11-xcb', required: get_option('xlib-xcb'))
  if x11xcb_dep.found()
    deps += [x11xcb_dep]
    feature_conf.set('CAIRO_HAS_XLIB_XCB_FUNCTIONS', 1)
    built_features += [{
      'name': 'cairo-xlib-xcb',
      'description': 'Xlib/XCB functions',
      'deps': [x11xcb_dep],
    }]
  endif
endif

if feature_conf.get('CAIRO_HAS_XCB_SURFACE', 0) == 1
  xcbshm_dep = dependency('xcb-shm', required: get_option('xcb'))
  if xcbshm_dep.found()
    feature_conf.set('CAIRO_HAS_XCB_SHM_FUNCTIONS', 1)
    deps += [xcbshm_dep]
    built_features += [{
      'name': 'cairo-xcb-shm',
      'description': 'XCB/SHM functions',
      'deps': [xcbshm_dep],
    }]
  endif
endif

if host_machine.system() == 'darwin' and not get_option('quartz').disabled()
  quartz_deps = dependency('appleframeworks', modules : ['CoreFoundation', 'ApplicationServices'], required: get_option('quartz'))

  if quartz_deps.found()
    deps += [quartz_deps]

    feature_conf.set('CAIRO_HAS_QUARTZ_SURFACE', 1)
    feature_conf.set('CAIRO_HAS_QUARTZ_IMAGE_SURFACE', 1)

    built_features += [
      {
        'name': 'cairo-quartz',
        'description': 'Quartz surface backend',
        'deps': quartz_deps,
      },
      {
        'name': 'cairo-quartz-image',
        'description': 'Quartz Image surface backend',
        'deps': quartz_deps,
      }]
    compiler = meson.get_compiler('c')
    if compiler.has_function('CTFontDrawGlyphs', prefix: '#include <ApplicationServices/ApplicationServices.h>',
                             dependencies: quartz_deps)
      built_features += [
           {
               'name': 'cairo-quartz-font',
               'description': 'Quartz font backend',
               'deps': quartz_deps,
           }]
      feature_conf.set('CAIRO_HAS_QUARTZ_FONT', 1)
    endif
  endif
endif

if host_machine.system() == 'windows'
  add_project_arguments('-DWIN32_LEAN_AND_MEAN', '-DNOMINMAX', language: ['c', 'cpp'])

  win32_extra_deps = [
    cc.find_library('gdi32'),
    cc.find_library('msimg32'),
  ]

  deps += win32_extra_deps

  feature_conf.set('CAIRO_HAS_WIN32_SURFACE', 1)
  feature_conf.set('CAIRO_HAS_WIN32_FONT', 1)

  built_features += [
    {
      'name': 'cairo-win32',
      'description': 'Microsoft Windows surface backend',
      'deps': win32_extra_deps,
    },
    {
      'name': 'cairo-win32-font',
      'description': 'Microsoft Windows font backend',
      'deps': win32_extra_deps,
    }
  ]

  cpp_compiler = meson.get_compiler('cpp')
  d2d_dep = cpp_compiler.find_library('d2d1', required: get_option('dwrite'))
  dwrite_dep = cpp_compiler.find_library('dwrite', required: get_option('dwrite'))
  d2d_header = cpp_compiler.has_header('d2d1.h')
  d2d_3_header = cpp_compiler.has_header('d2d1_3.h')
  dwrite_header = cpp_compiler.has_header('dwrite.h')
  dwrite_3_header = cpp_compiler.has_header('dwrite_3.h')
  wincodec_dep = cpp_compiler.find_library('windowscodecs', required: get_option('dwrite'))
  wincodec_header = cpp_compiler.has_header('wincodec.h')

  if d2d_dep.found() and dwrite_dep.found() and d2d_header and dwrite_header and wincodec_dep.found() and wincodec_header
    feature_conf.set('CAIRO_HAS_DWRITE_FONT', 1)
    built_features += [{
      'name': 'cairo-dwrite-font',
      'description': 'Microsoft Windows DWrite font backend',
      'deps': [dwrite_dep, d2d_dep, wincodec_dep],
    }]
    deps += [dwrite_dep, d2d_dep, wincodec_dep]

    if cpp_compiler.has_header('d2d1_3.h')
      conf.set('HAVE_D2D1_3_H', 1)
    endif

    # Exclude MinGW dwrite_3.h because it has a broken definition of DWRITE_COLOR_GLYPH_RUN1.
    if cpp_compiler.has_header('dwrite_3.h') and cpp_compiler.get_define('__MINGW32__') == ''
      conf.set('HAVE_DWRITE_3_H', 1)
    endif

    add_project_arguments('-DWINVER=_WIN32_WINNT_WIN10', '-D_WIN32_WINNT=_WIN32_WINNT_WIN10', '-DNTDDI_VERSION=NTDDI_WIN10_RS3', language: ['c', 'cpp'])
  else
    add_project_arguments('-DWINVER=_WIN32_WINNT_WIN2K', '-D_WIN32_WINNT=_WIN32_WINNT_WIN2K', language: ['c', 'cpp'])
  endif
endif

gobject_dep = dependency('gobject-2.0',
  required: get_option('glib'),
  fallback: ['glib', 'libgobject_dep']
)
glib_dep = dependency('glib-2.0',
  required: get_option('glib'),
  version: glib_required_version,
  fallback: ['glib', 'libglib_dep'],
)
if gobject_dep.found() and glib_dep.found()
  feature_conf.set('CAIRO_HAS_GOBJECT_FUNCTIONS', 1)
endif

if zlib_dep.found()
  feature_conf.set('CAIRO_HAS_SCRIPT_SURFACE', 1)
  built_features += [{
    'name': 'cairo-script',
    'description': 'script surface backend',
    'deps': [zlib_dep],
  }]
endif

if zlib_dep.found()
  feature_conf.set('CAIRO_HAS_PS_SURFACE', 1)
  built_features += [{
    'name': 'cairo-ps',
    'description': 'PostScript surface backend',
    'deps': [zlib_dep],
  }]
endif

if zlib_dep.found()
  feature_conf.set('CAIRO_HAS_PDF_SURFACE', 1)
  built_features += [{
    'name': 'cairo-pdf',
    'description': 'PDF surface backend',
    'deps': [zlib_dep],
  }]
endif

if zlib_dep.found()
  conf.set('CAIRO_HAS_INTERPRETER', 1)
endif

if zlib_dep.found() and png_dep.found() and get_option('xml').enabled()
  feature_conf.set('CAIRO_HAS_XML_SURFACE', 1)
  built_features += [{
    'name': 'cairo-xml',
    'description': 'XML surface backend',
    'deps': [zlib_dep],
  }]
endif

bfd_dep = cc.find_library('bfd', has_headers: ['bfd.h'], required: get_option('symbol-lookup'))
if bfd_dep.found() and \
   cc.has_function('bfd_openr', dependencies: [bfd_dep]) and \
   cc.links(files('meson-cc-tests/bfd-section-flags.c'), name: 'bfd_section_flags', dependencies: bfd_dep)
  conf.set('HAVE_BFD', 1)
  deps += [bfd_dep]
elif get_option('symbol-lookup').enabled()
  error('symbol lookup via bfd was enabled via options but is not available (bfd might be too old)')
endif

if conf.get('HAVE_BFD', 0) == 1
  conf.set('CAIRO_HAS_SYMBOL_LOOKUP', 1)
endif

if feature_conf.get('CAIRO_HAS_PS_SURFACE', 0) == 1
  gs = find_program('gs', required: get_option('tests'))
  libspectre_dep = dependency('libspectre', version: '>= 0.2.0',
                              required: get_option('spectre'))
  if gs.found() and libspectre_dep.found()
    conf.set('CAIRO_CAN_TEST_PS_SURFACE', 1)
  endif

  if libspectre_dep.found()
    conf.set('CAIRO_HAS_SPECTRE', 1)
    test_deps += [libspectre_dep]
  endif
endif

if feature_conf.get('CAIRO_HAS_PDF_SURFACE', 0) == 1
  poppler_dep = dependency('poppler-glib', version: '>= 0.17.4',
                           required: get_option('tests'))
  if poppler_dep.found() and cc.has_function('poppler_page_render', dependencies: [poppler_dep])
    conf.set('CAIRO_CAN_TEST_PDF_SURFACE', 1)
    test_deps += [poppler_dep]
  endif
endif

if feature_conf.get('CAIRO_HAS_SVG_SURFACE', 0) == 1
  librsvg_dep = dependency('librsvg-2.0', version: '>= 2.35.0',
                           required: get_option('tests'))
  if librsvg_dep.found()
    conf.set('CAIRO_CAN_TEST_SVG_SURFACE', 1)
    test_deps += [librsvg_dep]
  endif
endif

pixman_dep = dependency('pixman-1',
  version: '>= 0.36.0',
  fallback: ['pixman', 'idep_pixman'],
)
if pixman_dep.found()
  feature_conf.set('CAIRO_HAS_IMAGE_SURFACE', 1)
  conf.set('HAS_PIXMAN_GLYPHS', 1)
  if pixman_dep.type_name() == 'internal'
    internal_deps += [pixman_dep]
  else
    deps += [pixman_dep]
  endif
endif

feature_conf.set('CAIRO_FEATURES_H', true)
feature_conf.set('CAIRO_HAS_USER_FONT', 1)

feature_conf.set('CAIRO_HAS_MIME_SURFACE', 1)
feature_conf.set('CAIRO_HAS_RECORDING_SURFACE', 1)
feature_conf.set('CAIRO_HAS_OBSERVER_SURFACE', 1)

if not get_option('tee').disabled()
  feature_conf.set('CAIRO_HAS_TEE_SURFACE', 1)
  built_features += [{
    'name': 'cairo-tee',
    'description': 'Tee surface backend',
  }]
endif

incbase = include_directories('.')

foreach check : check_sizeofs
  type = check[0]
  opts = check.length() > 1 ? check[1] : {}

  conf_name = opts.get('conf-name', 'SIZEOF_@0@'.format(type.underscorify().to_upper()))

  conf.set(conf_name, cc.sizeof(type))
endforeach

foreach check : check_headers
  name = check[0]
  opts = check.length() > 1 ? check[1] : {}
  prefix = ''

  foreach header : opts.get('extra-headers', [])
    prefix += '#include <@0@>\n'.format(header)
  endforeach

  if cc.has_header(name, prefix: prefix)
    conf.set('HAVE_@0@'.format(name.to_upper().underscorify()), 1)
    check_funcs += check.length() > 1 ? check[1].get('check-funcs', []) : []
  endif
endforeach

foreach check : check_types
  name = check[0]
  opts = check.length() > 1 ? check[1] : {}
  prefix = ''

  foreach header : opts.get('headers', [])
    prefix += '#include <@0@>\n'.format(header)
  endforeach

  if cc.has_type(name, prefix: prefix)
    conf.set('HAVE_@0@'.format(name.to_upper()), 1)
  endif
endforeach

foreach name : check_funcs
  if cc.has_function(name, dependencies: deps)
    conf.set('HAVE_@0@'.format(name.to_upper()), 1)
  endif
endforeach

conf.set('HAVE_STRNDUP', cc.has_function('strndup', prefix : '#include <string.h>'))

pthread_c_args = []
pthread_link_args = []

foreach thread_flags : check_thread_flags
  if not conf.has('CAIRO_HAS_PTHREAD')
    cflags = thread_flags[0]
    lflags = thread_flags[1]
    real_pthread = thread_flags.length() > 2 ? thread_flags[2].get('real', true) : true

    if cc.links(files('meson-cc-tests/pthread.c'), args: cflags + lflags, name: 'pthreads')
      conf.set('CAIRO_HAS_PTHREAD', 1)
      if real_pthread
        conf.set('CAIRO_HAS_REAL_PTHREAD', 1)
      endif
      pthread_c_args = cflags
      pthread_link_args = lflags
    endif
  endif
endforeach

extra_link_args += pthread_link_args

if cc.links(files('meson-cc-tests/atomic-ops-cxx11.c'), name: 'Atomic ops: cxx11')
  conf.set('HAVE_CXX11_ATOMIC_PRIMITIVES', 1)
elif cc.links(files('meson-cc-tests/atomic-ops-gcc-legacy.c'), name: 'Atomic ops: gcc legacy')
  conf.set('HAVE_GCC_LEGACY_ATOMICS', 1)
elif cc.has_header('atomic_ops.h')
  conf.set('HAVE_LIB_ATOMIC_OPS', 1)
elif cc.has_header('libkern/OSAtomic.h')
  conf.set('HAVE_OS_ATOMIC_OPS', 1)
endif

test_mkdir_c_args = []
if conf.get('HAVE_SYS_STAT_H', 0) == 1
  test_mkdir_c_args += ['-DHAVE_SYS_STAT_H']
endif

if conf.get('HAVE_IO_H', 0) == 1
  test_mkdir_c_args += ['-DHAVE_IO_H']
endif

if cc.links(files('meson-cc-tests/mkdir-variant-1.c'), args: test_mkdir_c_args)
  conf.set('HAVE_MKDIR', 1)
elif cc.links(files('meson-cc-tests/mkdir-variant-2.c'), args: test_mkdir_c_args)
  conf.set('HAVE_MKDIR', 2)
else
  conf.set('HAVE_MKDIR', 0)
endif

if not ['x86', 'x86_64'].contains(host_machine.cpu_family())
  conf.set('ATOMIC_OP_NEEDS_MEMORY_BARRIER', 1)
endif

have_ld_preload = ['linux', 'freebsd', 'darwin', 'dragonfly'].contains(host_machine.system())

if have_ld_preload and zlib_dep.found() and conf.get('CAIRO_HAS_REAL_PTHREAD', 0) == 1 and conf.get('CAIRO_HAS_DLSYM', 0) == 1
  conf.set('CAIRO_HAS_TRACE', 1)
endif

rt_dep = cc.find_library('rt', required: false)
have_shm = false
if rt_dep.found() and cc.has_function('shm_open', dependencies: [rt_dep])
  have_shm = true
endif

# This to make sure we don't run checks against internal deps
deps += internal_deps

subdir('src')

if feature_conf.get('CAIRO_HAS_PNG_FUNCTIONS', 0) == 1
  subdir('boilerplate')
else
  cairoboilerplate_dep = dependency('', required: false)
endif

subdir('util')

if not get_option('tests').disabled() and feature_conf.get('CAIRO_HAS_PNG_FUNCTIONS', 0) == 1
  subdir('test')
  subdir('perf')
endif

if get_option('gtk_doc')
  doc_srcdir = include_directories('src')
  subdir('doc/public')
endif

configure_file(output: 'config.h', configuration: conf)

foreach feature: built_features
  feature_deps = feature.get('deps', [])
  feature_libs = feature.get('libs', [])
  feature_compile_deps = feature.get('compile-deps', [])
  pkgmod.generate(libraries: [libcairo, feature_deps, feature_libs],
    name: feature['name'],
    description: feature['description'] + ' for cairo graphics library',
  )
  meson.override_dependency(feature['name'],
    declare_dependency(dependencies: [libcairo_dep, feature_deps, feature_compile_deps],
      link_args: feature_libs,
    )
  )
endforeach

# summary
summary({
        'Image': true,
        'Recording': true,
        'Observer': true,
        'Mime': true,
        'Tee': feature_conf.get('CAIRO_HAS_TEE_SURFACE', 0) == 1,
        'XML': feature_conf.get('CAIRO_HAS_XML_SURFACE', 0) == 1,
        'Xlib': feature_conf.get('CAIRO_HAS_XLIB_SURFACE', 0) == 1,
        'Xlib Xrender': feature_conf.get('CAIRO_HAS_XLIB_XRENDER_SURFACE', 0) == 1,
        'Quartz': feature_conf.get('CAIRO_HAS_QUARTZ_SURFACE', 0) == 1,
        'Quartz-image': feature_conf.get('CAIRO_HAS_QUARTZ_IMAGE_SURFACE', 0) == 1,
        'XCB': feature_conf.get('CAIRO_HAS_XCB_SURFACE', 0) == 1,
        'Win32': feature_conf.get('CAIRO_HAS_WIN32_SURFACE', 0) == 1,
        'CairoScript': feature_conf.get('CAIRO_HAS_SCRIPT_SURFACE', 0) == 1,
        'PostScript':  feature_conf.get('CAIRO_HAS_PS_SURFACE', 0) == 1,
        'PDF':  feature_conf.get('CAIRO_HAS_PDF_SURFACE', 0) == 1,
        'SVG':  feature_conf.get('CAIRO_HAS_SVG_SURFACE', 0) == 1,
        }, section: 'Surface Backends', bool_yn: true)

summary({
        'User': true,
        'FreeType': feature_conf.get('CAIRO_HAS_FT_FONT', 0) == 1,
        'Fontconfig': feature_conf.get('CAIRO_HAS_FC_FONT', 0) == 1,
        'Win32': feature_conf.get('CAIRO_HAS_WIN32_FONT', 0) == 1,
        'Win32 DWrite': feature_conf.get('CAIRO_HAS_DWRITE_FONT', 0) == 1,
        'Quartz': feature_conf.get('CAIRO_HAS_QUARTZ_FONT', 0) == 1,
        }, section: 'Font Backends', bool_yn: true)

summary({
        'PNG functions': feature_conf.get('CAIRO_HAS_PNG_FUNCTIONS', 0) == 1,
        'X11-xcb': feature_conf.get('CAIRO_HAS_XLIB_XCB_FUNCTIONS', 0) == 1,
        'XCB-shm': feature_conf.get('CAIRO_HAS_XCB_SHM_FUNCTIONS', 0) == 1,
        }, section: 'Functions', bool_yn: true)

summary({
        'cairo-trace:': conf.get('CAIRO_HAS_TRACE', 0) == 1,
        'cairo-script-interpreter': conf.get('CAIRO_HAS_INTERPRETER', 0) == 1,
        'API reference': get_option('gtk_doc'),
        }, section: 'Features and Utilities', bool_yn: true)
