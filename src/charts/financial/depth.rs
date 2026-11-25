//! Depth (order book) chart component.

use nami::{Binding, Signal, signal::IntoComputed};
use waterui_core::{Environment, View};
use waterui_graphics::color::Color;

use crate::charts::canvas::{
    depth_bounds, depth_geometry, interactive_cartesian_signal_canvas, record_depth,
};
use crate::composition::ChartComposition;
use crate::data::DepthData;
use crate::interaction::{
    CartesianSelectionBindings, CartesianViewportBindings, DepthDatum, HitResult, SelectionBindings,
};
use crate::theme_color::into_color;

/// Depth chart for order book visualization.
pub struct DepthChart<S: Signal<Output = DepthData>> {
    data: S,
    bid_color: Color,
    ask_color: Color,
    selection: SelectionBindings<DepthDatum>,
    cartesian_selection: CartesianSelectionBindings,
    cartesian_viewport: CartesianViewportBindings,
    composition: ChartComposition<DepthDatum>,
}

crate::charts::impl_chart_debug!(DepthChart, S, DepthData);

impl<S: Signal<Output = DepthData>> DepthChart<S> {
    /// Creates a depth chart from reactive order-book data.
    #[must_use]
    pub fn new(data: S) -> Self {
        Self {
            data,
            bid_color: Color::accent(),
            ask_color: Color::error(),
            selection: SelectionBindings::default(),
            cartesian_selection: CartesianSelectionBindings::default(),
            cartesian_viewport: CartesianViewportBindings::default(),
            composition: ChartComposition::default(),
        }
    }

    crate::interaction::chart_x_selection_methods!();

    crate::composition::chart_composition_methods!(DepthDatum);

    /// Sets the bid-side fill color.
    #[must_use]
    pub fn bid_color(mut self, color: impl IntoComputed<Color>) -> Self {
        self.bid_color = into_color(color);
        self
    }

    /// Sets the ask-side fill color.
    #[must_use]
    pub fn ask_color(mut self, color: impl IntoComputed<Color>) -> Self {
        self.ask_color = into_color(color);
        self
    }

    /// Sets both bid and ask colors.
    #[must_use]
    pub fn colors(mut self, bid: impl IntoComputed<Color>, ask: impl IntoComputed<Color>) -> Self {
        self.bid_color = into_color(bid);
        self.ask_color = into_color(ask);
        self
    }

    /// Tracks the currently focused depth level in an external binding.
    #[must_use]
    pub fn chart_focus(mut self, focused: &Binding<Option<HitResult<DepthDatum>>>) -> Self {
        self.selection = self.selection.with_focused(focused);
        self
    }

    /// Tracks the currently selected depth level in an external binding.
    #[must_use]
    pub fn chart_selection(mut self, selected: &Binding<Option<HitResult<DepthDatum>>>) -> Self {
        self.selection = self.selection.with_selected(selected);
        self
    }
}

impl<S: Signal<Output = DepthData> + Clone + 'static> View for DepthChart<S> {
    fn body(self, env: &Environment) -> impl View {
        let bid_color = self.bid_color.resolve(env);
        let ask_color = self.ask_color.resolve(env);
        interactive_cartesian_signal_canvas(
            env,
            self.data,
            depth_bounds,
            depth_geometry,
            move |cx, frame| record_depth(cx, frame, &bid_color, &ask_color),
            self.selection,
            self.cartesian_selection,
            self.cartesian_viewport,
            self.composition,
        )
    }
}

/// Convenience constructor for [`DepthChart`]. Equivalent to [`DepthChart::new`].
#[must_use]
pub fn depth_chart<S: Signal<Output = DepthData>>(data: S) -> DepthChart<S> {
    DepthChart::new(data)
}
