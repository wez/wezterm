use input::InputEvent;
use std::cell::{Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};
use surface::{SequenceNo, Surface};

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
pub struct SizeConstraints {
    pub min_width: Option<usize>,
    pub min_height: Option<usize>,
    pub max_width: Option<usize>,
    pub max_height: Option<usize>,
    pub width: Option<usize>,
    pub height: Option<usize>,
}

impl SizeConstraints {
    pub fn deduce_initial_size(&self) -> (usize, usize) {
        match (self.width, self.height) {
            (Some(w), Some(h)) => return (w, h),
            _ => {}
        }
        match (self.max_width, self.max_height) {
            (Some(w), Some(h)) => return (w, h),
            _ => {}
        }
        match (self.min_width, self.min_height) {
            (Some(w), Some(h)) => return (w, h),
            _ => {}
        }

        (80, 24)
    }
}

pub trait WidgetImpl {
    /// Called once by the widget manager to inform the widget
    /// of its identifier
    fn set_widget_id(&mut self, id: WidgetId);

    /// Handle an event
    fn process_event(&mut self, event: &WidgetEvent) -> EventDisposition;

    /// Interrogates the widget to ask if it has any sizing constraints
    fn get_size_constraints(&self) -> SizeConstraints;

    fn render_to_surface(&self, surface: &mut Surface);
}

/// Relative to the top left of the parent container
pub struct ParentRelativeCoords {
    x: usize,
    y: usize,
}

impl ParentRelativeCoords {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

/// Relative to the top left of the screen
pub struct ScreenRelativeCoords {
    x: usize,
    y: usize,
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
    pub fn new(widget: Box<WidgetImpl>) -> WidgetHandle {
        let (width, height) = widget.get_size_constraints().deduce_initial_size();
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

    pub fn render_to_screen(&mut self, screen: &mut Surface) {
        self.inner.render_to_surface(&mut self.surface);
        for child in &mut self.children {
            child.borrow_mut().render_to_screen(&mut self.surface);
        }
        self.surface
            .flush_changes_older_than(SequenceNo::max_value());
        screen.draw_from_screen(&self.surface, self.coordinates.x, self.coordinates.y);
    }

    pub fn parent(&self) -> Option<WidgetHandle> {
        self.parent.handle()
    }

    pub fn add_child(&mut self, widget: &WidgetHandle) {
        self.children.push(widget.clone());
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
    pub fn render_to_screen(&mut self, screen: &mut Surface) {
        self.root_widget.borrow_mut().render_to_screen(screen);
    }
}
