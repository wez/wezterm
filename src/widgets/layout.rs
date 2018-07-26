//! This module provides some automatic layout functionality for widgets.
//! The parameters are similar to those that you may have encountered
//! in HTML, but do not fully recreate the layout model.
use cassowary::strength::{REQUIRED, STRONG, WEAK};
use cassowary::WeightedRelation::*;
use cassowary::{AddConstraintError, Expression, Solver, SuggestValueError, Variable};
use failure::{err_msg, Error};
use std::collections::HashMap;

use widgets::{ParentRelativeCoords, WidgetHandle};

/// Expands to an Expression holding the value of the variable,
/// or if there is no variable, a constant with the specified
/// value.
/// Equivalent to Option::unwrap_or().
fn unwrap_variable_or(var: Option<Variable>, value: f64) -> Expression {
    // The `v + 0.0` portion "converts" the variable to an Expression.
    var.map(|v| v + 0.0)
        .unwrap_or_else(|| Expression::from_constant(value))
}

/// Specify whether a width or a height has a preferred fixed size
/// or whether it should occupy a percentage of its parent container.
/// The default is 100% of the parent container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimensionSpec {
    /// Occupy a fixed number of cells
    Fixed(u16),
    /// Occupy a percentage of the space in the parent container
    Percentage(u8),
}

impl Default for DimensionSpec {
    fn default() -> Self {
        DimensionSpec::Percentage(100)
    }
}

/// Specifies the extent of a width or height.  The `spec` field
/// holds the preferred size, while the `minimum` and `maximum`
/// fields set optional lower and upper bounds.
#[derive(Clone, Default, Copy, Debug, PartialEq, Eq)]
pub struct Dimension {
    pub spec: DimensionSpec,
    pub maximum: Option<u16>,
    pub minimum: Option<u16>,
}

/// Specifies whether the children of a widget are laid out
/// vertically (top to bottom) or horizontally (left to right).
/// The default is horizontal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildOrientation {
    Vertical,
    Horizontal,
}

impl Default for ChildOrientation {
    fn default() -> Self {
        ChildOrientation::Horizontal
    }
}

/// Specifies whether the widget should be aligned to the top,
/// middle or bottom of the vertical space in its parent.
/// The default is Top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Middle,
    Bottom,
}

impl Default for VerticalAlignment {
    fn default() -> Self {
        VerticalAlignment::Top
    }
}

/// Specifies whether the widget should be aligned to the left,
/// center or right of the horizontal space in its parent.
/// The default is Left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
}

impl Default for HorizontalAlignment {
    fn default() -> Self {
        HorizontalAlignment::Left
    }
}

/// Specifies the size constraints for a widget
#[derive(Clone, Default, Copy, Debug, PartialEq, Eq)]
pub struct Constraints {
    pub width: Dimension,
    pub height: Dimension,
    pub valign: VerticalAlignment,
    pub halign: HorizontalAlignment,
    pub child_orientation: ChildOrientation,
}

impl Constraints {
    pub fn with_fixed_width_height(width: u16, height: u16) -> Self {
        Self::default()
            .set_fixed_width(width)
            .set_fixed_height(height)
            .clone()
    }

    pub fn set_fixed_width(&mut self, width: u16) -> &mut Self {
        self.width = Dimension {
            spec: DimensionSpec::Fixed(width),
            minimum: Some(width),
            maximum: Some(width),
            ..Default::default()
        };
        self
    }

    pub fn set_pct_width(&mut self, width: u8) -> &mut Self {
        self.width = Dimension {
            spec: DimensionSpec::Percentage(width),
            ..Default::default()
        };
        self
    }

    pub fn set_fixed_height(&mut self, height: u16) -> &mut Self {
        self.height = Dimension {
            spec: DimensionSpec::Fixed(height),
            minimum: Some(height),
            maximum: Some(height),
            ..Default::default()
        };
        self
    }

    pub fn set_pct_height(&mut self, height: u8) -> &mut Self {
        self.height = Dimension {
            spec: DimensionSpec::Percentage(height),
            ..Default::default()
        };
        self
    }

    pub fn set_valign(&mut self, valign: VerticalAlignment) -> &mut Self {
        self.valign = valign;
        self
    }

    pub fn set_halign(&mut self, halign: HorizontalAlignment) -> &mut Self {
        self.halign = halign;
        self
    }
}

/// Holds state used to compute the layout of a tree of widgets
pub struct LayoutState {
    solver: Solver,
    screen_width: Variable,
    screen_height: Variable,
    widget_states: HashMap<WidgetHandle, WidgetState>,
}

/// Each `WidgetHandle` has a `WidgetState` associated with it.
/// This allows us to look up the `Variable` for each of the
/// layout features of a widget by `WidgetHandle` after the solver
/// has computed the layout solution.
#[derive(Copy, Clone)]
struct WidgetState {
    left: Variable,
    top: Variable,
    width: Variable,
    height: Variable,
}

fn suggesterr(e: SuggestValueError) -> Error {
    match e {
        SuggestValueError::UnknownEditVariable => err_msg("Unknown edit variable"),
        SuggestValueError::InternalSolverError(e) => format_err!("Internal solver error: {}", e),
    }
}

fn adderr(e: AddConstraintError) -> Error {
    format_err!("{:?}", e)
}

impl LayoutState {
    /// Convenience function that performs the entire layout operation
    /// and updates the widget tree.
    pub fn compute_layout(
        screen_width: usize,
        screen_height: usize,
        root_widget: &WidgetHandle,
    ) -> Result<(), Error> {
        let mut layout = Self::new();
        layout.add_widget_recursive(root_widget);
        layout.update_constraints(screen_width, screen_height, root_widget)
    }

    /// Create a new `LayoutState`
    pub fn new() -> Self {
        let mut solver = Solver::new();
        let screen_width = Variable::new();
        let screen_height = Variable::new();
        solver
            .add_edit_variable(screen_width, STRONG)
            .expect("failed to add screen_width to solver");
        solver
            .add_edit_variable(screen_height, STRONG)
            .expect("failed to add screen_height to solver");
        Self {
            solver,
            screen_width,
            screen_height,
            widget_states: HashMap::new(),
        }
    }

    /// Creates a WidgetState entry for a widget.
    fn add_widget(&mut self, widget: &WidgetHandle) {
        let state = WidgetState {
            left: Variable::new(),
            top: Variable::new(),
            width: Variable::new(),
            height: Variable::new(),
        };
        self.widget_states.insert(widget.clone(), state);
    }

    pub fn add_widget_recursive(&mut self, widget: &WidgetHandle) {
        self.add_widget(widget);
        for child in &widget.borrow().children {
            self.add_widget_recursive(child);
        }
    }

    /// Assign the screen dimensions, compute constraints, solve
    /// the layout and then apply the size and positioning information
    /// to the widgets in the widget tree.
    pub fn update_constraints(
        &mut self,
        screen_width: usize,
        screen_height: usize,
        root_widget: &WidgetHandle,
    ) -> Result<(), Error> {
        self.solver
            .suggest_value(self.screen_width, screen_width as f64)
            .map_err(suggesterr)?;
        self.solver
            .suggest_value(self.screen_height, screen_height as f64)
            .map_err(suggesterr)?;

        let width = self.screen_width;
        let height = self.screen_height;
        self.update_widget_constraint(root_widget, width, height, None, None)?;

        // The updates are in an unspecified order, and the coordinates are in
        // the screen absolute coordinate space, rather than the parent-relative
        // coordinates that we desire.  So we need to either to accumulate the
        // deltas and then sort them such that we walk the widget tree to apply
        // them, or just walk the tree and apply them all anyway.  The latter
        // feels easier and probably has a low enough cardinality that it won't
        // feel too expensive.
        self.solver.fetch_changes();
        self.apply_widget_state(root_widget, 0, 0)?;

        Ok(())
    }

    fn apply_widget_state(
        &self,
        widget: &WidgetHandle,
        parent_left: usize,
        parent_top: usize,
    ) -> Result<(), Error> {
        let state = *self.widget_states
            .get(widget)
            .ok_or_else(|| err_msg("widget has no solver state"))?;
        let width = self.solver.get_value(state.width) as usize;
        let height = self.solver.get_value(state.height) as usize;
        let left = self.solver.get_value(state.left) as usize;
        let top = self.solver.get_value(state.top) as usize;

        widget.borrow_mut().set_size_and_position(
            width,
            height,
            ParentRelativeCoords::new(left - parent_left, top - parent_top),
        );

        for child in &widget.borrow().children {
            self.apply_widget_state(child, left, top)?;
        }

        Ok(())
    }

    fn update_widget_constraint(
        &mut self,
        widget: &WidgetHandle,
        parent_width: Variable,
        parent_height: Variable,
        parent_left: Option<Variable>,
        parent_top: Option<Variable>,
    ) -> Result<(WidgetState, Constraints), Error> {
        let state = *self.widget_states
            .get(widget)
            .ok_or_else(|| err_msg("widget has no solver state"))?;

        let is_root_widget = parent_left.is_none();

        let parent_left = unwrap_variable_or(parent_left, 0.0);
        let parent_top = unwrap_variable_or(parent_top, 0.0);

        // First, we should fit inside the parent container
        self.solver
            .add_constraint(
                state.left + state.width | LE(REQUIRED) | parent_left.clone() + parent_width,
            )
            .map_err(adderr)?;
        self.solver
            .add_constraint(state.left | GE(REQUIRED) | parent_left.clone())
            .map_err(adderr)?;

        self.solver
            .add_constraint(
                state.top + state.height | LE(REQUIRED) | parent_top.clone() + parent_height,
            )
            .map_err(adderr)?;
        self.solver
            .add_constraint(state.top | GE(REQUIRED) | parent_top.clone())
            .map_err(adderr)?;

        let constraints = widget.borrow().get_size_constraints();

        if is_root_widget {
            // We handle alignment on the root widget specially here;
            // for non-root widgets, we handle it when assessing children
            match constraints.halign {
                HorizontalAlignment::Left => self.solver
                    .add_constraint(state.left | EQ(STRONG) | 0.0)
                    .map_err(adderr)?,
                HorizontalAlignment::Right => self.solver
                    .add_constraint(state.left | EQ(STRONG) | parent_width - state.width)
                    .map_err(adderr)?,
                HorizontalAlignment::Center => self.solver
                    .add_constraint(state.left | EQ(STRONG) | (parent_width - state.width) / 2.0)
                    .map_err(adderr)?,
            }

            match constraints.valign {
                VerticalAlignment::Top => self.solver
                    .add_constraint(state.top | EQ(STRONG) | 0.0)
                    .map_err(adderr)?,
                VerticalAlignment::Bottom => self.solver
                    .add_constraint(state.top | EQ(STRONG) | parent_height - state.height)
                    .map_err(adderr)?,
                VerticalAlignment::Middle => self.solver
                    .add_constraint(state.top | EQ(STRONG) | (parent_height - state.height) / 2.0)
                    .map_err(adderr)?,
            }
        }

        match constraints.width.spec {
            DimensionSpec::Fixed(width) => {
                self.solver
                    .add_constraint(state.width | EQ(STRONG) | f64::from(width))
                    .map_err(adderr)?;
            }
            DimensionSpec::Percentage(pct) => {
                self.solver
                    .add_constraint(
                        state.width | EQ(STRONG) | f64::from(pct) * parent_width / 100.0,
                    )
                    .map_err(adderr)?;
            }
        }
        self.solver
            .add_constraint(
                state.width | GE(STRONG) | f64::from(constraints.width.minimum.unwrap_or(1).max(1)),
            )
            .map_err(adderr)?;
        if let Some(max_width) = constraints.width.maximum {
            self.solver
                .add_constraint(state.width | LE(STRONG) | f64::from(max_width))
                .map_err(adderr)?;
        }

        match constraints.height.spec {
            DimensionSpec::Fixed(height) => {
                self.solver
                    .add_constraint(state.height | EQ(STRONG) | f64::from(height))
                    .map_err(adderr)?;
            }
            DimensionSpec::Percentage(pct) => {
                self.solver
                    .add_constraint(
                        state.height | EQ(STRONG) | f64::from(pct) * parent_height / 100.0,
                    )
                    .map_err(adderr)?;
            }
        }
        self.solver
            .add_constraint(
                state.height
                    | GE(STRONG)
                    | f64::from(constraints.height.minimum.unwrap_or(1).max(1)),
            )
            .map_err(adderr)?;
        if let Some(max_height) = constraints.height.maximum {
            self.solver
                .add_constraint(state.height | LE(STRONG) | f64::from(max_height))
                .map_err(adderr)?;
        }

        let has_children = !widget.borrow().children.is_empty();
        if has_children {
            let mut left_edge: Expression = state.left + 0.0;
            let mut top_edge: Expression = state.top + 0.0;
            let mut width_constraint = Expression::from_constant(0.0);
            let mut height_constraint = Expression::from_constant(0.0);

            for child in &widget.borrow().children {
                let (child_state, child_constraints) = self.update_widget_constraint(
                    child,
                    state.width,
                    state.height,
                    Some(state.left),
                    Some(state.top),
                )?;

                match child_constraints.halign {
                    HorizontalAlignment::Left => self.solver
                        .add_constraint(child_state.left | EQ(STRONG) | left_edge.clone())
                        .map_err(adderr)?,
                    HorizontalAlignment::Right => self.solver
                        .add_constraint(
                            child_state.left + child_state.width
                                | EQ(STRONG)
                                | state.left + state.width,
                        )
                        .map_err(adderr)?,
                    HorizontalAlignment::Center => self.solver
                        .add_constraint(
                            child_state.left
                                | EQ(STRONG)
                                | state.left + (state.width - child_state.width) / 2.0,
                        )
                        .map_err(adderr)?,
                }

                match child_constraints.valign {
                    VerticalAlignment::Top => self.solver
                        .add_constraint(child_state.top | EQ(STRONG) | top_edge.clone())
                        .map_err(adderr)?,
                    VerticalAlignment::Bottom => self.solver
                        .add_constraint(
                            child_state.top + child_state.height
                                | EQ(STRONG)
                                | state.top + state.height,
                        )
                        .map_err(adderr)?,
                    VerticalAlignment::Middle => self.solver
                        .add_constraint(
                            child_state.top
                                | EQ(STRONG)
                                | state.top + (state.height - child_state.height) / 2.0,
                        )
                        .map_err(adderr)?,
                }

                match constraints.child_orientation {
                    ChildOrientation::Horizontal => {
                        left_edge = child_state.left + child_state.width;
                        width_constraint = width_constraint + child_state.width;
                    }
                    ChildOrientation::Vertical => {
                        top_edge = child_state.top + child_state.height;
                        height_constraint = height_constraint + child_state.height;
                    }
                }
            }

            // This constraint encourages the contents to fill out to the width
            // of the container, rather than clumping left
            self.solver
                .add_constraint(left_edge | EQ(STRONG) | state.left + state.width)
                .map_err(adderr)?;

            self.solver
                .add_constraint(state.width | GE(WEAK) | width_constraint)
                .map_err(adderr)?;

            // This constraint encourages the contents to fill out to the height
            // of the container, rather than clumping top
            self.solver
                .add_constraint(top_edge | EQ(STRONG) | state.top + state.height)
                .map_err(adderr)?;

            self.solver
                .add_constraint(state.height | GE(WEAK) | height_constraint)
                .map_err(adderr)?;
        }

        Ok((state, constraints))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use surface::Surface;
    use widgets::{Widget, WidgetImpl};

    struct UnspecWidget {}
    impl WidgetImpl for UnspecWidget {
        fn render_to_surface(&self, _surface: &mut Surface) {}
    }

    struct ConstrainedWidget {
        constraints: Constraints,
    }
    impl WidgetImpl for ConstrainedWidget {
        fn render_to_surface(&self, _surface: &mut Surface) {}
        fn get_size_constraints(&self) -> Constraints {
            self.constraints
        }
    }
    impl ConstrainedWidget {
        fn new(constraints: Constraints) -> Self {
            Self { constraints }
        }
    }

    #[test]
    fn single_widget_unspec() {
        let widget = Widget::new(Box::new(UnspecWidget {}));
        let mut layout = LayoutState::new();
        layout.add_widget_recursive(&widget);
        layout.update_constraints(40, 12, &widget).unwrap();

        assert_eq!(
            widget.borrow().get_size_and_position(),
            (40, 12, ParentRelativeCoords::new(0, 0))
        );
    }

    #[test]
    fn two_children_pct() {
        let root = Widget::new(Box::new(UnspecWidget {}));

        let a = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::default().set_pct_width(50).clone(),
        )));
        let b = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::default().set_pct_width(50).clone(),
        )));

        root.borrow_mut().add_child(&a);
        root.borrow_mut().add_child(&b);

        let mut layout = LayoutState::new();
        layout.add_widget_recursive(&root);
        layout.update_constraints(100, 100, &root).unwrap();

        assert_eq!(
            (
                root.borrow().get_size_and_position(),
                a.borrow().get_size_and_position(),
                b.borrow().get_size_and_position()
            ),
            (
                (100, 100, ParentRelativeCoords::new(0, 0)),
                (50, 100, ParentRelativeCoords::new(0, 0)),
                (50, 100, ParentRelativeCoords::new(50, 0))
            )
        );
    }

    #[test]
    fn three_children_pct() {
        let root = Widget::new(Box::new(UnspecWidget {}));

        let a = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::default().set_pct_width(20).clone(),
        )));
        let b = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::default().set_pct_width(20).clone(),
        )));
        let c = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::default().set_pct_width(20).clone(),
        )));

        root.borrow_mut().add_child(&a);
        root.borrow_mut().add_child(&b);
        root.borrow_mut().add_child(&c);

        let mut layout = LayoutState::new();
        layout.add_widget_recursive(&root);
        layout.update_constraints(100, 100, &root).unwrap();

        assert_eq!(
            (
                root.borrow().get_size_and_position(),
                a.borrow().get_size_and_position(),
                b.borrow().get_size_and_position(),
                c.borrow().get_size_and_position(),
            ),
            (
                (100, 100, ParentRelativeCoords::new(0, 0)),
                (20, 100, ParentRelativeCoords::new(0, 0)),
                (20, 100, ParentRelativeCoords::new(20, 0)),
                (20, 100, ParentRelativeCoords::new(40, 0))
            )
        );
    }

    #[test]
    fn two_children_a_b() {
        let root = Widget::new(Box::new(UnspecWidget {}));

        let a = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::with_fixed_width_height(5, 2),
        )));
        let b = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::with_fixed_width_height(3, 2),
        )));

        root.borrow_mut().add_child(&a);
        root.borrow_mut().add_child(&b);

        let mut layout = LayoutState::new();
        layout.add_widget_recursive(&root);
        layout.update_constraints(100, 100, &root).unwrap();

        assert_eq!(
            (
                root.borrow().get_size_and_position(),
                a.borrow().get_size_and_position(),
                b.borrow().get_size_and_position()
            ),
            (
                (100, 100, ParentRelativeCoords::new(0, 0)),
                (5, 2, ParentRelativeCoords::new(0, 0)),
                (3, 2, ParentRelativeCoords::new(5, 0))
            )
        );
    }

    #[test]
    fn two_children_b_a() {
        let root = Widget::new(Box::new(UnspecWidget {}));

        let a = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::with_fixed_width_height(5, 2),
        )));
        let b = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::with_fixed_width_height(3, 2),
        )));

        root.borrow_mut().add_child(&b);
        root.borrow_mut().add_child(&a);

        let mut layout = LayoutState::new();
        layout.add_widget_recursive(&root);
        layout.update_constraints(100, 100, &root).unwrap();

        assert_eq!(
            (
                root.borrow().get_size_and_position(),
                a.borrow().get_size_and_position(),
                b.borrow().get_size_and_position()
            ),
            (
                (100, 100, ParentRelativeCoords::new(0, 0)),
                (5, 2, ParentRelativeCoords::new(3, 0)),
                (3, 2, ParentRelativeCoords::new(0, 0)),
            )
        );
    }

    #[test]
    fn two_children_overflow() {
        let root = Widget::new(Box::new(UnspecWidget {}));

        let a = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::with_fixed_width_height(5, 2),
        )));
        let b = Widget::new(Box::new(ConstrainedWidget::new(
            Constraints::with_fixed_width_height(3, 2),
        )));

        root.borrow_mut().add_child(&a);
        root.borrow_mut().add_child(&b);

        let mut layout = LayoutState::new();
        layout.add_widget_recursive(&root);
        layout.update_constraints(6, 2, &root).unwrap();

        assert_eq!(
            (
                root.borrow().get_size_and_position(),
                a.borrow().get_size_and_position(),
                b.borrow().get_size_and_position()
            ),
            (
                (8, 2, ParentRelativeCoords::new(0, 0)),
                (5, 2, ParentRelativeCoords::new(0, 0)),
                (3, 2, ParentRelativeCoords::new(5, 0)),
            )
        );
    }

    macro_rules! single_constrain {
        ($name:ident, $constraint:expr, $width:expr, $height:expr, $coords:expr) => {
            #[test]
            fn $name() {
                let widget = Widget::new(Box::new(ConstrainedWidget::new($constraint)));
                let mut layout = LayoutState::new();
                layout.add_widget_recursive(&widget);
                layout.update_constraints(100, 100, &widget).unwrap();

                assert_eq!(
                    widget.borrow().get_size_and_position(),
                    ($width, $height, $coords)
                );
            }
        };
    }

    single_constrain!(
        single_constrained_widget_top,
        Constraints::with_fixed_width_height(10, 2),
        10,
        2,
        ParentRelativeCoords::new(0, 0)
    );

    single_constrain!(
        single_constrained_widget_bottom,
        Constraints::with_fixed_width_height(10, 2)
            .set_valign(VerticalAlignment::Bottom)
            .clone(),
        10,
        2,
        ParentRelativeCoords::new(0, 98)
    );

    single_constrain!(
        single_constrained_widget_middle,
        Constraints::with_fixed_width_height(10, 2)
            .set_valign(VerticalAlignment::Middle)
            .clone(),
        10,
        2,
        ParentRelativeCoords::new(0, 49)
    );

    single_constrain!(
        single_constrained_widget_right,
        Constraints::with_fixed_width_height(10, 2)
            .set_halign(HorizontalAlignment::Right)
            .clone(),
        10,
        2,
        ParentRelativeCoords::new(90, 0)
    );

    single_constrain!(
        single_constrained_widget_bottom_center,
        Constraints::with_fixed_width_height(10, 2)
            .set_valign(VerticalAlignment::Bottom)
            .set_halign(HorizontalAlignment::Center)
            .clone(),
        10,
        2,
        ParentRelativeCoords::new(45, 98)
    );

}
