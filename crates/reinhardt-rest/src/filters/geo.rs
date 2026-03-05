//! Geographic and spatial filtering for location-based queries
//!
//! Provides filters for working with geographic data, including distance-based
//! filtering, bounding box queries, and polygon containment.

use geo_types::{Point, Polygon, Rect};
use std::marker::PhantomData;

/// Unit for distance calculations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistanceUnit {
	/// Meters
	Meters,
	/// Kilometers
	Kilometers,
	/// Miles
	Miles,
	/// Feet
	Feet,
}

impl DistanceUnit {
	/// Convert distance to meters
	pub fn to_meters(self, distance: f64) -> f64 {
		match self {
			DistanceUnit::Meters => distance,
			DistanceUnit::Kilometers => distance * 1000.0,
			DistanceUnit::Miles => distance * 1609.34,
			DistanceUnit::Feet => distance * 0.3048,
		}
	}

	/// Convert distance from meters
	pub fn from_meters(self, meters: f64) -> f64 {
		match self {
			DistanceUnit::Meters => meters,
			DistanceUnit::Kilometers => meters / 1000.0,
			DistanceUnit::Miles => meters / 1609.34,
			DistanceUnit::Feet => meters / 0.3048,
		}
	}
}

/// Distance filter for finding points within a radius
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::geo::{DistanceFilter, DistanceUnit};
/// use geo_types::Point;
///
/// let center = Point::new(0.0, 0.0);
/// let filter = DistanceFilter::<()>::new(center, 1000.0, DistanceUnit::Meters);
///
/// assert_eq!(filter.distance(), 1000.0);
/// ```
pub struct DistanceFilter<M> {
	center: Point<f64>,
	distance: f64,
	unit: DistanceUnit,
	field_name: String,
	_phantom: PhantomData<M>,
}

impl<M> DistanceFilter<M> {
	/// Creates a new distance filter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::geo::{DistanceFilter, DistanceUnit};
	/// use geo_types::Point;
	///
	/// let center = Point::new(37.7749, -122.4194); // San Francisco
	/// let filter = DistanceFilter::<()>::new(center, 5.0, DistanceUnit::Kilometers);
	/// ```
	pub fn new(center: Point<f64>, distance: f64, unit: DistanceUnit) -> Self {
		Self {
			center,
			distance,
			unit,
			field_name: "location".to_string(),
			_phantom: PhantomData,
		}
	}

	/// Set the field name to filter on
	pub fn field(mut self, field_name: impl Into<String>) -> Self {
		self.field_name = field_name.into();
		self
	}

	/// Get the center point
	pub fn center(&self) -> Point<f64> {
		self.center
	}

	/// Get the distance
	pub fn distance(&self) -> f64 {
		self.distance
	}

	/// Get the distance unit
	pub fn unit(&self) -> DistanceUnit {
		self.unit
	}

	/// Calculate distance between two points (Haversine formula)
	pub fn calculate_distance(&self, point: Point<f64>) -> f64 {
		const EARTH_RADIUS_METERS: f64 = 6371000.0;

		let lat1 = self.center.y().to_radians();
		let lat2 = point.y().to_radians();
		let dlat = (point.y() - self.center.y()).to_radians();
		let dlon = (point.x() - self.center.x()).to_radians();

		let a = (dlat / 2.0).sin() * (dlat / 2.0).sin()
			+ lat1.cos() * lat2.cos() * (dlon / 2.0).sin() * (dlon / 2.0).sin();
		let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

		let distance_meters = EARTH_RADIUS_METERS * c;
		self.unit.from_meters(distance_meters)
	}

	/// Check if a point is within the filter radius
	pub fn contains(&self, point: Point<f64>) -> bool {
		self.calculate_distance(point) <= self.distance
	}
}

/// Bounding box filter for rectangular area queries
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::geo::BoundingBoxFilter;
/// use geo_types::Rect;
///
/// let bbox = Rect::new((0.0, 0.0), (10.0, 10.0));
/// let filter = BoundingBoxFilter::<()>::new(bbox);
/// ```
pub struct BoundingBoxFilter<M> {
	bbox: Rect<f64>,
	field_name: String,
	_phantom: PhantomData<M>,
}

impl<M> BoundingBoxFilter<M> {
	/// Creates a new bounding box filter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::geo::BoundingBoxFilter;
	/// use geo_types::Rect;
	///
	/// // San Francisco Bay Area approximate bounds
	/// let bbox = Rect::new((37.0, -123.0), (38.0, -121.0));
	/// let filter = BoundingBoxFilter::<()>::new(bbox);
	/// ```
	pub fn new(bbox: Rect<f64>) -> Self {
		Self {
			bbox,
			field_name: "location".to_string(),
			_phantom: PhantomData,
		}
	}

	/// Set the field name to filter on
	pub fn field(mut self, field_name: impl Into<String>) -> Self {
		self.field_name = field_name.into();
		self
	}

	/// Get the bounding box
	pub fn bbox(&self) -> Rect<f64> {
		self.bbox
	}

	/// Check if a point is within the bounding box
	pub fn contains(&self, point: Point<f64>) -> bool {
		let min = self.bbox.min();
		let max = self.bbox.max();
		point.x() >= min.x && point.x() <= max.x && point.y() >= min.y && point.y() <= max.y
	}
}

/// Polygon containment filter
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::geo::PolygonFilter;
/// use geo_types::{coord, Polygon};
///
/// let polygon = Polygon::new(
///     vec![
///         coord! { x: 0.0, y: 0.0 },
///         coord! { x: 10.0, y: 0.0 },
///         coord! { x: 10.0, y: 10.0 },
///         coord! { x: 0.0, y: 10.0 },
///         coord! { x: 0.0, y: 0.0 },
///     ].into(),
///     vec![],
/// );
/// let filter = PolygonFilter::<()>::new(polygon);
/// ```
pub struct PolygonFilter<M> {
	polygon: Polygon<f64>,
	field_name: String,
	_phantom: PhantomData<M>,
}

impl<M> PolygonFilter<M> {
	/// Creates a new polygon filter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::geo::PolygonFilter;
	/// use geo_types::{coord, Polygon};
	///
	/// // Triangle polygon
	/// let polygon = Polygon::new(
	///     vec![
	///         coord! { x: 0.0, y: 0.0 },
	///         coord! { x: 5.0, y: 10.0 },
	///         coord! { x: 10.0, y: 0.0 },
	///         coord! { x: 0.0, y: 0.0 },
	///     ].into(),
	///     vec![],
	/// );
	/// let filter = PolygonFilter::<()>::new(polygon);
	/// ```
	pub fn new(polygon: Polygon<f64>) -> Self {
		Self {
			polygon,
			field_name: "location".to_string(),
			_phantom: PhantomData,
		}
	}

	/// Set the field name to filter on
	pub fn field(mut self, field_name: impl Into<String>) -> Self {
		self.field_name = field_name.into();
		self
	}

	/// Get the polygon
	pub fn polygon(&self) -> &Polygon<f64> {
		&self.polygon
	}

	/// Check if a point is within the polygon (ray casting algorithm)
	pub fn contains(&self, point: Point<f64>) -> bool {
		let exterior = self.polygon.exterior();
		let mut inside = false;
		let mut j = exterior.0.len() - 1;

		for i in 0..exterior.0.len() {
			let vi = &exterior.0[i];
			let vj = &exterior.0[j];

			if ((vi.y > point.y()) != (vj.y > point.y()))
				&& (point.x() < (vj.x - vi.x) * (point.y() - vi.y) / (vj.y - vi.y) + vi.x)
			{
				inside = !inside;
			}
			j = i;
		}

		inside
	}
}

/// Nearby filter for finding closest points
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::geo::{NearbyFilter, DistanceUnit};
/// use geo_types::Point;
///
/// let center = Point::new(0.0, 0.0);
/// let filter = NearbyFilter::<()>::new(center, 10);
///
/// assert_eq!(filter.limit(), 10);
/// ```
pub struct NearbyFilter<M> {
	center: Point<f64>,
	limit: usize,
	max_distance: Option<f64>,
	unit: DistanceUnit,
	field_name: String,
	_phantom: PhantomData<M>,
}

impl<M> NearbyFilter<M> {
	/// Creates a new nearby filter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::geo::NearbyFilter;
	/// use geo_types::Point;
	///
	/// let center = Point::new(37.7749, -122.4194);
	/// let filter = NearbyFilter::<()>::new(center, 5); // Find 5 nearest points
	/// ```
	pub fn new(center: Point<f64>, limit: usize) -> Self {
		Self {
			center,
			limit,
			max_distance: None,
			unit: DistanceUnit::Meters,
			field_name: "location".to_string(),
			_phantom: PhantomData,
		}
	}

	/// Set maximum distance
	pub fn max_distance(mut self, distance: f64, unit: DistanceUnit) -> Self {
		self.max_distance = Some(distance);
		self.unit = unit;
		self
	}

	/// Set the field name to filter on
	pub fn field(mut self, field_name: impl Into<String>) -> Self {
		self.field_name = field_name.into();
		self
	}

	/// Get the center point
	pub fn center(&self) -> Point<f64> {
		self.center
	}

	/// Get the limit
	pub fn limit(&self) -> usize {
		self.limit
	}

	/// Calculate distance to center (Haversine formula)
	pub fn calculate_distance(&self, point: Point<f64>) -> f64 {
		const EARTH_RADIUS_METERS: f64 = 6371000.0;

		let lat1 = self.center.y().to_radians();
		let lat2 = point.y().to_radians();
		let dlat = (point.y() - self.center.y()).to_radians();
		let dlon = (point.x() - self.center.x()).to_radians();

		let a = (dlat / 2.0).sin() * (dlat / 2.0).sin()
			+ lat1.cos() * lat2.cos() * (dlon / 2.0).sin() * (dlon / 2.0).sin();
		let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

		let distance_meters = EARTH_RADIUS_METERS * c;
		self.unit.from_meters(distance_meters)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use geo_types::coord;

	#[test]
	fn test_distance_unit_conversion() {
		assert_eq!(DistanceUnit::Meters.to_meters(1000.0), 1000.0);
		assert_eq!(DistanceUnit::Kilometers.to_meters(1.0), 1000.0);
		assert!((DistanceUnit::Miles.to_meters(1.0) - 1609.34).abs() < 0.01);
		assert!((DistanceUnit::Feet.to_meters(1.0) - 0.3048).abs() < 0.0001);

		assert_eq!(DistanceUnit::Meters.from_meters(1000.0), 1000.0);
		assert_eq!(DistanceUnit::Kilometers.from_meters(1000.0), 1.0);
		assert!((DistanceUnit::Miles.from_meters(1609.34) - 1.0).abs() < 0.01);
		assert!((DistanceUnit::Feet.from_meters(0.3048) - 1.0).abs() < 0.0001);
	}

	#[test]
	fn test_distance_filter_creation() {
		let center = Point::new(0.0, 0.0);
		let filter = DistanceFilter::<()>::new(center, 1000.0, DistanceUnit::Meters);

		assert_eq!(filter.center(), center);
		assert_eq!(filter.distance(), 1000.0);
		assert_eq!(filter.unit(), DistanceUnit::Meters);
	}

	#[test]
	fn test_distance_calculation() {
		let center = Point::new(0.0, 0.0);
		let filter = DistanceFilter::<()>::new(center, 1000.0, DistanceUnit::Kilometers);

		// Point at (0, 0) should be 0km away
		let same_point = Point::new(0.0, 0.0);
		assert!(filter.calculate_distance(same_point) < 0.001);

		// Point at (0, 1) should be approximately 111km away
		let one_degree_north = Point::new(0.0, 1.0);
		let distance = filter.calculate_distance(one_degree_north);
		assert!(distance > 110.0 && distance < 112.0);
	}

	#[test]
	fn test_distance_filter_contains() {
		let center = Point::new(0.0, 0.0);
		let filter = DistanceFilter::<()>::new(center, 100.0, DistanceUnit::Kilometers);

		// Point within radius
		let near_point = Point::new(0.1, 0.1);
		assert!(filter.contains(near_point));

		// Point outside radius
		let far_point = Point::new(10.0, 10.0);
		assert!(!filter.contains(far_point));
	}

	#[test]
	fn test_bounding_box_filter() {
		let bbox = Rect::new(coord! { x: 0.0, y: 0.0 }, coord! { x: 10.0, y: 10.0 });
		let filter = BoundingBoxFilter::<()>::new(bbox);

		// Point inside
		assert!(filter.contains(Point::new(5.0, 5.0)));

		// Point on edge
		assert!(filter.contains(Point::new(0.0, 0.0)));
		assert!(filter.contains(Point::new(10.0, 10.0)));

		// Point outside
		assert!(!filter.contains(Point::new(11.0, 5.0)));
		assert!(!filter.contains(Point::new(5.0, 11.0)));
		assert!(!filter.contains(Point::new(-1.0, 5.0)));
	}

	#[test]
	fn test_polygon_filter_square() {
		let polygon = Polygon::new(
			vec![
				coord! { x: 0.0, y: 0.0 },
				coord! { x: 10.0, y: 0.0 },
				coord! { x: 10.0, y: 10.0 },
				coord! { x: 0.0, y: 10.0 },
				coord! { x: 0.0, y: 0.0 },
			]
			.into(),
			vec![],
		);
		let filter = PolygonFilter::<()>::new(polygon);

		// Point inside
		assert!(filter.contains(Point::new(5.0, 5.0)));

		// Point outside
		assert!(!filter.contains(Point::new(11.0, 5.0)));
		assert!(!filter.contains(Point::new(5.0, 11.0)));
		assert!(!filter.contains(Point::new(-1.0, 5.0)));
	}

	#[test]
	fn test_polygon_filter_triangle() {
		let polygon = Polygon::new(
			vec![
				coord! { x: 0.0, y: 0.0 },
				coord! { x: 10.0, y: 0.0 },
				coord! { x: 5.0, y: 10.0 },
				coord! { x: 0.0, y: 0.0 },
			]
			.into(),
			vec![],
		);
		let filter = PolygonFilter::<()>::new(polygon);

		// Point inside triangle
		assert!(filter.contains(Point::new(5.0, 5.0)));

		// Point outside triangle
		assert!(!filter.contains(Point::new(1.0, 9.0)));
		assert!(!filter.contains(Point::new(9.0, 9.0)));
	}

	#[test]
	fn test_nearby_filter_creation() {
		let center = Point::new(37.7749, -122.4194);
		let filter = NearbyFilter::<()>::new(center, 10);

		assert_eq!(filter.center(), center);
		assert_eq!(filter.limit(), 10);
	}

	#[test]
	fn test_nearby_filter_with_max_distance() {
		let center = Point::new(0.0, 0.0);
		let filter = NearbyFilter::<()>::new(center, 5).max_distance(1000.0, DistanceUnit::Meters);

		assert_eq!(filter.limit(), 5);
		assert_eq!(filter.max_distance, Some(1000.0));
		assert_eq!(filter.unit, DistanceUnit::Meters);
	}

	#[test]
	fn test_filter_field_customization() {
		let center = Point::new(0.0, 0.0);
		let filter =
			DistanceFilter::<()>::new(center, 1000.0, DistanceUnit::Meters).field("coordinates");

		assert_eq!(filter.field_name, "coordinates");
	}
}
