use cmake::Config;
use std::env;

fn libpng() {
    let mut config = Config::new("libpng");
    let dst = config.profile("Release").build();
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=png");
}

fn freetype() {
    let mut config = Config::new("freetype2");
    let dst = config
        .define("FT_WITH_PNG", "ON")
        .profile("Release")
        .build();
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=freetype");
    println!("cargo:rustc-link-search=native=/usr/lib");
    println!("cargo:rustc-link-lib=bz2");
    println!("cargo:rustc-link-lib=z");
    println!("cargo:include={}/include/freetype2", dst.display());
    println!("cargo:lib={}/lib/libfreetype.a", dst.display());
}

fn main() {
    libpng();
    freetype();
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:outdir={}", out_dir);
}
