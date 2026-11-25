//! Scatter chart component.

extern crate alloc;

use alloc::vec::Vec;

use nami::{Binding, Signal, signal::IntoComputed};
use waterui_core::{Environment, View};
use waterui_graphics::color::Color;

use crate::charts::canvas::{
    interactive_cartesian_signal_canvas, point_bounds, point_geometry, record_scatter,
};
use crate::composition::ChartComposition;
use crate::data::DataPoint;
use crate::interaction::{
    CartesianSelectionBindings, CartesianViewportBindings, HitResult, SelectionBindings,
};
use crate::params::PositiveF32;
use crate::theme_color::into_color;

/// Scatter chart visualization.
pub struct ScatterChart<S: Signal<Output = Vec<DataPoint>>> {
    data: S,
    color: Color,
    radius: f32,
    selection: SelectionBindings<DataPoint>,
    cartesian_selection: CartesianSelectionBindings,
    cartesian_viewport: CartesianViewportBindings,
    composition: ChartComposition<DataPoint>,
}

crate::charts::impl_chart_debug!(ScatterChart, S, Vec<DataPoint>);

impl<S: Signal<Output = Vec<DataPoint>>> ScatterChart<S> {
    /// Creates a scatter chart from reactive point data.
    #[must_use]
    pub fn new(data: S) -> Self {
        Self {
            data,
            color: Color::accent(),
            radius: 4.0,
            selection: SelectionBindings::default(),
            cartesian_selection: CartesianSelectionBindings::default(),
            cartesian_viewport: CartesianViewportBindings::default(),
            composition: ChartComposition::default(),
        }
    }

    crate::interaction::chart_x_selection_methods!();

    crate::composition::chart_composition_methods!(DataPoint);

    /// Sets the point color.
    #[must_use]
    pub fn color(mut self, color: impl IntoComputed<Color>) -> Self {
        self.color = into_color(color);
        self
    }

    /// Sets the point radius.
    ///
    /// Accepts any value convertible into [`PositiveF32`]. Passing a raw
    /// `f32` panics on `NaN`, infinity, or non-positive values.
    #[must_use]
    pub fn radius(mut self, radius: impl Into<PositiveF32>) -> Self {
        self.radius = radius.into().get();
        self
    }

    /// Tracks the currently focused point in an external binding.
    #[must_use]
    pub fn chart_focus(mut self, focused: &Binding<Option<HitResult<DataPoint>>>) -> Self {
        self.selection = self.selection.with_focused(focused);
        self
    }

    /// Tracks the currently selected point in an external binding.
    #[must_use]
    pub fn chart_selection(mut self, selected: &Binding<Option<HitResult<DataPoint>>>) -> Self {
        self.selection = self.selection.with_selected(selected);
        self
    }
}

impl<S: Signal<Output = Vec<DataPoint>> + Clone + 'static> View for ScatterChart<S> {
    fn body(self, env: &Environment) -> impl View {
        let color = self.color.resolve(env);
        let radius = self.radius;
        interactive_cartesian_signal_canvas(
            env,
            self.data,
            |data: &Vec<DataPoint>| point_bounds(data),
            move |size, data, bounds| point_geometry(size, data, bounds, radius.max(8.0)),
            move |cx, frame| record_scatter(cx, frame, &color, radius),
            self.selection,
            self.cartesian_selection,
            self.cartesian_viewport,
            self.composition,
        )
    }
}

/// Convenience constructor for [`ScatterChart`]. Equivalent to [`ScatterChart::new`].
#[must_use]
pub fn scatter_chart<S: Signal<Output = Vec<DataPoint>>>(data: S) -> ScatterChart<S> {
    ScatterChart::new(data)
}
