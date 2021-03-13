use crate::ICON_DATA;
use anyhow::anyhow;
use config::configuration;
use config::wezterm_version;
use http_req::request::{HttpVersion, Request};
use http_req::uri::Uri;
use mux::connui::ConnectionUI;
use portable_pty::PtySize;
use regex::Regex;
use serde::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use termwiz::cell::{AttributeChange, Hyperlink, Underline};
use termwiz::color::AnsiColor;
use termwiz::escape::csi::{Cursor, Sgr};
use termwiz::escape::osc::{ITermDimension, ITermFileData, ITermProprietary};
use termwiz::escape::{OneBased, OperatingSystemCommand, CSI};
use termwiz::surface::{Change, CursorVisibility};
use wezterm_toast_notification::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    if !configuration().show_update_window {
        return;
    }
    let mut updater = UPDATER_WINDOW.lock().unwrap();

    let enable_close_delay = false;
    let size = PtySize {
        cols: 80,
        rows: 35,
        pixel_width: 0,
        pixel_height: 0,
    };
    let ui = ConnectionUI::with_dimensions(size, enable_close_delay);
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
        Change::CursorVisibility(CursorVisibility::Hidden),
        Change::Attribute(AttributeChange::Underline(Underline::Single)),
        Change::Attribute(AttributeChange::Hyperlink(Some(Arc::new(Hyperlink::new(
            install,
        ))))),
        format!("\r\nVersion {} is now available!\r\n", release.tag_name).into(),
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

pub fn load_last_release_info_and_set_banner() {
    if !configuration().check_for_updates {
        return;
    }

    let update_file_name = config::RUNTIME_DIR.join("check_update");
    if let Ok(data) = std::fs::read(update_file_name) {
        let latest: Release = match serde_json::from_slice(&data) {
            Ok(d) => d,
            Err(_) => return,
        };

        let current = wezterm_version();
        let force_ui = std::env::var_os("WEZTERM_ALWAYS_SHOW_UPDATE_UI").is_some();
        if latest.tag_name.as_str() <= current && !force_ui {
            return;
        }

        set_banner_from_release_info(&latest);
    }
}

fn set_banner_from_release_info(latest: &Release) {
    let mux = crate::Mux::get().unwrap();
    let url = format!(
        "https://wezfurlong.org/wezterm/changelog.html#{}",
        latest.tag_name
    );

    let icon = ITermFileData {
        name: None,
        size: Some(ICON_DATA.len()),
        width: ITermDimension::Automatic,
        height: ITermDimension::Cells(2),
        preserve_aspect_ratio: true,
        inline: true,
        data: ICON_DATA.to_vec().into_boxed_slice(),
    };
    let icon = OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(icon)));
    let top_line_pos = CSI::Cursor(Cursor::CharacterAndLinePosition {
        line: OneBased::new(1),
        col: OneBased::new(6),
    });
    let second_line_pos = CSI::Cursor(Cursor::CharacterAndLinePosition {
        line: OneBased::new(2),
        col: OneBased::new(6),
    });
    let link_on = OperatingSystemCommand::SetHyperlink(Some(Hyperlink::new(url)));
    let underline_color = CSI::Sgr(Sgr::UnderlineColor(AnsiColor::Blue.into()));
    let underline_on = CSI::Sgr(Sgr::Underline(Underline::Single));
    let reset = CSI::Sgr(Sgr::Reset);
    let link_off = OperatingSystemCommand::SetHyperlink(None);
    mux.set_banner(Some(format!(
        "{}{}WezTerm Update Available\r\n{}{}{}{}Click to see what's new{}{}\r\n",
        icon,
        top_line_pos,
        second_line_pos,
        link_on,
        underline_color,
        underline_on,
        link_off,
        reset,
    )));
}

fn schedule_set_banner_from_release_info(latest: &Release) {
    let current = wezterm_version();
    if latest.tag_name.as_str() <= current {
        return;
    }
    promise::spawn::spawn_into_main_thread({
        let latest = latest.clone();
        async move {
            set_banner_from_release_info(&latest);
        }
    })
    .detach();
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

    let update_file_name = config::RUNTIME_DIR.join("check_update");
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
            schedule_set_banner_from_release_info(&latest);
            let current = wezterm_version();
            if latest.tag_name.as_str() > current || force_ui {
                log::info!(
                    "latest release {} is newer than current build {}",
                    latest.tag_name,
                    current
                );

                let url = format!(
                    "https://wezfurlong.org/wezterm/changelog.html#{}",
                    latest.tag_name
                );

                persistent_toast_notification_with_click_to_open_url(
                    "WezTerm Update Available",
                    "Click to see what's new",
                    &url,
                );

                show_update_available(latest.clone());
            }

            config::create_user_owned_dirs(update_file_name.parent().unwrap()).ok();

            // Record the time of this check
            if let Ok(f) = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&update_file_name)
            {
                serde_json::to_writer_pretty(f, &latest).ok();
            }
        }

        std::thread::sleep(update_interval);
    }
}

pub fn start_update_checker() {
    static CHECKER_STARTED: AtomicBool = AtomicBool::new(false);
    if configuration().check_for_updates {
        if let Ok(false) =
            CHECKER_STARTED.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        {
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
