use vergen::{generate_cargo_keys, ConstantsFlags};

fn main() {
    let mut flags = ConstantsFlags::all();
    flags.remove(ConstantsFlags::SEMVER_FROM_CARGO_PKG);
    // Generate the 'cargo:' key output
    generate_cargo_keys(ConstantsFlags::all()).expect("Unable to generate the cargo keys!");

    // If a file named `.tag` is present, we'll take its contents for the
    // version number that we report in wezterm -h.
    let mut ci_tag = String::new();
    if let Ok(tag) = std::fs::read(".tag") {
        if let Ok(s) = String::from_utf8(tag) {
            ci_tag = s.trim().to_string();
            println!("cargo:rerun-if-changed=.tag");
        }
    }
    println!("cargo:rustc-env=WEZTERM_CI_TAG={}", ci_tag);
    println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=10.9");

    #[cfg(windows)]
    {
        use std::path::Path;
        let profile = std::env::var("PROFILE").unwrap();
        let exe_output_dir = Path::new("target").join(profile);
        let exe_src_dir = Path::new("assets/windows/conhost");

        for name in &["conpty.dll", "OpenConsole.exe"] {
            let dest_name = exe_output_dir.join(name);
            let src_name = exe_src_dir.join(name);

            if !dest_name.exists() {
                std::fs::copy(src_name, dest_name).unwrap();
            }
        }
    }

    #[cfg(windows)]
    embed_resource::compile("assets/windows/resource.rc");
}
