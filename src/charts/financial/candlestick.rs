//! Candlestick chart component.

use nami::{Binding, Signal, signal::IntoComputed};
use waterui_core::{Environment, View};
use waterui_graphics::color::Color;

use crate::charts::canvas::{
    candlestick_bounds, candlestick_geometry, interactive_cartesian_signal_canvas,
    record_candlestick,
};
use crate::composition::ChartComposition;
use crate::data::Candle;
use crate::interaction::{
    CartesianSelectionBindings, CartesianViewportBindings, HitResult, SelectionBindings,
};
use crate::theme_color::into_color;

/// Candlestick chart for financial OHLC data.
pub struct CandlestickChart<S: Signal<Output = Vec<Candle>>> {
    data: S,
    bullish_color: Color,
    bearish_color: Color,
    selection: SelectionBindings<Candle>,
    cartesian_selection: CartesianSelectionBindings,
    cartesian_viewport: CartesianViewportBindings,
    composition: ChartComposition<Candle>,
}

crate::charts::impl_chart_debug!(CandlestickChart, S, Vec<Candle>);

impl<S: Signal<Output = Vec<Candle>>> CandlestickChart<S> {
    /// Creates a candlestick chart from reactive OHLC candle data.
    #[must_use]
    pub fn new(data: S) -> Self {
        Self {
            data,
            bullish_color: Color::accent(),
            bearish_color: Color::error(),
            selection: SelectionBindings::default(),
            cartesian_selection: CartesianSelectionBindings::default(),
            cartesian_viewport: CartesianViewportBindings::default(),
            composition: ChartComposition::default(),
        }
    }

    crate::interaction::chart_x_selection_methods!();

    crate::composition::chart_composition_methods!(Candle);

    /// Sets the color used for bullish candles.
    #[must_use]
    pub fn bullish_color(mut self, color: impl IntoComputed<Color>) -> Self {
        self.bullish_color = into_color(color);
        self
    }

    /// Sets the color used for bearish candles.
    #[must_use]
    pub fn bearish_color(mut self, color: impl IntoComputed<Color>) -> Self {
        self.bearish_color = into_color(color);
        self
    }

    /// Tracks the currently focused candle in an external binding.
    #[must_use]
    pub fn chart_focus(mut self, focused: &Binding<Option<HitResult<Candle>>>) -> Self {
        self.selection = self.selection.with_focused(focused);
        self
    }

    /// Tracks the currently selected candle in an external binding.
    #[must_use]
    pub fn chart_selection(mut self, selected: &Binding<Option<HitResult<Candle>>>) -> Self {
        self.selection = self.selection.with_selected(selected);
        self
    }
}

impl<S: Signal<Output = Vec<Candle>> + Clone + 'static> View for CandlestickChart<S> {
    fn body(self, env: &Environment) -> impl View {
        let bullish_color = self.bullish_color.resolve(env);
        let bearish_color = self.bearish_color.resolve(env);
        interactive_cartesian_signal_canvas(
            env,
            self.data,
            |data: &Vec<Candle>| candlestick_bounds(data),
            move |size, data, bounds| candlestick_geometry(size, data, bounds),
            move |cx, frame| record_candlestick(cx, frame, &bullish_color, &bearish_color),
            self.selection,
            self.cartesian_selection,
            self.cartesian_viewport,
            self.composition,
        )
    }
}

/// Convenience constructor for [`CandlestickChart`]. Equivalent to [`CandlestickChart::new`].
#[must_use]
pub fn candlestick_chart<S: Signal<Output = Vec<Candle>>>(data: S) -> CandlestickChart<S> {
    CandlestickChart::new(data)
}
