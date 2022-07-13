#![cfg(all(unix, not(target_os = "macos")))]

//! <https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.portal.Settings.xml>

use anyhow::Context;
use std::collections::HashMap;
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

pub async fn read_setting(namespace: &str, key: &str) -> anyhow::Result<OwnedValue> {
    let connection = zbus::ConnectionBuilder::session()?.build().await?;
    let proxy = PortalSettingsProxy::new(&connection)
        .await
        .context("make proxy")?;
    proxy.Read(namespace, key).await.context("Read")
}
