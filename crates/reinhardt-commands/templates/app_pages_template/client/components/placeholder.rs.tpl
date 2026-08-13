//! Placeholder route-backed component for the {{ app_name }} application.
//!
//! Replace this module with real components when the app gets its first page.

use reinhardt::pages::component::Page;
use reinhardt::pages::page;

{% if is_workspace != "true" %}#[cfg(client)]
use crate::client::components::nav::with_nav;
{% endif %}

#[reinhardt::pages::component("/{{ app_name }}/", name = "placeholder")]
pub fn placeholder() -> Page {
    {% if is_workspace == "true" %}page!(|| {
        div {
            class: "placeholder",
            "{{ app_name }} placeholder component"
        }
    })(){% else %}with_nav(page!(|| {
        div {
            class: "placeholder",
            "{{ app_name }} placeholder component"
        }
    })()){% endif %}
}
