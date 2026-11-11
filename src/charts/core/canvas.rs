//! The canvas every chart is drawn on.
//!
//! A chart derives one [`Frame`] — the data, the geometry laid out for the
//! canvas's current size, and the hit geometry — as a signal of the data, the
//! viewport and the [`CanvasSize`] the canvas is laid out into. The canvas
//! closure, which runs once at mount, records the chart as keyed
//! sub-recordings over that signal, so a change updates operands in place
//! instead of recording again; pointer input and chart composition read the
//! same signal. A data change cross-fades between two layers whose opacity
//! is a live, animated operand.

extern crate alloc;

use alloc::rc::Rc;
use core::{cell::RefCell, time::Duration};

use nami::{Binding, Computed, Signal, SignalExt as _};
use waterui_core::{
    AnyView, Environment, Metadata, View,
    accessibility::{AccessibilityHidden, AccessibilityRole},
    animation::Animation,
    event::{Event, HoverEvent, OnEvent},
    gesture::{DragEvent, DragGesture, GestureObserver, GesturePhase, TapEvent, TapGesture},
    layout::{Point, Size},
};
use waterui_graphics::canvas::{Canvas, CanvasSize, Cx, Layer};

pub(crate) use super::geometry::{
    area_bounds, area_geometry, bar_bounds, bar_geometry, bubble_bounds, bubble_geometry,
    candlestick_bounds, candlestick_geometry, choropleth_geometry, contour_geometry, depth_bounds,
    depth_geometry, gauge_geometry, heatmap_geometry, pie_geometry, point_bounds, point_geometry,
    radar_geometry,
};
pub(crate) use super::hit::{CartesianGeometry, HitGeometry};
pub(crate) use super::record::{
    record_area, record_bar, record_bubble, record_candlestick, record_choropleth, record_contour,
    record_depth, record_gauge, record_heatmap, record_line, record_pie, record_radar,
    record_scatter,
};
use super::{
    geometry::normalize_bounds,
    hit::{
        cartesian_x_from_point, cartesian_y_from_point, gesture_point_to_point,
        normalized_scalar_range,
    },
};
use crate::{
    composition::ChartComposition,
    data::DataBounds,
    interaction::{
        CartesianSelectionBindings, CartesianViewportBindings, ChartViewport, SelectionBindings,
    },
};

/// How long a data change takes to cross-fade.
const CHART_TRANSITION: Animation = Animation::ease_in_out(Duration::from_millis(240));

/// Everything a chart draws and answers input from, for one state of its
/// data, viewport and canvas size.
#[derive(Debug, Clone)]
pub(crate) struct Frame<D, G> {
    /// The data drawn.
    pub data: D,
    /// The geometry laid out for [`size`](Self::size), which also answers
    /// pointer input.
    pub geometry: G,
    /// The canvas's size the geometry is laid out for.
    pub size: Size,
}

/// A frame of a chart over a data window.
pub(crate) type CartesianFrame<D, T> = Frame<D, CartesianGeometry<T>>;

/// One of the two layers a data change cross-fades between.
#[derive(Clone)]
struct Slot<D: 'static, G: 'static> {
    frame: Binding<Frame<D, G>>,
    present: Binding<bool>,
    opacity: Binding<f32>,
}

impl<D: Clone + 'static, G: Clone + 'static> Slot<D, G> {
    fn new(frame: Frame<D, G>, shown: bool) -> Self {
        Self {
            frame: Binding::container(frame),
            present: Binding::container(shown),
            opacity: Binding::container(if shown { 1.0 } else { 0.0 }),
        }
    }
}

/// Which layer shows the data, and the data it shows.
struct Showing<D> {
    active: usize,
    data: D,
}

/// Records `draw` over `frame` into two layers and keeps them showing the
/// frame: a change of the geometry alone updates the showing layer in place;
/// a change of the data records into the other layer and cross-fades to it
/// under [`CHART_TRANSITION`].
fn record_frames<D, G>(
    cx: &mut Cx,
    frame: &Computed<Frame<D, G>>,
    draw: impl Fn(&mut Cx, &Computed<Frame<D, G>>) + 'static,
) where
    D: Clone + PartialEq + 'static,
    G: Clone + 'static,
{
    let initial = frame.snapshot();
    let showing = RefCell::new(Showing {
        active: 0,
        data: initial.data.clone(),
    });
    let slots = [Slot::new(initial.clone(), true), Slot::new(initial, false)];

    let draw = Rc::new(draw);
    for slot in &slots {
        let draw = Rc::clone(&draw);
        let content = slot.frame.computed();
        let opacity = slot.opacity.with(CHART_TRANSITION).map(Layer::opacity);
        let present = slot.present.clone();
        cx.layer(opacity, move |cx| {
            cx.scope(present, move |cx, present: &bool| {
                if *present {
                    draw(cx, &content);
                }
            });
        });
    }

    let follow = frame.watch(move |change| {
        let next = change.into_value();
        let mut showing = showing.borrow_mut();
        if showing.data == next.data {
            slots[showing.active].frame.set(next);
            return;
        }
        let incoming = &slots[1 - showing.active];
        incoming.frame.set(next.clone());
        if !incoming.present.snapshot() {
            incoming.present.set(true);
        }
        incoming.opacity.set(1.0);
        slots[showing.active].opacity.set(0.0);
        showing.active = 1 - showing.active;
        showing.data = next.data;
    });
    cx.keep(follow);
}

/// Keeps a chart canvas out of the accessibility tree when the chart itself
/// carries the role: the chart is one element, not a canvas inside it.
struct ChartCanvasAccessibilityBoundary {
    content: AnyView,
}

impl ChartCanvasAccessibilityBoundary {
    fn new(content: impl View + 'static) -> Self {
        Self {
            content: AnyView::new(content),
        }
    }
}

impl View for ChartCanvasAccessibilityBoundary {
    fn body(self, env: &Environment) -> impl View {
        let mut canvas_env = env.clone();
        if canvas_env.get::<AccessibilityRole>().is_some() {
            canvas_env.insert(AccessibilityHidden::new(true));
        }
        Metadata::new(self.content, canvas_env)
    }
}

/// The chart's frame and plot area, as the signals chart composition lays
/// out against.
fn composition_frames<D: Clone + 'static, G: Clone + 'static>(
    frame: &Computed<Frame<D, G>>,
    plot_area: impl Fn(&G) -> ChartViewport + Clone + 'static,
) -> (Computed<ChartViewport>, Computed<ChartViewport>) {
    let chart_frame = frame
        .map(|frame| ChartViewport::new(0.0, 0.0, frame.size.width, frame.size.height))
        .computed();
    let plot_area = frame
        .map(move |frame| plot_area(&frame.geometry))
        .computed();
    (chart_frame, plot_area)
}

#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "This internal adapter owns generic callbacks and bindings so they can move into Canvas' static event closures without extra type erasure"
)]
pub(crate) fn interactive_cartesian_signal_canvas<S, D, T, B, G, F>(
    _env: &Environment,
    signal: S,
    bounds_of: B,
    build_geometry: G,
    draw_chart: F,
    mut selection: SelectionBindings<T>,
    cartesian_selection: CartesianSelectionBindings,
    cartesian_viewport: CartesianViewportBindings,
    composition: ChartComposition<T>,
) -> impl waterui_core::View
where
    S: Signal<Output = D> + 'static,
    S::Guard: 'static,
    D: Clone + PartialEq + 'static,
    T: Clone + PartialEq + 'static,
    B: Fn(&D) -> DataBounds + Clone + 'static,
    G: Fn(Size, &D, DataBounds) -> CartesianGeometry<T> + Clone + 'static,
    F: Fn(&mut Cx, &Computed<CartesianFrame<D, T>>) + 'static,
{
    cartesian_selection.validate();
    cartesian_viewport.validate();
    if !composition.is_empty() {
        selection = selection.activate_proxy();
    }
    if selection.is_active() {
        selection = selection.persist_internal();
    }
    let x_range_start = Binding::container(None::<f32>);
    let y_range_start = Binding::container(None::<f32>);
    let size = CanvasSize::new();
    let frame = {
        let cartesian_viewport = cartesian_viewport.clone();
        signal
            .zip(&cartesian_viewport.state_signal())
            .zip(&size.signal())
            .map(move |((data, viewport_state), size)| {
                let base_bounds = normalize_bounds(bounds_of(&data));
                let visible_bounds = cartesian_viewport.resolve_bounds(base_bounds, viewport_state);
                let geometry = build_geometry(size, &data, visible_bounds).within(base_bounds);
                Frame {
                    data,
                    geometry,
                    size,
                }
            })
            .computed()
    };
    let canvas = {
        let frame = frame.clone();
        Canvas::new(move |cx| record_frames(cx, &frame, draw_chart)).sized_by(size)
    };
    let canvas = ChartCanvasAccessibilityBoundary::new(canvas);
    let canvas = Metadata::new(
        canvas,
        GestureObserver::new(TapGesture::new(), {
            let frame = frame.clone();
            let selection = selection.clone();
            let cartesian_selection = cartesian_selection.clone();
            move |env: Environment| {
                if !selection.is_active() && !cartesian_selection.is_active() {
                    return;
                }
                let tap = env
                    .get::<TapEvent>()
                    .expect("interactive_cartesian_signal_canvas: TapEvent missing from gesture environment");
                let point = gesture_point_to_point(tap.location());
                let Frame { geometry, .. } = frame.snapshot();
                let hit = geometry.hit_test(point);
                selection.set_focus(hit.clone());
                selection.set_selected(hit);
                if cartesian_selection.tracks_x_value() {
                    cartesian_selection
                        .set_x_value(cartesian_x_from_point(&geometry, point, false));
                }
                if cartesian_selection.tracks_y_value() {
                    cartesian_selection
                        .set_y_value(cartesian_y_from_point(&geometry, point, false));
                }
            }
        }),
    );
    let canvas = Metadata::new(
        canvas,
        GestureObserver::new(DragGesture::new(0.0), {
            let frame = frame.clone();
            let selection = selection.clone();
            let cartesian_selection = cartesian_selection;
            let cartesian_viewport = cartesian_viewport;
            let x_range_start = x_range_start;
            let y_range_start = y_range_start;
            move |env: Environment| {
                let handles_selection_drag = selection.is_active()
                    || cartesian_selection.is_active()
                    || (!cartesian_viewport.allows_horizontal_drag()
                        && !cartesian_viewport.allows_vertical_drag());
                if !handles_selection_drag
                    && !cartesian_viewport.allows_horizontal_drag()
                    && !cartesian_viewport.allows_vertical_drag()
                {
                    return;
                }

                let drag_event = env.get::<DragEvent>().expect(
                    "interactive_cartesian_signal_canvas: DragEvent missing from gesture environment",
                );
                let Frame { geometry, .. } = frame.snapshot();
                let point = gesture_point_to_point(drag_event.location());

                if handles_selection_drag {
                    match drag_event.phase() {
                        GesturePhase::Started => {
                            let hit = geometry.hit_test(point);
                            selection.set_focus(hit);
                            let start_point = Point::new(
                                point.x - drag_event.translation().x,
                                point.y - drag_event.translation().y,
                            );
                            if cartesian_selection.tracks_x_range() {
                                if let Some(start) =
                                    cartesian_x_from_point(&geometry, start_point, true)
                                {
                                    x_range_start.set(Some(start));
                                    if let Some(end) =
                                        cartesian_x_from_point(&geometry, point, true)
                                    {
                                        cartesian_selection
                                            .set_x_range(Some(normalized_scalar_range(start, end)));
                                    }
                                }
                            } else if cartesian_selection.tracks_x_value() {
                                cartesian_selection
                                    .set_x_value(cartesian_x_from_point(&geometry, point, true));
                            }
                            if cartesian_selection.tracks_y_range() {
                                if let Some(start) =
                                    cartesian_y_from_point(&geometry, start_point, true)
                                {
                                    y_range_start.set(Some(start));
                                    if let Some(end) =
                                        cartesian_y_from_point(&geometry, point, true)
                                    {
                                        cartesian_selection
                                            .set_y_range(Some(normalized_scalar_range(start, end)));
                                    }
                                }
                            } else if cartesian_selection.tracks_y_value() {
                                cartesian_selection
                                    .set_y_value(cartesian_y_from_point(&geometry, point, true));
                            }
                        }
                        GesturePhase::Updated => {
                            let hit = geometry.hit_test(point);
                            selection.set_focus(hit);
                            if cartesian_selection.tracks_x_range() {
                                if let (Some(start), Some(end)) = (
                                    x_range_start.snapshot(),
                                    cartesian_x_from_point(&geometry, point, true),
                                ) {
                                    cartesian_selection
                                        .set_x_range(Some(normalized_scalar_range(start, end)));
                                }
                            } else if cartesian_selection.tracks_x_value() {
                                cartesian_selection
                                    .set_x_value(cartesian_x_from_point(&geometry, point, true));
                            }
                            if cartesian_selection.tracks_y_range() {
                                if let (Some(start), Some(end)) = (
                                    y_range_start.snapshot(),
                                    cartesian_y_from_point(&geometry, point, true),
                                ) {
                                    cartesian_selection
                                        .set_y_range(Some(normalized_scalar_range(start, end)));
                                }
                            } else if cartesian_selection.tracks_y_value() {
                                cartesian_selection
                                    .set_y_value(cartesian_y_from_point(&geometry, point, true));
                            }
                        }
                        GesturePhase::Ended => {
                            let hit = geometry.hit_test(point);
                            selection.set_selected(hit);
                            selection.clear_focus();
                            if cartesian_selection.tracks_x_range() {
                                if let (Some(start), Some(end)) = (
                                    x_range_start.snapshot(),
                                    cartesian_x_from_point(&geometry, point, true),
                                ) {
                                    cartesian_selection
                                        .set_x_range(Some(normalized_scalar_range(start, end)));
                                }
                                x_range_start.set(None);
                            } else if cartesian_selection.tracks_x_value() {
                                cartesian_selection
                                    .set_x_value(cartesian_x_from_point(&geometry, point, true));
                            }
                            if cartesian_selection.tracks_y_range() {
                                if let (Some(start), Some(end)) = (
                                    y_range_start.snapshot(),
                                    cartesian_y_from_point(&geometry, point, true),
                                ) {
                                    cartesian_selection
                                        .set_y_range(Some(normalized_scalar_range(start, end)));
                                }
                                y_range_start.set(None);
                            } else if cartesian_selection.tracks_y_value() {
                                cartesian_selection
                                    .set_y_value(cartesian_y_from_point(&geometry, point, true));
                            }
                        }
                        GesturePhase::Cancelled => {
                            selection.clear_focus();
                            if cartesian_selection.tracks_x_range() {
                                x_range_start.set(None);
                            }
                            if cartesian_selection.tracks_y_range() {
                                y_range_start.set(None);
                            }
                        }
                    }
                    return;
                }

                let base_bounds = geometry.base_bounds;
                let plot_viewport = geometry.viewport();
                match drag_event.phase() {
                    GesturePhase::Started => {
                        selection.clear_focus();
                        if cartesian_viewport.allows_horizontal_drag() {
                            x_range_start.set(Some(geometry.bounds.min_x));
                        }
                        if cartesian_viewport.allows_vertical_drag() {
                            y_range_start.set(Some(geometry.bounds.min_y));
                        }
                    }
                    GesturePhase::Updated | GesturePhase::Ended => {
                        if cartesian_viewport.allows_horizontal_drag() {
                            if let Some(start) = x_range_start.snapshot() {
                                let visible_width = geometry.bounds.width();
                                let max_start =
                                    (base_bounds.max_x - visible_width).max(base_bounds.min_x);
                                let candidate = (drag_event.translation().x / plot_viewport.width)
                                    .mul_add(-visible_width, start);
                                let clamped = if max_start <= base_bounds.min_x {
                                    base_bounds.min_x
                                } else {
                                    candidate.clamp(base_bounds.min_x, max_start)
                                };
                                cartesian_viewport.set_x_position(clamped);
                            }
                            if matches!(drag_event.phase(), GesturePhase::Ended) {
                                x_range_start.set(None);
                            }
                        }
                        if cartesian_viewport.allows_vertical_drag() {
                            if let Some(start) = y_range_start.snapshot() {
                                let visible_height = geometry.bounds.height();
                                let max_start =
                                    (base_bounds.max_y - visible_height).max(base_bounds.min_y);
                                let candidate = (drag_event.translation().y / plot_viewport.height)
                                    .mul_add(-visible_height, start);
                                let clamped = if max_start <= base_bounds.min_y {
                                    base_bounds.min_y
                                } else {
                                    candidate.clamp(base_bounds.min_y, max_start)
                                };
                                cartesian_viewport.set_y_position(clamped);
                            }
                            if matches!(drag_event.phase(), GesturePhase::Ended) {
                                y_range_start.set(None);
                            }
                        }
                    }
                    GesturePhase::Cancelled => {
                        x_range_start.set(None);
                        y_range_start.set(None);
                    }
                }
            }
        }),
    );
    let (chart_frame, plot_area_frame) =
        composition_frames(&frame, |geometry: &CartesianGeometry<T>| {
            geometry.viewport()
        });
    let canvas = hover_events(canvas, &frame, &selection);
    composition.apply(canvas, chart_frame, plot_area_frame, &selection)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "This internal adapter moves callback ownership into the canvas' static closure"
)]
pub(crate) fn interactive_signal_canvas<S, D, G, T, B, F>(
    _env: &Environment,
    signal: S,
    build_geometry: B,
    draw_chart: F,
    mut selection: SelectionBindings<T>,
    composition: ChartComposition<T>,
) -> impl waterui_core::View
where
    S: Signal<Output = D> + 'static,
    S::Guard: 'static,
    D: Clone + PartialEq + 'static,
    G: HitGeometry<T>,
    T: Clone + PartialEq + 'static,
    B: Fn(Size, &D) -> G + Clone + 'static,
    F: Fn(&mut Cx, &Computed<Frame<D, G>>) + 'static,
{
    if !composition.is_empty() {
        selection = selection.activate_proxy();
    }
    if selection.is_active() {
        selection = selection.persist_internal();
    }

    let size = CanvasSize::new();
    let frame = signal
        .zip(&size.signal())
        .map(move |(data, size)| Frame {
            geometry: build_geometry(size, &data),
            data,
            size,
        })
        .computed();
    let canvas = {
        let frame = frame.clone();
        Canvas::new(move |cx| record_frames(cx, &frame, draw_chart)).sized_by(size)
    };
    let canvas = ChartCanvasAccessibilityBoundary::new(canvas);
    let canvas = Metadata::new(
        canvas,
        GestureObserver::new(TapGesture::new(), {
            let frame = frame.clone();
            let selection = selection.clone();
            move |env: Environment| {
                if !selection.is_active() {
                    return;
                }
                let tap = env
                    .get::<TapEvent>()
                    .expect("interactive_signal_canvas: TapEvent missing from gesture environment");
                let hit = frame
                    .snapshot()
                    .geometry
                    .hit_test(gesture_point_to_point(tap.location()));
                selection.set_focus(hit.clone());
                selection.set_selected(hit);
            }
        }),
    );
    let canvas = Metadata::new(
        canvas,
        GestureObserver::new(DragGesture::new(0.0), {
            let frame = frame.clone();
            let selection = selection.clone();
            move |env: Environment| {
                if !selection.is_active() {
                    return;
                }
                let drag_event = env.get::<DragEvent>().expect(
                    "interactive_signal_canvas: DragEvent missing from gesture environment",
                );
                let hit_at = || {
                    frame
                        .snapshot()
                        .geometry
                        .hit_test(gesture_point_to_point(drag_event.location()))
                };
                match drag_event.phase() {
                    GesturePhase::Started | GesturePhase::Updated => selection.set_focus(hit_at()),
                    GesturePhase::Ended => {
                        selection.set_selected(hit_at());
                        selection.clear_focus();
                    }
                    GesturePhase::Cancelled => selection.clear_focus(),
                }
            }
        }),
    );
    let (chart_frame, plot_area_frame) =
        composition_frames(&frame, |geometry: &G| geometry.viewport());
    let canvas = hover_events(canvas, &frame, &selection);
    composition.apply(canvas, chart_frame, plot_area_frame, &selection)
}

/// Adds hover tracking to `canvas`: the datum under the pointer is the
/// focus, and leaving the chart clears it.
fn hover_events<V, D, G, T>(
    canvas: V,
    frame: &Computed<Frame<D, G>>,
    selection: &SelectionBindings<T>,
) -> impl View
where
    V: View + 'static,
    D: Clone + 'static,
    G: HitGeometry<T>,
    T: Clone + PartialEq + 'static,
{
    let canvas = Metadata::new(
        canvas,
        OnEvent::new(Event::HoverMove, {
            let frame = frame.clone();
            let selection = selection.clone();
            move |env: Environment| {
                if !selection.is_active() {
                    return;
                }
                let hover = env
                    .get::<HoverEvent>()
                    .expect("hover_events: HoverEvent missing from event environment");
                let hit = frame.snapshot().geometry.hit_test(hover.location());
                selection.set_focus(hit);
            }
        }),
    );
    Metadata::new(
        canvas,
        OnEvent::new(Event::HoverExit, {
            let selection = selection.clone();
            move || selection.clear_focus()
        }),
    )
}
