extern crate cmake;
use std::env;

fn main() {
    let dst = cmake::Config::new("harfbuzz")
        .define("HB_HAVE_FREETYPE", "ON")
        .build();
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=harfbuzz");

    // Dependent crates that need to find hb.h can use DEP_HARFBUZZ_INCLUDE from their build.rs.
    println!(
        "cargo:include={}",
        env::current_dir().unwrap().join("harfbuzz/src").display()
    );
}
