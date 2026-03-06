//! # reinhardt-pages-components
//!
//! UI component library for Reinhardt web framework with declarative `page!` and `form!` macros.
//!
//! ## Features
//!
//! - **Declarative UI**: Build UIs using intuitive macro syntax
//! - **Type-Safe**: Compile-time validation with Rust's type system
//! - **Responsive**: Mobile-first design with 6 breakpoints
//! - **Accessible**: WAI-ARIA compliant components
//! - **Customizable**: Flexible theme system with CSS variables
//!
//! ## Quick Start
//!
//! ```rust
//! use reinhardt_pages_components::theme::Theme;
//!
//! // Create default theme
//! let theme = Theme::default();
//! let css = theme.to_css_variables();
//! ```
//!
//! ## Component Categories
//!
//! - **Layout**: Container, Grid, Layout, Sidebar, Spacer
//! - **Navigation**: Nav, Breadcrumb, Tabs, Pagination
//! - **Feedback**: Alert, Toast, Modal, Tooltip, Progress
//! - **Data Display**: Badge, Card, Avatar, Stat
//! - **Input**: Button, Dropdown, Accordion
//! - **Form**: LoginForm, RegisterForm, SearchForm, ContactForm, PasswordResetForm, SettingsForm

#![deny(missing_docs)]
#![warn(clippy::all)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// Re-export macros
pub use reinhardt_pages_components_macros::{form, page};

/// Core component trait and types
pub mod component;

/// Error types
pub mod error;

/// Theme system
pub mod theme;

/// Responsive utilities
pub mod responsive;

/// Accessibility attributes
pub mod accessibility;

/// Layout components
pub mod layout;

/// Navigation components
pub mod navigation;

/// Feedback components
pub mod feedback;

/// Data display components
pub mod data_display;

/// Input components
pub mod input;

/// Form components
pub mod form;

// Re-export common types
pub use component::*;
pub use error::*;
