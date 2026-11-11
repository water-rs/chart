//! Hit geometry: the shapes a chart's data answer pointer input with.
//!
//! A chart derives its hit geometry from the same inputs as its drawing —
//! data, viewport and the canvas's size — and publishes it as part of its
//! frame, so a gesture is resolved against exactly the geometry the canvas is
//! drawn from.

extern crate alloc;

use alloc::vec::Vec;
use core::f32::consts::TAU;

use waterui_core::{
    gesture::GesturePoint,
    layout::{Point, Rect},
};

use crate::{
    data::DataBounds,
    interaction::{ChartViewport, HitResult},
};

#[derive(Debug, Clone)]
enum HitShape {
    Circle {
        center: Point,
        radius: f32,
    },
    Rect {
        rect: Rect,
    },
    Sector {
        center: Point,
        inner_radius: f32,
        outer_radius: f32,
        start_angle: f32,
        end_angle: f32,
    },
    Polygon {
        vertices: Vec<Point>,
    },
}

#[derive(Debug, Clone)]
struct HitTarget<T> {
    result: HitResult<T>,
    shape: HitShape,
}

impl<T> HitTarget<T> {
    fn contains(&self, point: Point) -> bool {
        match &self.shape {
            HitShape::Circle { center, radius } => {
                distance_squared(*center, point) <= radius * radius
            }
            HitShape::Rect { rect } => rect_contains(*rect, point),
            HitShape::Sector {
                center,
                inner_radius,
                outer_radius,
                start_angle,
                end_angle,
            } => sector_contains(
                *center,
                point,
                *inner_radius,
                *outer_radius,
                *start_angle,
                *end_angle,
            ),
            HitShape::Polygon { vertices } => polygon_contains(vertices, point),
        }
    }

    fn score(&self, point: Point) -> Option<f32> {
        self.contains(point)
            .then_some(distance_squared(self.result.anchor.as_point(), point))
    }
}

/// What a chart answers a pointer position with.
pub trait HitGeometry<T: Clone>: Clone + 'static {
    /// The datum under `point`, nearest to its anchor when several overlap.
    fn hit_test(&self, point: Point) -> Option<HitResult<T>>;
    /// The plot area, in the canvas's coordinates.
    fn viewport(&self) -> ChartViewport;
}

#[derive(Debug, Clone)]
pub struct HitTargets<T> {
    viewport: ChartViewport,
    targets: Vec<HitTarget<T>>,
}

impl<T> HitTargets<T> {
    pub(super) const fn new(viewport: ChartViewport) -> Self {
        Self {
            viewport,
            targets: Vec::new(),
        }
    }

    pub(super) fn push_circle(&mut self, result: HitResult<T>, center: Point, radius: f32) {
        self.targets.push(HitTarget {
            result,
            shape: HitShape::Circle {
                center,
                radius: radius.max(1.0),
            },
        });
    }

    pub(super) fn push_rect(&mut self, result: HitResult<T>, rect: Rect) {
        self.targets.push(HitTarget {
            result,
            shape: HitShape::Rect { rect },
        });
    }

    pub(super) fn push_sector(
        &mut self,
        result: HitResult<T>,
        center: Point,
        inner_radius: f32,
        outer_radius: f32,
        start_angle: f32,
        end_angle: f32,
    ) {
        self.targets.push(HitTarget {
            result,
            shape: HitShape::Sector {
                center,
                inner_radius,
                outer_radius,
                start_angle,
                end_angle,
            },
        });
    }

    pub(super) fn push_polygon(&mut self, result: HitResult<T>, vertices: Vec<Point>) {
        self.targets.push(HitTarget {
            result,
            shape: HitShape::Polygon { vertices },
        });
    }
}

impl<T: Clone + 'static> HitGeometry<T> for HitTargets<T> {
    fn hit_test(&self, point: Point) -> Option<HitResult<T>> {
        let mut best: Option<(f32, &HitTarget<T>)> = None;
        for target in &self.targets {
            let Some(score) = target.score(point) else {
                continue;
            };
            if best
                .as_ref()
                .is_none_or(|(best_score, _)| score < *best_score)
            {
                best = Some((score, target));
            }
        }
        best.map(|(_, target)| target.result.clone())
    }

    fn viewport(&self) -> ChartViewport {
        self.viewport
    }
}

/// The hit geometry of a chart with a data window.
#[derive(Debug, Clone)]
pub struct CartesianGeometry<T> {
    /// The data window the chart shows.
    pub bounds: DataBounds,
    /// The data window of the whole data set, which scrolling stays inside.
    pub base_bounds: DataBounds,
    viewport: ChartViewport,
    targets: HitTargets<T>,
}

impl<T> CartesianGeometry<T> {
    pub(super) const fn new(
        bounds: DataBounds,
        viewport: ChartViewport,
        targets: HitTargets<T>,
    ) -> Self {
        Self {
            bounds,
            base_bounds: bounds,
            viewport,
            targets,
        }
    }

    /// The same geometry, scrolling inside `base_bounds`.
    pub(super) const fn within(mut self, base_bounds: DataBounds) -> Self {
        self.base_bounds = base_bounds;
        self
    }
}

impl<T: Clone + 'static> HitGeometry<T> for CartesianGeometry<T> {
    fn hit_test(&self, point: Point) -> Option<HitResult<T>> {
        self.targets.hit_test(point)
    }

    fn viewport(&self) -> ChartViewport {
        self.viewport
    }
}

pub const fn gesture_point_to_point(point: GesturePoint) -> Point {
    Point::new(point.x, point.y)
}

fn distance_squared(a: Point, b: Point) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx.mul_add(dx, dy * dy)
}

fn rect_contains(rect: Rect, point: Point) -> bool {
    point.x >= rect.min_x()
        && point.x <= rect.max_x()
        && point.y >= rect.min_y()
        && point.y <= rect.max_y()
}

fn normalize_angle(angle: f32) -> f32 {
    angle.rem_euclid(TAU)
}

fn angle_in_sweep(angle: f32, start: f32, end: f32) -> bool {
    let angle = normalize_angle(angle);
    let start = normalize_angle(start);
    let end = normalize_angle(end);
    if start <= end {
        angle >= start && angle <= end
    } else {
        angle >= start || angle <= end
    }
}

fn sector_contains(
    center: Point,
    point: Point,
    inner_radius: f32,
    outer_radius: f32,
    start_angle: f32,
    end_angle: f32,
) -> bool {
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    let distance = dx.hypot(dy);
    if distance < inner_radius || distance > outer_radius {
        return false;
    }
    let angle = dy.atan2(dx);
    angle_in_sweep(angle, start_angle, end_angle)
}

fn polygon_contains(vertices: &[Point], point: Point) -> bool {
    if vertices.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut previous = *vertices
        .last()
        .expect("polygon_contains requires non-empty vertices");
    for &current in vertices {
        let crosses = (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y)
                    / ((previous.y - current.y).abs().max(f32::EPSILON))
                    + current.x;
        if crosses {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// The data-space x under `point`, or under the nearest plot edge when
/// `clamp`; `None` outside the plot otherwise.
pub fn cartesian_x_from_point<T>(
    geometry: &CartesianGeometry<T>,
    point: Point,
    clamp: bool,
) -> Option<f32> {
    let viewport = geometry.viewport;
    if viewport.width <= 0.0 {
        return None;
    }
    let x = if clamp {
        point.x.clamp(viewport.x, viewport.x + viewport.width)
    } else if viewport.contains(point) {
        point.x
    } else {
        return None;
    };
    let normalized_x = ((x - viewport.x) / viewport.width).clamp(0.0, 1.0);
    Some(
        geometry
            .bounds
            .width()
            .mul_add(normalized_x, geometry.bounds.min_x),
    )
}

/// The data-space y under `point`, or under the nearest plot edge when
/// `clamp`; `None` outside the plot otherwise.
pub fn cartesian_y_from_point<T>(
    geometry: &CartesianGeometry<T>,
    point: Point,
    clamp: bool,
) -> Option<f32> {
    let viewport = geometry.viewport;
    if viewport.height <= 0.0 {
        return None;
    }
    let y = if clamp {
        point.y.clamp(viewport.y, viewport.y + viewport.height)
    } else if viewport.contains(point) {
        point.y
    } else {
        return None;
    };
    let normalized_y = ((y - viewport.y) / viewport.height).clamp(0.0, 1.0);
    Some(
        geometry
            .bounds
            .height()
            .mul_add(-normalized_y, geometry.bounds.max_y),
    )
}

/// `start..=end` in ascending order.
#[inline]
pub fn normalized_scalar_range(start: f32, end: f32) -> core::ops::RangeInclusive<f32> {
    if start <= end {
        start..=end
    } else {
        end..=start
    }
}
