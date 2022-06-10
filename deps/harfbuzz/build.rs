use std::env;
use std::path::{Path, PathBuf};

fn harfbuzz() {
    use std::fs;

    if !Path::new("harfbuzz/.git").exists() {
        git_submodule_update();
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let mut cfg = cc::Build::new();
    cfg.warnings(false);
    cfg.cpp(true);
    cfg.flag_if_supported("-fno-rtti");
    cfg.flag_if_supported("-fno-exceptions");
    cfg.flag_if_supported("-fno-threadsafe-statics");
    cfg.flag_if_supported("-std=c++11");
    cfg.flag_if_supported("-fno-stack-check");

    let build_dir = out_dir.join("harfbuzz-build");
    fs::create_dir_all(&build_dir).unwrap();
    cfg.out_dir(&build_dir);

    let target = env::var("TARGET").unwrap();

    for f in [
        "hb-aat-layout.cc",
        "hb-aat-map.cc",
        "hb-blob.cc",
        "hb-buffer-serialize.cc",
        "hb-buffer-verify.cc",
        "hb-buffer.cc",
        "hb-common.cc",
        "hb-face.cc",
        "hb-fallback-shape.cc",
        "hb-font.cc",
        "hb-ft.cc",
        "hb-map.cc",
        "hb-number.cc",
        "hb-ot-cff1-table.cc",
        "hb-ot-cff2-table.cc",
        "hb-ot-color.cc",
        "hb-ot-face.cc",
        "hb-ot-font.cc",
        "hb-ot-layout.cc",
        "hb-ot-map.cc",
        "hb-ot-math.cc",
        "hb-ot-metrics.cc",
        "hb-ot-name.cc",
        "hb-ot-shape-complex-arabic.cc",
        "hb-ot-shape-complex-default.cc",
        "hb-ot-shape-complex-hangul.cc",
        "hb-ot-shape-complex-hebrew.cc",
        "hb-ot-shape-complex-indic-table.cc",
        "hb-ot-shape-complex-indic.cc",
        "hb-ot-shape-complex-khmer.cc",
        "hb-ot-shape-complex-myanmar.cc",
        "hb-ot-shape-complex-syllabic.cc",
        "hb-ot-shape-complex-thai.cc",
        "hb-ot-shape-complex-use.cc",
        "hb-ot-shape-complex-vowel-constraints.cc",
        "hb-ot-shape-fallback.cc",
        "hb-ot-shape-normalize.cc",
        "hb-ot-shape.cc",
        "hb-ot-tag.cc",
        "hb-ot-var.cc",
        "hb-set.cc",
        "hb-shape-plan.cc",
        "hb-shape.cc",
        "hb-shaper.cc",
        "hb-static.cc",
        "hb-ucd.cc",
        "hb-unicode.cc",
    ]
    .iter()
    {
        cfg.file(format!("harfbuzz/src/{}", f));
    }

    cfg.define("HB_NO_MT", None);
    cfg.define("HAVE_FALLBACK", None);
    cfg.define("HAVE_UCDN", None);
    cfg.include("harfbuzz/src/hb-ucdn");

    if !target.contains("windows") {
        cfg.define("HAVE_UNISTD_H", None);
        cfg.define("HAVE_SYS_MMAN_H", None);
    }

    // We know that these are present in our vendored freetype
    cfg.define("HAVE_FREETYPE", Some("1"));

    cfg.define("HAVE_FT_GET_VAR_BLEND_COORDINATES", Some("1"));
    cfg.define("HAVE_FT_SET_VAR_BLEND_COORDINATES", Some("1"));
    cfg.define("HAVE_FT_DONE_MM_VAR", Some("1"));

    // Import the include dirs exported from deps/freetype/build.rs
    for inc in std::env::var("DEP_FREETYPE_INCLUDE").unwrap().split(';') {
        cfg.include(inc);
    }

    println!(
        "cargo:rustc-link-search={}",
        std::env::var("DEP_FREETYPE_LIB").unwrap()
    );
    println!("cargo:rustc-link-lib=freetype");
    println!("cargo:rustc-link-lib=png");
    println!("cargo:rustc-link-lib=z");

    cfg.compile("harfbuzz");
}

fn git_submodule_update() {
    let _ = std::process::Command::new("git")
        .args(&["submodule", "update", "--init"])
        .status();
}

fn main() {
    harfbuzz();
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:outdir={}", out_dir);
    println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=10.9");
}
