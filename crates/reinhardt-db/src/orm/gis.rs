/// GIS (Geographic Information System) Support
/// Spatial data types and operations
use serde::{Deserialize, Serialize};

// ============= UTILITY FUNCTIONS =============

/// Calculate distance between two points using the Haversine formula
/// This is accurate for geographic coordinates (latitude/longitude)
fn haversine_distance(p1: &Point, p2: &Point) -> f64 {
	const EARTH_RADIUS_KM: f64 = 6371.0;

	let lat1 = p1.y.to_radians();
	let lat2 = p2.y.to_radians();
	let delta_lat = (p2.y - p1.y).to_radians();
	let delta_lon = (p2.x - p1.x).to_radians();

	let a =
		(delta_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
	let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

	EARTH_RADIUS_KM * c * 1000.0 // Return in meters
}

// ============= SPATIAL DATA TYPES =============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
	pub x: f64,
	pub y: f64,
	pub srid: i32, // Spatial Reference System Identifier
}

impl Point {
	/// Create a new geographic Point with WGS 84 coordinate system (SRID 4326)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::gis::Point;
	///
	/// let tokyo = Point::new(139.6917, 35.6895); // Longitude, Latitude
	/// assert_eq!(tokyo.x, 139.6917);
	/// assert_eq!(tokyo.y, 35.6895);
	/// assert_eq!(tokyo.srid, 4326); // WGS 84 (GPS coordinates)
	/// ```
	pub fn new(x: f64, y: f64) -> Self {
		Self { x, y, srid: 4326 } // WGS 84
	}
	/// Documentation for `with_srid`
	pub fn with_srid(x: f64, y: f64, srid: i32) -> Self {
		Self { x, y, srid }
	}
	/// Calculate distance to another point
	/// Automatically selects appropriate method based on SRID:
	/// - SRID 4326 (WGS84): Uses Haversine formula for geographic distance in meters
	/// - Other SRIDs: Uses Euclidean distance for planar coordinates
	///
	pub fn distance_to(&self, other: &Point) -> f64 {
		// If both points use WGS84 (SRID 4326), use Haversine for accurate geographic distance
		if self.srid == 4326 && other.srid == 4326 {
			haversine_distance(self, other)
		} else if self.srid != other.srid {
			// Points have different SRIDs - this should be handled by transformation first
			eprintln!(
				"Warning: Computing distance between points with different SRIDs ({} and {}). \
                 Consider transforming to a common SRID first for accurate results.",
				self.srid, other.srid
			);
			// Fall back to Euclidean distance
			((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
		} else {
			// Same SRID, not WGS84 - use planar Euclidean distance
			((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineString {
	pub points: Vec<Point>,
	pub srid: i32,
}

impl LineString {
	/// Calculate length of the linestring using Euclidean distance
	/// Note: This calculates planar distance. For geographic coordinates,
	/// consider using great-circle or geodesic distance calculations.
	///
	pub fn length(&self) -> f64 {
		self.points
			.windows(2)
			.map(|pts| {
				let dx = pts[1].x - pts[0].x;
				let dy = pts[1].y - pts[0].y;
				(dx * dx + dy * dy).sqrt()
			})
			.sum()
	}
	/// Calculate length considering the SRID (geographic distance if using WGS84)
	///
	pub fn geodesic_length(&self) -> f64 {
		if self.srid == 4326 {
			// WGS84 - use Haversine formula for better accuracy
			self.points
				.windows(2)
				.map(|pts| haversine_distance(&pts[0], &pts[1]))
				.sum()
		} else {
			// For other SRIDs, fall back to planar distance
			self.length()
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Polygon {
	pub exterior: Vec<Point>,
	pub interiors: Vec<Vec<Point>>,
	pub srid: i32,
}

impl Polygon {
	/// Calculate area of the polygon using the shoelace formula
	/// Takes into account interior rings (holes) if present
	///
	pub fn area(&self) -> f64 {
		if self.exterior.len() < 3 {
			return 0.0;
		}

		// Calculate exterior area
		let exterior_area = calculate_ring_area(&self.exterior);

		// Subtract interior areas (holes)
		let interior_area: f64 = self
			.interiors
			.iter()
			.map(|ring| calculate_ring_area(ring))
			.sum();

		(exterior_area - interior_area).abs()
	}
	/// Calculate the exterior boundary area only (ignoring holes)
	///
	pub fn exterior_area(&self) -> f64 {
		calculate_ring_area(&self.exterior)
	}
	/// Calculate perimeter length
	///
	pub fn perimeter(&self) -> f64 {
		let mut total = calculate_ring_perimeter(&self.exterior);
		for interior in &self.interiors {
			total += calculate_ring_perimeter(interior);
		}
		total
	}
	/// Check if a point is inside the polygon (simple implementation)
	///
	pub fn contains_point(&self, point: &Point) -> bool {
		point_in_polygon(point, &self.exterior)
	}
}

/// Calculate area of a ring using the shoelace formula
fn calculate_ring_area(ring: &[Point]) -> f64 {
	if ring.len() < 3 {
		return 0.0;
	}

	let mut area = 0.0;
	for i in 0..ring.len() {
		let j = (i + 1) % ring.len();
		area += ring[i].x * ring[j].y;
		area -= ring[j].x * ring[i].y;
	}
	(area / 2.0).abs()
}

/// Calculate perimeter of a ring
fn calculate_ring_perimeter(ring: &[Point]) -> f64 {
	if ring.len() < 2 {
		return 0.0;
	}

	let mut total = 0.0;
	for i in 0..ring.len() {
		let j = (i + 1) % ring.len();
		let dx = ring[j].x - ring[i].x;
		let dy = ring[j].y - ring[i].y;
		total += (dx * dx + dy * dy).sqrt();
	}
	total
}

/// Point-in-polygon test using ray casting algorithm
fn point_in_polygon(point: &Point, polygon: &[Point]) -> bool {
	let mut inside = false;
	let n = polygon.len();

	for i in 0..n {
		let j = (i + 1) % n;
		let pi = &polygon[i];
		let pj = &polygon[j];

		if ((pi.y > point.y) != (pj.y > point.y))
			&& (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
		{
			inside = !inside;
		}
	}

	inside
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiPoint {
	pub points: Vec<Point>,
	pub srid: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiLineString {
	pub lines: Vec<LineString>,
	pub srid: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiPolygon {
	pub polygons: Vec<Polygon>,
	pub srid: i32,
}

// ============= SPATIAL OPERATIONS =============

pub trait SpatialOps {
	/// Check if geometry contains another
	fn contains(&self, other: &dyn SpatialOps) -> bool;

	/// Check if geometries intersect
	fn intersects(&self, other: &dyn SpatialOps) -> bool;

	/// Check if geometry is within another
	fn within(&self, other: &dyn SpatialOps) -> bool;

	/// Check if geometries touch
	fn touches(&self, other: &dyn SpatialOps) -> bool;

	/// Calculate distance
	fn distance(&self, other: &dyn SpatialOps) -> f64;

	/// Get bounding box
	fn bbox(&self) -> BoundingBox;
}

#[derive(Debug, Clone)]
pub struct BoundingBox {
	pub min_x: f64,
	pub min_y: f64,
	pub max_x: f64,
	pub max_y: f64,
}

impl BoundingBox {
	/// Check if bounding box contains a point
	///
	pub fn contains_point(&self, point: &Point) -> bool {
		point.x >= self.min_x
			&& point.x <= self.max_x
			&& point.y >= self.min_y
			&& point.y <= self.max_y
	}
	/// Check if two bounding boxes intersect
	///
	pub fn intersects(&self, other: &BoundingBox) -> bool {
		!(self.max_x < other.min_x
			|| self.min_x > other.max_x
			|| self.max_y < other.min_y
			|| self.min_y > other.max_y)
	}
}

// ============= SPATIAL QUERIES =============

pub enum SpatialLookup {
	Contains(Point),
	Within(Polygon),
	Intersects(Polygon),
	DWithin(Point, f64), // Distance within
	BBContains(BoundingBox),
	BBOverlaps(BoundingBox),
}

impl SpatialLookup {
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		match self {
			SpatialLookup::Contains(point) => {
				format!(
					"ST_Contains(geometry, ST_GeomFromText('POINT({} {})', 4326))",
					point.x, point.y
				)
			}
			SpatialLookup::DWithin(point, distance) => {
				format!(
					"ST_DWithin(geometry, ST_GeomFromText('POINT({} {})', 4326), {})",
					point.x, point.y, distance
				)
			}
			SpatialLookup::Within(polygon) => {
				let coords = polygon
					.exterior
					.iter()
					.map(|p| format!("{} {}", p.x, p.y))
					.collect::<Vec<_>>()
					.join(", ");
				format!(
					"ST_Within(geometry, ST_GeomFromText('POLYGON(({})))', 4326)",
					coords
				)
			}
			SpatialLookup::Intersects(polygon) => {
				let coords = polygon
					.exterior
					.iter()
					.map(|p| format!("{} {}", p.x, p.y))
					.collect::<Vec<_>>()
					.join(", ");
				format!(
					"ST_Intersects(geometry, ST_GeomFromText('POLYGON(({})))', 4326)",
					coords
				)
			}
			SpatialLookup::BBContains(bbox) => {
				format!(
					"geometry && ST_MakeEnvelope({}, {}, {}, {}, 4326)",
					bbox.min_x, bbox.min_y, bbox.max_x, bbox.max_y
				)
			}
			SpatialLookup::BBOverlaps(bbox) => {
				format!(
					"ST_Overlaps(geometry, ST_MakeEnvelope({}, {}, {}, {}, 4326))",
					bbox.min_x, bbox.min_y, bbox.max_x, bbox.max_y
				)
			}
		}
	}
}

// ============= COORDINATE SYSTEMS =============

pub struct CoordinateTransform {
	pub from_srid: i32,
	pub to_srid: i32,
}

impl CoordinateTransform {
	/// Create a coordinate system transformation between SRIDs
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::gis::CoordinateTransform;
	///
	/// let transform = CoordinateTransform::new(4326, 3857);
	/// assert_eq!(transform.from_srid, 4326); // WGS 84
	/// assert_eq!(transform.to_srid, 3857);   // Web Mercator
	/// // Converts GPS coordinates to map projection
	/// ```
	pub fn new(from_srid: i32, to_srid: i32) -> Self {
		Self { from_srid, to_srid }
	}
	/// Transform point coordinates
	/// Supports common transformations: WGS84 (4326) <-> Web Mercator (3857)
	/// For other transformations, consider integrating with PROJ library
	///
	pub fn transform_point(&self, point: &Point) -> Point {
		// WGS84 to Web Mercator (EPSG:3857)
		if self.from_srid == 4326 && self.to_srid == 3857 {
			return wgs84_to_web_mercator(point);
		}

		// Web Mercator to WGS84
		if self.from_srid == 3857 && self.to_srid == 4326 {
			return web_mercator_to_wgs84(point);
		}

		// Same SRID - no transformation needed
		if self.from_srid == self.to_srid {
			return point.clone();
		}

		// For unsupported transformations, return original point with updated SRID
		// In production, this should integrate with PROJ library
		eprintln!(
			"Warning: Coordinate transformation from SRID {} to {} is not implemented. \
             Consider using PROJ library for full support.",
			self.from_srid, self.to_srid
		);

		Point {
			x: point.x,
			y: point.y,
			srid: self.to_srid,
		}
	}
	/// Get SQL for ST_Transform function
	///
	pub fn to_sql(&self, geometry_expr: &str) -> String {
		format!("ST_Transform({}, {})", geometry_expr, self.to_srid)
	}
}

/// Convert WGS84 (EPSG:4326) coordinates to Web Mercator (EPSG:3857)
fn wgs84_to_web_mercator(point: &Point) -> Point {
	const EARTH_RADIUS: f64 = 6378137.0; // Earth radius in meters

	let lon = point.x;
	let lat = point.y;

	// Clamp latitude to avoid infinity
	let lat = lat.clamp(-85.0511, 85.0511);

	let x = lon.to_radians() * EARTH_RADIUS;
	let y = ((std::f64::consts::PI / 4.0 + lat.to_radians() / 2.0).tan()).ln() * EARTH_RADIUS;

	Point { x, y, srid: 3857 }
}

/// Convert Web Mercator (EPSG:3857) coordinates to WGS84 (EPSG:4326)
fn web_mercator_to_wgs84(point: &Point) -> Point {
	const EARTH_RADIUS: f64 = 6378137.0;

	let x = point.x;
	let y = point.y;

	let lon = (x / EARTH_RADIUS).to_degrees();
	let lat = (2.0 * ((y / EARTH_RADIUS).exp()).atan() - std::f64::consts::PI / 2.0).to_degrees();

	Point {
		x: lon,
		y: lat,
		srid: 4326,
	}
}

// ============= SPATIAL INDEXES =============

pub struct GiSTIndex {
	pub column: String,
}

impl GiSTIndex {
	/// Create a GiST spatial index for geometry columns
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::gis::GiSTIndex;
	///
	/// let index = GiSTIndex::new("location");
	/// assert_eq!(index.column, "location");
	/// // GiST indexes enable fast spatial queries
	/// ```
	pub fn new(column: impl Into<String>) -> Self {
		Self {
			column: column.into(),
		}
	}
	/// Documentation for `create_sql`
	///
	pub fn create_sql(&self, table: &str, index_name: &str) -> String {
		format!(
			"CREATE INDEX {} ON {} USING GIST ({})",
			index_name, table, self.column
		)
	}
}

// ============= MEASUREMENTS =============

pub struct Distance {
	value: f64,
	unit: DistanceUnit,
}

#[derive(Debug, Clone, Copy)]
pub enum DistanceUnit {
	Meters,
	Kilometers,
	Miles,
	Feet,
}

impl Distance {
	/// Create a distance value with specified unit
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::gis::{Distance, DistanceUnit};
	///
	/// let distance = Distance::new(1500.0, DistanceUnit::Meters);
	/// // Represents 1.5 kilometers
	/// ```
	pub fn new(value: f64, unit: DistanceUnit) -> Self {
		Self { value, unit }
	}
	/// Documentation for `km`
	///
	pub fn km(value: f64) -> Self {
		Self::new(value, DistanceUnit::Kilometers)
	}
	/// Documentation for `m`
	///
	pub fn m(value: f64) -> Self {
		Self::new(value, DistanceUnit::Meters)
	}
	/// Documentation for `mi`
	///
	pub fn mi(value: f64) -> Self {
		Self::new(value, DistanceUnit::Miles)
	}
	/// Documentation for `ft`
	///
	pub fn ft(value: f64) -> Self {
		Self::new(value, DistanceUnit::Feet)
	}
	/// Documentation for `to_meters`
	///
	pub fn to_meters(&self) -> f64 {
		match self.unit {
			DistanceUnit::Meters => self.value,
			DistanceUnit::Kilometers => self.value * 1000.0,
			DistanceUnit::Miles => self.value * 1609.34,
			DistanceUnit::Feet => self.value * 0.3048,
		}
	}
}

// Spatial aggregates implementation
pub struct SpatialAggregate;

impl SpatialAggregate {
	/// Generate SQL for ST_Union aggregate
	///
	pub fn union_sql(column: &str) -> String {
		format!("ST_Union({})", column)
	}
	/// Generate SQL for ST_Collect aggregate
	///
	pub fn collect_sql(column: &str) -> String {
		format!("ST_Collect({})", column)
	}
	/// Generate SQL for ST_Extent aggregate
	///
	pub fn extent_sql(column: &str) -> String {
		format!("ST_Extent({})", column)
	}
}
// - Measure operations (Area, Length, Perimeter)
// - Simplify, Buffer, Centroid
// - GeoJSON, WKT, WKB format support

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_point_creation() {
		let point = Point::new(10.0, 20.0);
		assert_eq!(point.x, 10.0);
		assert_eq!(point.y, 20.0);
		assert_eq!(point.srid, 4326);
	}

	#[test]
	fn test_point_distance() {
		// Use Cartesian coordinates (not WGS84) for simple Euclidean distance test
		let p1 = Point::with_srid(0.0, 0.0, 0);
		let p2 = Point::with_srid(3.0, 4.0, 0);
		assert_eq!(p1.distance_to(&p2), 5.0);
	}

	#[test]
	fn test_distance_conversions() {
		let d = Distance::km(1.0);
		assert_eq!(d.to_meters(), 1000.0);

		let d2 = Distance::mi(1.0);
		assert!((d2.to_meters() - 1609.34).abs() < 0.01);
	}

	#[test]
	fn test_gist_index_sql() {
		let index = GiSTIndex::new("location");
		let sql = index.create_sql("places", "places_location_idx");
		assert!(sql.contains("GIST"));
		assert!(sql.contains("location"));
	}

	#[test]
	fn test_spatial_lookup_sql() {
		let point = Point::new(10.0, 20.0);
		let lookup = SpatialLookup::Contains(point);
		let sql = lookup.to_sql();
		assert!(sql.contains("ST_Contains"));
		assert!(sql.contains("POINT(10 20)"));
	}

	// Additional comprehensive GIS tests
	#[test]
	fn test_point_with_custom_srid() {
		let point = Point::with_srid(10.0, 20.0, 3857);
		assert_eq!(point.srid, 3857);
		assert_eq!(point.x, 10.0);
		assert_eq!(point.y, 20.0);
	}

	#[test]
	fn test_linestring_length() {
		let line = LineString {
			points: vec![
				Point::new(0.0, 0.0),
				Point::new(3.0, 0.0),
				Point::new(3.0, 4.0),
			],
			srid: 4326,
		};
		assert_eq!(line.length(), 7.0); // 3 + 4
	}

	#[test]
	fn test_linestring_empty() {
		let line = LineString {
			points: vec![],
			srid: 4326,
		};
		assert_eq!(line.length(), 0.0);
	}

	#[test]
	fn test_polygon_area() {
		let polygon = Polygon {
			exterior: vec![
				Point::new(0.0, 0.0),
				Point::new(4.0, 0.0),
				Point::new(4.0, 3.0),
				Point::new(0.0, 3.0),
				Point::new(0.0, 0.0),
			],
			interiors: vec![],
			srid: 4326,
		};
		assert_eq!(polygon.area(), 12.0);
	}

	#[test]
	fn test_polygon_with_hole() {
		let polygon = Polygon {
			exterior: vec![
				Point::new(0.0, 0.0),
				Point::new(10.0, 0.0),
				Point::new(10.0, 10.0),
				Point::new(0.0, 10.0),
				Point::new(0.0, 0.0),
			],
			interiors: vec![vec![
				Point::new(2.0, 2.0),
				Point::new(8.0, 2.0),
				Point::new(8.0, 8.0),
				Point::new(2.0, 8.0),
				Point::new(2.0, 2.0),
			]],
			srid: 4326,
		};
		assert_eq!(polygon.interiors.len(), 1);
		assert_eq!(polygon.interiors[0].len(), 5);
	}

	#[test]
	fn test_multipoint_creation() {
		let mp = MultiPoint {
			points: vec![
				Point::new(0.0, 0.0),
				Point::new(1.0, 1.0),
				Point::new(2.0, 2.0),
			],
			srid: 4326,
		};
		assert_eq!(mp.points.len(), 3);
	}

	#[test]
	fn test_bounding_box_contains_point() {
		let bbox = BoundingBox {
			min_x: 0.0,
			min_y: 0.0,
			max_x: 10.0,
			max_y: 10.0,
		};

		assert!(bbox.contains_point(&Point::new(5.0, 5.0)));
		assert!(bbox.contains_point(&Point::new(0.0, 0.0)));
		assert!(bbox.contains_point(&Point::new(10.0, 10.0)));
		assert!(!bbox.contains_point(&Point::new(-1.0, 5.0)));
		assert!(!bbox.contains_point(&Point::new(11.0, 5.0)));
		assert!(!bbox.contains_point(&Point::new(5.0, 11.0)));
	}

	#[test]
	fn test_bounding_box_intersects() {
		let bbox1 = BoundingBox {
			min_x: 0.0,
			min_y: 0.0,
			max_x: 10.0,
			max_y: 10.0,
		};

		let bbox2 = BoundingBox {
			min_x: 5.0,
			min_y: 5.0,
			max_x: 15.0,
			max_y: 15.0,
		};

		assert!(bbox1.intersects(&bbox2));

		let bbox3 = BoundingBox {
			min_x: 20.0,
			min_y: 20.0,
			max_x: 30.0,
			max_y: 30.0,
		};

		assert!(!bbox1.intersects(&bbox3));
	}

	#[test]
	fn test_distance_meters() {
		let d = Distance::m(500.0);
		assert_eq!(d.to_meters(), 500.0);
	}

	#[test]
	fn test_distance_feet() {
		let d = Distance::ft(100.0);
		assert!((d.to_meters() - 30.48).abs() < 0.01);
	}

	#[test]
	fn test_coordinate_transform_creation() {
		let transform = CoordinateTransform {
			from_srid: 4326,
			to_srid: 3857,
		};
		assert_eq!(transform.from_srid, 4326);
		assert_eq!(transform.to_srid, 3857);
	}

	#[test]
	fn test_spatial_lookup_contains() {
		let point = Point::new(5.0, 5.0);
		let lookup = SpatialLookup::Contains(point);
		matches!(lookup, SpatialLookup::Contains(_));
	}

	#[test]
	fn test_spatial_lookup_within() {
		let polygon = Polygon {
			exterior: vec![
				Point::new(0.0, 0.0),
				Point::new(10.0, 0.0),
				Point::new(10.0, 10.0),
				Point::new(0.0, 10.0),
				Point::new(0.0, 0.0),
			],
			interiors: vec![],
			srid: 4326,
		};
		let lookup = SpatialLookup::Within(polygon);
		matches!(lookup, SpatialLookup::Within(_));
	}

	#[test]
	fn test_spatial_lookup_intersects() {
		let polygon = Polygon {
			exterior: vec![
				Point::new(0.0, 0.0),
				Point::new(10.0, 0.0),
				Point::new(10.0, 10.0),
				Point::new(0.0, 10.0),
				Point::new(0.0, 0.0),
			],
			interiors: vec![],
			srid: 4326,
		};
		let lookup = SpatialLookup::Intersects(polygon);
		matches!(lookup, SpatialLookup::Intersects(_));
	}

	#[test]
	fn test_spatial_lookup_dwithin() {
		let point = Point::new(0.0, 0.0);
		let lookup = SpatialLookup::DWithin(point, 1000.0);
		matches!(lookup, SpatialLookup::DWithin(_, _));
	}

	#[test]
	fn test_gist_index_with_different_column() {
		let index = GiSTIndex::new("geometry");
		let sql = index.create_sql("shapes", "shapes_geom_idx");
		assert!(sql.contains("geometry"));
		assert!(sql.contains("shapes"));
		assert!(sql.contains("shapes_geom_idx"));
	}

	#[test]
	fn test_multilinestring_total_length() {
		let mls = MultiLineString {
			lines: vec![
				LineString {
					points: vec![Point::new(0.0, 0.0), Point::new(5.0, 0.0)],
					srid: 4326,
				},
				LineString {
					points: vec![Point::new(0.0, 0.0), Point::new(0.0, 3.0)],
					srid: 4326,
				},
			],
			srid: 4326,
		};

		let total_length: f64 = mls.lines.iter().map(|l| l.length()).sum();
		assert_eq!(total_length, 8.0); // 5 + 3
	}

	#[test]
	fn test_multipolygon_total_area() {
		let mp = MultiPolygon {
			polygons: vec![
				Polygon {
					exterior: vec![
						Point::new(0.0, 0.0),
						Point::new(5.0, 0.0),
						Point::new(5.0, 5.0),
						Point::new(0.0, 5.0),
						Point::new(0.0, 0.0),
					],
					interiors: vec![],
					srid: 4326,
				},
				Polygon {
					exterior: vec![
						Point::new(10.0, 10.0),
						Point::new(13.0, 10.0),
						Point::new(13.0, 14.0),
						Point::new(10.0, 14.0),
						Point::new(10.0, 10.0),
					],
					interiors: vec![],
					srid: 4326,
				},
			],
			srid: 4326,
		};

		let total_area = mp.polygons.iter().map(|p| p.area()).sum::<f64>();
		assert_eq!(total_area, 37.0); // 25 + 12
	}

	#[test]
	fn test_point_distance_negative_coords() {
		// Use Cartesian coordinates (not WGS84) for simple Euclidean distance test
		let p1 = Point::with_srid(-3.0, -4.0, 0);
		let p2 = Point::with_srid(0.0, 0.0, 0);
		assert_eq!(p1.distance_to(&p2), 5.0);
	}

	#[test]
	fn test_point_distance_same_point() {
		let p = Point::new(5.0, 5.0);
		assert_eq!(p.distance_to(&p), 0.0);
	}

	#[test]
	fn test_linestring_single_point() {
		let line = LineString {
			points: vec![Point::new(0.0, 0.0)],
			srid: 4326,
		};
		assert_eq!(line.length(), 0.0);
	}

	#[test]
	fn test_complex_polygon_with_multiple_holes() {
		let polygon = Polygon {
			exterior: vec![
				Point::new(0.0, 0.0),
				Point::new(20.0, 0.0),
				Point::new(20.0, 20.0),
				Point::new(0.0, 20.0),
				Point::new(0.0, 0.0),
			],
			interiors: vec![
				vec![
					Point::new(2.0, 2.0),
					Point::new(5.0, 2.0),
					Point::new(5.0, 5.0),
					Point::new(2.0, 5.0),
					Point::new(2.0, 2.0),
				],
				vec![
					Point::new(10.0, 10.0),
					Point::new(15.0, 10.0),
					Point::new(15.0, 15.0),
					Point::new(10.0, 15.0),
					Point::new(10.0, 10.0),
				],
			],
			srid: 4326,
		};

		assert_eq!(polygon.interiors.len(), 2);
		// Area = 20*20 - 3*3 - 5*5 = 400 - 9 - 25 = 366
		assert_eq!(polygon.area(), 366.0);
	}

	#[test]
	fn test_spatial_reference_systems() {
		let wgs84_point = Point::new(139.6917, 35.6895); // Tokyo in WGS84
		assert_eq!(wgs84_point.srid, 4326);

		let web_mercator_point = Point::with_srid(15540445.0, 4253018.0, 3857);
		assert_eq!(web_mercator_point.srid, 3857);
	}

	#[test]
	fn test_distance_unit_conversions() {
		let km = Distance::km(1.0);
		let m = Distance::m(1000.0);

		assert_eq!(km.to_meters(), 1000.0);
		assert_eq!(m.to_meters(), 1000.0);
	}

	#[test]
	fn test_wgs84_to_web_mercator() {
		// Test transformation from WGS84 to Web Mercator
		let wgs84_point = Point::with_srid(0.0, 0.0, 4326); // Equator at prime meridian
		let transform = CoordinateTransform::new(4326, 3857);
		let web_mercator = transform.transform_point(&wgs84_point);

		assert_eq!(web_mercator.srid, 3857);
		assert!((web_mercator.x - 0.0).abs() < 0.01);
		assert!((web_mercator.y - 0.0).abs() < 0.01);
	}

	#[test]
	fn test_web_mercator_to_wgs84() {
		// Test transformation from Web Mercator to WGS84
		let web_mercator = Point::with_srid(0.0, 0.0, 3857);
		let transform = CoordinateTransform::new(3857, 4326);
		let wgs84 = transform.transform_point(&web_mercator);

		assert_eq!(wgs84.srid, 4326);
		assert!((wgs84.x - 0.0).abs() < 0.01);
		assert!((wgs84.y - 0.0).abs() < 0.01);
	}

	#[test]
	fn test_coordinate_transform_tokyo() {
		// Test Tokyo coordinates (139.6917°E, 35.6895°N)
		let tokyo_wgs84 = Point::with_srid(139.6917, 35.6895, 4326);
		let transform = CoordinateTransform::new(4326, 3857);
		let tokyo_web_mercator = transform.transform_point(&tokyo_wgs84);

		assert_eq!(tokyo_web_mercator.srid, 3857);
		// Web Mercator values for Tokyo: x ≈ 15550409, y ≈ 4257981
		assert!(tokyo_web_mercator.x > 15500000.0 && tokyo_web_mercator.x < 15600000.0);
		assert!(tokyo_web_mercator.y > 4200000.0 && tokyo_web_mercator.y < 4300000.0);

		// Test reverse transformation
		let transform_back = CoordinateTransform::new(3857, 4326);
		let tokyo_back = transform_back.transform_point(&tokyo_web_mercator);
		assert!((tokyo_back.x - 139.6917).abs() < 0.01);
		assert!((tokyo_back.y - 35.6895).abs() < 0.01);
	}

	#[test]
	fn test_polygon_with_holes_area() {
		// 100x100 square with 20x20 hole
		let polygon = Polygon {
			exterior: vec![
				Point::new(0.0, 0.0),
				Point::new(100.0, 0.0),
				Point::new(100.0, 100.0),
				Point::new(0.0, 100.0),
				Point::new(0.0, 0.0),
			],
			interiors: vec![vec![
				Point::new(40.0, 40.0),
				Point::new(60.0, 40.0),
				Point::new(60.0, 60.0),
				Point::new(40.0, 60.0),
				Point::new(40.0, 40.0),
			]],
			srid: 4326,
		};

		// Area should be 100*100 - 20*20 = 10000 - 400 = 9600
		assert_eq!(polygon.area(), 9600.0);
		assert_eq!(polygon.exterior_area(), 10000.0);
	}

	#[test]
	fn test_polygon_perimeter() {
		// 3x4 rectangle
		let polygon = Polygon {
			exterior: vec![
				Point::new(0.0, 0.0),
				Point::new(3.0, 0.0),
				Point::new(3.0, 4.0),
				Point::new(0.0, 4.0),
				Point::new(0.0, 0.0),
			],
			interiors: vec![],
			srid: 4326,
		};

		// Perimeter = 2*(3+4) = 14
		assert_eq!(polygon.perimeter(), 14.0);
	}

	#[test]
	fn test_point_in_polygon() {
		let polygon = Polygon {
			exterior: vec![
				Point::new(0.0, 0.0),
				Point::new(10.0, 0.0),
				Point::new(10.0, 10.0),
				Point::new(0.0, 10.0),
				Point::new(0.0, 0.0),
			],
			interiors: vec![],
			srid: 4326,
		};

		assert!(polygon.contains_point(&Point::new(5.0, 5.0)));
		assert!(!polygon.contains_point(&Point::new(15.0, 5.0)));
		assert!(!polygon.contains_point(&Point::new(-1.0, 5.0)));
	}

	#[test]
	fn test_linestring_geodesic_length() {
		// Simple line for WGS84
		let line = LineString {
			points: vec![Point::new(0.0, 0.0), Point::new(0.0, 1.0)],
			srid: 4326,
		};

		let planar = line.length();
		let geodesic = line.geodesic_length();

		// Geodesic should be significantly different from planar for lat/lon
		assert!(geodesic > 100000.0); // ~111km for 1 degree at equator
		assert!(planar < geodesic); // planar is just 1.0
	}
}
