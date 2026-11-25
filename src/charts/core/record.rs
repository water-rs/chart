//! How each chart is recorded onto its canvas.
//!
//! Every function here runs once per layer, inside the canvas closure, and
//! records the chart as keyed sub-recordings over a signal of its frame: one
//! item per series, bar, point or cell, keyed by the datum's place in the
//! data. A frame change that keeps a key updates that item's operands in
//! place; one that adds or drops keys records or drops only those items.

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::{f32::consts::TAU, iter};

use nami::{Computed, SignalExt as _};
use waterui_core::{
    Angle, Environment,
    layout::{Point, Rect, Size},
};
use waterui_graphics::{
    canvas::{Circle, Cx, Line, Path, StrokeStyle},
    color::{Color, WorkingColor},
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
        AreaData, BubblePoint, Candle, ChoroplethData, ContourData, DataPoint, DepthData,
        DepthLevel, GaugeData, HeatmapData, RadarData, candle_epoch,
    },
    interaction::{AreaDatum, DepthDatum, GridDatum, RadarDatum, RegionDatum, SliceDatum},
    theme_color::{ResolvedColorScale, categorical_color, with_resolved_colors},
};

const DEFAULT_AREA_FILL_ALPHA: f32 = 0.7;

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
    color: &Computed<WorkingColor>,
    line_width: f32,
    show_fill: bool,
    fill_opacity: f32,
) {
    let fill_color = color
        .map(move |color| color.with_alpha((color.components[3] * fill_opacity).clamp(0.0, 1.0)))
        .computed();
    let stroke_color = color.clone();
    let lines = items(frame, move |frame| line_items(frame, show_fill));
    cx.each(lines, key_of, move |cx, line| {
        if show_fill {
            cx.fill(line.map(|line| line.value.area), fill_color.clone());
        }
        cx.stroke(
            line.map(|line| line.value.line),
            StrokeStyle::new(line_width),
            stroke_color.clone(),
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
    color: &Computed<WorkingColor>,
) {
    let color = color.clone();
    let bars = items(frame, bar_items);
    cx.each(bars, key_of, move |cx, bar| {
        cx.fill(bar.map(|bar| bar.value), color.clone());
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
    color: &Computed<WorkingColor>,
    radius: f32,
) {
    let color = color.clone();
    let dots = items(frame, move |frame| scatter_items(frame, radius));
    cx.each(dots, key_of, move |cx, dot| {
        cx.fill(dot.map(|dot| dot.value), color.clone());
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
    colors: &[WorkingColor],
    min_radius: f32,
    max_radius: f32,
    opacity: f32,
) -> Vec<Keyed<BubbleItem>> {
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    let radii = BubbleRadii::new(&frame.data, min_radius, max_radius);
    enumerated(
        frame
            .data
            .iter()
            .zip(colors)
            .map(|(point, color)| BubbleItem {
                circle: Circle::new(map_xy(plot, bounds, point.x, point.y), radii.radius(point)),
                color: color.with_alpha((color.components[3] * opacity).clamp(0.0, 1.0)),
            }),
    )
}

pub fn record_bubble(
    cx: &mut Cx,
    frame: &Computed<CartesianFrame<Vec<BubblePoint>, BubblePoint>>,
    env: &Environment,
    default_color: &Computed<WorkingColor>,
    min_radius: f32,
    max_radius: f32,
    opacity: f32,
) {
    let default_color = default_color.clone();
    let resolved = with_resolved_colors(frame, env, move |frame, env| {
        frame
            .data
            .iter()
            .map(|point| {
                point
                    .color
                    .as_ref()
                    .map_or_else(|| default_color.clone(), |color| color.resolve(env))
            })
            .collect()
    });
    let bubbles = resolved
        .map(move |(frame, colors)| bubble_items(&frame, &colors, min_radius, max_radius, opacity))
        .computed();
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
    bullish: WorkingColor,
    bearish: WorkingColor,
) -> Vec<Keyed<CandleItem>> {
    let data = &frame.data;
    if data.is_empty() {
        return Vec::new();
    }
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    let candle_width = (plot.width() / usize_to_f32(data.len()) * 0.65).max(1.0);
    let epoch = candle_epoch(data);
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
            color: if candle.close >= candle.open {
                bullish
            } else {
                bearish
            },
        }
    }))
}

pub fn record_candlestick(
    cx: &mut Cx,
    frame: &Computed<CartesianFrame<Vec<Candle>, Candle>>,
    bullish: &Computed<WorkingColor>,
    bearish: &Computed<WorkingColor>,
) {
    let candles = frame
        .zip(bullish)
        .zip(bearish)
        .map(|((frame, bullish), bearish)| candle_items(&frame, bullish, bearish))
        .computed();
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
    frame: &CartesianFrame<DepthData, DepthDatum>,
    bid: WorkingColor,
    ask: WorkingColor,
) -> Vec<Keyed<DepthLayer>> {
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    let layer = |levels: &[DepthLevel], color: WorkingColor| {
        let (first, last) = (levels.first()?, levels.last()?);
        let curve: Vec<Point> = levels
            .iter()
            .map(|level| map_xy(plot, bounds, level.price, level.cumulative_volume))
            .collect();
        Some(DepthLayer {
            fill: polygon(
                iter::once(map_xy(plot, bounds, first.price, 0.0))
                    .chain(curve.iter().copied())
                    .chain(iter::once(map_xy(plot, bounds, last.price, 0.0))),
            ),
            outline: polyline(curve),
            color,
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
    frame: &Computed<CartesianFrame<DepthData, DepthDatum>>,
    bid: &Computed<WorkingColor>,
    ask: &Computed<WorkingColor>,
) {
    let layers = frame
        .zip(bid)
        .zip(ask)
        .map(|((frame, bid), ask)| depth_items(&frame, bid, ask))
        .computed();
    cx.each(layers, key_of, move |cx, layer| {
        cx.fill(
            layer.map(|layer| layer.value.fill),
            layer.map(|layer| {
                layer
                    .value
                    .color
                    .with_alpha((layer.value.color.components[3] * 0.28).clamp(0.0, 1.0))
            }),
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
    frame: &CartesianFrame<AreaData, AreaDatum>,
    colors: &[WorkingColor],
) -> Vec<Keyed<AreaLayer>> {
    let data = &frame.data;
    if data.x_values.is_empty() {
        return Vec::new();
    }
    let plot = plot_rect(frame.size, PLOT_PADDING_RATIO);
    let bounds = frame.geometry.bounds;
    area_layers(data)
        .into_iter()
        .zip(colors)
        .enumerate()
        .filter(|(_, ((base, _), _))| !base.is_empty())
        .map(|(key, ((base, top), &color))| {
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
                    fill_color: color.with_alpha(
                        (color.components[3] * DEFAULT_AREA_FILL_ALPHA).clamp(0.0, 1.0),
                    ),
                    line_color: color,
                },
            }
        })
        .collect()
}

pub fn record_area(
    cx: &mut Cx,
    frame: &Computed<CartesianFrame<AreaData, AreaDatum>>,
    env: &Environment,
) {
    let resolved = with_resolved_colors(frame, env, |frame, env| {
        frame
            .data
            .series
            .iter()
            .enumerate()
            .map(|(index, series)| {
                series
                    .color
                    .as_ref()
                    .map_or_else(|| categorical_color(index), Clone::clone)
                    .resolve(env)
            })
            .collect()
    });
    let layers = resolved
        .map(|(frame, colors)| area_items(&frame, &colors))
        .computed();
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
        Angle::radians(start),
        Angle::radians(sweep),
    );
    if inner_r > 0.0 {
        builder.line_to(polar(center, inner_r, end)).arc(
            center,
            (inner_r, inner_r),
            Angle::radians(end),
            Angle::radians(-sweep),
        )
    } else {
        builder.line_to(center)
    }
    .close()
    .build()
}

fn pie_items(
    frame: &Frame<Vec<DataPoint>, HitTargets<SliceDatum>>,
    colors: &[WorkingColor],
    defaults: &[WorkingColor],
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
                color: colors
                    .get(key)
                    .copied()
                    .unwrap_or_else(|| defaults[key % defaults.len()]),
            },
        })
        .collect()
}

pub fn record_pie(
    cx: &mut Cx,
    frame: &Computed<Frame<Vec<DataPoint>, HitTargets<SliceDatum>>>,
    colors: &Computed<Vec<WorkingColor>>,
    defaults: &Computed<Vec<WorkingColor>>,
    inner_radius: f32,
) {
    let slices = frame
        .zip(colors)
        .zip(defaults)
        .map(move |((frame, colors), defaults)| pie_items(&frame, &colors, &defaults, inner_radius))
        .computed();
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
}

fn gauge_arcs(
    frame: &Frame<GaugeData, HitTargets<SliceDatum>>,
    style: GaugeStyle,
    colors: &[WorkingColor],
) -> Vec<Keyed<RingArc>> {
    let data = &frame.data;
    let (center, inner_r, outer_r) = gauge_ring(frame.size, style.inner_radius, style.outer_radius);
    let width = (outer_r - inner_r).max(1.0);
    let ring_r = width.mul_add(0.5, inner_r);
    let arc = |from: f32, to: f32, color: WorkingColor| RingArc {
        path: Path::builder()
            .arc(
                center,
                (ring_r, ring_r),
                Angle::radians(from),
                Angle::radians(to - from),
            )
            .build(),
        width,
        color,
    };

    let span = style.end_angle - style.start_angle;
    let background = colors[0];
    let value_color = colors[1];
    let mut arcs = vec![arc(style.start_angle, style.end_angle, background)];
    if data.regions.is_empty() {
        let value_end = span.mul_add(data.normalized_value(), style.start_angle);
        arcs.push(arc(style.start_angle, value_end, value_color));
    } else {
        let value_span = data.max_value - data.min_value;
        assert!(
            value_span.is_finite() && value_span > 0.0,
            "a gauge with regions requires finite max_value > min_value"
        );
        let mut last_threshold = data.min_value;
        for (region, &color) in data.regions.iter().zip(&colors[2..]) {
            let start_t = ((last_threshold - data.min_value) / value_span).clamp(0.0, 1.0);
            let end_t = ((region.threshold - data.min_value) / value_span).clamp(0.0, 1.0);
            arcs.push(arc(
                span.mul_add(start_t, style.start_angle),
                span.mul_add(end_t, style.start_angle),
                color,
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
    env: &Environment,
    start_angle: f32,
    end_angle: f32,
    inner_radius: f32,
    outer_radius: f32,
    background: &Computed<WorkingColor>,
    value: &Computed<WorkingColor>,
    needle: &Computed<WorkingColor>,
) {
    let style = GaugeStyle {
        start_angle,
        end_angle,
        inner_radius,
        outer_radius,
    };
    let background = background.clone();
    let value = value.clone();
    let resolved = with_resolved_colors(frame, env, move |frame, env| {
        let mut colors = Vec::with_capacity(2 + frame.data.regions.len());
        colors.push(background.clone());
        colors.push(value.clone());
        colors.extend(
            frame
                .data
                .regions
                .iter()
                .map(|region| region.color.resolve(env)),
        );
        colors
    });
    let arcs = resolved
        .map(move |(frame, colors)| gauge_arcs(&frame, style, &colors))
        .computed();
    cx.each(arcs, key_of, move |cx, arc| {
        cx.stroke(
            arc.map(|arc| arc.value.path),
            arc.map(|arc| StrokeStyle::new(arc.value.width)),
            arc.map(|arc| arc.value.color),
        );
    });
    let needle_color = needle.clone();
    let needles = items(frame, move |frame| gauge_needle(frame, style));
    cx.each(needles, key_of, move |cx, needle| {
        cx.stroke(
            needle.map(|needle| needle.value.line),
            needle.map(|needle| StrokeStyle::new(needle.value.width)),
            needle_color.clone(),
        );
        cx.fill(needle.map(|needle| needle.value.hub), needle_color.clone());
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
    colors: &[WorkingColor],
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
        .zip(colors)
        .enumerate()
        .filter(|(_, (series, _))| series.values.len() >= axis_count)
        .map(|(key, (series, &color))| Keyed {
            key,
            value: RadarSeriesItem {
                polygon: polygon((0..axis_count).map(|axis| {
                    let ratio = (series.values[axis] / max_value).clamp(0.0, 1.0);
                    polar(center, radius * ratio, radar_angle(axis, axis_count))
                })),
                color,
                fill_alpha: (fill_opacity * color.components[3]).clamp(0.0, 1.0),
            },
        })
        .collect()
}

pub fn record_radar(
    cx: &mut Cx,
    frame: &Computed<Frame<RadarData, HitTargets<RadarDatum>>>,
    env: &Environment,
    ring_count: u32,
    line_width: f32,
    fill_opacity: f32,
) {
    let grid = Color::border().resolve(env);
    let rings = items(frame, move |frame| radar_rings(frame, ring_count));
    let ring_grid = grid.clone();
    cx.each(rings, key_of, move |cx, ring| {
        cx.stroke(
            ring.map(|ring| ring.value),
            StrokeStyle::new(1.0),
            ring_grid.clone(),
        );
    });
    let axes = items(frame, radar_axes);
    cx.each(axes, key_of, move |cx, axis| {
        cx.stroke(
            axis.map(|axis| axis.value),
            StrokeStyle::new(1.0),
            grid.clone(),
        );
    });
    let resolved = with_resolved_colors(frame, env, |frame, env| {
        frame
            .data
            .series
            .iter()
            .enumerate()
            .map(|(index, series)| {
                series
                    .color
                    .as_ref()
                    .map_or_else(|| categorical_color(index), Clone::clone)
                    .resolve(env)
            })
            .collect()
    });
    let series = resolved
        .map(move |(frame, colors)| radar_series(&frame, &colors, fill_opacity))
        .computed();
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

/// One cell for each value of a `rows` by `cols` grid, coloured by `scale`
/// at `alpha`.
fn heat_cells(
    size: Size,
    rows: u32,
    cols: u32,
    values: &[f32],
    range: (f32, f32),
    scale: &ResolvedColorScale,
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
            let color = scale.color_at(values[key], range.0, range.1);
            Keyed {
                key,
                value: Cell {
                    rect: grid_cell(plot, rows, cols, row, col),
                    color: color.with_alpha((color.components[3] * alpha).clamp(0.0, 1.0)),
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

pub fn record_heatmap(
    cx: &mut Cx,
    frame: &Computed<Frame<HeatmapData, HitTargets<GridDatum>>>,
    scale: &Computed<ResolvedColorScale>,
) {
    let cells = frame
        .zip(scale)
        .map(|(frame, scale)| {
            let data = &frame.data;
            heat_cells(
                frame.size,
                data.rows,
                data.cols,
                &data.values,
                (data.min_value, data.max_value),
                &scale,
                1.0,
            )
        })
        .computed();
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

fn contour_levels(
    frame: &Frame<ContourData, HitTargets<GridDatum>>,
    scale: &ResolvedColorScale,
) -> Vec<Keyed<Level>> {
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
                    color: scale.color_at(level, data.min_value, data.max_value),
                },
            })
        })
        .collect()
}

pub fn record_contour(
    cx: &mut Cx,
    frame: &Computed<Frame<ContourData, HitTargets<GridDatum>>>,
    scale: &Computed<ResolvedColorScale>,
    line_width: f32,
) {
    // A faint tint of the field under the lines.
    let tint = frame
        .zip(scale)
        .map(|(frame, scale)| {
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
                &scale,
                0.08,
            )
        })
        .computed();
    record_cells(cx, tint);
    let levels = frame
        .zip(scale)
        .map(|(frame, scale)| contour_levels(&frame, &scale))
        .computed();
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
    colors: &[WorkingColor],
) -> Vec<Keyed<Region>> {
    let data = &frame.data;
    let positions: Vec<f32> = data
        .color_scale
        .stops
        .iter()
        .map(|(stop, _)| *stop)
        .collect();
    let fallback = colors
        .last()
        .copied()
        .expect("resolved choropleth colors include fallback");
    let scale = ResolvedColorScale::from_parts(&positions, &colors[..positions.len()], fallback);
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
                color: scale.color_at(region.value, data.min_value, data.max_value),
            },
        })
        .collect()
}

pub fn record_choropleth(
    cx: &mut Cx,
    frame: &Computed<Frame<ChoroplethData, HitTargets<RegionDatum>>>,
    env: &Environment,
    stroke_width: f32,
    stroke_color: &Computed<WorkingColor>,
    show_stroke: bool,
) {
    let stroke_color = stroke_color.clone();
    let resolved = with_resolved_colors(frame, env, |frame, env| {
        let mut colors: Vec<Computed<WorkingColor>> = frame
            .data
            .color_scale
            .stops
            .iter()
            .map(|(_, color)| color.resolve(env))
            .collect();
        colors.push(Color::muted_foreground().resolve(env));
        colors
    });
    let regions = resolved
        .map(|(frame, colors)| choropleth_regions(&frame, &colors))
        .computed();
    cx.each(regions, key_of, move |cx, region| {
        cx.fill(
            region.map(|region| region.value.path),
            region.map(|region| region.value.color),
        );
        if show_stroke {
            cx.stroke(
                region.map(|region| region.value.path),
                StrokeStyle::new(stroke_width),
                stroke_color.clone(),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::ColorScale;
    use crate::theme_color::resolve_color_scale;
    use nami::Signal;
    use waterui_graphics::color::{Srgb, install_color_signal, token::MutedForeground};

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
        let mut env = Environment::new();
        install_color_signal::<MutedForeground>(
            &mut env,
            Computed::constant(WorkingColor::from(Srgb::new(0.5, 0.5, 0.5))),
        );
        let scale = resolve_color_scale(&ColorScale::viridis(), &env).snapshot();
        assert_eq!(
            scale.color_at(0.0, 0.0, 1.0),
            WorkingColor::from(Srgb::from_hex("#440154"))
        );
        assert_eq!(
            scale.color_at(1.0, 0.0, 1.0),
            WorkingColor::from(Srgb::from_hex("#FDE725"))
        );
    }
}
