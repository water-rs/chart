//! Where a chart's data lands in the canvas: data windows, the plot area and
//! the hit geometry derived from them.

extern crate alloc;

use core::f32::consts::{FRAC_PI_2, TAU};

use num_traits::ToPrimitive as _;
use waterui_core::layout::{Point, Rect, Size};

use super::hit::{CartesianGeometry, HitTargets};
use crate::{
    data::{
        AreaData, BubblePoint, Candle, ChoroplethData, ContourData, DataBounds, DataPoint,
        DepthData, GaugeData, HeatmapData, RadarData,
    },
    interaction::{
        AreaDatum, ChartViewport, DepthDatum, DepthSide, GridDatum, HitResult, RadarDatum,
        RegionDatum, SliceDatum,
    },
};

pub(super) const PLOT_PADDING_RATIO: f32 = 0.1;

pub(super) fn usize_to_f32(value: usize) -> f32 {
    value
        .to_f32()
        .expect("chart collection length should fit f32")
}

pub(super) fn u32_to_f32(value: u32) -> f32 {
    value.to_f32().expect("chart dimension should fit f32")
}

pub(super) fn u32_to_usize(value: u32) -> usize {
    usize::try_from(value).expect("chart dimension should fit usize")
}

#[inline]
pub(super) fn normalize_bounds(mut bounds: DataBounds) -> DataBounds {
    assert!(
        !(!bounds.min_x.is_finite()
            || !bounds.max_x.is_finite()
            || !bounds.min_y.is_finite()
            || !bounds.max_y.is_finite()),
        "chart data contains non-finite bounds"
    );
    if bounds.max_x <= bounds.min_x {
        bounds.max_x = bounds.min_x + 1.0;
    }
    if bounds.max_y <= bounds.min_y {
        bounds.max_y = bounds.min_y + 1.0;
    }
    bounds
}

#[inline]
pub fn point_bounds(data: &[DataPoint]) -> DataBounds {
    normalize_bounds(DataBounds::from_points(data).with_padding(0.1))
}

pub fn bar_bounds(data: &[DataPoint]) -> DataBounds {
    let source_bounds = DataBounds::from_points(data);
    normalize_bounds(DataBounds::new(
        source_bounds.min_x,
        source_bounds.max_x,
        source_bounds.min_y.min(0.0),
        source_bounds.max_y.max(0.0),
    ))
}

pub fn bubble_bounds(data: &[BubblePoint]) -> DataBounds {
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for point in data {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }

    normalize_bounds(DataBounds::new(min_x, max_x, min_y, max_y).with_padding(0.1))
}

pub fn candlestick_bounds(data: &[Candle]) -> DataBounds {
    normalize_bounds(DataBounds::from_candles(data).with_padding(0.05))
}

pub fn depth_bounds(data: &DepthData) -> DataBounds {
    normalize_bounds(data.bounds().with_padding(0.04))
}

pub fn area_bounds(data: &AreaData) -> DataBounds {
    normalize_bounds(data.bounds().with_padding(0.05))
}

/// The plot area: the canvas inset by `padding_ratio` of each dimension.
pub(super) fn plot_rect(size: Size, padding_ratio: f32) -> Rect {
    let inset_x = size.width * padding_ratio;
    let inset_y = size.height * padding_ratio;
    Rect::new(
        Point::new(inset_x, inset_y),
        Size::new(
            inset_x.mul_add(-2.0, size.width).max(1.0),
            inset_y.mul_add(-2.0, size.height).max(1.0),
        ),
    )
}

#[inline]
const fn chart_viewport_from_rect(rect: Rect) -> ChartViewport {
    ChartViewport::new(rect.min_x(), rect.min_y(), rect.width(), rect.height())
}

/// Where data coordinates `(x, y)` land in `plot`, showing the window
/// `bounds`.
#[inline]
pub(super) fn map_xy(plot: Rect, bounds: DataBounds, x: f32, y: f32) -> Point {
    let nx = (x - bounds.min_x) / (bounds.max_x - bounds.min_x);
    let ny = (y - bounds.min_y) / (bounds.max_y - bounds.min_y);
    Point::new(
        nx.mul_add(plot.width(), plot.min_x()),
        ny.mul_add(-plot.height(), plot.max_y()),
    )
}

pub fn point_geometry(
    size: Size,
    data: &[DataPoint],
    bounds: DataBounds,
    radius: f32,
) -> CartesianGeometry<DataPoint> {
    let plot = plot_rect(size, PLOT_PADDING_RATIO);
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    for (index, point) in data.iter().enumerate() {
        let anchor = map_xy(plot, bounds, point.x, point.y);
        targets.push_circle(HitResult::new(0, index, *point, anchor), anchor, radius);
    }
    CartesianGeometry::new(bounds, chart_viewport_from_rect(plot), targets)
}

pub fn bar_geometry(
    size: Size,
    data: &[DataPoint],
    bounds: DataBounds,
) -> CartesianGeometry<DataPoint> {
    let plot = plot_rect(size, PLOT_PADDING_RATIO);
    let bar_width = if data.is_empty() {
        1.0
    } else {
        (plot.width() / usize_to_f32(data.len()) * 0.7).max(1.0)
    };
    let baseline_y = map_xy(plot, bounds, bounds.min_x, 0.0).y;
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    for (index, point) in data.iter().enumerate() {
        let center = map_xy(plot, bounds, point.x, point.y);
        let top = center.y.min(baseline_y);
        let bottom = center.y.max(baseline_y);
        let rect = Rect::new(
            Point::new(f32::mul_add(bar_width, -0.5, center.x), top),
            Size::new(bar_width, (bottom - top).max(1.0)),
        );
        let anchor = Point::new(center.x, top);
        targets.push_rect(HitResult::new(0, index, *point, anchor), rect);
    }
    CartesianGeometry::new(bounds, chart_viewport_from_rect(plot), targets)
}

pub fn bubble_geometry(
    size: Size,
    data: &[BubblePoint],
    bounds: DataBounds,
    min_radius: f32,
    max_radius: f32,
) -> CartesianGeometry<BubblePoint> {
    let plot = plot_rect(size, PLOT_PADDING_RATIO);
    let radii = BubbleRadii::new(data, min_radius, max_radius);
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    for (index, point) in data.iter().enumerate() {
        let anchor = map_xy(plot, bounds, point.x, point.y);
        targets.push_circle(
            HitResult::new(0, index, *point, anchor),
            anchor,
            radii.radius(point).max(6.0),
        );
    }
    CartesianGeometry::new(bounds, chart_viewport_from_rect(plot), targets)
}

/// Scales bubble sizes into `min_radius..=max_radius` across a data set.
pub(super) struct BubbleRadii {
    min_size: f32,
    size_span: f32,
    min_radius: f32,
    max_radius: f32,
}

impl BubbleRadii {
    pub(super) fn new(data: &[BubblePoint], min_radius: f32, max_radius: f32) -> Self {
        let mut min_size = f32::MAX;
        let mut max_size = f32::MIN;
        for point in data {
            min_size = min_size.min(point.size);
            max_size = max_size.max(point.size);
        }
        Self {
            min_size,
            size_span: (max_size - min_size).max(1.0),
            min_radius,
            max_radius,
        }
    }

    /// The radius of `point`'s bubble.
    pub(super) fn radius(&self, point: &BubblePoint) -> f32 {
        let t = (point.size - self.min_size) / self.size_span;
        (self.max_radius - self.min_radius).mul_add(t, self.min_radius)
    }
}

/// The layout of one candle in the plot: its x, wick ends, and body edges.
pub(super) struct CandleLayout {
    pub x: f32,
    pub high: f32,
    pub low: f32,
    pub open: f32,
    pub close: f32,
}

pub(super) fn candle_layout(
    plot: Rect,
    bounds: DataBounds,
    candle: &Candle,
    epoch: f64,
) -> CandleLayout {
    let time = candle.x_offset(epoch);
    CandleLayout {
        x: map_xy(plot, bounds, time, candle.close).x,
        high: map_xy(plot, bounds, time, candle.high).y,
        low: map_xy(plot, bounds, time, candle.low).y,
        open: map_xy(plot, bounds, time, candle.open).y,
        close: map_xy(plot, bounds, time, candle.close).y,
    }
}

pub fn candlestick_geometry(
    size: Size,
    data: &[Candle],
    bounds: DataBounds,
) -> CartesianGeometry<Candle> {
    let plot = plot_rect(size, PLOT_PADDING_RATIO);
    let candle_width = if data.is_empty() {
        1.0
    } else {
        (plot.width() / usize_to_f32(data.len()) * 0.65).max(1.0)
    };
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    let epoch = crate::data::candle_epoch(data);
    for (index, candle) in data.iter().enumerate() {
        let CandleLayout {
            x,
            high,
            low,
            open,
            close,
        } = candle_layout(plot, bounds, candle, epoch);
        let top = open.min(close);
        let bottom = open.max(close);
        let rect = Rect::new(
            Point::new(f32::mul_add(candle_width, -0.5, x), top.min(high)),
            Size::new(
                candle_width.max(4.0),
                (bottom.max(low) - top.min(high)).max(4.0),
            ),
        );
        let anchor = Point::new(x, f32::midpoint(top, bottom));
        targets.push_rect(HitResult::new(0, index, *candle, anchor), rect);
    }
    CartesianGeometry::new(bounds, chart_viewport_from_rect(plot), targets)
}

pub fn depth_geometry(
    size: Size,
    data: &DepthData,
    bounds: DataBounds,
) -> CartesianGeometry<DepthDatum> {
    let plot = plot_rect(size, PLOT_PADDING_RATIO);
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    for (index, level) in data.bids.iter().enumerate() {
        let anchor = map_xy(plot, bounds, level.price, level.cumulative_volume);
        let value = DepthDatum::new(DepthSide::Bid, level.price, level.cumulative_volume);
        targets.push_circle(HitResult::new(0, index, value, anchor), anchor, 10.0);
    }
    for (index, level) in data.asks.iter().enumerate() {
        let anchor = map_xy(plot, bounds, level.price, level.cumulative_volume);
        let value = DepthDatum::new(DepthSide::Ask, level.price, level.cumulative_volume);
        targets.push_circle(HitResult::new(1, index, value, anchor), anchor, 10.0);
    }
    CartesianGeometry::new(bounds, chart_viewport_from_rect(plot), targets)
}

/// The baseline and top edge of each series of `data` at each x, in data
/// coordinates: stacked series sit on the ones before them.
pub(super) fn area_layers(
    data: &AreaData,
) -> alloc::vec::Vec<(alloc::vec::Vec<f32>, alloc::vec::Vec<f32>)> {
    let mut cumulative = alloc::vec![0.0f32; data.x_values.len()];
    data.series
        .iter()
        .map(|series| {
            let count = series.values.len().min(data.x_values.len());
            let mut base = alloc::vec![0.0f32; count];
            let mut top = alloc::vec![0.0f32; count];
            for index in 0..count {
                if data.stacked {
                    base[index] = cumulative[index];
                    top[index] = cumulative[index] + series.values[index];
                    cumulative[index] = top[index];
                } else {
                    top[index] = series.values[index];
                }
            }
            (base, top)
        })
        .collect()
}

pub fn area_geometry(
    size: Size,
    data: &AreaData,
    bounds: DataBounds,
) -> CartesianGeometry<AreaDatum> {
    let plot = plot_rect(size, PLOT_PADDING_RATIO);
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    for (series_index, (_, top)) in area_layers(data).into_iter().enumerate() {
        for (index, (y, x)) in top.iter().zip(&data.x_values).enumerate() {
            let anchor = map_xy(plot, bounds, *x, *y);
            let value = AreaDatum::new(series_index, *x, *y);
            targets.push_circle(
                HitResult::new(series_index, index, value, anchor),
                anchor,
                10.0,
            );
        }
    }
    CartesianGeometry::new(bounds, chart_viewport_from_rect(plot), targets)
}

/// The slices of a pie: `(data index, start angle, end angle)` for each
/// positive value, clockwise from the top.
pub(super) fn pie_slices(data: &[DataPoint]) -> alloc::vec::Vec<(usize, f32, f32)> {
    let total: f32 = data.iter().map(|point| point.y.max(0.0)).sum();
    let mut angle = -FRAC_PI_2;
    let mut slices = alloc::vec::Vec::new();
    if total <= 0.0 {
        return slices;
    }
    for (index, point) in data.iter().enumerate() {
        let value = point.y.max(0.0);
        if value <= 0.0 {
            continue;
        }
        let sweep = TAU * (value / total);
        let end = angle + sweep;
        slices.push((index, angle, end));
        angle = end;
    }
    slices
}

pub(super) const PIE_PADDING_RATIO: f32 = 0.06;

/// The centre and outer radius of a pie in `size`.
pub(super) fn pie_circle(size: Size) -> (Point, f32) {
    let plot = plot_rect(size, PIE_PADDING_RATIO);
    (plot.center(), plot.width().min(plot.height()) * 0.45)
}

pub fn pie_geometry(size: Size, data: &[DataPoint], inner_radius: f32) -> HitTargets<SliceDatum> {
    let plot = plot_rect(size, PIE_PADDING_RATIO);
    let (center, outer_r) = pie_circle(size);
    let inner_r = outer_r * inner_radius;
    let mid_r = f32::midpoint(inner_r, outer_r);
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    for (index, start, end) in pie_slices(data) {
        let mid = (end - start).mul_add(0.5, start);
        let anchor = Point::new(
            mid.cos().mul_add(mid_r, center.x),
            mid.sin().mul_add(mid_r, center.y),
        );
        let datum = SliceDatum::new(index, data[index].y, start, end);
        targets.push_sector(
            HitResult::new(0, index, datum, anchor),
            center,
            inner_r,
            outer_r,
            start,
            end,
        );
    }
    targets
}

pub(super) const GAUGE_PADDING_RATIO: f32 = 0.02;

/// The centre, inner radius and outer radius of a gauge in `size`.
pub(super) fn gauge_ring(size: Size, inner_radius: f32, outer_radius: f32) -> (Point, f32, f32) {
    let area = plot_rect(size, GAUGE_PADDING_RATIO);
    let min_dim = area.width().min(area.height());
    (
        area.center(),
        (min_dim * inner_radius).max(0.5),
        (min_dim * outer_radius).max(1.0),
    )
}

pub fn gauge_geometry(
    size: Size,
    data: &GaugeData,
    start_angle: f32,
    end_angle: f32,
    inner_radius: f32,
    outer_radius: f32,
) -> HitTargets<SliceDatum> {
    let area = plot_rect(size, GAUGE_PADDING_RATIO);
    let (center, inner_r, outer_r) = gauge_ring(size, inner_radius, outer_radius);
    let normalized = data.normalized_value();
    let value_end = (end_angle - start_angle).mul_add(normalized, start_angle);
    let mid = (value_end - start_angle).mul_add(0.5, start_angle);
    let mid_r = f32::midpoint(inner_r, outer_r);
    let anchor = Point::new(
        mid.cos().mul_add(mid_r, center.x),
        mid.sin().mul_add(mid_r, center.y),
    );
    let mut targets = HitTargets::new(chart_viewport_from_rect(area));
    let datum = SliceDatum::new(0, data.value, start_angle, value_end);
    targets.push_sector(
        HitResult::new(0, 0, datum, anchor),
        center,
        inner_r,
        outer_r,
        start_angle,
        value_end,
    );
    targets
}

pub(super) const RADAR_PADDING_RATIO: f32 = 0.08;

/// The centre and radius of a radar's web in `size`.
pub(super) fn radar_circle(size: Size) -> (Point, f32) {
    let plot = plot_rect(size, RADAR_PADDING_RATIO);
    (plot.center(), plot.width().min(plot.height()) * 0.45)
}

/// The angle of radar axis `axis` of `axis_count`, from the top.
pub(super) fn radar_angle(axis: usize, axis_count: usize) -> f32 {
    usize_to_f32(axis).mul_add(TAU / usize_to_f32(axis_count), -FRAC_PI_2)
}

pub fn radar_geometry(size: Size, data: &RadarData) -> HitTargets<RadarDatum> {
    let plot = plot_rect(size, RADAR_PADDING_RATIO);
    let (center, radius) = radar_circle(size);
    let axis_count = u32_to_usize(data.axis_count);
    let max_value = data.max_value.max(1.0);
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    for (series_index, series) in data.series.iter().enumerate() {
        if series.values.len() < axis_count {
            continue;
        }
        for axis in 0..axis_count {
            let ratio = (series.values[axis] / max_value).clamp(0.0, 1.0);
            let angle = radar_angle(axis, axis_count);
            let anchor = Point::new(
                (angle.cos() * radius).mul_add(ratio, center.x),
                (angle.sin() * radius).mul_add(ratio, center.y),
            );
            let label = data.labels.as_slice().get(axis).cloned();
            let datum = RadarDatum::new(axis, label, series.values[axis]);
            targets.push_circle(
                HitResult::new(series_index, axis, datum, anchor),
                anchor,
                10.0,
            );
        }
    }
    targets
}

/// The rectangle of grid cell `(row, col)` in a `rows` by `cols` grid
/// filling `plot`.
pub(super) fn grid_cell(plot: Rect, rows: u32, cols: u32, row: usize, col: usize) -> Rect {
    let cell_w = plot.width() / u32_to_f32(cols.max(1));
    let cell_h = plot.height() / u32_to_f32(rows.max(1));
    Rect::new(
        Point::new(
            usize_to_f32(col).mul_add(cell_w, plot.min_x()),
            usize_to_f32(row).mul_add(cell_h, plot.min_y()),
        ),
        Size::new(cell_w.max(1.0), cell_h.max(1.0)),
    )
}

pub fn heatmap_geometry(size: Size, data: &HeatmapData) -> HitTargets<GridDatum> {
    let plot = plot_rect(size, PLOT_PADDING_RATIO);
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    let rows = u32_to_usize(data.rows);
    let cols = u32_to_usize(data.cols);
    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col;
            let rect = grid_cell(plot, data.rows, data.cols, row, col);
            let datum = GridDatum::new(row, col, data.values[idx]);
            targets.push_rect(HitResult::new(0, idx, datum, rect.center()), rect);
        }
    }
    targets
}

pub fn contour_geometry(size: Size, data: &ContourData) -> HitTargets<GridDatum> {
    let plot = plot_rect(size, PLOT_PADDING_RATIO);
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    if data.rows < 2 || data.cols < 2 {
        return targets;
    }
    let cell_w = plot.width() / u32_to_f32(data.cols - 1);
    let cell_h = plot.height() / u32_to_f32(data.rows - 1);
    let rows = u32_to_usize(data.rows);
    let cols = u32_to_usize(data.cols);
    for row in 0..(rows - 1) {
        for col in 0..(cols - 1) {
            let i00 = row * cols + col;
            let i10 = i00 + 1;
            let i01 = (row + 1) * cols + col;
            let i11 = i01 + 1;
            let average =
                (data.values[i00] + data.values[i10] + data.values[i01] + data.values[i11]) * 0.25;
            let rect = Rect::new(
                Point::new(
                    usize_to_f32(col).mul_add(cell_w, plot.min_x()),
                    usize_to_f32(row).mul_add(cell_h, plot.min_y()),
                ),
                Size::new(cell_w.max(1.0), cell_h.max(1.0)),
            );
            let datum = GridDatum::new(row, col, average);
            targets.push_rect(HitResult::new(0, i00, datum, rect.center()), rect);
        }
    }
    targets
}

pub(super) const CHOROPLETH_PADDING_RATIO: f32 = 0.04;

/// Maps a choropleth's geographic coordinates into `size`, keeping the
/// aspect ratio and centring the map.
pub(super) struct MapProjection {
    min_x: f32,
    max_y: f32,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
}

impl MapProjection {
    pub(super) fn new(size: Size, data: &ChoroplethData) -> Self {
        let plot = plot_rect(size, CHOROPLETH_PADDING_RATIO);
        let [min_x, min_y, max_x, max_y] = data.bounds();
        let width = (max_x - min_x).max(1.0);
        let height = (max_y - min_y).max(1.0);
        let scale = (plot.width() / width).min(plot.height() / height);
        let content_w = width * scale;
        let content_h = height * scale;
        Self {
            min_x,
            max_y,
            scale,
            offset_x: (plot.width() - content_w).mul_add(0.5, plot.min_x()),
            offset_y: (plot.height() - content_h).mul_add(0.5, plot.min_y()),
        }
    }

    pub(super) fn point(&self, vertex: [f32; 2]) -> Point {
        Point::new(
            (vertex[0] - self.min_x).mul_add(self.scale, self.offset_x),
            (self.max_y - vertex[1]).mul_add(self.scale, self.offset_y),
        )
    }

    fn center_of(&self, bounds: [f32; 4]) -> Point {
        let [min_x, min_y, max_x, max_y] = bounds;
        Point::new(
            (min_x + max_x)
                .mul_add(0.5, -self.min_x)
                .mul_add(self.scale, self.offset_x),
            (min_y + max_y)
                .mul_add(-0.5, self.max_y)
                .mul_add(self.scale, self.offset_y),
        )
    }
}

pub fn choropleth_geometry(size: Size, data: &ChoroplethData) -> HitTargets<RegionDatum> {
    let plot = plot_rect(size, CHOROPLETH_PADDING_RATIO);
    let projection = MapProjection::new(size, data);
    let mut targets = HitTargets::new(chart_viewport_from_rect(plot));
    for (index, polygon) in data.polygons.iter().enumerate() {
        if polygon.vertices.len() < 3 {
            continue;
        }
        let vertices = polygon
            .vertices
            .iter()
            .map(|vertex| projection.point(*vertex))
            .collect();
        let anchor = projection.center_of(polygon.bounds());
        let datum = RegionDatum::new(index, polygon.id, polygon.value);
        targets.push_polygon(HitResult::new(0, index, datum, anchor), vertices);
    }
    targets
}
