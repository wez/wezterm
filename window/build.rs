fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    use gl_generator::{Api, Fallbacks, Profile, Registry};
    use std::env;
    use std::fs::File;
    use std::path::PathBuf;

    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();
    let mut file = File::create(dest.join("egl_bindings.rs")).unwrap();
    let reg = Registry::new(
        Api::Egl,
        (1, 5),
        Profile::Core,
        Fallbacks::All,
        [
            "EGL_KHR_create_context",
            "EGL_EXT_create_context_robustness",
            "EGL_KHR_create_context_no_error",
            "EGL_KHR_platform_x11",
            "EGL_KHR_platform_android",
            "EGL_KHR_platform_wayland",
            "EGL_KHR_platform_gbm",
            "EGL_EXT_platform_base",
            "EGL_EXT_platform_x11",
            "EGL_MESA_platform_gbm",
            "EGL_EXT_platform_wayland",
            "EGL_EXT_platform_device",
            "EGL_KHR_swap_buffers_with_damage",
        ],
    );

    if target.contains("android") || target.contains("ios") {
        reg.write_bindings(gl_generator::StaticStructGenerator, &mut file)
    } else {
        reg.write_bindings(gl_generator::StructGenerator, &mut file)
    }
    .unwrap();

    if target.contains("apple") {
        println!("cargo:rustc-link-lib=framework=Carbon");
    }

    if target.contains("windows") {
        let mut file = File::create(dest.join("wgl_bindings.rs")).unwrap();
        let reg = Registry::new(Api::Wgl, (1, 0), Profile::Core, Fallbacks::All, []);

        reg.write_bindings(gl_generator::StructGenerator, &mut file)
            .unwrap();

        let mut file = File::create(dest.join("wgl_extra_bindings.rs")).unwrap();
        Registry::new(
            Api::Wgl,
            (1, 0),
            Profile::Core,
            Fallbacks::All,
            [
                "WGL_ARB_create_context",
                "WGL_ARB_create_context_profile",
                "WGL_ARB_create_context_robustness",
                "WGL_ARB_context_flush_control",
                "WGL_ARB_extensions_string",
                "WGL_ARB_framebuffer_sRGB",
                "WGL_ARB_multisample",
                "WGL_ARB_pixel_format",
                "WGL_ARB_pixel_format_float",
                "WGL_EXT_create_context_es2_profile",
                "WGL_EXT_extensions_string",
                "WGL_EXT_framebuffer_sRGB",
                "WGL_EXT_swap_control",
            ],
        )
        .write_bindings(gl_generator::StructGenerator, &mut file)
        .unwrap();
    }
}
