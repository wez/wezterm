use pkg_config;
use std::path::Path;

fn main() {
    if let Ok(lib) = pkg_config::Config::new()
        .atleast_version("2.10.1")
        .find("fontconfig")
    {
        for inc in &lib.include_paths {
            println!(
                "cargo:incdir={}",
                inc.clone().into_os_string().into_string().unwrap()
            );
        }
        for libdir in &lib.link_paths {
            println!(
                "cargo:rustc-link-search=native={}",
                libdir.clone().into_os_string().into_string().unwrap()
            );
        }
        for libname in &lib.libs {
            println!("cargo:rustc-link-lib={}", libname);
        }
    } else {
        panic!("no fontconfig");
    }
}
