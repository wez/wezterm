use crate::config::configuration;
use crate::connui::ConnectionUI;
use crate::wezterm_version;
use anyhow::anyhow;
use http_req::request::{HttpVersion, Request};
use http_req::uri::Uri;
use regex::Regex;
use serde::*;
use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use termwiz::cell::{AttributeChange, Hyperlink, Underline};
use termwiz::surface::{Change, CursorShape};

#[derive(Debug, Deserialize, Clone)]
pub struct Release {
    pub url: String,
    pub body: String,
    pub html_url: String,
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn classify_assets(&self) -> HashMap<AssetKind, Asset> {
        let mut map = HashMap::new();
        for asset in &self.assets {
            let kind = classify_asset_name(&asset.name);
            map.insert(kind, asset.clone());
        }
        map
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Asset {
    pub name: String,
    pub size: usize,
    pub url: String,
    pub browser_download_url: String,
}

pub type DistVers = String;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum AssetKind {
    SourceCode,
    AppImage,
    AppImageZSync,
    DebianDeb(DistVers),
    UbuntuDeb(DistVers),
    CentOSRpm(DistVers),
    FedoraRpm(DistVers),
    MacOSZip,
    WindowsZip,
    WindowsSetupExe,
    Unknown,
}

fn classify_asset_name(name: &str) -> AssetKind {
    let winzip = Regex::new(r"WezTerm-windows-.*\.zip$").unwrap();
    let winsetup = Regex::new(r"WezTerm-.*-setup.exe$").unwrap();
    let maczip = Regex::new(r"WezTerm-macos-.*\.zip$").unwrap();
    let appimage = Regex::new(r"WezTerm-.*\.AppImage$").unwrap();
    let appimage_zsync = Regex::new(r"WezTerm-.*\.AppImage\.zsync$").unwrap();
    let source = Regex::new(r"wezterm-.*src\.tar\.gz$").unwrap();

    let rpm = Regex::new(r"wezterm-.*-1\.([a-z]+)(\d+)\.x86_64\.rpm$").unwrap();
    for cap in rpm.captures_iter(name) {
        match &cap[1] {
            "fc" => return AssetKind::FedoraRpm(cap[2].to_string()),
            "el" => return AssetKind::CentOSRpm(cap[2].to_string()),
            _ => {}
        }
    }

    let nightly_rpm = Regex::new(r"wezterm-nightly-(fedora|centos)(\d+)\.rpm$").unwrap();
    for cap in nightly_rpm.captures_iter(name) {
        match &cap[1] {
            "fedora" => return AssetKind::FedoraRpm(cap[2].to_string()),
            "centos" => return AssetKind::CentOSRpm(cap[2].to_string()),
            _ => {}
        }
    }

    let dot_deb = Regex::new(r"wezterm-.*\.(Ubuntu|Debian)([0-9.]+)\.deb$").unwrap();
    for cap in dot_deb.captures_iter(name) {
        match &cap[1] {
            "Ubuntu" => return AssetKind::UbuntuDeb(cap[2].to_string()),
            "Debian" => return AssetKind::DebianDeb(cap[2].to_string()),
            _ => {}
        }
    }

    if winzip.is_match(name) {
        AssetKind::WindowsZip
    } else if winsetup.is_match(name) {
        AssetKind::WindowsSetupExe
    } else if maczip.is_match(name) {
        AssetKind::MacOSZip
    } else if appimage.is_match(name) {
        AssetKind::AppImage
    } else if appimage_zsync.is_match(name) {
        AssetKind::AppImageZSync
    } else if source.is_match(name) {
        AssetKind::SourceCode
    } else {
        AssetKind::Unknown
    }
}

fn get_github_release_info(uri: &str) -> anyhow::Result<Release> {
    let uri = uri.parse::<Uri>().expect("URL to be valid!?");

    let mut latest = Vec::new();
    let _res = Request::new(&uri)
        .version(HttpVersion::Http10)
        .header("User-Agent", &format!("wez/wezterm-{}", wezterm_version()))
        .send(&mut latest)
        .map_err(|e| anyhow!("failed to query github releases: {}", e))?;

    /*
    println!("Status: {} {}", _res.status_code(), _res.reason());
    println!("{}", String::from_utf8_lossy(&latest));
    */

    let latest: Release = serde_json::from_slice(&latest)?;
    Ok(latest)
}

pub fn get_latest_release_info() -> anyhow::Result<Release> {
    get_github_release_info("https://api.github.com/repos/wez/wezterm/releases/latest")
}

#[allow(unused)]
pub fn get_nightly_release_info() -> anyhow::Result<Release> {
    get_github_release_info("https://api.github.com/repos/wez/wezterm/releases/tags/nightly")
}

lazy_static::lazy_static! {
    static ref UPDATER_WINDOW: Mutex<Option<ConnectionUI>> = Mutex::new(None);
}

fn show_update_available(release: Release) {
    let mut updater = UPDATER_WINDOW.lock().unwrap();

    let enable_close_delay = false;
    let ui = ConnectionUI::with_dimensions(80, 35, enable_close_delay);
    ui.title("WezTerm Update Available");

    let install = if cfg!(windows) {
        "https://wezfurlong.org/wezterm/install/windows.html"
    } else if cfg!(target_os = "macos") {
        "https://wezfurlong.org/wezterm/install/macos.html"
    } else if cfg!(target_os = "linux") {
        "https://wezfurlong.org/wezterm/install/linux.html"
    } else {
        "https://wezfurlong.org/wezterm/installation.html"
    };

    let change_log = format!(
        "https://wezfurlong.org/wezterm/changelog.html#{}",
        release.tag_name
    );

    let brief_blurb = release
        .body
        // The default for the release body is a series of newlines.
        // Trim that so that it doesn't make the window look weird
        .trim_end()
        // Normalize any dos line endings that might have wound
        // up in the body field...
        .replace("\r\n", "\n");

    let mut render = crate::markdown::RenderState::new(
        78,
        termwiz::terminal::ScreenSize {
            cols: 80,
            rows: 35,
            xpixel: 0,
            ypixel: 0,
        },
    );
    render.parse_str(&brief_blurb);

    let mut output = vec![
        Change::CursorShape(CursorShape::Hidden),
        Change::Attribute(AttributeChange::Underline(Underline::Single)),
        Change::Attribute(AttributeChange::Hyperlink(Some(Arc::new(Hyperlink::new(
            install,
        ))))),
        format!("Version {} is now available!\r\n", release.tag_name).into(),
        Change::Attribute(AttributeChange::Hyperlink(None)),
        Change::Attribute(AttributeChange::Underline(Underline::None)),
        format!("(this is version {})\r\n", wezterm_version()).into(),
    ];
    output.append(&mut render.into_changes());
    output.extend_from_slice(&[
        "\r\n".into(),
        Change::Attribute(AttributeChange::Hyperlink(Some(Arc::new(Hyperlink::new(
            change_log,
        ))))),
        Change::Attribute(AttributeChange::Underline(Underline::Single)),
        "View Change Log\r\n".into(),
        Change::Attribute(AttributeChange::Hyperlink(None)),
    ]);
    ui.output(output);

    let assets = release.classify_assets();
    let appimage = assets.get(&AssetKind::AppImage);
    let setupexe = assets.get(&AssetKind::WindowsSetupExe);

    fn emit_direct_download_link(asset: &Option<&Asset>, ui: &ConnectionUI) {
        ui.output(vec![
            Change::Attribute(AttributeChange::Hyperlink(Some(Arc::new(Hyperlink::new(
                &asset.unwrap().browser_download_url,
            ))))),
            Change::Attribute(AttributeChange::Underline(Underline::Single)),
            format!("Download {}\r\n", asset.unwrap().name).into(),
            Change::Attribute(AttributeChange::Hyperlink(None)),
        ]);
    }

    if cfg!(target_os = "linux") && std::env::var_os("APPIMAGE").is_some() && appimage.is_some() {
        emit_direct_download_link(&appimage, &ui);
    } else if cfg!(windows) && setupexe.is_some() {
        emit_direct_download_link(&setupexe, &ui);
    } else {
        ui.output(vec![
            Change::Attribute(AttributeChange::Hyperlink(Some(Arc::new(Hyperlink::new(
                install,
            ))))),
            Change::Attribute(AttributeChange::Underline(Underline::Single)),
            "Open Download Page\r\n".into(),
            Change::Attribute(AttributeChange::Hyperlink(None)),
        ]);
    }

    updater.replace(ui);
}

fn update_checker() {
    // Compute how long we should sleep for;
    // if we've never checked, give it a few seconds after the first
    // launch, otherwise compute the interval based on the time of
    // the last check.
    let config = configuration();
    let update_interval = Duration::new(config.check_for_updates_interval_seconds, 0);
    let initial_interval = Duration::new(10, 0);

    let force_ui = std::env::var_os("WEZTERM_ALWAYS_SHOW_UPDATE_UI").is_some();

    let update_file_name = crate::config::RUNTIME_DIR.join("check_update");
    let delay = update_file_name
        .metadata()
        .and_then(|metadata| metadata.modified())
        .map_err(|_| ())
        .and_then(|systime| {
            let elapsed = systime.elapsed().unwrap_or(Duration::new(0, 0));
            update_interval.checked_sub(elapsed).ok_or(())
        })
        .unwrap_or(initial_interval);

    std::thread::sleep(if force_ui { initial_interval } else { delay });

    loop {
        if let Ok(latest) = get_latest_release_info() {
            let current = crate::wezterm_version();
            if latest.tag_name.as_str() > current || force_ui {
                log::info!(
                    "latest release {} is newer than current build {}",
                    latest.tag_name,
                    current
                );
                show_update_available(latest.clone());
            }
        }

        crate::create_user_owned_dirs(update_file_name.parent().unwrap()).ok();

        // Record the time of this check
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&update_file_name)
        {
            f.write(b"_").ok();
        }

        std::thread::sleep(update_interval);
    }
}

pub fn start_update_checker() {
    static CHECKER_STARTED: AtomicBool = AtomicBool::new(false);
    if crate::frontend::has_gui_front_end() && configuration().check_for_updates {
        if CHECKER_STARTED.compare_and_swap(false, true, Ordering::Relaxed) == false {
            std::thread::Builder::new()
                .name("update_checker".into())
                .spawn(update_checker)
                .expect("failed to spawn update checker thread");
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn classify_names() {
        assert_eq!(
            classify_asset_name("WezTerm-windows-20200505-090057-31c6155f.zip"),
            AssetKind::WindowsZip
        );
        assert_eq!(
            classify_asset_name("WezTerm-windows-nightly.zip"),
            AssetKind::WindowsZip
        );
        assert_eq!(
            classify_asset_name("WezTerm-nightly-setup.exe"),
            AssetKind::WindowsSetupExe
        );
        assert_eq!(
            classify_asset_name("WezTerm-20200505-090057-31c6155f-setup.exe"),
            AssetKind::WindowsSetupExe
        );

        assert_eq!(
            classify_asset_name("WezTerm-macos-20200505-090057-31c6155f.zip"),
            AssetKind::MacOSZip
        );
        assert_eq!(
            classify_asset_name("WezTerm-macos-nightly.zip"),
            AssetKind::MacOSZip
        );

        assert_eq!(
            classify_asset_name("wezterm-20200505_090057_31c6155f-1.fc32.x86_64.rpm"),
            AssetKind::FedoraRpm("32".into())
        );
        assert_eq!(
            classify_asset_name("wezterm-nightly-fedora32.rpm"),
            AssetKind::FedoraRpm("32".into())
        );

        assert_eq!(
            classify_asset_name("wezterm-20200505_090057_31c6155f-1.fc31.x86_64.rpm"),
            AssetKind::FedoraRpm("31".into())
        );
        assert_eq!(
            classify_asset_name("wezterm-20200505_090057_31c6155f-1.el8.x86_64.rpm"),
            AssetKind::CentOSRpm("8".into())
        );
        assert_eq!(
            classify_asset_name("wezterm-20200505_090057_31c6155f-1.el7.x86_64.rpm"),
            AssetKind::CentOSRpm("7".into())
        );
        assert_eq!(
            classify_asset_name("wezterm-20200505-090057-31c6155f.Ubuntu20.04.tar.xz"),
            AssetKind::Unknown
        );
        assert_eq!(
            classify_asset_name("wezterm-20200505-090057-31c6155f.Ubuntu20.04.deb"),
            AssetKind::UbuntuDeb("20.04".into())
        );
        assert_eq!(
            classify_asset_name("wezterm-20200505-090057-31c6155f.Ubuntu19.10.deb"),
            AssetKind::UbuntuDeb("19.10".into())
        );
        assert_eq!(
            classify_asset_name("wezterm-20200505-090057-31c6155f.Debian9.12.deb"),
            AssetKind::DebianDeb("9.12".into())
        );
        assert_eq!(
            classify_asset_name("wezterm-20200505-090057-31c6155f.Debian10.deb"),
            AssetKind::DebianDeb("10".into())
        );
        assert_eq!(
            classify_asset_name("WezTerm-20200505-090057-31c6155f-Ubuntu16.04.AppImage.zsync"),
            AssetKind::AppImageZSync
        );
        assert_eq!(
            classify_asset_name("WezTerm-20200505-090057-31c6155f-Ubuntu16.04.AppImage"),
            AssetKind::AppImage
        );
        assert_eq!(
            classify_asset_name("wezterm-20200505-090057-31c6155f-src.tar.gz"),
            AssetKind::SourceCode
        );
    }
}
