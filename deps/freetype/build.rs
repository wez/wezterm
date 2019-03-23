use cmake::Config;
use fs_extra;
use std::env;
use std::path::PathBuf;

fn zlib() {
    // The out-of-source build for zlib unfortunately modifies some of
    // the sources, leaving the repo with a dirty status.  Let's take
    // a copy of the sources so that we don't trigger this.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let src_dir = out_dir.join("zlib-src");
    if src_dir.exists() {
        fs_extra::remove_items(&vec![&src_dir]).expect("failed to remove zlib-src");
    }
    std::fs::create_dir(&src_dir).expect("failed to create zlib-src");
    fs_extra::copy_items(&vec!["zlib"], &src_dir, &fs_extra::dir::CopyOptions::new())
        .expect("failed to copy zlib to zlib-src");

    let mut config = Config::new(src_dir.join("zlib"));
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
