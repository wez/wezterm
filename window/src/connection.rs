use crate::screen::Screens;
use crate::{Appearance, Connection, GeometryOrigin, RequestedWindowGeometry, ResolvedGeometry};
use anyhow::Result as Fallible;
use config::keyassignment::KeyAssignment;
use config::DimensionContext;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

thread_local! {
    static CONN: RefCell<Option<Rc<Connection>>> = RefCell::new(None);
}

fn nop_event_handler(_event: ApplicationEvent) {}

static EVENT_HANDLER: Mutex<fn(ApplicationEvent)> = Mutex::new(nop_event_handler);

pub fn shutdown() {
    CONN.with(|m| drop(m.borrow_mut().take()));
}

#[derive(Debug)]
pub enum ApplicationEvent {
    /// The system wants to open a command in the terminal
    OpenCommandScript(String),
    PerformKeyAssignment(KeyAssignment),
}

pub trait ConnectionOps {
    fn get() -> Option<Rc<Connection>> {
        let mut res = None;
        CONN.with(|m| {
            if let Some(mux) = &*m.borrow() {
                res = Some(Rc::clone(mux));
            }
        });
        res
    }

    fn name(&self) -> String;

    fn set_event_handler(&self, func: fn(ApplicationEvent)) {
        let mut handler = EVENT_HANDLER.lock().unwrap();
        *handler = func;
    }

    fn dispatch_app_event(&self, event: ApplicationEvent) {
        let func = EVENT_HANDLER.lock().unwrap();
        func(event);
    }

    fn default_dpi(&self) -> f64 {
        crate::DEFAULT_DPI
    }

    fn init() -> Fallible<Rc<Connection>> {
        let conn = Rc::new(Connection::create_new()?);
        CONN.with(|m| *m.borrow_mut() = Some(Rc::clone(&conn)));
        crate::spawn::SPAWN_QUEUE.register_promise_schedulers();
        Ok(conn)
    }

    fn terminate_message_loop(&self);
    fn run_message_loop(&self) -> Fallible<()>;

    /// Retrieve the current appearance for the application.
    fn get_appearance(&self) -> Appearance {
        Appearance::Light
    }

    /// Hide the application.
    /// This actions hides all of the windows of the application and switches
    /// focus away from it.
    fn hide_application(&self) {}

    /// Perform the system beep/notification sound
    fn beep(&self) {}

    /// Returns information about the screens
    fn screens(&self) -> anyhow::Result<Screens> {
        anyhow::bail!("Unable to query screen information");
    }

    fn resolve_geometry(&self, geometry: RequestedWindowGeometry) -> ResolvedGeometry {
        let bounds = match self.screens() {
            Ok(screens) => {
                log::warn!("ConnectionOps.resolve_geometry {screens:?}");

                match geometry.origin {
                    GeometryOrigin::ScreenCoordinateSystem => screens.virtual_rect,
                    GeometryOrigin::MainScreen => screens.main.rect,
                    GeometryOrigin::ActiveScreen => screens.active.rect,
                    GeometryOrigin::Named(name) => match screens.by_name.get(&name) {
                        Some(info) => info.rect,
                        None => {
                            log::error!(
                            "Requested display {} was not found; available displays are: {:?}. \
                             Using primary display instead",
                            name,
                            screens.by_name,
                        );
                            screens.main.rect
                        }
                    },
                }
            }
            Err(_) => euclid::rect(0, 0, 65535, 65535),
        };

        let dpi = self.default_dpi();
        let width_context = DimensionContext {
            dpi: dpi as f32,
            pixel_max: bounds.width() as f32,
            pixel_cell: bounds.width() as f32,
        };
        let height_context = DimensionContext {
            dpi: dpi as f32,
            pixel_max: bounds.height() as f32,
            pixel_cell: bounds.height() as f32,
        };
        let width = geometry.width.evaluate_as_pixels(width_context) as usize;
        let height = geometry.height.evaluate_as_pixels(height_context) as usize;
        let x = geometry
            .x
            .map(|x| x.evaluate_as_pixels(width_context) as i32 + bounds.origin.x as i32);
        let y = geometry
            .y
            .map(|y| y.evaluate_as_pixels(height_context) as i32 + bounds.origin.y as i32);

        ResolvedGeometry {
            x,
            y,
            width,
            height,
        }
    }
}
