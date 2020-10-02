pub fn wezterm_version() -> &'static str {
    // Prefer our version override, if present (see build.rs)
    let tag = env!("WEZTERM_CI_TAG");
    if tag.is_empty() {
        // Otherwise, fallback to the vergen-generated information,
        // which is basically `git describe --tags` computed in build.rs
        env!("VERGEN_SEMVER_LIGHTWEIGHT")
    } else {
        tag
    }
}

pub fn wezterm_target_triple() -> &'static str {
    env!("VERGEN_TARGET_TRIPLE")
}
