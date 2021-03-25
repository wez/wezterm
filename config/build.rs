use std::path::Path;

fn bake_color_schemes() {
    let dir = std::fs::read_dir("../assets/colors").unwrap();

    let mut schemes = vec![];

    for entry in dir {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name = name.to_str().unwrap();

        if name.ends_with(".toml") {
            let len = name.len();
            let scheme_name = &name[..len - 5];
            let data = String::from_utf8(std::fs::read(entry.path()).unwrap()).unwrap();
            schemes.push((scheme_name.to_string(), data));

            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }

    let mut code = String::new();
    code.push_str(&format!(
        "pub const SCHEMES: [(&'static str, &'static str); {}] = [",
        schemes.len()
    ));
    for (name, data) in schemes {
        code.push_str(&format!(
            "(\"{}\", \"{}\"),\n",
            name.escape_default(),
            data.escape_default(),
        ));
    }
    code.push_str("];\n");

    std::fs::write(
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("scheme_data.rs"),
        code,
    )
    .unwrap();
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    bake_color_schemes();

    // If a file named `.tag` is present, we'll take its contents for the
    // version number that we report in wezterm -h.
    let mut ci_tag = String::new();
    if let Ok(tag) = std::fs::read("../.tag") {
        if let Ok(s) = String::from_utf8(tag) {
            ci_tag = s.trim().to_string();
            println!("cargo:rerun-if-changed=../.tag");
        }
    } else {
        // Otherwise we'll derive it from the git information
        let head = Path::new("../.git/HEAD");
        if head.exists() {
            let head = head.canonicalize().unwrap();
            println!("cargo:rerun-if-changed={}", head.display());
            if let Ok(output) = std::process::Command::new("git")
                .args(&["describe", "--tags", "--match", "20*"])
                .output()
            {
                let info = String::from_utf8_lossy(&output.stdout);
                ci_tag = info.trim().to_string();
            }
        }
    }

    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=WEZTERM_TARGET_TRIPLE={}", target);
    println!("cargo:rustc-env=WEZTERM_CI_TAG={}", ci_tag);
    println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=10.9");
}
