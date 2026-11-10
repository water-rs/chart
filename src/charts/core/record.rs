//! How each chart is recorded onto its canvas.
//!
//! Every function here runs once per layer, inside the canvas closure, and
//! records the chart as keyed sub-recordings over a signal of its frame: one
//! item per series, bar, point or cell, keyed by the datum's place in the
//! data. A frame change that keeps a key updates that item's operands in
//! place; one that adds or drops keys records or drops only those items.

extern crate alloc;

use alloc::vec::Vec;
use core::f32::consts::TAU;

use nami::{Computed, SignalExt as _};
use waterui_core::layout::{Point, Rect, Size};
use waterui_graphics::{
    canvas::{Circle, Cx, Line, Path, StrokeStyle},
    color::{Srgb, WorkingColor},
};

use super::{
    canvas::{CartesianFrame, Frame},
    geometry::{
        BubbleRadii, CandleLayout, MapProjection, PLOT_PADDING_RATIO, area_layers, candle_layout,
        gauge_ring, grid_cell, map_xy, pie_circle, pie_slices, plot_rect, radar_angle,
        radar_circle, u32_to_f32, u32_to_usize, usize_to_f32,
    },
    hit::HitTargets,
};
use crate::{
    data::{
        AreaData, BubblePoint, Candle, ChoroplethData, ColorScale, ContourData, DataPoint,
        DepthData, GaugeData, HeatmapData, RadarData,
    },
    interaction::{GridDatum, RadarDatum, RegionDatum, SliceDatum},
};

const PIE_DEFAULT_COLORS: [u32; 8] = [
    0x003B_82F6,
    0x0022_C55E,
    0x00EF_4444,
    0x00F5_9E0B,
    0x008B_5CF6,
    0x00EC_4899,
    0x0006_B6D4,
    0x00F9_7316,
];
const VIRIDIS_STOPS: [(f32, Srgb); 5] = [
    (0.0, Srgb::from_hex("#440154")),
    (0.25, Srgb::from_hex("#3B528B")),
    (0.5, Srgb::from_hex("#21918C")),
    (0.75, Srgb::from_hex("#5EC962")),
    (1.0, Srgb::from_hex("#FDE725")),
];

/// One item of a keyed collection: the datum's place in the data, which
/// identifies it across frames, and what is drawn for it.
#[derive(Debug, Clone, PartialEq)]
struct Keyed<T> {
    key: usize,
    value: T,
}

/// `values` keyed by their position.
fn enumerated<T>(values: impl IntoIterator<Item = T>) -> Vec<Keyed<T>> {
    values
        .into_iter()
        .enumerate()
        .map(|(key, value)| Keyed { key, value })
        .collect()
}

const fn key_of<T>(item: &Keyed<T>) -> usize {
    item.key
}

/// The items `derive` makes of each frame, as a signal.
fn items<D, G, T>(
    frame: &Computed<Frame<D, G>>,
    derive: impl Fn(&Frame<D, G>) -> Vec<Keyed<T>> + Clone + 'static,
) -> Computed<Vec<Keyed<T>>>
where
    D: Clone + 'static,
    G: Clone + 'static,
    T: Clone + 'static,
{
    frame.map(move |frame| derive(&frame)).computed()
}

fn working(color: Srgb) -> WorkingColor {
    WorkingColor::from(color)
}

/// The colour of a `[r, g, b, a]` series colour, without its alpha.
const fn rgb(color: [f32; 4]) -> Srgb {
    Srgb::new(color[0], color[1], color[2])
}

/// The alpha of a `[r, g, b, a]` series colour.
const fn alpha(color: [f32; 4]) -> f32 {
    color[3].clamp(0.0, 1.0)
}

/// An open path through `points`.
fn polyline(points: impl IntoIterator<Item = Point>) -> Path {
    let mut points = points.into_iter();
    let Some(first) = points.next() else {
        return Path::builder().build();
    };
    points
        .fold(Path::builder().move_to(first), |path, point| {
            path.line_to(point)
        })
        .build()
}

/// A closed path through `points`.
fn polygon(points: impl IntoIterator<Item = Point>) -> Path {
    let mut points = points.into_iter();
    let Some(first) = points.next() else {
        return Path::builder().build();
    };
    points
        .fold(Path::builder().move_to(first), |path, point| {
            path.line_to(point)
        })
        .close()
        .build()
}

fn polar(center: Point, radius: f32, angle: f32) -> Point {
    Point::new(
        angle.cos().mul_add(radius, center.x),
        angle.sin().mul_add(radius, center.y),
    )
}

fn srgb_lerp(a: Srgb, b: Srgb, t: f32) -> Srgb {
    let t = t.clamp(0.0, 1.0);
    Srgb::new(
        (b.red - a.red).mul_add(t, a.red),
        (b.green - a.green).mul_add(t, a.green),
        (b.blue - a.blue).mul_add(t, a.blue),
    )
}

/// Interpolates `stops` at `value` within `min..=max`.
fn color_at(stops: &[(f32, Srgb)], value: f32, min: f32, max: f32) -> Srgb {
    let t = if max > min {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let mut previous = stops[0];
    if t <= previous.0 {
        return previous.1;
    }

    for &(stop_t, stop_color) in &stops[1..] {
        if t <= stop_t {
            let span = (stop_t - previous.0).max(f32::EPSILON);
            return srgb_lerp(previous.1, stop_color, (t - previous.0) / span);
        }
        previous = (stop_t, stop_color);
    }

    previous.1
}

fn color_from_scale(scale: &ColorScale, value: f32, min: f32, max: f32) -> Srgb {
    if scale.stops.is_empty() {
        return Srgb::new(0.5, 0.5, 0.5);
    }
    color_at(&scale.stops, value, min, max)
}

fn viridis(value: f32, min: f32, max: f32) -> Srgb {
    color_at(&VIRIDIS_STOPS, value, min, max)
}

// ---------------------------------------------------------------- line

#[derive(Debug, Clone, PartialEq)]
struct LineItem {
    line: Path,
    area: Path,
}

fn line_items(
    frame: &CartesianFrame<Vec<DataPoint>, DataPoint>,
    show_fill: bool,
) -> Vec<Keyed<LineItem>> {
    let data = &frame.data;
    if data.len() < 2 {
        return Vec::new();
    }
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    let points: Vec<Point> = data
        .iter()
        .map(|point| map_xy(plot, bounds, point.x, point.y))
        .collect();
    let area = if show_fill {
        let first = data[0].x;
        let last = data[data.len() - 1].x;
        polygon(points.iter().copied().chain([
            map_xy(plot, bounds, last, bounds.min_y),
            map_xy(plot, bounds, first, bounds.min_y),
        ]))
    } else {
        Path::builder().build()
    };
    enumerated([LineItem {
        line: polyline(points),
        area,
    }])
}

pub fn record_line(
    cx: &mut Cx,
    frame: &Computed<CartesianFrame<Vec<DataPoint>, DataPoint>>,
    color: Srgb,
    line_width: f32,
    show_fill: bool,
    fill_opacity: f32,
) {
    let color = working(color);
    let lines = items(frame, move |frame| line_items(frame, show_fill));
    cx.each(lines, key_of, move |cx, line| {
        if show_fill {
            cx.fill(
                line.map(|line| line.value.area),
                color.with_alpha(fill_opacity),
            );
        }
        cx.stroke(
            line.map(|line| line.value.line),
            StrokeStyle::new(line_width),
            color,
        );
    });
}

// ----------------------------------------------------------------- bar

fn bar_items(frame: &CartesianFrame<Vec<DataPoint>, DataPoint>) -> Vec<Keyed<Rect>> {
    let data = &frame.data;
    if data.is_empty() {
        return Vec::new();
    }
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    let bar_width = (plot.width() / usize_to_f32(data.len())) * 0.7;
    let baseline_y = map_xy(plot, bounds, bounds.min_x, 0.0).y;
    enumerated(data.iter().map(|point| {
        let center = map_xy(plot, bounds, point.x, point.y);
        let top = center.y.min(baseline_y);
        let bottom = center.y.max(baseline_y);
        Rect::new(
            Point::new(bar_width.mul_add(-0.5, center.x), top),
            Size::new(bar_width.max(1.0), (bottom - top).max(1.0)),
        )
    }))
}

pub fn record_bar(
    cx: &mut Cx,
    frame: &Computed<CartesianFrame<Vec<DataPoint>, DataPoint>>,
    color: Srgb,
) {
    let color = working(color);
    let bars = items(frame, bar_items);
    cx.each(bars, key_of, move |cx, bar| {
        cx.fill(bar.map(|bar| bar.value), color);
    });
}

// ------------------------------------------------------------- scatter

fn scatter_items(
    frame: &CartesianFrame<Vec<DataPoint>, DataPoint>,
    radius: f32,
) -> Vec<Keyed<Circle>> {
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    enumerated(
        frame
            .data
            .iter()
            .map(|point| Circle::new(map_xy(plot, bounds, point.x, point.y), radius)),
    )
}

pub fn record_scatter(
    cx: &mut Cx,
    frame: &Computed<CartesianFrame<Vec<DataPoint>, DataPoint>>,
    color: Srgb,
    radius: f32,
) {
    let color = working(color);
    let dots = items(frame, move |frame| scatter_items(frame, radius));
    cx.each(dots, key_of, move |cx, dot| {
        cx.fill(dot.map(|dot| dot.value), color);
    });
}

// -------------------------------------------------------------- bubble

#[derive(Debug, Clone, PartialEq)]
struct BubbleItem {
    circle: Circle,
    color: WorkingColor,
}

fn bubble_items(
    frame: &CartesianFrame<Vec<BubblePoint>, BubblePoint>,
    default_color: Srgb,
    min_radius: f32,
    max_radius: f32,
    opacity: f32,
) -> Vec<Keyed<BubbleItem>> {
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    let radii = BubbleRadii::new(&frame.data, min_radius, max_radius);
    enumerated(frame.data.iter().map(|point| {
        let (color, point_alpha) = if point.color[3] > 0.0 {
            (rgb(point.color), alpha(point.color))
        } else {
            (default_color, opacity)
        };
        BubbleItem {
            circle: Circle::new(map_xy(plot, bounds, point.x, point.y), radii.radius(point)),
            color: working(color).with_alpha((point_alpha * opacity).clamp(0.0, 1.0)),
        }
    }))
}

pub fn record_bubble(
    cx: &mut Cx,
    frame: &Computed<CartesianFrame<Vec<BubblePoint>, BubblePoint>>,
    default_color: Srgb,
    min_radius: f32,
    max_radius: f32,
    opacity: f32,
) {
    let bubbles = items(frame, move |frame| {
        bubble_items(frame, default_color, min_radius, max_radius, opacity)
    });
    cx.each(bubbles, key_of, move |cx, bubble| {
        cx.fill(
            bubble.map(|bubble| bubble.value.circle),
            bubble.map(|bubble| bubble.value.color),
        );
    });
}

// --------------------------------------------------------- candlestick

#[derive(Debug, Clone, PartialEq)]
struct CandleItem {
    wick: Line,
    body: Rect,
    color: WorkingColor,
}

fn candle_items(
    frame: &CartesianFrame<Vec<Candle>, Candle>,
    bullish: Srgb,
    bearish: Srgb,
) -> Vec<Keyed<CandleItem>> {
    let data = &frame.data;
    if data.is_empty() {
        return Vec::new();
    }
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    let candle_width = (plot.width() / usize_to_f32(data.len()) * 0.65).max(1.0);
    let epoch = crate::data::candle_epoch(data);
    enumerated(data.iter().map(|candle| {
        let CandleLayout {
            x,
            high,
            low,
            open,
            close,
        } = candle_layout(plot, bounds, candle, epoch);
        let top = open.min(close);
        let bottom = open.max(close);
        CandleItem {
            wick: Line::new(Point::new(x, high), Point::new(x, low)),
            body: Rect::new(
                Point::new(candle_width.mul_add(-0.5, x), top),
                Size::new(candle_width, (bottom - top).max(1.0)),
            ),
            color: working(if candle.close >= candle.open {
                bullish
            } else {
                bearish
            }),
        }
    }))
}

pub fn record_candlestick(
    cx: &mut Cx,
    frame: &Computed<CartesianFrame<Vec<Candle>, Candle>>,
    bullish: Srgb,
    bearish: Srgb,
) {
    let candles = items(frame, move |frame| candle_items(frame, bullish, bearish));
    cx.each(candles, key_of, move |cx, candle| {
        cx.stroke(
            candle.map(|candle| candle.value.wick),
            StrokeStyle::new(1.0),
            candle.map(|candle| candle.value.color),
        );
        cx.fill(
            candle.map(|candle| candle.value.body),
            candle.map(|candle| candle.value.color),
        );
    });
}

// --------------------------------------------------------------- depth

#[derive(Debug, Clone, PartialEq)]
struct DepthLayer {
    fill: Path,
    outline: Path,
    color: WorkingColor,
}

fn depth_items(
    frame: &CartesianFrame<DepthData, crate::interaction::DepthDatum>,
    bid: Srgb,
    ask: Srgb,
) -> Vec<Keyed<DepthLayer>> {
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    let layer = |levels: &[crate::data::DepthLevel], color: Srgb| {
        let (first, last) = (levels.first()?, levels.last()?);
        let curve: Vec<Point> = levels
            .iter()
            .map(|level| map_xy(plot, bounds, level.price, level.cumulative_volume))
            .collect();
        Some(DepthLayer {
            fill: polygon(
                core::iter::once(map_xy(plot, bounds, first.price, 0.0))
                    .chain(curve.iter().copied())
                    .chain(core::iter::once(map_xy(plot, bounds, last.price, 0.0))),
            ),
            outline: polyline(curve),
            color: working(color),
        })
    };
    [
        (0, layer(&frame.data.bids, bid)),
        (1, layer(&frame.data.asks, ask)),
    ]
    .into_iter()
    .filter_map(|(key, layer)| layer.map(|value| Keyed { key, value }))
    .collect()
}

pub fn record_depth(
    cx: &mut Cx,
    frame: &Computed<CartesianFrame<DepthData, crate::interaction::DepthDatum>>,
    bid: Srgb,
    ask: Srgb,
) {
    let layers = items(frame, move |frame| depth_items(frame, bid, ask));
    cx.each(layers, key_of, move |cx, layer| {
        cx.fill(
            layer.map(|layer| layer.value.fill),
            layer.map(|layer| layer.value.color.with_alpha(0.28)),
        );
        cx.stroke(
            layer.map(|layer| layer.value.outline),
            StrokeStyle::new(2.0),
            layer.map(|layer| layer.value.color),
        );
    });
}

// ---------------------------------------------------------------- area

#[derive(Debug, Clone, PartialEq)]
struct AreaLayer {
    fill: Path,
    top: Path,
    fill_color: WorkingColor,
    line_color: WorkingColor,
}

fn area_items(
    frame: &CartesianFrame<AreaData, crate::interaction::AreaDatum>,
) -> Vec<Keyed<AreaLayer>> {
    let data = &frame.data;
    if data.x_values.is_empty() {
        return Vec::new();
    }
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    area_layers(data)
        .into_iter()
        .zip(&data.series)
        .enumerate()
        .filter(|(_, ((base, _), _))| !base.is_empty())
        .map(|(key, ((base, top), series))| {
            let edge = |values: &[f32]| -> Vec<Point> {
                data.x_values
                    .iter()
                    .zip(values)
                    .map(|(x, y)| map_xy(plot, bounds, *x, *y))
                    .collect()
            };
            let top_edge = edge(&top);
            let base_edge = edge(&base);
            Keyed {
                key,
                value: AreaLayer {
                    fill: polygon(top_edge.iter().copied().chain(base_edge.into_iter().rev())),
                    top: polyline(top_edge),
                    fill_color: working(rgb(series.color)).with_alpha(alpha(series.color)),
                    line_color: working(rgb(series.color)),
                },
            }
        })
        .collect()
}

pub fn record_area(
    cx: &mut Cx,
    frame: &Computed<CartesianFrame<AreaData, crate::interaction::AreaDatum>>,
) {
    let layers = items(frame, area_items);
    cx.each(layers, key_of, move |cx, layer| {
        cx.fill(
            layer.map(|layer| layer.value.fill),
            layer.map(|layer| layer.value.fill_color),
        );
        cx.stroke(
            layer.map(|layer| layer.value.top),
            StrokeStyle::new(1.5),
            layer.map(|layer| layer.value.line_color),
        );
    });
}

// ----------------------------------------------------------------- pie

#[derive(Debug, Clone, PartialEq)]
struct Slice {
    path: Path,
    color: WorkingColor,
}

fn slice_path(center: Point, inner_r: f32, outer_r: f32, start: f32, end: f32) -> Path {
    // `end` was accumulated from `start`; rounding can leave their
    // difference a hair past a full turn, which an arc rejects.
    let sweep = (end - start).clamp(-TAU, TAU);
    let builder = Path::builder().move_to(polar(center, outer_r, start)).arc(
        center,
        (outer_r, outer_r),
        start,
        sweep,
    );
    if inner_r > 0.0 {
        builder
            .line_to(polar(center, inner_r, end))
            .arc(center, (inner_r, inner_r), end, -sweep)
    } else {
        builder.line_to(center)
    }
    .close()
    .build()
}

fn pie_items(
    frame: &Frame<Vec<DataPoint>, HitTargets<SliceDatum>>,
    colors: &[Srgb],
    inner_radius: f32,
) -> Vec<Keyed<Slice>> {
    let (center, outer_r) = pie_circle(frame.size);
    let inner_r = outer_r * inner_radius;
    pie_slices(&frame.data)
        .into_iter()
        .map(|(key, start, end)| Keyed {
            key,
            value: Slice {
                path: slice_path(center, inner_r, outer_r, start, end),
                color: working(colors.get(key).copied().unwrap_or_else(|| {
                    Srgb::from_u32(PIE_DEFAULT_COLORS[key % PIE_DEFAULT_COLORS.len()])
                })),
            },
        })
        .collect()
}

pub fn record_pie(
    cx: &mut Cx,
    frame: &Computed<Frame<Vec<DataPoint>, HitTargets<SliceDatum>>>,
    colors: Vec<Srgb>,
    inner_radius: f32,
) {
    let slices = items(frame, move |frame| pie_items(frame, &colors, inner_radius));
    cx.each(slices, key_of, move |cx, slice| {
        cx.fill(
            slice.map(|slice| slice.value.path),
            slice.map(|slice| slice.value.color),
        );
    });
}

// --------------------------------------------------------------- gauge

#[derive(Debug, Clone, PartialEq)]
struct RingArc {
    path: Path,
    width: f32,
    color: WorkingColor,
}

#[derive(Debug, Clone, PartialEq)]
struct Needle {
    line: Line,
    width: f32,
    hub: Circle,
}

/// The angles and radii a gauge is drawn with.
#[derive(Debug, Clone, Copy)]
struct GaugeStyle {
    start_angle: f32,
    end_angle: f32,
    inner_radius: f32,
    outer_radius: f32,
    background: Srgb,
    value: Srgb,
}

fn gauge_arcs(
    frame: &Frame<GaugeData, HitTargets<SliceDatum>>,
    style: GaugeStyle,
) -> Vec<Keyed<RingArc>> {
    let data = &frame.data;
    let (center, inner_r, outer_r) = gauge_ring(frame.size, style.inner_radius, style.outer_radius);
    let width = (outer_r - inner_r).max(1.0);
    let ring_r = width.mul_add(0.5, inner_r);
    let arc = |from: f32, to: f32, color: WorkingColor| RingArc {
        path: Path::builder()
            .arc(center, (ring_r, ring_r), from, to - from)
            .build(),
        width,
        color,
    };

    let span = style.end_angle - style.start_angle;
    let mut arcs = alloc::vec![arc(
        style.start_angle,
        style.end_angle,
        working(style.background)
    )];
    if data.regions.is_empty() {
        let value_end = span.mul_add(data.normalized_value(), style.start_angle);
        arcs.push(arc(style.start_angle, value_end, working(style.value)));
    } else {
        let value_span = data.max_value - data.min_value;
        assert!(
            value_span.is_finite() && value_span > 0.0,
            "a gauge with regions requires finite max_value > min_value"
        );
        let mut last_threshold = data.min_value;
        for region in &data.regions {
            let start_t = ((last_threshold - data.min_value) / value_span).clamp(0.0, 1.0);
            let end_t = ((region.threshold - data.min_value) / value_span).clamp(0.0, 1.0);
            arcs.push(arc(
                span.mul_add(start_t, style.start_angle),
                span.mul_add(end_t, style.start_angle),
                working(rgb(region.color)),
            ));
            last_threshold = region.threshold;
        }
    }
    enumerated(arcs)
}

fn gauge_needle(
    frame: &Frame<GaugeData, HitTargets<SliceDatum>>,
    style: GaugeStyle,
) -> Vec<Keyed<Needle>> {
    let data = &frame.data;
    if !data.show_needle {
        return Vec::new();
    }
    let (center, inner_r, outer_r) = gauge_ring(frame.size, style.inner_radius, style.outer_radius);
    let width = (outer_r - inner_r).max(1.0);
    let value_end =
        (style.end_angle - style.start_angle).mul_add(data.normalized_value(), style.start_angle);
    enumerated([Needle {
        line: Line::new(center, polar(center, outer_r * 0.95, value_end)),
        width: (width * 0.12).max(2.0),
        hub: Circle::new(center, (width * 0.16).max(3.0)),
    }])
}

#[expect(
    clippy::too_many_arguments,
    reason = "Gauge recording consumes independent geometric and color parameters from the public builder state without allocating an intermediate style object"
)]
pub fn record_gauge(
    cx: &mut Cx,
    frame: &Computed<Frame<GaugeData, HitTargets<SliceDatum>>>,
    start_angle: f32,
    end_angle: f32,
    inner_radius: f32,
    outer_radius: f32,
    background: Srgb,
    value: Srgb,
    needle: Srgb,
) {
    let style = GaugeStyle {
        start_angle,
        end_angle,
        inner_radius,
        outer_radius,
        background,
        value,
    };
    let arcs = items(frame, move |frame| gauge_arcs(frame, style));
    cx.each(arcs, key_of, move |cx, arc| {
        cx.stroke(
            arc.map(|arc| arc.value.path),
            arc.map(|arc| StrokeStyle::new(arc.value.width)),
            arc.map(|arc| arc.value.color),
        );
    });
    let needle_color = working(needle);
    let needles = items(frame, move |frame| gauge_needle(frame, style));
    cx.each(needles, key_of, move |cx, needle| {
        cx.stroke(
            needle.map(|needle| needle.value.line),
            needle.map(|needle| StrokeStyle::new(needle.value.width)),
            needle_color,
        );
        cx.fill(needle.map(|needle| needle.value.hub), needle_color);
    });
}

// --------------------------------------------------------------- radar

#[derive(Debug, Clone, PartialEq)]
struct RadarSeriesItem {
    polygon: Path,
    color: WorkingColor,
    fill_alpha: f32,
}

/// Whether a radar has anything to draw.
const fn radar_drawn(data: &RadarData) -> bool {
    data.axis_count >= 3 && !data.series.is_empty()
}

fn radar_rings(
    frame: &Frame<RadarData, HitTargets<RadarDatum>>,
    ring_count: u32,
) -> Vec<Keyed<Path>> {
    let data = &frame.data;
    if !radar_drawn(data) {
        return Vec::new();
    }
    let (center, radius) = radar_circle(frame.size);
    let axis_count = u32_to_usize(data.axis_count);
    let rings = ring_count.max(1);
    enumerated((1..=rings).map(|ring| {
        let r = radius * (u32_to_f32(ring) / u32_to_f32(rings));
        polygon((0..axis_count).map(|axis| polar(center, r, radar_angle(axis, axis_count))))
    }))
}

fn radar_axes(frame: &Frame<RadarData, HitTargets<RadarDatum>>) -> Vec<Keyed<Line>> {
    let data = &frame.data;
    if !radar_drawn(data) {
        return Vec::new();
    }
    let (center, radius) = radar_circle(frame.size);
    let axis_count = u32_to_usize(data.axis_count);
    enumerated(
        (0..axis_count)
            .map(|axis| Line::new(center, polar(center, radius, radar_angle(axis, axis_count)))),
    )
}

fn radar_series(
    frame: &Frame<RadarData, HitTargets<RadarDatum>>,
    fill_opacity: f32,
) -> Vec<Keyed<RadarSeriesItem>> {
    let data = &frame.data;
    if !radar_drawn(data) {
        return Vec::new();
    }
    let (center, radius) = radar_circle(frame.size);
    let axis_count = u32_to_usize(data.axis_count);
    let max_value = data.max_value.max(1.0);
    data.series
        .iter()
        .enumerate()
        .filter(|(_, series)| series.values.len() >= axis_count)
        .map(|(key, series)| Keyed {
            key,
            value: RadarSeriesItem {
                polygon: polygon((0..axis_count).map(|axis| {
                    let ratio = (series.values[axis] / max_value).clamp(0.0, 1.0);
                    polar(center, radius * ratio, radar_angle(axis, axis_count))
                })),
                color: working(rgb(series.color)),
                fill_alpha: (fill_opacity * alpha(series.color)).clamp(0.0, 1.0),
            },
        })
        .collect()
}

pub fn record_radar(
    cx: &mut Cx,
    frame: &Computed<Frame<RadarData, HitTargets<RadarDatum>>>,
    ring_count: u32,
    line_width: f32,
    fill_opacity: f32,
) {
    let grid = working(Srgb::new(0.5, 0.5, 0.5));
    let rings = items(frame, move |frame| radar_rings(frame, ring_count));
    cx.each(rings, key_of, move |cx, ring| {
        cx.stroke(ring.map(|ring| ring.value), StrokeStyle::new(1.0), grid);
    });
    let axes = items(frame, radar_axes);
    cx.each(axes, key_of, move |cx, axis| {
        cx.stroke(axis.map(|axis| axis.value), StrokeStyle::new(1.0), grid);
    });
    let series = items(frame, move |frame| radar_series(frame, fill_opacity));
    cx.each(series, key_of, move |cx, series| {
        cx.fill(
            series.map(|series| series.value.polygon),
            series.map(|series| series.value.color.with_alpha(series.value.fill_alpha)),
        );
        cx.stroke(
            series.map(|series| series.value.polygon),
            StrokeStyle::new(line_width),
            series.map(|series| series.value.color),
        );
    });
}

// ------------------------------------------------------------- heatmap

#[derive(Debug, Clone, PartialEq)]
struct Cell {
    rect: Rect,
    color: WorkingColor,
}

/// One cell for each value of a `rows` by `cols` grid, coloured by `viridis`
/// at `alpha`.
fn heat_cells(
    size: Size,
    rows: u32,
    cols: u32,
    values: &[f32],
    range: (f32, f32),
    alpha: f32,
) -> Vec<Keyed<Cell>> {
    if rows == 0 || cols == 0 || values.is_empty() {
        return Vec::new();
    }
    let plot = plot_rect(size, PLOT_PADDING_RATIO);
    let cols_n = u32_to_usize(cols);
    (0..u32_to_usize(rows))
        .flat_map(|row| (0..cols_n).map(move |col| (row, col)))
        .map(|(row, col)| {
            let key = row * cols_n + col;
            Keyed {
                key,
                value: Cell {
                    rect: grid_cell(plot, rows, cols, row, col),
                    color: working(viridis(values[key], range.0, range.1)).with_alpha(alpha),
                },
            }
        })
        .collect()
}

fn record_cells(cx: &mut Cx, cells: Computed<Vec<Keyed<Cell>>>) {
    cx.each(cells, key_of, move |cx, cell| {
        cx.fill(
            cell.map(|cell| cell.value.rect),
            cell.map(|cell| cell.value.color),
        );
    });
}

pub fn record_heatmap(cx: &mut Cx, frame: &Computed<Frame<HeatmapData, HitTargets<GridDatum>>>) {
    let cells = items(frame, |frame| {
        let data = &frame.data;
        heat_cells(
            frame.size,
            data.rows,
            data.cols,
            &data.values,
            (data.min_value, data.max_value),
            1.0,
        )
    });
    record_cells(cx, cells);
}

// ------------------------------------------------------------- contour

#[derive(Debug, Clone, PartialEq)]
struct Level {
    path: Path,
    color: WorkingColor,
}

fn contour_interpolate(p1: Point, p2: Point, v1: f32, v2: f32, level: f32) -> Point {
    let denom = (v2 - v1).abs();
    let t = if denom <= f32::EPSILON {
        0.5
    } else {
        (level - v1) / (v2 - v1)
    }
    .clamp(0.0, 1.0);

    Point::new(
        (p2.x - p1.x).mul_add(t, p1.x),
        (p2.y - p1.y).mul_add(t, p1.y),
    )
}

/// Whether a contour has anything to draw.
const fn contour_drawn(data: &ContourData) -> bool {
    data.rows >= 2 && data.cols >= 2 && !data.values.is_empty() && !data.levels.is_empty()
}

fn contour_levels(frame: &Frame<ContourData, HitTargets<GridDatum>>) -> Vec<Keyed<Level>> {
    let data = &frame.data;
    if !contour_drawn(data) {
        return Vec::new();
    }
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let step_x = plot.width() / u32_to_f32(data.cols - 1);
    let step_y = plot.height() / u32_to_f32(data.rows - 1);

    data.levels
        .iter()
        .enumerate()
        .filter_map(|(key, &level)| {
            let mut path = Path::builder();
            let mut has_segment = false;

            for row in 0..(data.rows - 1) {
                for col in 0..(data.cols - 1) {
                    let i00 = u32_to_usize(row * data.cols + col);
                    let i10 = i00 + 1;
                    let i01 = u32_to_usize((row + 1) * data.cols + col);
                    let i11 = i01 + 1;

                    let v00 = data.values[i00];
                    let v10 = data.values[i10];
                    let v01 = data.values[i01];
                    let v11 = data.values[i11];

                    let p00 = Point::new(
                        u32_to_f32(col).mul_add(step_x, plot.min_x()),
                        u32_to_f32(row).mul_add(step_y, plot.min_y()),
                    );
                    let p10 = Point::new(p00.x + step_x, p00.y);
                    let p01 = Point::new(p00.x, p00.y + step_y);
                    let p11 = Point::new(p00.x + step_x, p00.y + step_y);

                    let mut points = [Point::new(0.0, 0.0); 4];
                    let mut point_count = 0;
                    if (v00 > level) != (v10 > level) {
                        points[point_count] = contour_interpolate(p00, p10, v00, v10, level);
                        point_count += 1;
                    }
                    if (v10 > level) != (v11 > level) {
                        points[point_count] = contour_interpolate(p10, p11, v10, v11, level);
                        point_count += 1;
                    }
                    if (v01 > level) != (v11 > level) {
                        points[point_count] = contour_interpolate(p01, p11, v01, v11, level);
                        point_count += 1;
                    }
                    if (v00 > level) != (v01 > level) {
                        points[point_count] = contour_interpolate(p00, p01, v00, v01, level);
                        point_count += 1;
                    }

                    if point_count == 2 || point_count == 4 {
                        for segment in points[..point_count].as_chunks::<2>().0 {
                            path = path.move_to(segment[0]).line_to(segment[1]);
                        }
                        has_segment = true;
                    }
                }
            }

            has_segment.then(|| Keyed {
                key,
                value: Level {
                    path: path.build(),
                    color: working(viridis(level, data.min_value, data.max_value)),
                },
            })
        })
        .collect()
}

pub fn record_contour(
    cx: &mut Cx,
    frame: &Computed<Frame<ContourData, HitTargets<GridDatum>>>,
    line_width: f32,
) {
    // A faint tint of the field under the lines.
    let tint = items(frame, |frame| {
        let data = &frame.data;
        if !contour_drawn(data) {
            return Vec::new();
        }
        heat_cells(
            frame.size,
            data.rows,
            data.cols,
            &data.values,
            (data.min_value, data.max_value),
            0.08,
        )
    });
    record_cells(cx, tint);
    let levels = items(frame, contour_levels);
    cx.each(levels, key_of, move |cx, level| {
        cx.stroke(
            level.map(|level| level.value.path),
            StrokeStyle::new(line_width),
            level.map(|level| level.value.color),
        );
    });
}

// ---------------------------------------------------------- choropleth

#[derive(Debug, Clone, PartialEq)]
struct Region {
    path: Path,
    color: WorkingColor,
}

fn choropleth_regions(
    frame: &Frame<ChoroplethData, HitTargets<RegionDatum>>,
) -> Vec<Keyed<Region>> {
    let data = &frame.data;
    let projection = MapProjection::new(frame.size, data);
    data.polygons
        .iter()
        .enumerate()
        .filter(|(_, polygon)| polygon.vertices.len() >= 3)
        .map(|(key, region)| Keyed {
            key,
            value: Region {
                path: polygon(
                    region
                        .vertices
                        .iter()
                        .map(|vertex| projection.point(*vertex)),
                ),
                color: working(color_from_scale(
                    &data.color_scale,
                    region.value,
                    data.min_value,
                    data.max_value,
                )),
            },
        })
        .collect()
}

pub fn record_choropleth(
    cx: &mut Cx,
    frame: &Computed<Frame<ChoroplethData, HitTargets<RegionDatum>>>,
    stroke_width: f32,
    stroke_color: Srgb,
    show_stroke: bool,
) {
    let stroke_color = working(stroke_color);
    let regions = items(frame, choropleth_regions);
    cx.each(regions, key_of, move |cx, region| {
        cx.fill(
            region.map(|region| region.value.path),
            region.map(|region| region.value.color),
        );
        if show_stroke {
            cx.stroke(
                region.map(|region| region.value.path),
                StrokeStyle::new(stroke_width),
                stroke_color,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_circle_slice_is_one_closed_path() {
        let center = Point::new(50.0, 50.0);
        let path = slice_path(center, 0.0, 40.0, 0.0, TAU);
        assert!(path.contains(center));
        assert!(!path.contains(Point::new(95.0, 50.0)));
    }

    #[test]
    fn a_donut_slice_leaves_its_hole_open() {
        let center = Point::new(50.0, 50.0);
        let path = slice_path(center, 20.0, 40.0, 0.0, TAU / 2.0);
        assert!(!path.contains(center));
        assert!(path.contains(Point::new(50.0, 80.0)));
        assert!(!path.contains(Point::new(50.0, 20.0)));
    }

    #[test]
    fn viridis_runs_from_its_first_to_its_last_stop() {
        assert_eq!(viridis(0.0, 0.0, 1.0), VIRIDIS_STOPS[0].1);
        assert_eq!(viridis(1.0, 0.0, 1.0), VIRIDIS_STOPS[4].1);
    }
}
