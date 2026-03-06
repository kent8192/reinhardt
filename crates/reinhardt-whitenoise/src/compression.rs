//! File compression and content negotiation
//!
//! This module handles:
//! - File scanning and pre-compression
//! - Accept-Encoding header parsing
//! - Compressed variant selection

pub mod compressor;
pub mod negotiation;
pub mod scanner;

pub use compressor::WhiteNoiseCompressor;
pub use negotiation::parse_accept_encoding;
pub use scanner::FileScanner;
