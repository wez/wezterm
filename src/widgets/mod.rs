use color::ColorAttribute;
use failure::Error;
use input::InputEvent;
use std::cell::{Ref, RefCell, RefMut};
use std::fmt::{Debug, Error as FmtError, Formatter};
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};
use surface::{Change, CursorShape, Position, SequenceNo, Surface};

pub mod layout;

/// Describes an event that may need to be processed by the widget
pub enum WidgetEvent {
    Input(InputEvent),
    FocusLost,
    FocusGained,
}

pub enum EventDisposition {
    /// Allow the event to bubble up through the containing hierarchy
    Propagate,
    /// The widget processed the event and further processing should cease
    Stop,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CursorShapeAndPosition {
    pub shape: CursorShape,
    pub coords: ParentRelativeCoords,
    pub color: ColorAttribute,
}

pub trait WidgetImpl {
    /// Called once by the widget manager to inform the widget
    /// of its identifier
    fn set_widget_id(&mut self, _id: WidgetId) {}

    /// Handle an event
    fn process_event(&mut self, _event: &WidgetEvent) -> EventDisposition {
        EventDisposition::Propagate
    }

    /// Interrogates the widget to ask if it has any sizing constraints
    fn get_size_constraints(&self) -> layout::Constraints {
        Default::default()
    }

    fn render_to_surface(&self, surface: &mut Surface);

    /// Called for the focused widget to determine how to render
    /// the cursor.
    fn get_cursor_shape_and_position(&self) -> CursorShapeAndPosition {
        Default::default()
    }
}

/// Relative to the top left of the parent container
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParentRelativeCoords {
    pub x: usize,
    pub y: usize,
}

impl ParentRelativeCoords {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

/// Relative to the top left of the screen
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScreenRelativeCoords {
    pub x: usize,
    pub y: usize,
}

impl ScreenRelativeCoords {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    pub fn offset_by(&self, rel: &ParentRelativeCoords) -> Self {
        Self {
            x: self.x + rel.x,
            y: self.y + rel.y,
        }
    }
}

pub struct Widget {
    id: WidgetId,
    inner: Box<WidgetImpl>,
    surface: Surface,
    coordinates: ParentRelativeCoords,
    children: Vec<WidgetHandle>,
    parent: WidgetId,
}

#[derive(Clone)]
pub struct WidgetHandle {
    inner: Rc<RefCell<Widget>>,
}

impl WidgetHandle {
    pub fn borrow(&self) -> Ref<Widget> {
        self.inner.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<Widget> {
        self.inner.borrow_mut()
    }

    pub fn new(widget: Widget) -> Self {
        Self {
            inner: Rc::new(RefCell::new(widget)),
        }
    }

    pub fn id(&self) -> WidgetId {
        WidgetId {
            inner: Rc::downgrade(&self.inner),
        }
    }
}

impl PartialEq for WidgetHandle {
    fn eq(&self, other: &WidgetHandle) -> bool {
        self.inner.as_ptr() == other.inner.as_ptr()
    }
}

impl Eq for WidgetHandle {}

impl Hash for WidgetHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.as_ptr().hash(state)
    }
}

impl Debug for WidgetHandle {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        write!(f, "WidgetHandle({:?})", self.inner.as_ptr())
    }
}

/// An identifier assigned by the widget management machinery.
#[derive(Clone)]
pub struct WidgetId {
    inner: Weak<RefCell<Widget>>,
}

impl WidgetId {
    pub fn new() -> Self {
        Self { inner: Weak::new() }
    }

    pub fn handle(&self) -> Option<WidgetHandle> {
        self.inner.upgrade().map(|r| WidgetHandle { inner: r })
    }
}

impl Widget {
    pub fn new<W: WidgetImpl + 'static>(widget: W) -> WidgetHandle {
        let widget = Box::new(widget);
        let (width, height) = (80, 24); //widget.get_size_constraints().deduce_initial_size();
        let surface = Surface::new(width, height);
        let coordinates = ParentRelativeCoords::new(0, 0);
        let children = Vec::new();
        let id = WidgetId::new();
        let handle = WidgetHandle::new(Widget {
            id,
            inner: widget,
            surface,
            coordinates,
            children,
            parent: WidgetId::new(),
        });
        let id = handle.id();
        {
            let mut widget = handle.borrow_mut();
            widget.id = id.clone();
            widget.inner.set_widget_id(id);
        }
        handle
    }

    pub fn widget_id(&self) -> WidgetId {
        self.id.clone()
    }

    pub fn process_event(&mut self, event: &WidgetEvent) -> EventDisposition {
        self.inner.process_event(event)
    }

    pub fn get_size_constraints(&self) -> layout::Constraints {
        self.inner.get_size_constraints()
    }

    pub fn render_to_screen(&mut self, screen: &mut Surface) {
        self.inner.render_to_surface(&mut self.surface);
        for child in &mut self.children {
            child.borrow_mut().render_to_screen(&mut self.surface);
        }
        self.surface
            .flush_changes_older_than(SequenceNo::max_value());
        screen.draw_from_screen(&self.surface, self.coordinates.x, self.coordinates.y);
    }

    pub fn get_size_and_position(&self) -> (usize, usize, ParentRelativeCoords) {
        let (width, height) = self.surface.dimensions();
        (width, height, self.coordinates)
    }

    pub fn set_size_and_position(
        &mut self,
        width: usize,
        height: usize,
        coords: ParentRelativeCoords,
    ) {
        self.surface.resize(width, height);
        self.coordinates = coords;
    }

    pub fn parent(&self) -> Option<WidgetHandle> {
        self.parent.handle()
    }

    pub fn add_child(&mut self, widget: &WidgetHandle) {
        self.children.push(widget.clone());
    }

    pub fn to_screen_coords(&self, coords: &ParentRelativeCoords) -> ScreenRelativeCoords {
        let mut x = coords.x;
        let mut y = coords.y;
        let mut widget = self.parent();
        loop {
            let parent = match widget {
                Some(parent) => {
                    let p = parent.borrow();
                    x += p.coordinates.x;
                    y += p.coordinates.y;
                    p.parent()
                }
                None => break,
            };
            widget = parent;
        }
        ScreenRelativeCoords { x, y }
    }
}

pub struct Screen {
    focused_widget: WidgetHandle,
    root_widget: WidgetHandle,
}

impl Screen {
    pub fn new(root_widget: WidgetHandle) -> Self {
        let focused_widget = root_widget.clone();
        Self {
            focused_widget,
            root_widget,
        }
    }

    /// Change the focused widget using the new widget handle
    pub fn set_focus(&mut self, widget: WidgetHandle) {
        self.focused_widget
            .borrow_mut()
            .process_event(&WidgetEvent::FocusLost);
        self.focused_widget = widget;
        self.focused_widget
            .borrow_mut()
            .process_event(&WidgetEvent::FocusGained);
    }

    /// Route an event to an appropriate widget.
    /// Routing starts with the focused widget and then propagates
    /// up through its parents until we reach the root widget.
    pub fn route_event(&self, event: &WidgetEvent) {
        let mut widget = self.focused_widget.clone();
        loop {
            match widget.borrow_mut().process_event(event) {
                EventDisposition::Stop => return,
                EventDisposition::Propagate => {}
            }

            let parent = match widget.borrow().parent() {
                Some(p) => p,
                None => return,
            };

            widget = parent;
        }
    }

    /// Rendering starts at the root (which can be considered the lowest layer),
    /// and then progresses up through its children.
    pub fn render_to_screen(&mut self, screen: &mut Surface) -> Result<(), Error> {
        let (width, height) = screen.dimensions();
        layout::LayoutState::compute_layout(width, height, &self.root_widget)?;

        self.root_widget.borrow_mut().render_to_screen(screen);

        let focused = self.focused_widget.borrow();
        let cursor = focused.inner.get_cursor_shape_and_position();
        let coords = focused.to_screen_coords(&cursor.coords);

        screen.add_changes(vec![
            Change::CursorShape(cursor.shape),
            Change::CursorColor(cursor.color),
            Change::CursorPosition {
                x: Position::Absolute(coords.x),
                y: Position::Absolute(coords.y),
            },
        ]);

        Ok(())
    }
}
