//! Dealing with Wayland outputs

use crate::os::wayland::wl_id;
use crate::screen::{ScreenInfo, Screens};
use crate::ScreenRect;
use smithay_client_toolkit::environment::GlobalHandler;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wayland_client::protocol::wl_output::Transform;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Attached, DispatchData, Main};
use wayland_protocols::wlr::unstable::output_management::v1::client::zwlr_output_head_v1::{
    Event as ZwlrOutputHeadEvent, ZwlrOutputHeadV1,
};
use wayland_protocols::wlr::unstable::output_management::v1::client::zwlr_output_manager_v1::{
    Event as ZwlrOutputEvent, ZwlrOutputManagerV1,
};
use wayland_protocols::wlr::unstable::output_management::v1::client::zwlr_output_mode_v1::{
    Event as ZwlrOutputModeEvent, ZwlrOutputModeV1,
};

#[derive(Debug, Default, Clone)]
pub struct ModeInfo {
    pub id: u32,
    pub width: i32,
    pub height: i32,
    pub refresh: i32,
    pub preferred: bool,
}

#[derive(Debug, Default, Clone)]
pub struct HeadInfo {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub physical_width: i32,
    pub physical_height: i32,
    /// List of ids that correspond to ModeInfo's
    pub mode_ids: Vec<u32>,
    pub enabled: bool,
    pub current_mode_id: Option<u32>,
    pub x: i32,
    pub y: i32,
    pub transform: Option<Transform>,
    pub scale: f64,
    pub make: String,
    pub model: String,
    pub serial_number: String,
}

#[derive(Default, Debug)]
struct Inner {
    zwlr_heads: HashMap<u32, Attached<ZwlrOutputHeadV1>>,
    zwlr_modes: HashMap<u32, Attached<ZwlrOutputModeV1>>,
    zwlr_mode_info: HashMap<u32, ModeInfo>,
    zwlr_head_info: HashMap<u32, HeadInfo>,
}

impl Inner {
    fn handle_zwlr_mode_event(
        &mut self,
        mode: Main<ZwlrOutputModeV1>,
        event: ZwlrOutputModeEvent,
        _ddata: DispatchData,
        _inner: &Arc<Mutex<Self>>,
    ) {
        log::debug!("handle_zwlr_mode_event {event:?}");
        let id = wl_id(mode.detach());
        let info = self.zwlr_mode_info.entry(id).or_insert_with(|| ModeInfo {
            id,
            ..ModeInfo::default()
        });

        match event {
            ZwlrOutputModeEvent::Size { width, height } => {
                info.width = width;
                info.height = height;
            }
            ZwlrOutputModeEvent::Refresh { refresh } => {
                info.refresh = refresh;
            }
            ZwlrOutputModeEvent::Preferred => {
                info.preferred = true;
            }
            ZwlrOutputModeEvent::Finished => {
                self.zwlr_mode_info.remove(&id);
                self.zwlr_modes.remove(&id);
            }
            _ => {}
        }
    }

    fn handle_zwlr_head_event(
        &mut self,
        head: Main<ZwlrOutputHeadV1>,
        event: ZwlrOutputHeadEvent,
        _ddata: DispatchData,
        inner: &Arc<Mutex<Self>>,
    ) {
        log::debug!("handle_zwlr_head_event {event:?}");
        let id = wl_id(head.detach());
        let info = self.zwlr_head_info.entry(id).or_insert_with(|| HeadInfo {
            id,
            ..HeadInfo::default()
        });
        match event {
            ZwlrOutputHeadEvent::Name { name } => {
                info.name = name;
            }
            ZwlrOutputHeadEvent::Description { description } => {
                info.description = description;
            }
            ZwlrOutputHeadEvent::PhysicalSize { width, height } => {
                info.physical_width = width;
                info.physical_height = height;
            }
            ZwlrOutputHeadEvent::Mode { mode } => {
                let inner = Arc::clone(inner);
                mode.quick_assign(move |mode, event, ddata| {
                    inner
                        .lock()
                        .unwrap()
                        .handle_zwlr_mode_event(mode, event, ddata, &inner);
                });
                let mode_id = wl_id(mode.detach());
                info.mode_ids.push(mode_id);
                self.zwlr_modes.insert(mode_id, mode.into());
            }
            ZwlrOutputHeadEvent::Enabled { enabled } => {
                info.enabled = enabled != 0;
            }
            ZwlrOutputHeadEvent::CurrentMode { mode } => {
                let mode_id = wl_id(mode);
                info.current_mode_id.replace(mode_id);
            }
            ZwlrOutputHeadEvent::Position { x, y } => {
                info.x = x;
                info.y = y;
            }
            ZwlrOutputHeadEvent::Transform { transform } => {
                info.transform.replace(transform);
            }
            ZwlrOutputHeadEvent::Scale { scale } => {
                info.scale = scale;
            }
            ZwlrOutputHeadEvent::Make { make } => {
                info.make = make;
            }
            ZwlrOutputHeadEvent::Model { model } => {
                info.model = model;
            }
            ZwlrOutputHeadEvent::SerialNumber { serial_number } => {
                info.serial_number = serial_number;
            }
            ZwlrOutputHeadEvent::Finished => {
                log::debug!("remove head with id {id}");
                self.zwlr_heads.remove(&id);
                self.zwlr_head_info.remove(&id);
            }

            _ => {}
        }
    }

    fn handle_zwlr_output_event(
        &mut self,
        _output: Main<ZwlrOutputManagerV1>,
        event: ZwlrOutputEvent,
        _ddata: DispatchData,
        inner: &Arc<Mutex<Self>>,
    ) {
        log::debug!("handle_zwlr_output_event {event:?}");
        match event {
            ZwlrOutputEvent::Head { head } => {
                let inner = Arc::clone(inner);
                head.quick_assign(move |output, event, ddata| {
                    inner
                        .lock()
                        .unwrap()
                        .handle_zwlr_head_event(output, event, ddata, &inner);
                });
                self.zwlr_heads.insert(wl_id(head.detach()), head.into());
            }
            ZwlrOutputEvent::Done { serial: _ } => {}
            ZwlrOutputEvent::Finished => {}
            _ => {}
        }
    }
}

pub struct OutputHandler {
    zwlr: Option<Attached<ZwlrOutputManagerV1>>,
    inner: Arc<Mutex<Inner>>,
}

impl OutputHandler {
    pub fn new() -> Self {
        Self {
            zwlr: None,
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    pub fn screens(&self) -> Option<Screens> {
        let inner = self.inner.lock().unwrap();

        let mut by_name = HashMap::new();
        let mut virtual_rect: ScreenRect = euclid::rect(0, 0, 0, 0);
        let config = config::configuration();

        log::debug!("zwlr_head_info: {:#?}", inner.zwlr_head_info);

        for head in inner.zwlr_head_info.values() {
            let name = head.name.clone();
            let (width, height) = match head.current_mode_id {
                Some(mode_id) => match inner.zwlr_mode_info.get(&mode_id) {
                    Some(mode) => (mode.width, mode.height),
                    None => continue,
                },
                None => continue,
            };

            let scale = head.scale;
            let rect = euclid::rect(
                head.x as isize,
                head.y as isize,
                width as isize,
                height as isize,
            );
            virtual_rect = virtual_rect.union(&rect);
            // FIXME: teach this how to resolve dpi_by_screen once
            // dispatch_pending_event knows how to do the same
            let effective_dpi = Some(config.dpi.unwrap_or(scale * crate::DEFAULT_DPI));
            by_name.insert(
                name.clone(),
                ScreenInfo {
                    name,
                    rect,
                    scale,
                    max_fps: None,
                    effective_dpi,
                },
            );
        }

        if by_name.is_empty() {
            return None;
        }

        // The main screen is the one either at the origin of
        // the virtual area, or if that doesn't exist for some weird
        // reason, the screen closest to the origin.
        let main = by_name
            .values()
            .min_by_key(|screen| {
                screen
                    .rect
                    .origin
                    .to_f32()
                    .distance_to(euclid::Point2D::origin())
                    .abs() as isize
            })?
            .clone();

        // We don't yet know how to determine the active screen,
        // so assume the main screen.
        let active = main.clone();

        Some(Screens {
            main,
            active,
            by_name,
            virtual_rect,
        })
    }
}

impl GlobalHandler<ZwlrOutputManagerV1> for OutputHandler {
    fn created(
        &mut self,
        registry: Attached<WlRegistry>,
        id: u32,
        version: u32,
        _ddata: DispatchData,
    ) {
        if !config::configuration().enable_zwlr_output_manager {
            return;
        }
        log::debug!("created ZwlrOutputManagerV1 {id} {version}");
        let zwlr = registry.bind::<ZwlrOutputManagerV1>(2, id);

        let inner = Arc::clone(&self.inner);
        zwlr.quick_assign(move |output, event, ddata| {
            inner
                .lock()
                .unwrap()
                .handle_zwlr_output_event(output, event, ddata, &inner);
        });

        self.zwlr.replace(zwlr.into());
    }

    fn get(&self) -> std::option::Option<Attached<ZwlrOutputManagerV1>> {
        self.zwlr.clone()
    }
}
