#![cfg(all(unix, not(target_os = "macos")))]

//! <https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.portal.Settings.xml>

use crate::{Appearance, Connection, ConnectionOps};
use anyhow::Context;
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use zbus::dbus_proxy;
use zvariant::OwnedValue;

#[dbus_proxy(
    interface = "org.freedesktop.portal.Settings",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait PortalSettings {
    fn ReadAll(
        &self,
        namespaces: &[&str],
    ) -> zbus::Result<HashMap<String, HashMap<String, OwnedValue>>>;

    fn Read(&self, namespace: &str, key: &str) -> zbus::Result<OwnedValue>;

    #[dbus_proxy(signal)]
    fn SettingChanged(&self, namespace: &str, key: &str, value: OwnedValue) -> Result<()>;
}

struct State {
    appearance: Option<Appearance>,
    subscribe_running: bool,
    last_update: Instant,
}

lazy_static::lazy_static! {
  static ref STATE: Mutex<State> = Mutex::new(
          State {
              appearance: None,
              subscribe_running: false,
              last_update: Instant::now(),
          }
   );
}

pub async fn read_setting(namespace: &str, key: &str) -> anyhow::Result<OwnedValue> {
    let connection = zbus::ConnectionBuilder::session()?.build().await?;
    let proxy = PortalSettingsProxy::new(&connection)
        .await
        .context("make proxy")?;
    proxy.Read(namespace, key).await.context("Read")
}

fn value_to_appearance(value: OwnedValue) -> anyhow::Result<Appearance> {
    Ok(match value.downcast_ref::<u32>() {
        Some(1) => Appearance::Dark,
        Some(_) => Appearance::Light,
        None => {
            anyhow::bail!(
                "Unable to resolve appearance \
                 using xdg-desktop-portal: expected a u32 value but got {value:#?}"
            );
        }
    })
}

pub async fn get_appearance() -> anyhow::Result<Appearance> {
    let mut state = STATE.lock().unwrap();
    if state.appearance.is_some()
        && (state.subscribe_running || state.last_update.elapsed().as_secs() < 1)
    {
        Ok(state.appearance.unwrap())
    } else {
        let value = read_setting("org.freedesktop.appearance", "color-scheme").await?;
        let appearance = value_to_appearance(value)?;
        state.appearance = Some(appearance);
        state.last_update = Instant::now();
        Ok(appearance)
    }
}

pub async fn run_signal_loop(stream: &mut SettingChangedStream<'_>) -> Result<(), anyhow::Error> {
    // query appearance again as it might have changed without us knowing
    if let Ok(value) =
        value_to_appearance(read_setting("org.freedesktop.appearance", "color-scheme").await?)
    {
        let mut state = STATE.lock().unwrap();
        if state.appearance != Some(value) {
            state.appearance = Some(value);
            state.last_update = Instant::now();
            drop(state);
            let conn = Connection::get().ok_or_else(|| anyhow::anyhow!("connection is dead"))?;
            conn.advise_of_appearance_change(value);
        }
    }

    while let Some(signal) = stream.next().await {
        let args = signal.args()?;
        if args.namespace == "org.freedesktop.appearance" && args.key == "color-scheme" {
            if let Ok(appearance) = value_to_appearance(args.value) {
                let mut state = STATE.lock().unwrap();
                state.appearance = Some(appearance);
                state.last_update = Instant::now();
                drop(state);
                let conn =
                    Connection::get().ok_or_else(|| anyhow::anyhow!("connection is dead"))?;
                conn.advise_of_appearance_change(appearance);
            }
        }
    }
    Result::<(), anyhow::Error>::Ok(())
}

pub fn subscribe() {
    promise::spawn::spawn(async move {
        let connection = zbus::ConnectionBuilder::session()?.build().await?;
        let proxy = PortalSettingsProxy::new(&connection)
            .await
            .context("make proxy")?;
        let mut stream = proxy.receive_SettingChanged().await?;

        STATE.lock().unwrap().subscribe_running = true;
        let res = run_signal_loop(&mut stream).await;
        STATE.lock().unwrap().subscribe_running = false;

        res
    })
    .detach();
}
