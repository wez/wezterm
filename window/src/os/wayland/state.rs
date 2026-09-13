use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use smithay_client_toolkit::activation::{ActivationHandler, ActivationState, RequestDataExt};
use smithay_client_toolkit::compositor::{CompositorState, SurfaceData};
use smithay_client_toolkit::data_device_manager::data_device::DataDevice;
use smithay_client_toolkit::data_device_manager::data_source::CopyPasteSource;
use smithay_client_toolkit::data_device_manager::DataDeviceManagerState;
use smithay_client_toolkit::globals::GlobalData;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::primary_selection::device::PrimarySelectionDevice;
use smithay_client_toolkit::primary_selection::selection::PrimarySelectionSource;
use smithay_client_toolkit::primary_selection::PrimarySelectionManagerState;
use smithay_client_toolkit::reexports::protocols_wlr::output_management::v1::client::zwlr_output_head_v1::ZwlrOutputHeadV1;
use smithay_client_toolkit::reexports::protocols_wlr::output_management::v1::client::zwlr_output_manager_v1::ZwlrOutputManagerV1;
use smithay_client_toolkit::reexports::protocols_wlr::output_management::v1::client::zwlr_output_mode_v1::ZwlrOutputModeV1;
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::ThemedPointer;
use smithay_client_toolkit::seat::SeatState;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::subcompositor::SubcompositorState;
use smithay_client_toolkit::{
    delegate_activation, delegate_compositor, delegate_data_device, delegate_output, delegate_pointer, delegate_primary_selection, delegate_registry, delegate_seat, delegate_shm, delegate_subcompositor, delegate_xdg_shell, delegate_xdg_window, registry_handlers
};
use wayland_client::backend::ObjectId;
use wayland_client::globals::GlobalList;
use wayland_client::protocol::wl_keyboard::WlKeyboard;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{delegate_dispatch, Connection, QueueHandle};
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3;
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager;

use crate::x11::KeyboardWithFallback;

use super::copy_and_paste::CopyPasteOffer;
use super::inputhandler::{TextInputData, TextInputState};
use super::pointer::{PendingMouse, PointerUserData};
use super::{OutputManagerData, OutputManagerState, SurfaceUserData, WaylandWindowInner};

// We can't combine WaylandState and WaylandConnection together because
// the run_message_loop has &self(WaylandConnection) and needs to update WaylandState as mut
pub(super) struct WaylandState {
    registry: RegistryState,
    pub(super) output: OutputState,
    pub(super) compositor: CompositorState,
    pub(super) subcompositor: Arc<SubcompositorState>,
    pub(super) text_input: Option<TextInputState>,
    pub(super) output_manager: Option<OutputManagerState>,
    pub(super) seat: SeatState,
    pub(super) xdg: XdgShell,
    pub(super) activation: Option<ActivationState>,
    pub(super) windows: RefCell<HashMap<usize, Rc<RefCell<WaylandWindowInner>>>>,

    pub(super) active_surface_id: RefCell<Option<ObjectId>>,
    pub(super) last_serial: RefCell<u32>,
    pub(super) last_press_serial: RefCell<u32>,
    pub(super) focused_surface: RefCell<Option<WlSurface>>,
    pub(super) keyboard: Option<WlKeyboard>,
    pub(super) keyboard_mapper: Option<KeyboardWithFallback>,
    pub(super) key_repeat_delay: i32,
    pub(super) key_repeat_rate: i32,
    pub(super) keyboard_window_id: Option<usize>,

    pub(super) pointer: Option<ThemedPointer<PointerUserData>>,
    pub(super) surface_to_pending: HashMap<ObjectId, Arc<Mutex<PendingMouse>>>,

    /// Bound global for wl_data_device_manager; factory for data devices and copy-paste sources.
    pub(super) data_device_manager_state: DataDeviceManagerState,
    /// Protocol object for clipboard and drag-and-drop.
    /// None until first seat capability event is received.
    pub(super) data_device: Option<DataDevice>,
    /// Most recent incoming clipboard offer from another Wayland client (regular clipboard).
    /// Updated on every `wl_data_device::selection` event; read when pasting data.
    pub(super) copy_paste_offer: Arc<Mutex<CopyPasteOffer>>,
    /// Outgoing regular clipboard: our data source while we own clipboard selection.
    /// Dropped when another Wayland client takes ownership.
    pub(super) copy_paste_source: Option<(CopyPasteSource, String)>,
    /// Protocol manager for primary selection.
    /// None if unsupported by compositor.
    pub(super) primary_selection_manager: Option<PrimarySelectionManagerState>,
    /// Protocol object for primary selection device.
    /// None if unsupported by compositor, or until first seat capability event is received.
    pub(super) primary_selection_device: Option<PrimarySelectionDevice>,
    /// Outgoing primary selection: our source while we own primary selection.
    /// Dropped when another client takes ownership.
    pub(super) primary_selection_source: Option<(PrimarySelectionSource, String)>,

    pub(super) shm: Shm,
    pub(super) mem_pool: RefCell<SlotPool>,
    pub(super) kde_blur_manager: Option<OrgKdeKwinBlurManager>,
    pub(super) ext_background_effect_manager: Option<ExtBackgroundEffectManagerV1>,
    pub(super) ext_background_effect_can_blur: bool,
}

impl WaylandState {
    pub(super) fn new(globals: &GlobalList, qh: &QueueHandle<Self>) -> anyhow::Result<Self> {
        let shm = Shm::bind(&globals, qh)?;
        let mem_pool = SlotPool::new(1, &shm)?;

        let compositor = CompositorState::bind(globals, qh)?;
        let subcompositor =
            SubcompositorState::bind(compositor.wl_compositor().clone(), globals, qh)?;

        let kde_blur_manager: Option<OrgKdeKwinBlurManager> =
            globals.bind(qh, 1..=1, GlobalData).ok();
        let ext_background_effect_manager: Option<ExtBackgroundEffectManagerV1> =
            globals.bind(qh, 1..=1, GlobalData).ok();
        let wayland_state = WaylandState {
            registry: RegistryState::new(globals),
            output: OutputState::new(globals, qh),
            compositor,
            subcompositor: Arc::new(subcompositor),
            text_input: TextInputState::bind(globals, qh).ok(),
            output_manager: if config::configuration().enable_zwlr_output_manager {
                Some(OutputManagerState::bind(globals, qh)?)
            } else {
                None
            },
            windows: RefCell::new(HashMap::new()),
            seat: SeatState::new(globals, qh),
            xdg: XdgShell::bind(globals, qh)?,
            activation: ActivationState::bind(globals, qh).ok(),
            active_surface_id: RefCell::new(None),
            last_serial: RefCell::new(0),
            last_press_serial: RefCell::new(0),
            focused_surface: RefCell::new(None),
            keyboard: None,
            keyboard_mapper: None,
            key_repeat_rate: 25,
            key_repeat_delay: 400,
            keyboard_window_id: None,
            pointer: None,
            surface_to_pending: HashMap::new(),
            data_device_manager_state: DataDeviceManagerState::bind(globals, qh)?,
            data_device: None,
            copy_paste_offer: CopyPasteOffer::create(),
            copy_paste_source: None,
            primary_selection_manager: PrimarySelectionManagerState::bind(globals, qh).ok(),
            primary_selection_device: None,
            primary_selection_source: None,
            shm,
            mem_pool: RefCell::new(mem_pool),
            kde_blur_manager,
            ext_background_effect_manager,
            ext_background_effect_can_blur: false,
        };
        Ok(wayland_state)
    }
}

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }

    registry_handlers![OutputState, SeatState];
}

pub(super) struct ActivationRequest {
    pub(super) app_id: String,
    pub(super) seat_and_serial: Option<(WlSeat, u32)>,
    pub(super) requesting_surface: Option<WlSurface>,
    pub(super) surface_to_activate: WlSurface,
}

impl RequestDataExt for ActivationRequest {
    fn app_id(&self) -> Option<&str> {
        Some(&self.app_id)
    }

    fn seat_and_serial(&self) -> Option<(&WlSeat, u32)> {
        self.seat_and_serial
            .as_ref()
            .map(|(seat, serial)| (seat, *serial))
    }

    fn surface(&self) -> Option<&WlSurface> {
        self.requesting_surface.as_ref()
    }
}

impl ActivationHandler for WaylandState {
    type RequestData = ActivationRequest;

    fn new_token(&mut self, token: String, data: &Self::RequestData) {
        let Some(activation) = self.activation.as_ref() else {
            return;
        };
        // A refused request still yields a token; undetectable here.
        activation.activate::<Self>(&data.surface_to_activate, token);
    }
}

impl ShmHandler for WaylandState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output
    }

    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {
        log::trace!("new output: OutputHandler");
    }

    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {
        log::trace!("update output: OutputHandler");
    }

    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {
        log::trace!("output destroyed: OutputHandler");
    }
}

delegate_registry!(WaylandState);

delegate_shm!(WaylandState);

delegate_output!(WaylandState);
delegate_compositor!(WaylandState, surface: [SurfaceData, SurfaceUserData]);
delegate_subcompositor!(WaylandState);

delegate_seat!(WaylandState);

delegate_data_device!(WaylandState);

delegate_pointer!(WaylandState, pointer: [PointerUserData]);

delegate_activation!(WaylandState, ActivationRequest);

delegate_xdg_shell!(WaylandState);
delegate_xdg_window!(WaylandState);

delegate_primary_selection!(WaylandState);

delegate_dispatch!(WaylandState: [ZwpTextInputManagerV3: GlobalData] => TextInputState);
delegate_dispatch!(WaylandState: [ZwpTextInputV3: TextInputData] => TextInputState);

delegate_dispatch!(WaylandState: [ZwlrOutputManagerV1: GlobalData] => OutputManagerState);
delegate_dispatch!(WaylandState: [ZwlrOutputHeadV1: OutputManagerData] => OutputManagerState);
delegate_dispatch!(WaylandState: [ZwlrOutputModeV1: OutputManagerData] => OutputManagerState);
