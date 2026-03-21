/// A single metric data point (e.g., counter, gauge, or histogram value).
pub struct Metric;
/// Collects and aggregates metrics from static file serving operations.
pub struct MetricsCollector;
/// Aggregated metrics for a single request (latency, bytes served, cache status).
pub struct RequestMetrics;
/// A timer that measures the duration of a static file request.
pub struct RequestTimer;
