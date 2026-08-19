//! View and shortcut re-exports.

pub use reinhardt_views::{
	Context, DetailView, ListView, MultipleObjectMixin, SingleObjectMixin, View,
};

pub use reinhardt_views::viewsets::{
	Action, ActionType, CreateMixin, DestroyMixin, GenericViewSet, ListMixin, ModelViewSet,
	QuerySetProvider, ReadOnlyModelViewSet, RetrieveMixin, UpdateMixin, ViewSet,
};

#[cfg(feature = "shortcuts")]
pub use reinhardt_shortcuts::{redirect, render_html, render_json, render_text};

#[cfg(all(feature = "shortcuts", feature = "database"))]
pub use reinhardt_shortcuts::{get_list_or_404, get_object_or_404};
