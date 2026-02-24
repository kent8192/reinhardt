//! Type-safe filtering backends for Reinhardt framework
//!
//! Provides compile-time type-safe filtering using reinhardt-orm's Field<M, T> system.

// Core filter trait
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

// Type-safe filtering system
pub mod field_extensions;
pub mod fulltext;
pub mod fuzzy;
pub mod geo;
pub mod multi_term;
pub mod ordering_field;
pub mod query_filter;
pub mod range;
pub mod searchable;

// Advanced filtering features
pub mod index_hint;
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
pub use field_extensions::FieldOrderingExt;
pub use fulltext::{FullTextSearchFilter, FullTextSearchMode};
pub use fuzzy::{FuzzyAlgorithm, FuzzySearchFilter};
pub use geo::{BoundingBoxFilter, DistanceFilter, DistanceUnit, NearbyFilter, PolygonFilter};
pub use multi_term::{MultiTermSearch, Operator, SearchTerm, TermType};
pub use ordering_field::{OrderDirection, OrderingField};
pub use query_filter::QueryFilter;
pub use range::{DateRangeFilter, NumericRangeFilter, RangeFilter};
pub use searchable::SearchableModel;

// Advanced filtering exports
pub use index_hint::{IndexHint, IndexHintFilter, IndexStrategy};
pub use optimizer::{
	DatabaseType, OptimizationHint, QueryAnalysis, QueryComplexity, QueryOptimizer, QueryPlan,
};
pub use relevance::{FieldBoost, RelevanceScorer, ScoredResult, ScoringAlgorithm};
pub use synonym::{SynonymDictionary, SynonymExpander};
