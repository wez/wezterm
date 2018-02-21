extern crate cmake;
use std::env;

fn main() {
    let mut dst = cmake::Config::new("harfbuzz");

    if cfg!(any(
        target_os = "android",
        all(unix, not(target_os = "macos")),
    ))
    {
        dst.define("HB_HAVE_FREETYPE", "ON");
    }
    let dst = dst.build();
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=harfbuzz");

    // Dependent crates that need to find hb.h can use DEP_HARFBUZZ_INCLUDE from their build.rs.
    println!(
        "cargo:include={}",
        env::current_dir().unwrap().join("harfbuzz/src").display()
    );
}
