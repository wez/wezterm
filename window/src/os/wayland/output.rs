//! Dealing with Wayland outputs

use crate::screen::{ScreenInfo, Screens};
use crate::ScreenRect;
use smithay_client_toolkit::globals::GlobalData;
use smithay_client_toolkit::reexports::protocols_wlr::output_management::v1::client::zwlr_output_head_v1::{ZwlrOutputHeadV1, self, Event as ZwlrOutputHeadEvent};
use smithay_client_toolkit::reexports::protocols_wlr::output_management::v1::client::zwlr_output_manager_v1::{ZwlrOutputManagerV1, self, Event as ZwlrOutputEvent};
use smithay_client_toolkit::reexports::protocols_wlr::output_management::v1::client::zwlr_output_mode_v1::{ZwlrOutputModeV1, Event as ZwlrOutputModeEvent};
use wayland_client::{Dispatch, event_created_child, Proxy};
use wayland_client::globals::{GlobalList, BindError};
use std::collections::HashMap;
use std::sync::Mutex;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_output::Transform;

use super::state::WaylandState;

#[derive(Debug, Default, Clone)]
pub struct ModeInfo {
    pub id: Option<ObjectId>,
    pub width: i32,
    pub height: i32,
    pub refresh: i32,
    pub preferred: bool,
}

#[derive(Debug, Default, Clone)]
pub struct HeadInfo {
    pub id: Option<ObjectId>,
    pub name: String,
    pub description: String,
    pub physical_width: i32,
    pub physical_height: i32,
    /// List of ids that correspond to ModeInfo's
    pub mode_ids: Vec<ObjectId>,
    pub enabled: bool,
    pub current_mode_id: Option<ObjectId>,
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
    zwlr_heads: HashMap<ObjectId, ZwlrOutputHeadV1>,
    zwlr_modes: HashMap<ObjectId, ZwlrOutputModeV1>,
    zwlr_mode_info: HashMap<ObjectId, ModeInfo>,
    zwlr_head_info: HashMap<ObjectId, HeadInfo>,
}

pub struct OutputManagerState {
    _zwlr: ZwlrOutputManagerV1,
    inner: Mutex<Inner>,
}

impl OutputManagerState {
    pub(super) fn bind(
        globals: &GlobalList,
        queue_handle: &wayland_client::QueueHandle<WaylandState>,
    ) -> Result<Self, BindError> {
        let _zwlr = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self {
            _zwlr,
            inner: Mutex::new(Inner::default()),
        })
    }

    pub fn screens(&self) -> Option<Screens> {
        let inner = self.inner.lock().unwrap();

        let mut by_name = HashMap::new();
        let mut virtual_rect: ScreenRect = euclid::rect(0, 0, 0, 0);
        let config = config::configuration();

        log::debug!("zwlr_head_info: {:#?}", inner.zwlr_head_info);

        for head in inner.zwlr_head_info.values() {
            let name = head.name.clone();
            let (width, height) = match head.current_mode_id.clone() {
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

#[derive(Default)]
pub(super) struct OutputManagerData {}

impl Dispatch<ZwlrOutputManagerV1, GlobalData, WaylandState> for OutputManagerState {
    fn event(
        state: &mut WaylandState,
        _proxy: &ZwlrOutputManagerV1,
        event: <ZwlrOutputManagerV1 as wayland_client::Proxy>::Event,
        _data: &GlobalData,
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        log::debug!("handle_zwlr_output_event {event:?}");
        let mut inner = state.output_manager.as_mut().unwrap().inner.lock().unwrap();

        match event {
            ZwlrOutputEvent::Head { head } => {
                inner.zwlr_heads.insert(head.id(), head);
            }
            _ => {}
        }
    }

    event_created_child!(WaylandState, ZwlrOutputManagerV1, [
        zwlr_output_manager_v1::EVT_HEAD_OPCODE => (ZwlrOutputHeadV1, OutputManagerData::default())
    ]);
}

impl Dispatch<ZwlrOutputHeadV1, OutputManagerData, WaylandState> for OutputManagerState {
    fn event(
        state: &mut WaylandState,
        head: &ZwlrOutputHeadV1,
        event: <ZwlrOutputHeadV1 as wayland_client::Proxy>::Event,
        _data: &OutputManagerData,
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        log::debug!("handle_zwlr_head_event {event:?}");

        let mut inner = state.output_manager.as_mut().unwrap().inner.lock().unwrap();
        let id = head.id();
        let info = inner
            .zwlr_head_info
            .entry(id.clone())
            .or_insert_with(|| HeadInfo {
                id: Some(id.clone()),
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
                let mode_id = mode.id();
                info.mode_ids.push(mode_id.clone().into());
                inner.zwlr_modes.insert(mode_id, mode.into());
            }
            ZwlrOutputHeadEvent::Enabled { enabled } => {
                info.enabled = enabled != 0;
            }
            ZwlrOutputHeadEvent::CurrentMode { mode } => {
                let mode_id = mode.id();
                info.current_mode_id.replace(mode_id);
            }
            ZwlrOutputHeadEvent::Position { x, y } => {
                info.x = x;
                info.y = y;
            }
            ZwlrOutputHeadEvent::Transform { transform } => {
                info.transform = transform.into_result().ok();
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
                inner.zwlr_heads.remove(&id);
                inner.zwlr_head_info.remove(&id);
            }
            _ => {}
        }
    }

    event_created_child!(WaylandState, ZwlrOutputModeV1, [
       zwlr_output_head_v1::EVT_CURRENT_MODE_OPCODE => (ZwlrOutputModeV1, OutputManagerData::default()),
       zwlr_output_head_v1::EVT_MODE_OPCODE => (ZwlrOutputModeV1, OutputManagerData::default()),
    ]);
}

impl Dispatch<ZwlrOutputModeV1, OutputManagerData, WaylandState> for OutputManagerState {
    fn event(
        state: &mut WaylandState,
        mode: &ZwlrOutputModeV1,
        event: <ZwlrOutputModeV1 as wayland_client::Proxy>::Event,
        _data: &OutputManagerData,
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        log::debug!("handle_zwlr_mode_event {event:?}");
        let mut inner = state.output_manager.as_mut().unwrap().inner.lock().unwrap();

        let id = mode.id();
        let info = inner
            .zwlr_mode_info
            .entry(id.clone())
            .or_insert_with(|| ModeInfo {
                id: Some(id.clone()),
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
                inner.zwlr_mode_info.remove(&id);
                inner.zwlr_modes.remove(&id);
            }
            _ => {}
        }
    }
}
