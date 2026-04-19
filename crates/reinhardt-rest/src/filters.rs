//! Type-safe filtering backends for Reinhardt framework
//!
//! Provides compile-time type-safe filtering using reinhardt-orm's Field<M, T> system.

/// Core filter trait and error types.
pub mod filter;

/// Supported database dialects for query generation
///
/// Different databases use different identifier quoting styles:
/// - MySQL uses backticks: \`column\`
/// - PostgreSQL uses double quotes: "column"
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DatabaseDialect {
	/// MySQL dialect (uses backticks for identifier quoting)
	#[default]
	MySQL,
	/// PostgreSQL dialect (uses double quotes for identifier quoting)
	PostgreSQL,
}

// Custom filter backends
pub mod backend;

#[cfg(feature = "caching")]
pub mod caching;

pub mod highlighting;

// Type-safe filtering system (DB-dependent modules gated on serializers feature)
#[cfg(feature = "serializers")]
pub mod field_extensions;
pub mod fulltext;
pub mod fuzzy;
pub mod geo;
#[cfg(feature = "serializers")]
pub mod multi_term;
#[cfg(feature = "serializers")]
pub mod ordering_field;
#[cfg(feature = "serializers")]
pub mod query_filter;
pub mod range;
#[cfg(feature = "serializers")]
pub mod searchable;

// Advanced filtering features
#[cfg(feature = "serializers")]
pub mod index_hint;
#[cfg(feature = "serializers")]
pub mod optimizer;
pub mod relevance;
pub mod synonym;

// Core exports
pub use filter::{FilterBackend, FilterError, FilterResult};

// Custom filter backend exports
pub use backend::{CustomFilterBackend, SimpleOrderingBackend, SimpleSearchBackend};

#[cfg(feature = "caching")]
pub use caching::{CacheStats, CachedFilterBackend, generate_cache_key};

pub use highlighting::{
	HighlightedResult, HtmlHighlighter, MultiFieldHighlighter, PlainTextHighlighter,
	SearchHighlighter,
};

// Type-safe exports
#[cfg(feature = "serializers")]
pub use field_extensions::FieldOrderingExt;
pub use fulltext::{FullTextSearchFilter, FullTextSearchMode};
pub use fuzzy::{FuzzyAlgorithm, FuzzySearchFilter};
pub use geo::{BoundingBoxFilter, DistanceFilter, DistanceUnit, NearbyFilter, PolygonFilter};
#[cfg(feature = "serializers")]
pub use multi_term::{MultiTermSearch, Operator, SearchTerm, TermType};
#[cfg(feature = "serializers")]
pub use ordering_field::{OrderDirection, OrderingField};
#[cfg(feature = "serializers")]
pub use query_filter::QueryFilter;
pub use range::{DateRangeFilter, NumericRangeFilter, RangeFilter};
#[cfg(feature = "serializers")]
pub use searchable::SearchableModel;

// Advanced filtering exports
#[cfg(feature = "serializers")]
pub use index_hint::{IndexHint, IndexHintFilter, IndexStrategy};
#[cfg(feature = "serializers")]
pub use optimizer::{
	DatabaseType, OptimizationHint, QueryAnalysis, QueryComplexity, QueryOptimizer, QueryPlan,
};
pub use relevance::{FieldBoost, RelevanceScorer, ScoredResult, ScoringAlgorithm};
pub use synonym::{SynonymDictionary, SynonymExpander};
