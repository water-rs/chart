//! Theme-aware color helpers and reactive resolution utilities for charts.

extern crate alloc;

use alloc::{rc::Rc, vec, vec::Vec};

use nami::{
    Computed, Signal, SignalExt as _,
    signal::IntoComputed,
    watcher::{BoxWatcherGuard, Context, WatcherGuard},
};
use waterui_core::{Environment, flatten_signal};
use waterui_graphics::color::{Color, WorkingColor, working};

use crate::data::ColorScale;

const CATEGORICAL_PALETTE_LEN: usize = 6;

/// Converts any color value, theme token, or reactive color signal into a [`Color`].
#[must_use]
pub fn into_color(color: impl IntoComputed<Color>) -> Color {
    Color::from_signal(color.into_computed())
}

/// Returns the theme-token color for the categorical series at `index`.
#[must_use]
pub fn categorical_color(index: usize) -> Color {
    match index % CATEGORICAL_PALETTE_LEN {
        0 => Color::accent(),
        1 => Color::tertiary(),
        2 => Color::error(),
        3 => Color::accent_container(),
        4 => Color::tertiary_container(),
        _ => Color::muted_foreground(),
    }
}

/// Default continuous color scale stops derived from semantic theme tokens.
#[must_use]
pub fn default_scale_stops() -> Vec<(f32, Color)> {
    vec![
        (0.0, Color::surface_variant()),
        (0.35, Color::accent_container()),
        (0.7, Color::accent()),
        (1.0, Color::tertiary()),
    ]
}

#[derive(Clone)]
struct ColorList(Vec<Computed<WorkingColor>>);

struct ColorListGuard {
    _guards: Vec<BoxWatcherGuard>,
}

impl WatcherGuard for ColorListGuard {}

impl Signal for ColorList {
    type Output = Vec<WorkingColor>;
    type Guard = ColorListGuard;

    fn snapshot(&self) -> Self::Output {
        self.0.iter().map(Signal::snapshot).collect()
    }

    fn watch(&self, watcher: impl Fn(Context<Self::Output>) + 'static) -> Self::Guard {
        let watcher = Rc::new(watcher);
        let guards = self
            .0
            .iter()
            .enumerate()
            .map(|(index, signal)| {
                let signals = self.0.clone();
                let watcher = Rc::clone(&watcher);
                signal.watch(move |ctx| {
                    let metadata = ctx.metadata().clone();
                    let changed = ctx.into_value();
                    let mut values: Vec<WorkingColor> =
                        signals.iter().map(Signal::snapshot).collect();
                    values[index] = changed;
                    watcher(Context::new(values, metadata));
                })
            })
            .collect();
        ColorListGuard { _guards: guards }
    }
}

/// Combines a list of resolved color signals into one reactive color list.
#[must_use]
pub fn combine_resolved_colors(colors: Vec<Computed<WorkingColor>>) -> Computed<Vec<WorkingColor>> {
    Computed::new(ColorList(colors))
}

/// Resolves a slice of semantic [`Color`] values against `env`.
#[must_use]
pub fn resolve_colors(colors: &[Color], env: &Environment) -> Computed<Vec<WorkingColor>> {
    combine_resolved_colors(colors.iter().map(|color| color.resolve(env)).collect())
}

/// Resolves the default categorical theme palette against `env`.
#[must_use]
pub fn resolve_categorical_palette(env: &Environment) -> Computed<Vec<WorkingColor>> {
    let colors: Vec<Computed<WorkingColor>> = (0..CATEGORICAL_PALETTE_LEN)
        .map(|index| categorical_color(index).resolve(env))
        .collect();
    combine_resolved_colors(colors)
}

/// Pairs a reactive chart frame with resolved [`WorkingColor`] values extracted
/// from that frame against `env`.
///
/// Updates when either `frame` or any extracted color signal changes, keeping
/// animation metadata attached to the triggering update.
#[must_use]
pub fn with_resolved_colors<F, E>(
    frame: &Computed<F>,
    env: &Environment,
    extract: E,
) -> Computed<(F, Vec<WorkingColor>)>
where
    F: Clone + 'static,
    E: Fn(&F, &Environment) -> Vec<Computed<WorkingColor>> + Clone + 'static,
{
    let env = env.clone();
    flatten_signal(frame.map(move |frame| {
        let resolved = combine_resolved_colors(extract(&frame, &env));
        resolved
            .map(move |colors| (frame.clone(), colors))
            .computed()
    }))
}

/// A [`ColorScale`] resolved into linear [`WorkingColor`] stops.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedColorScale {
    stops: Vec<(f32, WorkingColor)>,
    fallback: WorkingColor,
}

impl ResolvedColorScale {
    /// Builds a resolved scale from stop positions, resolved stop colors, and a
    /// fallback color used when `stops` is empty.
    #[must_use]
    pub fn from_parts(positions: &[f32], colors: &[WorkingColor], fallback: WorkingColor) -> Self {
        let stops = positions
            .iter()
            .copied()
            .zip(colors.iter().copied())
            .collect();
        Self { stops, fallback }
    }

    /// Interpolates the scale at `value` within `min..=max`.
    #[must_use]
    pub fn color_at(&self, value: f32, min: f32, max: f32) -> WorkingColor {
        if self.stops.is_empty() {
            return self.fallback;
        }
        let t = if max > min {
            ((value - min) / (max - min)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let mut previous = self.stops[0];
        if t <= previous.0 {
            return previous.1;
        }

        for &(stop_t, stop_color) in &self.stops[1..] {
            if t <= stop_t {
                let span = (stop_t - previous.0).max(f32::EPSILON);
                return working::lerp(previous.1, stop_color, (t - previous.0) / span);
            }
            previous = (stop_t, stop_color);
        }

        previous.1
    }
}

/// Resolves a [`ColorScale`] against `env` so theme switches and signal stops
/// update live.
#[must_use]
pub fn resolve_color_scale(scale: &ColorScale, env: &Environment) -> Computed<ResolvedColorScale> {
    let positions: Vec<f32> = scale.stops.iter().map(|(stop, _)| *stop).collect();
    let mut signals: Vec<Computed<WorkingColor>> = scale
        .stops
        .iter()
        .map(|(_, color)| color.resolve(env))
        .collect();
    signals.push(Color::muted_foreground().resolve(env));
    combine_resolved_colors(signals)
        .map(move |colors| {
            let fallback = colors
                .last()
                .copied()
                .expect("resolved color scale includes fallback");
            ResolvedColorScale::from_parts(&positions, &colors[..positions.len()], fallback)
        })
        .computed()
}
