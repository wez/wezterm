fn new_build() -> cc::Build {
    let mut cfg = cc::Build::new();
    cfg.warnings(false);
    cfg.extra_warnings(false);
    cfg.flag_if_supported("-fno-stack-check");
    cfg.flag_if_supported("-Wno-attributes");
    cfg.flag_if_supported("-Wno-enum-conversion");
    cfg.flag_if_supported("-Wno-incompatible-pointer-types");
    cfg.flag_if_supported("-Wno-parentheses-equality");
    cfg.flag_if_supported("-Wno-unused-value");
    cfg
}

fn pixman() {
    let mut cfg = new_build();
    for f in [
        "pixman.c",
        "pixman-access.c",
        "pixman-access-accessors.c",
        "pixman-bits-image.c",
        "pixman-combine32.c",
        "pixman-combine-float.c",
        "pixman-conical-gradient.c",
        "pixman-filter.c",
        "pixman-x86.c",
        "pixman-mips.c",
        "pixman-arm.c",
        "pixman-ppc.c",
        "pixman-edge.c",
        "pixman-edge-accessors.c",
        "pixman-fast-path.c",
        "pixman-glyph.c",
        "pixman-general.c",
        "pixman-gradient-walker.c",
        "pixman-image.c",
        "pixman-implementation.c",
        "pixman-linear-gradient.c",
        "pixman-matrix.c",
        "pixman-noop.c",
        "pixman-radial-gradient.c",
        "pixman-region16.c",
        "pixman-region32.c",
        "pixman-solid-fill.c",
        "pixman-timer.c",
        "pixman-trap.c",
        "pixman-utils.c",
    ] {
        cfg.file(&format!("pixman/pixman/{f}"));
    }

    cfg.include("pixman/pixman");
    cfg.define("PIXMAN_NO_TLS", None);
    cfg.define("PACKAGE", "pixman-1");

    cfg.compile("pixman");
}

fn cairo() {
    let mut cfg = new_build();
    for f in [
        // This was extracted by running the meson build with these options:
        // meson setup --reconfigure --default-library static -D dwrite=disabled \
        //        -D fontconfig=disabled -Dfreetype=disabled -Dglib=disabled \
        //        -Dpng=disabled -Dquartz=disabled -Dspectre=disabled -Dtests=disabled \
        //        -Dxcb=disabled -Dxlib=disabled -Dxlib-xcb=disabled -Dzlib=disabled \
        //        --prefix=$PWD/i b
        // and then examining the ninja file:
        // grep ': c_COMPILER' b/build.ninja | awk '{print $4}' | sort
        "src/cairo-analysis-surface.c",
        "src/cairo-arc.c",
        "src/cairo-array.c",
        "src/cairo-atomic.c",
        "src/cairo-base64-stream.c",
        "src/cairo-base85-stream.c",
        "src/cairo-bentley-ottmann-rectangular.c",
        "src/cairo-bentley-ottmann-rectilinear.c",
        "src/cairo-bentley-ottmann.c",
        "src/cairo-botor-scan-converter.c",
        "src/cairo-boxes-intersect.c",
        "src/cairo-boxes.c",
        "src/cairo-cache.c",
        "src/cairo-cff-subset.c",
        "src/cairo-clip-boxes.c",
        "src/cairo-clip-polygon.c",
        "src/cairo-clip-region.c",
        "src/cairo-clip-surface.c",
        "src/cairo-clip-tor-scan-converter.c",
        "src/cairo-clip.c",
        "src/cairo-color.c",
        "src/cairo-composite-rectangles.c",
        "src/cairo-compositor.c",
        "src/cairo-contour.c",
        "src/cairo-damage.c",
        "src/cairo-debug.c",
        "src/cairo-default-context.c",
        "src/cairo-deflate-stream.c",
        "src/cairo-device.c",
        "src/cairo-error.c",
        "src/cairo-fallback-compositor.c",
        "src/cairo-fixed.c",
        "src/cairo-font-face-twin-data.c",
        "src/cairo-font-face-twin.c",
        "src/cairo-font-face.c",
        "src/cairo-font-options.c",
        "src/cairo-freed-pool.c",
        "src/cairo-freelist.c",
        "src/cairo-gstate.c",
        "src/cairo-hash.c",
        "src/cairo-hull.c",
        "src/cairo-image-compositor.c",
        "src/cairo-image-info.c",
        "src/cairo-image-source.c",
        "src/cairo-image-surface.c",
        "src/cairo-line.c",
        "src/cairo-lzw.c",
        "src/cairo-mask-compositor.c",
        "src/cairo-matrix.c",
        "src/cairo-mempool.c",
        "src/cairo-mesh-pattern-rasterizer.c",
        "src/cairo-misc.c",
        "src/cairo-mono-scan-converter.c",
        "src/cairo-mutex.c",
        "src/cairo-no-compositor.c",
        "src/cairo-observer.c",
        "src/cairo-output-stream.c",
        "src/cairo-paginated-surface.c",
        "src/cairo-path-bounds.c",
        "src/cairo-path-fill.c",
        "src/cairo-path-fixed.c",
        "src/cairo-path-in-fill.c",
        "src/cairo-path-stroke-boxes.c",
        "src/cairo-path-stroke-polygon.c",
        "src/cairo-path-stroke-traps.c",
        "src/cairo-path-stroke-tristrip.c",
        "src/cairo-path-stroke.c",
        "src/cairo-path.c",
        "src/cairo-pattern.c",
        "src/cairo-pdf-operators.c",
        "src/cairo-pdf-shading.c",
        "src/cairo-pen.c",
        "src/cairo-polygon-intersect.c",
        "src/cairo-polygon-reduce.c",
        "src/cairo-polygon.c",
        "src/cairo-raster-source-pattern.c",
        "src/cairo-recording-surface.c",
        "src/cairo-rectangle.c",
        "src/cairo-rectangular-scan-converter.c",
        "src/cairo-region.c",
        "src/cairo-rtree.c",
        "src/cairo-scaled-font-subsets.c",
        "src/cairo-scaled-font.c",
        "src/cairo-shape-mask-compositor.c",
        "src/cairo-slope.c",
        "src/cairo-spans-compositor.c",
        "src/cairo-spans.c",
        "src/cairo-spline.c",
        "src/cairo-stroke-dash.c",
        "src/cairo-stroke-style.c",
        "src/cairo-surface-clipper.c",
        "src/cairo-surface-fallback.c",
        "src/cairo-surface-observer.c",
        "src/cairo-surface-offset.c",
        "src/cairo-surface-snapshot.c",
        "src/cairo-surface-subsurface.c",
        "src/cairo-surface-wrapper.c",
        "src/cairo-surface.c",
        "src/cairo-tag-attributes.c",
        "src/cairo-tag-stack.c",
        // "src/cairo-tee-surface.c", // doesn't compile in 1.17.8: https://gitlab.freedesktop.org/cairo/cairo/-/issues/646
        "src/cairo-time.c",
        "src/cairo-tor-scan-converter.c",
        "src/cairo-tor22-scan-converter.c",
        "src/cairo-toy-font-face.c",
        "src/cairo-traps-compositor.c",
        "src/cairo-traps.c",
        "src/cairo-tristrip.c",
        "src/cairo-truetype-subset.c",
        "src/cairo-type1-fallback.c",
        "src/cairo-type1-glyph-names.c",
        "src/cairo-type1-subset.c",
        "src/cairo-type3-glyph-surface.c",
        "src/cairo-unicode.c",
        "src/cairo-user-font.c",
        "src/cairo-version.c",
        "src/cairo-wideint.c",
        "src/cairo.c",
        "util/cairo-missing/getline.c",
        // Cairo has two conflicting ways to satisfy strndup.
        // Let's remove this one from the build.
        // "util/cairo-missing/strndup.c",
        // "util/malloc-stats.c",
    ] {
        cfg.file(&format!("cairo/{f}"));
    }

    cfg.include("cairo/src");
    cfg.include("pixman/pixman");

    let ptr_width_bits: usize = std::env::var("CARGO_CFG_TARGET_POINTER_WIDTH")
        .unwrap()
        .parse()
        .unwrap();
    let ptr_width_bytes = format!("{}", ptr_width_bits / 8);
    cfg.define("CAIRO_NO_MUTEX", Some("1"));
    cfg.define("SIZE_VOID_P", Some(ptr_width_bytes.as_str()));
    cfg.define("HAVE_STDINT_H", Some("1"));
    cfg.define("HAVE_UINT64_T", Some("1"));

    cfg.compile("cairo");
}

fn main() {
    pixman();
    cairo();
}
