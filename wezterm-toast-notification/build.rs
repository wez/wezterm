fn main() {
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=framework=UserNotifications");
    }
    #[cfg(windows)]
    {
        windows::build!(
            Windows::Data::Xml::Dom::XmlDocument,
            Windows::Foundation::*,
            Windows::UI::Notifications::*,
            Windows::Win32::Foundation::E_POINTER,
        );
    }
}
