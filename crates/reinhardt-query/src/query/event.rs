//! Event DDL statement builders (MySQL-specific)
//!
//! This module provides builders for event-related DDL statements:
//!
//! - [`CreateEventStatement`]: CREATE EVENT statement
//! - [`AlterEventStatement`]: ALTER EVENT statement
//! - [`DropEventStatement`]: DROP EVENT statement
//!
//! # Examples
//!
//! ```rust,ignore
//! use reinhardt_query::prelude::*;
//!
//! // CREATE EVENT my_event
//! // ON SCHEDULE AT '2026-12-31 23:59:59'
//! // DO INSERT INTO logs VALUES (NOW())
//! let stmt = Query::create_event()
//!     .name("my_event")
//!     .on_schedule_at("2026-12-31 23:59:59")
//!     .do_body("INSERT INTO logs VALUES (NOW())");
//!
//! // CREATE EVENT recurring_event
//! // ON SCHEDULE EVERY 1 DAY
//! // ON COMPLETION PRESERVE
//! // DO DELETE FROM temp_data WHERE created_at < NOW() - INTERVAL 7 DAY
//! let stmt = Query::create_event()
//!     .name("recurring_event")
//!     .on_schedule_every("1 DAY")
//!     .on_completion_preserve()
//!     .do_body("DELETE FROM temp_data WHERE created_at < NOW() - INTERVAL 7 DAY");
//! ```

pub mod alter_event;
pub mod create_event;
pub mod drop_event;

pub use alter_event::AlterEventStatement;
pub use create_event::CreateEventStatement;
pub use drop_event::DropEventStatement;
