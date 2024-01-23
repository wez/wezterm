use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use smithay_client_toolkit::compositor::CompositorState;
use smithay_client_toolkit::data_device_manager::data_device::DataDevice;
use smithay_client_toolkit::data_device_manager::data_source::CopyPasteSource;
use smithay_client_toolkit::data_device_manager::DataDeviceManagerState;
use smithay_client_toolkit::globals::GlobalData;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::ThemedPointer;
use smithay_client_toolkit::seat::SeatState;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_data_device, delegate_data_device_manager, delegate_data_offer,
    delegate_data_source, delegate_output, delegate_registry, delegate_seat, delegate_shm,
    delegate_xdg_shell, delegate_xdg_window, registry_handlers,
};
use wayland_client::backend::ObjectId;
use wayland_client::globals::GlobalList;
use wayland_client::protocol::wl_keyboard::WlKeyboard;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_pointer::WlPointer;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{delegate_dispatch, Connection, QueueHandle};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3;

use crate::x11::KeyboardWithFallback;

use super::inputhandler::{TextInputData, TextInputState};
use super::pointer::{PendingMouse, PointerUserData};
use super::{SurfaceUserData, WaylandWindowInner};

// We can't combine WaylandState and WaylandConnection together because
// the run_message_loop has &self(WaylandConnection) and needs to update WaylandState as mut
pub(super) struct WaylandState {
    registry: RegistryState,
    pub(super) output: OutputState,
    pub(super) compositor: CompositorState,
    pub(super) text_input: TextInputState,
    pub(super) seat: SeatState,
    pub(super) xdg: XdgShell,
    pub(super) windows: RefCell<HashMap<usize, Rc<RefCell<WaylandWindowInner>>>>,

    pub(super) active_surface_id: RefCell<Option<ObjectId>>,
    pub(super) last_serial: RefCell<u32>,
    pub(super) keyboard: Option<WlKeyboard>,
    pub(super) keyboard_mapper: Option<KeyboardWithFallback>,
    pub(super) key_repeat_delay: i32,
    pub(super) key_repeat_rate: i32,
    pub(super) keyboard_window_id: Option<usize>,

    pub(super) pointer: Option<ThemedPointer<PointerUserData>>,
    pub(super) surface_to_pending: HashMap<ObjectId, Arc<Mutex<PendingMouse>>>,

    pub(super) data_device_manager_state: DataDeviceManagerState,
    pub(super) data_device: Option<DataDevice>,
    pub(super) copy_paste_source: Option<(CopyPasteSource, String)>,
    pub(super) shm: Shm,
    pub(super) mem_pool: RefCell<SlotPool>,
}

impl WaylandState {
    pub(super) fn new(globals: &GlobalList, qh: &QueueHandle<Self>) -> anyhow::Result<Self> {
        let shm = Shm::bind(&globals, qh)?;
        let mem_pool = SlotPool::new(1, &shm)?;
        let wayland_state = WaylandState {
            registry: RegistryState::new(globals),
            output: OutputState::new(globals, qh),
            compositor: CompositorState::bind(globals, qh)?,
            text_input: TextInputState::bind(globals, qh)?,
            windows: RefCell::new(HashMap::new()),
            seat: SeatState::new(globals, qh),
            xdg: XdgShell::bind(globals, qh)?,
            active_surface_id: RefCell::new(None),
            last_serial: RefCell::new(0),
            keyboard: None,
            keyboard_mapper: None,
            key_repeat_rate: 25,
            key_repeat_delay: 400,
            keyboard_window_id: None,
            pointer: None,
            surface_to_pending: HashMap::new(),
            data_device_manager_state: DataDeviceManagerState::bind(globals, qh)?,
            data_device: None,
            copy_paste_source: None,
            shm,
            mem_pool: RefCell::new(mem_pool),
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
        todo!()
    }

    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {
        log::trace!("output destroyed: OutputHandler");
        todo!()
    }
}
// Undocumented in sctk 0.17: This is required to use have user data with a surface
// Will be just delegate_compositor!(WaylandState, surface: [SurfaceData, SurfaceUserData]) in 0.18
delegate_dispatch!(WaylandState: [ WlSurface: SurfaceUserData] => CompositorState);

delegate_registry!(WaylandState);

delegate_shm!(WaylandState);

delegate_output!(WaylandState);
delegate_compositor!(WaylandState);

delegate_seat!(WaylandState);

delegate_data_device_manager!(WaylandState);
delegate_data_device!(WaylandState);
delegate_data_source!(WaylandState);
delegate_data_offer!(WaylandState);

// Updating to 0.18 should have this be able to work
// delegate_pointer!(WaylandState, pointer: [PointerUserData]);
delegate_dispatch!(WaylandState: [WlPointer: PointerUserData] => SeatState);

delegate_xdg_shell!(WaylandState);
delegate_xdg_window!(WaylandState);

delegate_dispatch!(WaylandState: [ZwpTextInputManagerV3: GlobalData] => TextInputState);
delegate_dispatch!(WaylandState: [ZwpTextInputV3: TextInputData] => TextInputState);
