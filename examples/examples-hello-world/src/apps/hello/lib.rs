//! hello application module
//!
//! A simple hello world application

use reinhardt::app_config;

pub mod urls;
pub mod views;

#[app_config(name = "hello", label = "hello")]
pub struct HelloConfig;
