//! Pixel-level verification that chart defaults, tooltips, axes, and reactive
//! color signals re-theme live on Light/Dark switches and signal updates.

use hydrolysis_m3::Material3;
use waterui::graphics::color::{Color, ColorScheme, Srgb};
use waterui::reactive::binding;
use waterui::theme::Theme;
use waterui::{Binding, View, ViewExt as _};
use waterui_chart::{
    AxisConfig, BarChart, ChartExt as _, DataBounds, DataPoint, GaugeChart, GaugeData,
    HeatmapChart, HeatmapData, PieChart, RadarChart, RadarData, RadarSeries, Tooltip,
    TooltipContent,
};
use waterui_testing::{OffscreenApp, Snapshot, ui as test_ui};

const VIEWPORT_WIDTH: u32 = 240;
const VIEWPORT_HEIGHT: u32 = 180;
const CHART_WIDTH: f32 = 240.0;
const CHART_HEIGHT: f32 = 180.0;
const BACKDROP: Srgb = Srgb::BLACK;

fn mount_with_scheme<V: View + 'static>(
    scheme: &Binding<ColorScheme>,
    view: impl Fn() -> V + 'static,
) -> OffscreenApp {
    let scheme = scheme.clone();
    test_ui()
        .viewport(VIEWPORT_WIDTH, VIEWPORT_HEIGHT)
        .theme(Material3::defaults())
        .mount_offscreen(move || {
            view()
                .size(CHART_WIDTH, CHART_HEIGHT)
                .background(BACKDROP)
                .install(Theme::new().color_scheme(scheme.clone()))
        })
}

fn non_black_pixel_count(snapshot: &Snapshot) -> usize {
    snapshot
        .rgba8()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] > 8 || px[1] > 8 || px[2] > 8)
        .count()
}

fn changed_pixel_count(before: &Snapshot, after: &Snapshot) -> usize {
    before
        .rgba8()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(after.rgba8().as_chunks::<4>().0.iter())
        .filter(|(a, b)| {
            a[0].abs_diff(b[0]) > 4 || a[1].abs_diff(b[1]) > 4 || a[2].abs_diff(b[2]) > 4
        })
        .count()
}

fn assert_rethemes_live<V: View + 'static>(name: &str, view: impl Fn() -> V + 'static) {
    let scheme = binding(ColorScheme::Light);
    let mut app = mount_with_scheme(&scheme, view);
    let light = app.snapshot();
    assert!(
        non_black_pixel_count(&light) > 50,
        "{name}: expected chart to draw visible pixels in light mode"
    );

    scheme.set(ColorScheme::Dark);
    let dark = app.snapshot();
    assert!(
        non_black_pixel_count(&dark) > 50,
        "{name}: expected chart to draw visible pixels in dark mode"
    );
    assert!(
        changed_pixel_count(&light, &dark) > 50,
        "{name}: expected default theme-token colors to re-theme live from Light to Dark"
    );
}

#[test]
fn default_bar_chart_rethemes_live_between_light_and_dark() {
    let data = binding(vec![
        DataPoint::new(0.0, 40.0),
        DataPoint::new(1.0, 85.0),
        DataPoint::new(2.0, 60.0),
    ]);
    assert_rethemes_live("bar", move || BarChart::new(data.clone()));
}

#[test]
fn default_pie_chart_rethemes_live_between_light_and_dark() {
    let data = binding(vec![
        DataPoint::new(0.0, 35.0),
        DataPoint::new(1.0, 40.0),
        DataPoint::new(2.0, 25.0),
    ]);
    assert_rethemes_live("pie", move || PieChart::new(data.clone()));
}

#[test]
fn default_gauge_chart_rethemes_live_between_light_and_dark() {
    let data = binding(GaugeData::new(68.0, 0.0, 100.0));
    assert_rethemes_live("gauge", move || GaugeChart::new(data.clone()));
}

#[test]
fn default_radar_chart_rethemes_live_between_light_and_dark() {
    let data = binding(
        RadarData::new(5)
            .max_value(10.0)
            .series(RadarSeries::new("A", vec![8.0, 6.0, 9.0, 5.0, 7.0]))
            .series(RadarSeries::new("B", vec![5.0, 9.0, 4.0, 8.0, 6.0])),
    );
    assert_rethemes_live("radar", move || RadarChart::new(data.clone()));
}

#[test]
fn default_heatmap_chart_rethemes_live_between_light_and_dark() {
    let data = binding(HeatmapData::new(
        4,
        4,
        vec![
            0.0, 0.2, 0.4, 0.6, 0.2, 0.4, 0.6, 0.8, 0.4, 0.6, 0.8, 1.0, 0.6, 0.8, 1.0, 0.9,
        ],
    ));
    assert_rethemes_live("heatmap", move || HeatmapChart::new(data.clone()));
}

#[test]
fn default_tooltip_and_axes_retheme_live_between_light_and_dark() {
    assert_rethemes_live("tooltip", || {
        Tooltip::new(TooltipContent::new().title("Revenue").value("Q4", "$1.4M"))
    });

    let points = vec![
        DataPoint::new(0.0, 20.0),
        DataPoint::new(1.0, 75.0),
        DataPoint::new(2.0, 50.0),
    ];
    let bounds = DataBounds::from_points(&points);
    let data = binding(points);
    assert_rethemes_live("axes", move || {
        BarChart::new(data.clone())
            .axes(bounds)
            .y_axis(AxisConfig::new().tick_count(4).show_grid())
    });
}

#[test]
fn bar_chart_color_updates_live_from_binding_signal() {
    let data = binding(vec![DataPoint::new(0.0, 80.0), DataPoint::new(1.0, 100.0)]);
    let color = binding(Color::from(Srgb::from_hex("#22C55E")));
    let mut app = test_ui()
        .viewport(VIEWPORT_WIDTH, VIEWPORT_HEIGHT)
        .theme(Material3::defaults())
        .mount_offscreen({
            let data = data;
            let color = color.clone();
            move || {
                BarChart::new(data.clone())
                    .color(color.clone())
                    .size(CHART_WIDTH, CHART_HEIGHT)
                    .background(BACKDROP)
            }
        });

    let green = app.snapshot();
    let green_pixels = green
        .rgba8()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[1] > 150 && px[0] < 100)
        .count();
    assert!(
        green_pixels > 100,
        "expected green bars before signal update"
    );

    color.set(Color::from(Srgb::from_hex("#EF4444")));
    let red = app.snapshot();
    let red_pixels = red
        .rgba8()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] > 180 && px[1] < 100)
        .count();
    assert!(red_pixels > 100, "expected red bars after signal update");
}

#[test]
fn radar_series_color_updates_live_from_binding_signal() {
    let series_color = binding(Color::from(Srgb::from_hex("#22C55E")));
    let data = binding(RadarData::new(5).max_value(10.0).series(
        RadarSeries::new("Live", vec![9.0, 8.0, 9.0, 8.0, 9.0]).color(series_color.clone()),
    ));
    let mut app = test_ui()
        .viewport(VIEWPORT_WIDTH, VIEWPORT_HEIGHT)
        .theme(Material3::defaults())
        .mount_offscreen(move || {
            RadarChart::new(data.clone())
                .size(CHART_WIDTH, CHART_HEIGHT)
                .background(BACKDROP)
        });

    let before = app.snapshot();
    let green_pixels = before
        .rgba8()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[1] > 120 && px[0] < 90)
        .count();
    assert!(green_pixels > 50, "expected green radar series initially");

    series_color.set(Color::from(Srgb::from_hex("#EF4444")));
    let after = app.snapshot();
    let red_pixels = after
        .rgba8()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] > 140 && px[1] < 90)
        .count();
    assert!(
        red_pixels > 50,
        "expected red radar series after color binding update"
    );
}
