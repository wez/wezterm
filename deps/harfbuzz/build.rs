use cmake::Config;
use std::env;
use std::path::Path;

fn harfbuzz() {
    let mut config = Config::new("harfbuzz");
    for (key, value) in std::env::vars() {
        println!("{}: {}", key, value);
    }

    let ft_outdir = std::env::var("DEP_FREETYPE_OUTDIR").unwrap();

    let dst = config
        .env("CMAKE_PREFIX_PATH", &ft_outdir)
        .cxxflag("-DHB_NO_PRAGMA_GCC_DIAGNOSTIC_ERROR")
        .define("HB_HAVE_FREETYPE", "ON")
        .define("HB_BUILD_TESTS", "OFF")
        .define(
            "FREETYPE_LIBRARY",
            std::env::var("DEP_FREETYPE_LIB").unwrap(),
        )
        .define(
            "FREETYPE_INCLUDE_DIR_ft2build",
            std::env::var("DEP_FREETYPE_INCLUDE").unwrap(),
        )
        .define(
            "FREETYPE_INCLUDE_DIR_freetype2",
            std::env::var("DEP_FREETYPE_INCLUDE").unwrap(),
        )
        .profile("Release")
        .build();
    emit_libdirs(Path::new(&ft_outdir));
    emit_libdirs(&dst);
    emit_libdirs(Path::new("/usr"));
    println!("cargo:rustc-link-lib=static=harfbuzz");
}

fn emit_libdirs(p: &Path) {
    for d in &["lib64", "lib"] {
        let libdir = p.join(d);
        if libdir.is_dir() {
            println!("cargo:rustc-link-search=native={}", libdir.display());
        }
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=WEZRERM_SYSDEPS");
    if cfg!(unix) && env::var("WEZRERM_SYSDEPS").map(|x| x == "1").unwrap_or(false) {
        println!("cargo:rustc-link-lib=harfbuzz");
    } else {
        harfbuzz();
    }
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:outdir={}", out_dir);
}
