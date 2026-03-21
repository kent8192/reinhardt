//! {{ app_name }} application crate
//!
//! A RESTful API application

use reinhardt::app_config;

pub mod admin;
pub mod models;
pub mod serializers;
pub mod urls;
pub mod views;

#[app_config(name = "{{ app_name }}", label = "{{ app_name }}")]
pub struct {{ camel_case_app_name }}Config;
