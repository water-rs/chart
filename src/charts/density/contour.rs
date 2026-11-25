//! Contour chart component.

use nami::{Binding, Signal};
use waterui_core::{Environment, View};

use crate::charts::canvas::{contour_geometry, interactive_signal_canvas, record_contour};
use crate::composition::ChartComposition;
use crate::data::{ColorScale, ContourData};
use crate::interaction::{GridDatum, HitResult, SelectionBindings};
use crate::params::PositiveF32;
use crate::theme_color::resolve_color_scale;

/// Contour chart for isoline visualization.
pub struct ContourChart<S: Signal<Output = ContourData>> {
    data: S,
    line_width: f32,
    color_scale: ColorScale,
    selection: SelectionBindings<GridDatum>,
    composition: ChartComposition<GridDatum>,
}

crate::charts::impl_chart_debug!(ContourChart, S, ContourData);

impl<S: Signal<Output = ContourData>> ContourChart<S> {
    /// Creates a contour chart from reactive scalar-field data.
    #[must_use]
    pub fn new(data: S) -> Self {
        Self {
            data,
            line_width: 2.0,
            color_scale: ColorScale::default(),
            selection: SelectionBindings::new(),
            composition: ChartComposition::default(),
        }
    }

    crate::composition::chart_composition_methods!(GridDatum);

    /// Sets the contour line width.
    ///
    /// Accepts any value convertible into [`PositiveF32`]. Passing a raw
    /// `f32` panics on `NaN`, infinity, or non-positive values.
    #[must_use]
    pub fn line_width(mut self, width: impl Into<PositiveF32>) -> Self {
        self.line_width = width.into().get();
        self
    }

    /// Sets the color scale used to map scalar levels to colors.
    #[must_use]
    pub fn color_scale(mut self, scale: ColorScale) -> Self {
        self.color_scale = scale;
        self
    }

    /// Tracks the currently focused contour cell in an external binding.
    #[must_use]
    pub fn chart_focus(mut self, focused: &Binding<Option<HitResult<GridDatum>>>) -> Self {
        self.selection = self.selection.with_focused(focused);
        self
    }

    /// Tracks the currently selected contour cell in an external binding.
    #[must_use]
    pub fn chart_selection(mut self, selected: &Binding<Option<HitResult<GridDatum>>>) -> Self {
        self.selection = self.selection.with_selected(selected);
        self
    }
}

impl<S: Signal<Output = ContourData> + Clone + 'static> View for ContourChart<S> {
    fn body(self, env: &Environment) -> impl View {
        let color_scale = resolve_color_scale(&self.color_scale, env);
        let line_width = self.line_width;
        interactive_signal_canvas(
            env,
            self.data,
            contour_geometry,
            move |cx, frame| record_contour(cx, frame, &color_scale, line_width),
            self.selection,
            self.composition,
        )
    }
}

/// Convenience constructor for [`ContourChart`]. Equivalent to [`ContourChart::new`].
#[must_use]
pub fn contour_chart<S: Signal<Output = ContourData>>(data: S) -> ContourChart<S> {
    ContourChart::new(data)
}
