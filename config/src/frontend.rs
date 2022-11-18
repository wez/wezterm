use luahelper::impl_lua_conversion_dynamic;
use wezterm_dynamic::{FromDynamic, ToDynamic};

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromDynamic, ToDynamic)]
pub enum FrontEndSelection {
    OpenGL,
    WebGpu,
    Software,
}

impl Default for FrontEndSelection {
    fn default() -> Self {
        FrontEndSelection::OpenGL
    }
}

/// Corresponds to <https://docs.rs/wgpu/latest/wgpu/struct.AdapterInfo.html>
#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct GpuInfo {
    pub name: String,
    pub device_type: String,
    pub backend: String,
    pub driver: Option<String>,
    pub driver_info: Option<String>,
    pub vendor: Option<usize>,
    pub device: Option<usize>,
}
impl_lua_conversion_dynamic!(GpuInfo);

impl ToString for GpuInfo {
    fn to_string(&self) -> String {
        let mut result = format!(
            "name={}, device_type={}, backend={}",
            self.name, self.device_type, self.backend
        );
        if let Some(driver) = &self.driver {
            result.push_str(&format!(", driver={driver}"));
        }
        if let Some(driver_info) = &self.driver_info {
            result.push_str(&format!(", driver_info={driver_info}"));
        }
        if let Some(vendor) = &self.vendor {
            result.push_str(&format!(", vendor={vendor}"));
        }
        if let Some(device) = &self.device {
            result.push_str(&format!(", device={device}"));
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromDynamic, ToDynamic)]
pub enum WebGpuPowerPreference {
    LowPower,
    HighPerformance,
}

impl Default for WebGpuPowerPreference {
    fn default() -> Self {
        Self::LowPower
    }
}
