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

    #[cfg(windows)]
    embed_resource::compile("assets/windows/resource.rc");
}
