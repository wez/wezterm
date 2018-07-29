use color::ColorAttribute;
use failure::Error;
use input::InputEvent;
use std::any::Any;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap, VecDeque};
use std::fmt::{Debug, Error as FmtError, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

pub struct WidgetUpdate<'a, T: Widget2> {
    pub id: WidgetIdNr,
    pub parent_id: Option<WidgetIdNr>,
    pub rect: Rect,
    cursor: &'a RefCell<CursorShapeAndPosition>,
    surface: &'a RefCell<Surface>,
    state: &'a RefCell<Box<Any + Send>>,
    ui: &'a mut Ui,
    _phantom: PhantomData<T>,
}

pub struct StateRef<'a, T: 'a> {
    cell: Ref<'a, Box<Any + Send>>,
    _phantom: PhantomData<T>,
}

impl<'a, T: 'static> Deref for StateRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.cell.downcast_ref().unwrap()
    }
}

pub struct StateRefMut<'a, T: 'a> {
    cell: RefMut<'a, Box<Any + Send>>,
    _phantom: PhantomData<T>,
}

impl<'a, T: 'static> Deref for StateRefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.cell.downcast_ref().unwrap()
    }
}

impl<'a, T: 'static> DerefMut for StateRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.cell.downcast_mut().unwrap()
    }
}

impl<'a, T: Widget2> WidgetUpdate<'a, T> {
    pub fn state(&'a self) -> StateRef<'a, T::State> {
        StateRef {
            cell: self.state.borrow(),
            _phantom: PhantomData,
        }
    }

    pub fn surface(&self) -> Ref<Surface> {
        self.surface.borrow()
    }

    pub fn state_mut(&self) -> StateRefMut<'a, T::State> {
        StateRefMut {
            cell: self.state.borrow_mut(),
            _phantom: PhantomData,
        }
    }

    pub fn surface_mut(&self) -> RefMut<Surface> {
        self.surface.borrow_mut()
    }

    pub fn cursor_mut(&self) -> RefMut<CursorShapeAndPosition> {
        self.cursor.borrow_mut()
    }

    pub fn events(&'a self) -> impl Iterator<Item = WidgetEvent> + 'a {
        self.ui.input_queue.iter().filter_map(move |evt| {
            match evt {
                WidgetEvent::FocusLost | WidgetEvent::FocusGained => None,
                WidgetEvent::Input(InputEvent::Resized { .. }) => None,
                WidgetEvent::Input(InputEvent::Mouse(m)) => {
                    let mut m = m.clone();
                    // TODO: screen to client coords
                    Some(WidgetEvent::Input(InputEvent::Mouse(m)))
                }
                WidgetEvent::Input(InputEvent::Paste(s)) => match self.ui.focused {
                    Some(id) if id == self.id => {
                        Some(WidgetEvent::Input(InputEvent::Paste(s.clone())))
                    }
                    _ => None,
                },
                WidgetEvent::Input(InputEvent::Key(key)) => match self.ui.focused {
                    Some(id) if id == self.id => {
                        let key = key.clone();
                        Some(WidgetEvent::Input(InputEvent::Key(key)))
                    }
                    _ => None,
                },
            }
        })
    }
}

pub trait Widget2: Sized {
    type State: Any + Send;
    type Event;

    /// Called by the Ui the first time that a give WidgetIdNr is used.
    /// The widget shall return its initial state value.
    fn init_state(&self) -> Self::State;

    /// Called by the Ui on each update cycle.
    /// The widget should process state updates and draw itself to
    /// the surface embedded in `args`.
    fn update_state(&self, args: &mut WidgetUpdate<Self>) -> Self::Event;

    fn get_size_constraints(&self, _state: &Self::State) -> layout::Constraints {
        Default::default()
    }

    fn build_ui_root(self, id: WidgetIdNr, ui: &mut Ui) -> Self::Event {
        ui.add_or_update_widget(id, None, self)
    }

    fn build_ui_child(self, id: WidgetIdNr, args: &mut WidgetUpdate<Self>) -> Self::Event {
        args.ui.add_or_update_widget(id, args.parent_id, self)
    }
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

impl From<(usize, usize)> for ParentRelativeCoords {
    fn from(coords: (usize, usize)) -> ParentRelativeCoords {
        ParentRelativeCoords::new(coords.0, coords.1)
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

static WIDGET_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::ATOMIC_USIZE_INIT;

#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct WidgetIdNr(usize);

impl WidgetIdNr {
    pub fn new() -> Self {
        WidgetIdNr(WIDGET_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed))
    }
}

pub struct WidgetState {
    surface: RefCell<Surface>,
    coordinates: ParentRelativeCoords,
    cursor: RefCell<CursorShapeAndPosition>,
    constraints: RefCell<layout::Constraints>,
    children: Vec<WidgetIdNr>,
    parent: Option<WidgetIdNr>,
    state: RefCell<Box<Any + Send>>,
}

pub struct Ui {
    state: HashMap<WidgetIdNr, WidgetState>,
    focused: Option<WidgetIdNr>,
    root: Option<WidgetIdNr>,
    input_queue: VecDeque<WidgetEvent>,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            state: HashMap::new(),
            focused: None,
            root: None,
            input_queue: VecDeque::new(),
        }
    }

    fn add_or_update_widget<W: Widget2>(
        &mut self,
        id: WidgetIdNr,
        parent: Option<WidgetIdNr>,
        w: W,
    ) -> W::Event {
        let widget_state = self.state.remove(&id).unwrap_or_else(|| WidgetState {
            surface: RefCell::new(Surface::new(80, 24)),
            coordinates: ParentRelativeCoords::new(0, 0),
            children: Vec::new(),
            parent: parent,
            state: RefCell::new(Box::new(w.init_state())),
            cursor: RefCell::new(Default::default()),
            constraints: RefCell::new(Default::default()),
        });

        // Dealing with re-parenting is a PITA, so we just panic for now
        assert_eq!(parent, widget_state.parent);

        // Ensure that the parent links to this child
        match widget_state.parent {
            Some(parent_id) => match self.state.get_mut(&parent_id) {
                Some(parent) => {
                    if !parent.children.contains(&id) {
                        parent.children.push(id);
                    }
                }
                None => panic!("missing parent!"),
            },
            None => {
                // It has no parent, therefore it is the root
                self.root = Some(id);
            }
        }

        *widget_state.constraints.borrow_mut() =
            w.get_size_constraints(widget_state.state.borrow().downcast_ref().unwrap());

        // TODO: incrementally update the layout solver

        let coords = widget_state.coordinates;
        let dims = widget_state.surface.borrow().dimensions();

        let event = {
            let mut args = WidgetUpdate {
                id,
                parent_id: None,
                rect: Rect {
                    x: coords.x,
                    y: coords.y,
                    width: dims.0,
                    height: dims.1,
                },
                surface: &widget_state.surface,
                state: &widget_state.state,
                cursor: &widget_state.cursor,
                ui: self,
                _phantom: PhantomData,
            };
            w.update_state(&mut args)
        };

        self.state.insert(id, widget_state);

        event
    }

    pub fn queue_event(&mut self, event: WidgetEvent) {
        self.input_queue.push_back(event);
    }

    pub fn set_focus(&mut self, id: WidgetIdNr) {
        self.focused = Some(id);
    }

    fn render_recursive(
        &mut self,
        id: WidgetIdNr,
        screen: &mut Surface,
        abs_coords: &ScreenRelativeCoords,
    ) -> Result<(), Error> {
        let (child_ids, abs_coords) = match self.state.get_mut(&id) {
            Some(widget_state) => {
                let abs_coords = ScreenRelativeCoords::new(
                    widget_state.coordinates.x + abs_coords.x,
                    widget_state.coordinates.y + abs_coords.y,
                );
                screen.draw_from_screen(
                    &widget_state.surface.borrow_mut(),
                    abs_coords.x,
                    abs_coords.y,
                );
                widget_state
                    .surface
                    .borrow_mut()
                    .flush_changes_older_than(SequenceNo::max_value());

                (widget_state.children.clone(), abs_coords)
            }
            None => bail!("missing state for widget {:?}", id),
        };

        for child in child_ids {
            self.render_recursive(child, screen, &abs_coords)?;
        }

        Ok(())
    }

    pub fn render_to_screen(&mut self, screen: &mut Surface) -> Result<(), Error> {
        let root_id = self.root.unwrap();
        self.render_recursive(root_id, screen, &ScreenRelativeCoords::new(0, 0))?;
        self.input_queue.clear();

        match self.focused {
            Some(id) => match self.state.get(&id) {
                Some(widget_state) => {
                    let cursor = widget_state.cursor.borrow();
                    let coords = self.to_screen_coords(id, &cursor.coords);

                    screen.add_changes(vec![
                        Change::CursorShape(cursor.shape),
                        Change::CursorColor(cursor.color),
                        Change::CursorPosition {
                            x: Position::Absolute(coords.x),
                            y: Position::Absolute(coords.y),
                        },
                    ]);
                }
                _ => {}
            },
            _ => {}
        }

        Ok(())
    }

    pub fn to_screen_coords(
        &self,
        widget: WidgetIdNr,
        coords: &ParentRelativeCoords,
    ) -> ScreenRelativeCoords {
        let mut x = coords.x;
        let mut y = coords.y;
        let mut widget = widget;
        loop {
            let parent = match self.state.get(&widget) {
                Some(widget_state) => {
                    x += widget_state.coordinates.x;
                    y += widget_state.coordinates.y;
                    match widget_state.parent {
                        Some(parent) => parent,
                        None => break,
                    }
                }
                None => break,
            };
            widget = parent;
        }
        ScreenRelativeCoords { x, y }
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
        let surface = Surface::new(1, 1);
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
