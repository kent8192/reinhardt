//! Utility functions for message handling

pub mod bisect;
pub mod filter;

pub use bisect::{bisect_keep_left, bisect_keep_right};
#[allow(deprecated)] // re-exporting deprecated items for backward compatibility
pub use filter::{
	filter_by_level, filter_by_level_range, filter_by_max_level, filter_by_min_level,
	filter_by_tag, filter_messages,
};
