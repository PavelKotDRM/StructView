use crate::{RoutingError, RoutingResult};
use std::ops::{Add, Div, Mul, Neg, Sub};

/// A point in a two-dimensional routing plane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
}

impl Point {
    /// The origin.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Create a point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Distance to another point.
    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    /// Squared distance to another point.
    pub fn distance_sq(self, other: Self) -> f32 {
        (self - other).length_sq()
    }

    /// Whether both coordinates are finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add<Vector> for Point {
    type Output = Self;

    fn add(self, vector: Vector) -> Self::Output {
        Self::new(self.x + vector.x, self.y + vector.y)
    }
}

impl Sub<Point> for Point {
    type Output = Vector;

    fn sub(self, point: Point) -> Self::Output {
        Vector::new(self.x - point.x, self.y - point.y)
    }
}

/// A vector in a two-dimensional routing plane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector {
    /// Horizontal component.
    pub x: f32,
    /// Vertical component.
    pub y: f32,
}

impl Vector {
    /// The zero vector.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    /// The unit vector pointing right.
    pub const X: Self = Self { x: 1.0, y: 0.0 };
    /// The unit vector pointing down.
    pub const Y: Self = Self { x: 0.0, y: 1.0 };

    /// Create a vector.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Vector length.
    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }

    /// Squared vector length.
    pub fn length_sq(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// Dot product with another vector.
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Unit vector in the same direction, or zero for a zero-length vector.
    pub fn normalized(self) -> Self {
        let length = self.length();
        if length > f32::EPSILON {
            self / length
        } else {
            Self::ZERO
        }
    }

    /// Whether both components are finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Vector {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Vector {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f32> for Vector {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self::Output {
        Self::new(self.x * scalar, self.y * scalar)
    }
}

impl Div<f32> for Vector {
    type Output = Self;

    fn div(self, scalar: f32) -> Self::Output {
        Self::new(self.x / scalar, self.y / scalar)
    }
}

impl Neg for Vector {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

/// A positive width and height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Size {
    /// Create a size.
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub(crate) fn is_valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0
    }
}

/// An axis-aligned rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Minimum corner.
    pub min: Point,
    /// Maximum corner.
    pub max: Point,
}

impl Rect {
    /// Create a rectangle from ordered minimum and maximum corners.
    pub const fn from_min_max(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    /// Create a rectangle centered on `center`.
    pub fn from_center_size(center: Point, size: Size) -> Self {
        let half = Vector::new(size.width / 2.0, size.height / 2.0);
        Self {
            min: center + -half,
            max: center + half,
        }
    }

    /// Create the smallest rectangle containing both points.
    pub fn from_two_points(first: Point, second: Point) -> Self {
        Self {
            min: Point::new(first.x.min(second.x), first.y.min(second.y)),
            max: Point::new(first.x.max(second.x), first.y.max(second.y)),
        }
    }

    /// Expand all sides by `amount`.
    pub fn expand(self, amount: f32) -> Self {
        let margin = Vector::new(amount, amount);
        Self {
            min: self.min + -margin,
            max: self.max + margin,
        }
    }

    /// Left coordinate.
    pub fn left(self) -> f32 {
        self.min.x
    }

    /// Right coordinate.
    pub fn right(self) -> f32 {
        self.max.x
    }

    /// Top coordinate.
    pub fn top(self) -> f32 {
        self.min.y
    }

    /// Bottom coordinate.
    pub fn bottom(self) -> f32 {
        self.max.y
    }

    pub(crate) fn is_valid(self) -> bool {
        self.min.is_finite()
            && self.max.is_finite()
            && self.min.x <= self.max.x
            && self.min.y <= self.max.y
    }
}

/// Whether a segment intersects the interior of an axis-aligned rectangle.
pub fn segment_intersects_rect(start: Point, end: Point, rect: Rect) -> bool {
    let direction = end - start;
    let mut first_intersection: f32 = 0.0;
    let mut last_intersection: f32 = 1.0;
    for (origin, delta, minimum, maximum) in [
        (start.x, direction.x, rect.left(), rect.right()),
        (start.y, direction.y, rect.top(), rect.bottom()),
    ] {
        if delta == 0.0 {
            if origin <= minimum || origin >= maximum {
                return false;
            }
            continue;
        }

        let first = (minimum - origin) / delta;
        let last = (maximum - origin) / delta;
        first_intersection = first_intersection.max(first.min(last));
        last_intersection = last_intersection.min(first.max(last));
        if first_intersection >= last_intersection {
            return false;
        }
    }
    first_intersection < last_intersection
}

/// Cross product of two 2D vectors.
pub fn cross_product(first: Vector, second: Vector) -> f32 {
    first.x * second.y - first.y * second.x
}

/// Closest point on the segment from `start` to `end`.
pub fn closest_point_on_segment(point: Point, start: Point, end: Point) -> Point {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared == 0.0 {
        return start;
    }
    let projection = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    start + segment * projection
}

/// Distance from `point` to the segment from `start` to `end`.
pub fn point_to_segment_distance(point: Point, start: Point, end: Point) -> f32 {
    point.distance(closest_point_on_segment(point, start, end))
}

/// Whether two non-collinear segments intersect.
///
/// Collinear overlap is considered by [`segments_within_clearance`].
pub fn segments_intersect(
    first_start: Point,
    first_end: Point,
    second_start: Point,
    second_end: Point,
) -> bool {
    let first_direction = first_end - first_start;
    let second_direction = second_end - second_start;
    let denominator = cross_product(first_direction, second_direction);
    if denominator == 0.0 {
        return false;
    }

    let between_starts = second_start - first_start;
    let first_position = cross_product(between_starts, second_direction) / denominator;
    let second_position = cross_product(between_starts, first_direction) / denominator;
    (0.0..=1.0).contains(&first_position) && (0.0..=1.0).contains(&second_position)
}

/// Whether two segments intersect or are within `clearance` of each other.
pub fn segments_within_clearance(
    first_start: Point,
    first_end: Point,
    second_start: Point,
    second_end: Point,
    clearance: f32,
) -> bool {
    if segments_intersect(first_start, first_end, second_start, second_end) {
        return true;
    }
    [
        point_to_segment_distance(first_start, second_start, second_end),
        point_to_segment_distance(first_end, second_start, second_end),
        point_to_segment_distance(second_start, first_start, first_end),
        point_to_segment_distance(second_end, first_start, first_end),
    ]
    .into_iter()
    .any(|distance| distance <= clearance)
}

pub(crate) fn validate_point(point: Point) -> RoutingResult<()> {
    if point.is_finite() {
        Ok(())
    } else {
        Err(RoutingError::InvalidGeometry)
    }
}
