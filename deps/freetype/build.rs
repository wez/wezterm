use cmake::Config;
use std::env;

fn zlib() {
    let mut config = Config::new("zlib");
    let dst = config.profile("Release").build();
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    if cfg!(unix) {
        println!("cargo:rustc-link-lib=static=z");
    } else {
        println!("cargo:rustc-link-lib=static=zlibstatic");
    }
}

fn libpng() {
    let mut config = Config::new("libpng");
    let dst = config.profile("Release").build();
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    if cfg!(unix) {
        println!("cargo:rustc-link-lib=static=png");
    } else {
        println!("cargo:rustc-link-lib=static=libpng16_static");
    }
}

fn freetype() {
    let mut config = Config::new("freetype2");
    let dst = config
        .define("FT_WITH_PNG", "ON")
        .define("CMAKE_DISABLE_FIND_PACKAGE_BZip2", "TRUE")
        .profile("Release")
        .build();
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=freetype");
    println!("cargo:rustc-link-search=native=/usr/lib");
    println!("cargo:include={}/include/freetype2", dst.display());
    println!("cargo:lib={}/lib/libfreetype.a", dst.display());
}

fn main() {
    zlib();
    libpng();
    freetype();
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:outdir={}", out_dir);
}
