//! Renders every chart through the Hydrolysis backend and writes the frames
//! out to be looked at.
//!
//! What a chart's geometry has to be is asserted by the semantic tests and by
//! the unit tests next to the geometry; these tests export what the canvas
//! draws from it, so the drawing is judged as pictures. The frames land under
//! `CARGO_TARGET_TMPDIR`, in `chart-render/`.

mod support;

use core::time::Duration;
use std::path::PathBuf;

use waterui::animation::Animation;
use waterui::graphics::color::Srgb;
use waterui::reactive::{SignalExt as _, binding};
use waterui::{Binding, View, ViewExt as _};
use waterui_chart::{
    AreaChart, BarChart, BubbleChart, CandlestickChart, ChoroplethChart, ChoroplethData,
    ContourChart, ContourData, DataPoint, DepthChart, GaugeChart, GaugeData, GaugeRegion,
    GeoPolygon, HeatmapChart, HeatmapData, LineChart, PieChart, RadarChart, RadarData, RadarSeries,
    ScatterChart,
};
use waterui_testing::{OffscreenApp, Snapshot, ui as test_ui};

use support::{
    CHART_HEIGHT, CHART_WIDTH, area_data, bubble_series, candle_series, depth_data, pie_data,
    point_series,
};

const BACKDROP: Srgb = Srgb::new(0.07, 0.08, 0.10);

fn output(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("chart-render");
    std::fs::create_dir_all(&directory).expect("the output directory must be creatable");
    directory.join(format!("{name}.png"))
}

fn mount<V: View + 'static>(chart: impl Fn() -> V + 'static) -> OffscreenApp {
    test_ui()
        .viewport(360, 260)
        .theme(hydrolysis_m3::Material3::defaults())
        .mount_offscreen(move || {
            chart()
                .size(CHART_WIDTH + 80.0, CHART_HEIGHT + 60.0)
                .background(BACKDROP)
        })
}

fn export(app: &mut OffscreenApp, name: &str) {
    let captured = app.capture_snapshot("render_charts", name, "00_rendered");
    captured
        .snapshot()
        .save_png(output(name))
        .expect("the frame must be written");
}

fn render<V: View + 'static>(name: &str, chart: impl Fn() -> V + 'static) {
    let mut app = mount(chart);
    export(&mut app, name);
}

#[test]
fn line_chart() {
    render("line", || {
        LineChart::new(Binding::container(point_series()))
            .color(Srgb::from_hex("#22C55E"))
            .line_width(2.5)
    });
    render("line-filled", || {
        LineChart::new(Binding::container(point_series()))
            .color(Srgb::from_hex("#38BDF8"))
            .fill(0.3)
    });
}

#[test]
fn bar_chart() {
    render("bar", || {
        BarChart::new(Binding::container(point_series())).color(Srgb::from_hex("#F59E0B"))
    });
}

#[test]
fn scatter_chart() {
    render("scatter", || {
        ScatterChart::new(Binding::container(point_series())).color(Srgb::from_hex("#EC4899"))
    });
}

#[test]
fn bubble_chart() {
    render("bubble", || {
        BubbleChart::new(Binding::container(bubble_series())).color(Srgb::from_hex("#8B5CF6"))
    });
}

#[test]
fn area_chart() {
    render("area", || AreaChart::new(Binding::container(area_data())));
    render("area-overlapping", || {
        AreaChart::new(Binding::container(area_data().stacked(false)))
    });
}

#[test]
fn candlestick_chart() {
    render("candlestick", || {
        CandlestickChart::new(Binding::container(candle_series()))
    });
}

#[test]
fn depth_chart() {
    render("depth", || {
        DepthChart::new(Binding::container(depth_data()))
    });
}

#[test]
fn pie_chart() {
    render("pie", || PieChart::new(Binding::container(pie_data())));
    render("donut", || {
        PieChart::new(Binding::container(pie_data())).donut(0.55)
    });
}

#[test]
fn gauge_chart() {
    render("gauge", || {
        GaugeChart::new(Binding::container(GaugeData::new(72.0, 0.0, 100.0)))
    });
    render("gauge-regions", || {
        GaugeChart::new(Binding::container(
            GaugeData::new(72.0, 0.0, 100.0)
                .region(GaugeRegion::hex(40.0, "#22C55E"))
                .region(GaugeRegion::hex(75.0, "#F59E0B"))
                .region(GaugeRegion::hex(100.0, "#EF4444")),
        ))
    });
}

#[test]
fn radar_chart() {
    render("radar", || {
        RadarChart::new(Binding::container(
            RadarData::new(6)
                .max_value(10.0)
                .series(
                    RadarSeries::new("Team", vec![8.0, 6.0, 7.0, 4.0, 9.0, 5.0])
                        .color(0.23, 0.51, 0.96, 1.0),
                )
                .series(
                    RadarSeries::new("Rival", vec![5.0, 8.0, 4.0, 7.0, 6.0, 8.0])
                        .color(0.94, 0.27, 0.27, 1.0),
                ),
        ))
    });
}

/// A Gaussian bump of `sharpness` centred on `(cx, cy)`.
fn bump(x: f32, y: f32, (cx, cy): (f32, f32), sharpness: f32) -> f32 {
    let (dx, dy) = (x - cx, y - cy);
    (-dx.mul_add(dx, dy * dy) * sharpness).exp()
}

/// A smooth field with two peaks, as `rows` by `cols` values.
fn field(rows: u32, cols: u32) -> Vec<f32> {
    (0..rows)
        .flat_map(|row| (0..cols).map(move |col| (row, col)))
        .map(|(row, col)| {
            let x = f32::from(u16::try_from(col).expect("a small grid")) / 12.0;
            let y = f32::from(u16::try_from(row).expect("a small grid")) / 12.0;
            0.7f32.mul_add(bump(x, y, (1.6, 1.2), 9.0), bump(x, y, (0.7, 0.6), 6.0))
        })
        .collect()
}

#[test]
fn heatmap_chart() {
    render("heatmap", || {
        HeatmapChart::new(Binding::container(HeatmapData::new(16, 24, field(16, 24))))
    });
}

#[test]
fn contour_chart() {
    render("contour", || {
        ContourChart::new(Binding::container(ContourData::new(
            16,
            24,
            field(16, 24),
            8,
        )))
    });
}

#[test]
fn choropleth_chart() {
    let square = |id: u32, value: f32, x: f32, y: f32| {
        GeoPolygon::new(
            id,
            value,
            vec![[x, y], [x + 1.0, y], [x + 1.0, y + 1.0], [x, y + 1.0]],
        )
    };
    let regions = vec![
        square(0, 12.0, 0.0, 0.0),
        square(1, 48.0, 1.0, 0.0),
        square(2, 80.0, 0.0, 1.0),
        GeoPolygon::new(3, 95.0, vec![[1.0, 1.0], [2.0, 1.0], [1.5, 2.0]]),
    ];
    render("choropleth", move || {
        ChoroplethChart::new(Binding::container(
            ChoroplethData::new(regions.clone()).range(0.0, 100.0),
        ))
    });
}

/// Finds the top-most y row (smallest y) of bright amber bar pixels (`#F59E0B`)
/// in the left half of `snapshot` (bar 0 of a two-bar chart).
fn first_bar_top_y(snapshot: &Snapshot) -> u32 {
    let width = snapshot.width();
    let height = snapshot.height();
    let rgba = snapshot.rgba8();
    let half_w = width / 2;
    for y in 0..height {
        for x in 0..half_w {
            let idx = ((y * width + x) * 4) as usize;
            let (r, g, b) = (rgba[idx], rgba[idx + 1], rgba[idx + 2]);
            if r > 200 && g > 120 && b < 60 {
                return y;
            }
        }
    }
    panic!("expected amber bar pixels in left half of snapshot");
}

/// An unanimated data change applies immediately on the next frame with no
/// hardcoded transition delay.
#[test]
fn an_unanimated_data_change_applies_immediately() {
    let data = binding(vec![DataPoint::new(0.0, 20.0), DataPoint::new(1.0, 100.0)]);
    let mut app = mount({
        let data = data.clone();
        move || BarChart::new(data.clone()).color(Srgb::from_hex("#F59E0B"))
    });
    let before = app.snapshot();
    let y_before = first_bar_top_y(&before);

    data.set(vec![DataPoint::new(0.0, 60.0), DataPoint::new(1.0, 100.0)]);
    let immediate = app.snapshot();
    let y_immediate = first_bar_top_y(&immediate);

    app.pump_for(Duration::from_millis(600));
    let settled = app.snapshot();
    let y_settled = first_bar_top_y(&settled);

    assert!(
        y_immediate + 20 < y_before,
        "bar 0 must immediately grow taller (before top y={y_before}, immediate top y={y_immediate})"
    );
    assert_eq!(
        immediate.rgba8(),
        settled.rgba8(),
        "unanimated update must already match the settled frame (immediate top y={y_immediate}, settled top y={y_settled})"
    );
}

/// A spring-animated data change follows the attached `Animation::spring` and
/// visibly overshoots its target height before settling.
#[test]
fn a_spring_animated_data_change_visibly_springs() {
    let data = binding(vec![DataPoint::new(0.0, 20.0), DataPoint::new(1.0, 100.0)]);
    let mut app = mount({
        let data = data.clone();
        move || {
            BarChart::new(data.with(Animation::spring(180.0, 8.0))).color(Srgb::from_hex("#F59E0B"))
        }
    });
    let y_before = first_bar_top_y(&app.snapshot());
    export(&mut app, "transition-1-before");

    data.set(vec![DataPoint::new(0.0, 60.0), DataPoint::new(1.0, 100.0)]);
    let mut min_top_y = y_before;
    for step in 0..30 {
        app.pump_for(Duration::from_millis(20));
        let top_y = first_bar_top_y(&app.snapshot());
        min_top_y = min_top_y.min(top_y);
        if step == 5 {
            export(&mut app, "transition-2-midway");
        }
    }

    app.pump_for(Duration::from_millis(2000));
    let y_settled = first_bar_top_y(&app.snapshot());
    export(&mut app, "transition-3-settled");

    assert!(
        y_settled + 20 < y_before,
        "bar 0 must settle taller (before top y={y_before}, settled top y={y_settled})"
    );
    assert!(
        min_top_y + 4 < y_settled,
        "underdamped spring must visibly overshoot above target height (peak top y={min_top_y}, settled top y={y_settled})"
    );
}
