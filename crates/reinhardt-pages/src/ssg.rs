//! Static Site Generation (SSG) Infrastructure
//!
//! This module provides SSG capabilities for reinhardt-pages, enabling
//! pre-rendering of routes to static HTML files at build time.
//!
//! ## Features
//!
//! - **Route Enumeration**: Collect renderable routes from the application
//! - **Static Rendering**: Use `SsrRenderer` to generate HTML for each route
//! - **Directory Mirroring**: Output structure mirrors URL paths
//! - **Sitemap Generation**: Automatic `sitemap.xml` creation
//!
//! ## Usage
//!
//! ```ignore
//! use reinhardt_pages::ssg::{SsgBuilder, SsgRoute};
//! use reinhardt_pages::ssr::SsrOptions;
//!
//! let routes = vec![
//!     SsgRoute::new("/", render_home),
//!     SsgRoute::new("/about/", render_about),
//!     SsgRoute::new("/blog/", render_blog),
//! ];
//!
//! let output = SsgBuilder::new("/tmp/dist")
//!     .with_base_url("https://example.com")
//!     .with_routes(routes)
//!     .build()?;
//!
//! println!("Generated {} files", output.files_written);
//! ```

mod builder;
mod output;
mod route;
mod sitemap;

pub use builder::SsgBuilder;
pub use output::SsgOutput;
pub use route::SsgRoute;
pub use sitemap::SitemapGenerator;
