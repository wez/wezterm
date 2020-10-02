use std::path::Path;
use vergen::{generate_cargo_keys, ConstantsFlags};

fn bake_color_schemes() {
    let dir = std::fs::read_dir("assets/colors").unwrap();

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

    let mut flags = ConstantsFlags::all();
    flags.remove(ConstantsFlags::SEMVER_FROM_CARGO_PKG);

    bake_color_schemes();

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
        use anyhow::Context as _;
        use std::io::Write;
        let profile = std::env::var("PROFILE").unwrap();
        let exe_output_dir = Path::new("target").join(profile);
        let windows_dir = std::env::current_dir()
            .unwrap()
            .join("assets")
            .join("windows");
        let conhost_dir = windows_dir.join("conhost");

        for name in &["conpty.dll", "OpenConsole.exe"] {
            let dest_name = exe_output_dir.join(name);
            let src_name = conhost_dir.join(name);

            if !dest_name.exists() {
                std::fs::copy(&src_name, &dest_name)
                    .context(format!(
                        "copy {} -> {}",
                        src_name.display(),
                        dest_name.display()
                    ))
                    .unwrap();
            }
        }

        {
            let dest_mesa = exe_output_dir.join("mesa");
            let _ = std::fs::create_dir(&dest_mesa);
            let dest_name = dest_mesa.join("opengl32.dll");
            let src_name = windows_dir.join("mesa").join("opengl32.dll");
            if !dest_name.exists() {
                std::fs::copy(&src_name, &dest_name)
                    .context(format!(
                        "copy {} -> {}",
                        src_name.display(),
                        dest_name.display()
                    ))
                    .unwrap();
            }
        }

        let version = if ci_tag.is_empty() {
            let mut cmd = std::process::Command::new("git");
            cmd.args(&["describe", "--tags"]);
            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    String::from_utf8_lossy(&output.stdout).trim().to_owned()
                } else {
                    "UNKNOWN".to_owned()
                }
            } else {
                "UNKNOWN".to_owned()
            }
        } else {
            ci_tag
        };

        let rcfile_name = Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("resource.rc");
        let mut rcfile = std::fs::File::create(&rcfile_name).unwrap();
        write!(
            rcfile,
            r#"
#include <winres.h>
// This ID is coupled with code in window/src/os/windows/window.rs
#define IDI_ICON 0x101
IDI_ICON ICON "{win}\\terminal.ico"
APP_MANIFEST RT_MANIFEST "{win}\\manifest.manifest"
VS_VERSION_INFO VERSIONINFO
FILEVERSION     1,0,0,0
PRODUCTVERSION  1,0,0,0
FILEFLAGSMASK   VS_FFI_FILEFLAGSMASK
FILEFLAGS       0
FILEOS          VOS__WINDOWS32
FILETYPE        VFT_APP
FILESUBTYPE     VFT2_UNKNOWN
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904E4"
        BEGIN
            VALUE "CompanyName",      "Wez Furlong\0"
            VALUE "FileDescription",  "WezTerm - Wez's Terminal Emulator\0"
            VALUE "FileVersion",      "{version}\0"
            VALUE "LegalCopyright",   "Wez Furlong, MIT licensed\0"
            VALUE "InternalName",     "\0"
            VALUE "OriginalFilename", "\0"
            VALUE "ProductName",      "WezTerm\0"
            VALUE "ProductVersion",   "{version}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1252
    END
END
"#,
            win = windows_dir.display().to_string().replace("\\", "\\\\"),
            version = version,
        )
        .unwrap();
        drop(rcfile);

        // Obtain MSVC environment so that the rc compiler can find the right headers.
        // https://github.com/nabijaczleweli/rust-embed-resource/issues/11#issuecomment-603655972
        let target = std::env::var("TARGET").unwrap();
        if let Some(tool) = cc::windows_registry::find_tool(target.as_str(), "cl.exe") {
            for (key, value) in tool.env() {
                std::env::set_var(key, value);
            }
        }
        embed_resource::compile(rcfile_name);
    }
}
