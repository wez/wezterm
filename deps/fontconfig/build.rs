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
        // deliberately not erroring out here as fontconfig is an
        // optional dependency that can be activated in test builds.
        // I don't like this but don't want to solve this right now.
        // panic!("no fontconfig");
    }
}
