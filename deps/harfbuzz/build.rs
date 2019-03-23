use cmake::Config;
use std::env;

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
    println!("cargo:rustc-link-search=native={}/lib", ft_outdir);
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=harfbuzz");
    println!("cargo:rustc-link-search=native=/usr/lib");
    println!("cargo:rustc-link-lib=z");
}

fn main() {
    harfbuzz();
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:outdir={}", out_dir);
}
