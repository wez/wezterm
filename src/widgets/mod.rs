use color::ColorAttribute;
use failure::Error;
use input::InputEvent;
use std::any::Any;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use surface::{Change, CursorShape, Position, SequenceNo, Surface};

pub mod layout;

/// Describes an event that may need to be processed by the widget
pub enum WidgetEvent {
    Input(InputEvent),
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

/// WidgetUpdate provides access to the widget and UI state during
/// a call to `Widget::update_state`
pub struct WidgetUpdate<'a, T: Widget> {
    /// The id of the current widget
    pub id: WidgetId,
    /// The id of its parent widget
    pub parent_id: Option<WidgetId>,
    /// The bounding rectangle of the widget relative to its parent
    pub rect: Rect,
    cursor: &'a RefCell<CursorShapeAndPosition>,
    surface: &'a RefCell<Surface>,
    state: &'a RefCell<Box<Any + Send>>,
    ui: &'a mut Ui,
    _phantom: PhantomData<T>,
}

/// `StateRef` borrows a reference to the widget specific state.
/// It is returned via the `WidgetUpdate::state` method.
/// It derefs to the `Widget::State` type.
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

/// `StateRefMut` borrows a mutable reference to the widget specific state.
/// It is returned via the `WidgetUpdate::state` method.
/// It mutably derefs to the `Widget::State` type.
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

impl<'a, T: Widget> WidgetUpdate<'a, T> {
    /// Borrow an immutable reference to the widget state
    pub fn state(&'a self) -> StateRef<'a, T::State> {
        StateRef {
            cell: self.state.borrow(),
            _phantom: PhantomData,
        }
    }

    /// Borrow a mutable reference to the widget state
    pub fn state_mut(&self) -> StateRefMut<'a, T::State> {
        StateRefMut {
            cell: self.state.borrow_mut(),
            _phantom: PhantomData,
        }
    }

    /// Borrow an immutable reference to the render `Surface`.
    pub fn surface(&self) -> Ref<Surface> {
        self.surface.borrow()
    }

    /// Borrow a mutable reference to the render `Surface`.
    pub fn surface_mut(&self) -> RefMut<Surface> {
        self.surface.borrow_mut()
    }

    /// Borrow a mutable reference to the cursor information.
    /// The cursor information from the focused widget is used to
    /// control the appearance of the cursor on the terminal screen
    /// during rendering.
    pub fn cursor_mut(&self) -> RefMut<CursorShapeAndPosition> {
        self.cursor.borrow_mut()
    }

    /// Iterate over events that are specified to the current widget.
    /// If the widget has focus then it will receive keyboard input
    /// events, including pastes.
    /// If the mouse is over the widget then mouse events, with the coordinates
    /// adjusted to the same coordinate space as the widget, will be included
    /// in the returned set of events.
    pub fn events(&'a self) -> impl Iterator<Item = WidgetEvent> + 'a {
        self.ui.input_queue.iter().filter_map(move |evt| {
            match evt {
                WidgetEvent::Input(InputEvent::Resized { .. }) => None,
                WidgetEvent::Input(InputEvent::Mouse(m)) => {
                    let mut m = m.clone();
                    // convert from screen to widget coords
                    let coords = self.ui.to_widget_coords(
                        self.id,
                        &ScreenRelativeCoords::new(m.x as usize, m.y as usize),
                    );
                    m.x = coords.x as u16;
                    m.y = coords.y as u16;
                    // TODO: exclude if these are outside of the rect!
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

/// Implementing the `Widget` trait allows for defining a potentially
/// interactive component in a UI layout.
/// The `State` associated with a widget is managed by the `Ui` instance.
/// The first time a given `WidgetId` is added to the `Ui`, the `init_state`
/// function is called to initialize that state, but subsequent updates will
/// use the recorded state.
/// The key method is `Widget::update_state` which processes events made
/// available via `WidgetUpdate` and applies them to the `State` and renders
/// them to the `Surface` associated with the widget.
pub trait Widget: Sized {
    /// The state maintained and managed for this widget.
    type State: Any + Send;
    /// The data returned from `update_state`.  It may simply be `()`, or
    /// `State` to return the associated state, or it may be a more complex
    /// type depending on the purpose of the widget.
    type Event;

    /// Called by the Ui the first time that a given WidgetId is seen
    /// by `build_ui_root` or `build_ui_child`.
    /// The widget shall return its initial state value.
    fn init_state(&self) -> Self::State;

    /// Called by the Ui on each update cycle.
    /// The widget should process state updates and draw itself to
    /// the surface embedded in `args`.
    fn update_state(&self, args: &mut WidgetUpdate<Self>) -> Self::Event;

    /// Override this to have your widget specify its layout constraints.
    /// You may wish to have your widget constructor receive a `Constraints`
    /// instance to make this more easily configurable in more generic widgets.
    fn get_size_constraints(&self, _state: &Self::State) -> layout::Constraints {
        Default::default()
    }

    /// Consume the widget and configure it as the root of the Ui.  The first
    /// time the widget is seen it is given the keyboard focus.
    fn build_ui_root(self, id: WidgetId, ui: &mut Ui) -> Self::Event {
        ui.add_or_update_widget(id, None, self)
    }

    /// Consume the widget and configure it as a child of the widget described
    /// by the `WidgetUpdate` argument.
    /// # Panics
    /// If `id` was previously a child of a different widget, this function will
    /// panic.
    fn build_ui_child(self, id: WidgetId, args: &mut WidgetUpdate<Self>) -> Self::Event {
        args.ui.add_or_update_widget(id, args.parent_id, self)
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

/// The `WidgetId` uniquely describes an instance of a widget.
/// Creating a new `WidgetId` generates a new unique identifier which can
/// be safely copied and moved around; each copy refers to the same widget.
/// The intent is that you set up the identifiers once and re-use them,
/// rather than generating new ids on each iteration of the UI loop so that
/// the widget state is maintained correctly by the Ui.
#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct WidgetId(usize);

impl WidgetId {
    pub fn new() -> Self {
        WidgetId(WIDGET_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed))
    }
}

struct WidgetState {
    surface: RefCell<Surface>,
    coordinates: ParentRelativeCoords,
    cursor: RefCell<CursorShapeAndPosition>,
    constraints: RefCell<layout::Constraints>,
    children: Vec<WidgetId>,
    parent: Option<WidgetId>,
    state: RefCell<Box<Any + Send>>,
}

/// Manages the widgets on the display
pub struct Ui {
    state: HashMap<WidgetId, WidgetState>,
    focused: Option<WidgetId>,
    root: Option<WidgetId>,
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

    fn add_or_update_widget<W: Widget>(
        &mut self,
        id: WidgetId,
        parent: Option<WidgetId>,
        w: W,
    ) -> W::Event {
        let widget_state = self.state.remove(&id).unwrap_or_else(|| {
            let state = w.init_state();
            let constraints = w.get_size_constraints(&state);

            WidgetState {
                surface: RefCell::new(Surface::new(80, 24)),
                coordinates: ParentRelativeCoords::new(0, 0),
                children: Vec::new(),
                parent: parent,
                state: RefCell::new(Box::new(state)),
                cursor: RefCell::new(Default::default()),
                constraints: RefCell::new(constraints),
            }
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
                // The root, if it is the first widget being added,
                // should have the focus by default
                if self.state.len() == 0 {
                    self.focused = Some(id);
                }
            }
        }

        *widget_state.constraints.borrow_mut() =
            w.get_size_constraints(widget_state.state.borrow().downcast_ref().unwrap());

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

    /// Queue up an event.  Events are processed by the appropriate
    /// `Widget::update_state` method.  Events may be re-processed to
    /// simplify handling for widgets. eg: a TODO: is to synthesize double
    /// and triple click events.
    pub fn queue_event(&mut self, event: WidgetEvent) {
        self.input_queue.push_back(event);
    }

    /// Assign keyboard focus to the specified widget.
    pub fn set_focus(&mut self, id: WidgetId) {
        self.focused = Some(id);
    }

    /// Helper for applying the surfaces from the widgets to the target
    /// screen in the correct order (from the root to the leaves)
    fn render_recursive(
        &mut self,
        id: WidgetId,
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

    /// Reconsider the layout constraints and apply them.
    /// Returns true if the layout was changed, false if no changes were made.
    fn compute_layout(&mut self, width: usize, height: usize) -> Result<bool, Error> {
        let mut layout = layout::LayoutState::new();
        let root = self.root.unwrap();
        self.add_widget_to_layout(&mut layout, root)?;
        let mut changed = false;

        for result in layout.compute_constraints(width, height, root)? {
            if let Some(widget_state) = self.state.get_mut(&result.widget) {
                let coords = ParentRelativeCoords::new(result.rect.x, result.rect.y);
                if coords != widget_state.coordinates {
                    widget_state.coordinates = coords;
                    changed = true;
                }

                let mut surface = widget_state.surface.borrow_mut();
                if (result.rect.width, result.rect.height) != surface.dimensions() {
                    surface.resize(result.rect.width, result.rect.height);
                    changed = true;
                }
            }
        }
        Ok(changed)
    }

    /// Recursive helper for building up the LayoutState
    fn add_widget_to_layout(
        &mut self,
        layout: &mut layout::LayoutState,
        widget: WidgetId,
    ) -> Result<(), Error> {
        let children = {
            let state = self.state.get(&widget).unwrap();
            layout.add_widget(widget, &state.constraints.borrow(), &state.children);
            state.children.clone()
        };
        for child in &children {
            self.add_widget_to_layout(layout, *child)?;
        }
        Ok(())
    }

    /// Apply the current state of the widgets to the screen.
    /// This has the side effect of clearing out any unconsumed input queue.
    /// Returns true if the Ui may need to be updated again; for example,
    /// if the most recent update operation changed layout.
    pub fn render_to_screen(&mut self, screen: &mut Surface) -> Result<bool, Error> {
        let root_id = self.root.unwrap();
        self.render_recursive(root_id, screen, &ScreenRelativeCoords::new(0, 0))?;
        self.input_queue.clear();
        // TODO: garbage collect unreachable WidgetId's from self.state

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

        let (width, height) = screen.dimensions();
        self.compute_layout(width, height)
    }

    /// Convert coordinates that are relative to widget into coordinates
    /// that are relative to the screen origin (top left).
    pub fn to_screen_coords(
        &self,
        widget: WidgetId,
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

    /// Convert coordinates that are relative to the screen origin (top left)
    /// into coordinates that are relative to the widget.
    pub fn to_widget_coords(
        &self,
        widget: WidgetId,
        coords: &ScreenRelativeCoords,
    ) -> ParentRelativeCoords {
        let mut x = coords.x;
        let mut y = coords.y;
        let mut widget = widget;
        loop {
            let parent = match self.state.get(&widget) {
                Some(widget_state) => {
                    x -= widget_state.coordinates.x;
                    y -= widget_state.coordinates.y;
                    match widget_state.parent {
                        Some(parent) => parent,
                        None => break,
                    }
                }
                None => break,
            };
            widget = parent;
        }
        ParentRelativeCoords { x, y }
    }
}
