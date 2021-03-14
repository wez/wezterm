fn main() {
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=framework=UserNotifications");
    }
    #[cfg(windows)]
    {
        windows::build!(
            windows::data::xml::dom::XmlDocument,
            windows::foundation::*,
            windows::ui::notifications::*,
        );
    }
}
