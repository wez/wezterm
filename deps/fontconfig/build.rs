use pkg_config;
use std::env;

fn main() {
    if let Ok(lib) = pkg_config::Config::new()
        .atleast_version("2.11.1")
        .find("fontconfig")
    {
        println!(
            "cargo:incdir={}",
            lib.include_paths[0]
                .clone()
                .into_os_string()
                .into_string()
                .unwrap()
        );
        return;
    }

    panic!("You need to install fontconfig");
}
