//! Unified prelude for reinhardt-pages.
//!
//! This module provides all commonly used types with a single import statement,
//! eliminating the need for conditional compilation in most user code.
//!
//! # Example
//!
//! ```rust,ignore
//! // Instead of multiple imports with cfg guards:
//! // #[cfg(wasm)]
//! // use reinhardt_pages::{Signal, Page, use_state, ...};
//! // #[cfg(native)]
//! // use reinhardt::pages::{Signal, Page, use_state, ...};
//!
//! // Use the unified prelude:
//! use reinhardt_pages::prelude::*;
//! // or
//! use reinhardt::pages::prelude::*;
//!
//! fn my_component() -> Page {
//!     let (count, set_count) = use_state(0);
//!     // Component logic...
//!     Page::empty()
//! }
//! ```
//!
//! # Included Types
//!
//! ## Reactive System
//! - [`Signal`], [`Effect`], [`Memo`], [`Resource`], [`ResourceState`]
//! - [`LatestResourceValue`], [`LatestResourceState`],
//!   [`use_latest_resource_value`]
//! - Context: [`Context`], [`ContextGuard`], [`create_context`], [`get_context`],
//!   [`provide_context`], [`remove_context`]
//!
//! ## Hooks
//! - [`use_state`], [`use_effect`], [`use_memo`], [`use_callback`], [`use_context`]
//! - [`use_ref`], [`use_reducer`], [`use_transition`], [`use_deferred_value`]
//! - [`use_id`], [`use_layout_effect`], [`use_debug_value`]
//! - [`use_optimistic`], [`use_shared_state`]
//! - [`use_action`], [`use_action_state`], [`use_sync_external_store`]
//! - [`use_query`], [`queries`], [`use_action`], [`use_head`], [`use_page_title`]
//! - Query cache: [`QueryClient`], [`QueryFamily`], [`QueryKey`],
//!   [`QueryDescriptor`], [`QueryOptions`], [`QuerySnapshot`], [`QueryStatus`]
//!
//! ## Normalized entity cache
//! - [`Entity`], [`EntityArena`], [`EntityHandle`]
//! - [`EntityValue`], [`OptionalEntity`], [`EntityVec`]
//! - [`EntityProjection`], [`EntityDependencies`], [`EntityReader`],
//!   [`EntityWriter`]
//! - [`QueryClient::entity`], [`QueryClient::upsert_entity`],
//!   [`QueryClient::remove_entity`], and [`QueryClient::update_entities`]
//!
//! Normalization is opt-in per [`QueryDescriptor`] with
//! [`QueryDescriptor::with_entities`]. Plain Query Client V2 descriptors and
//! [`QueryHandle<T, E>`] remain source-compatible, and handles continue to
//! expose the original `T` snapshots. Use [`EntityValue`] for a required
//! entity, [`OptionalEntity`] for an optional entity, or [`EntityVec`] for an
//! ordered vector. Implement [`EntityProjection`] for a zero-sized custom
//! projection when a result combines entities or needs an explicit versioned
//! recipe schema.
//!
//! ```rust,no_run
//! use reinhardt_pages::prelude::*;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct Project { id: u64, name: String }
//! impl Entity for Project {
//!     type Id = u64;
//!     const TYPE: &'static str = "example.project";
//!     fn entity_id(&self) -> Self::Id { self.id }
//! }
//!
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct LoadError;
//! let family = QueryFamily::<u64, Project, LoadError>::new("projects.detail.v1");
//! let descriptor = family
//!     .query(7, || async {
//!         Ok::<_, LoadError>(Project { id: 7, name: String::from("Pages") })
//!     })
//!     .with_entities(EntityValue::<Project>::new());
//! let _ = descriptor;
//! ```
//!
//! ## Component System
//! - [`Component`], [`PageElement`], [`IntoPage`], [`Page`], [`Props`]
//! - [`PageEventHandler`]
//! - [`SuspenseBoundary`], [`ErrorBoundary`], [`ActivityBoundary`],
//!   [`ViewTransitionBoundary`], [`BoundaryError`]
//!
//! ## Headless UI Primitives
//! - [`ActionButton`], [`ActionResultPanel`], [`ResourcePanel`]
//!
//! ## Events and Callbacks
//! - [`Callback`], [`IntoEventHandler`], [`into_event_handler`]
//! - [`Event`] (platform-agnostic event type)
//! - Custom event detail: [`CustomEvent`], [`CustomEventDetailError`]
//! - Controlled form support: [`ControlBindingError`], [`NumberParseError`],
//!   [`NumberParseErrorKind`], [`NumberValue`]
//!
//! ## DOM
//! - [`Document`], [`Element`], [`CustomEventOptions`], [`EventHandle`],
//!   [`EventType`], [`document`](fn@document)
//!
//! ## Routing
//! - [`Link`], [`PrefetchMode`], [`Loader`], `RouterOutlet`, `RouteLoaderError`
//!
//! ## Macros
//! - [`page`] - Component DSL for defining views
//! - [`head`] - HTML head section DSL
//! - `#[component]` / `#[layout]` - Route-backed SPA component declarations
//! - `#[client_form]` - DTO-derived client form declarations (import explicitly from the crate root)
//!
//! ## Static Files
//! - [`resolve_static`] - Resolve static file URLs
//! - [`init_static_resolver`] - Initialize the static resolver
//! - [`is_initialized`] - Check if static resolver is initialized
//!
//! ## Typed Forms
//! - [`use_form`], [`use_form_action`], [`UseFormReturn`], [`UseFormBuilder`], [`FormAction`]
//!
//! ## Task Spawning
//! - [`spawn_task`], [`defer_yield`] - cross-target async task spawning
//!   (no-op on native; replaces the deprecated `spawn_local` re-export)
//!
//! ## I18n
//! - `I18nContext`, `I18nError`, `I18nStateError`, `LazyString`, `MessageCatalog`,
//!   `TranslatedText`, `TranslationContext`, `TranslationGuard`, `tr`, `tn`, `tp`, `tnp`
//! - `provide_i18n_context`, `use_i18n_context`, `with_i18n_context`
//! - `set_locale`, `locale`
//! - `t!` for inline page translations with named interpolation
//! - Requires the `i18n` feature.

// ============================================================================
// Reactive System
// ============================================================================

pub use crate::reactive::{
	Effect, Entity, EntityArena, EntityDependencies, EntityHandle, EntityProjection, EntityReader,
	EntityValue, EntityVec, EntityWriter, LatestResourceState, LatestResourceValue,
	LatestResourceValueBuilder, Memo, NoRetry, OptionalEntity, ProjectionMaterialization,
	ProjectionRemoval, QueryClient, QueryDefaults, QueryDescriptor, QueryFamily, QueryHandle,
	QueryKey, QueryOptions, QuerySnapshot, QueryStatus, RemovedEntities, Resource, ResourceState,
	RetryPolicy, Signal, use_latest_resource_value,
};

// Context system
pub use crate::reactive::{
	Context, ContextGuard, create_context, get_context, provide_context, remove_context,
};

// Hooks API
pub use crate::reactive::{Action, ActionPhase, ActionStateBuilder, use_action, use_action_state};
pub use crate::reactive::{
	Dispatch, EffectReturn, OptimisticState, Ref, SetState, SetStateExt, SharedSetState,
	SharedSignal, TransitionState, use_callback, use_context, use_debug_value, use_deferred_value,
	use_effect, use_head, use_id, use_layout_effect, use_memo, use_optimistic, use_page_title,
	use_reducer, use_ref, use_retained_effect, use_retained_layout_effect, use_shared_state,
	use_state, use_sync_external_store, use_transition,
};
pub use crate::reactive::{queries, use_query};

// Unified resource hooks (available on all targets)
pub use crate::reactive::{use_resource, use_resource_with_key};

// ============================================================================
// Component System
// ============================================================================

pub use crate::component::{
	ActivityBoundary, ActivityMode, BoundaryError, Component, ErrorBoundary, ErrorTracker, Head,
	IntoPage, LinkTag, MetaTag, Outlet, Page, PageElement, PageEventHandler, PageExt, Props,
	ResourceTracker, ScriptTag, StyleTag, SuspenseBoundary, ViewTransitionBoundary,
	ViewTransitionHandle, ViewTransitionStatus, start_view_transition,
};

// ============================================================================
// Headless UI primitives
// ============================================================================

pub use crate::ui::{
	ActionButton, ActionResultPanel, FormActionButton, FormActionResultPanel, ResourcePanel,
};

// ============================================================================
// Events and Callbacks
// ============================================================================

pub use crate::callback::{
	Callback, IntoEventHandler, IntoTypedEventHandler, into_event_handler, raw_async_event_handler,
	raw_event_handler, typed_async_custom_event_handler, typed_async_event_handler,
	typed_custom_event_handler, typed_event_handler,
};

pub use crate::event::{
	CustomEvent, CustomEventDetailError, EventConversionError, EventFile, EventPayload,
	EventTarget, EventTargetError, Modifiers, MouseButton, MouseButtons, Point, PointerKind,
};

pub use crate::control_binding::{
	ControlBindingError, NumberParseError, NumberParseErrorKind, NumberValue,
};

// Platform-agnostic Event type
pub use crate::cancellation::{CancellationHandle, CancellationToken, Cancelled};
pub use crate::platform::Event;

// Platform-agnostic task spawning (cross-target)
pub use crate::platform::{defer_yield, spawn_task};
pub use crate::portal::{Portal, PortalError, PortalHandle, PortalTarget, mount_portal};

// ============================================================================
// DOM
// ============================================================================

pub use crate::dom::{CustomEventOptions, Document, Element, EventHandle, EventType, document};

// ============================================================================
// Routing
// ============================================================================

// Non-deprecated rendering primitives.
pub use crate::route_params;
pub use crate::router::Link;
pub use crate::router::PrefetchMode;
pub use crate::router::RouteLoaderId;
pub use crate::router::loader::{
	Loader, LoaderInputError, LoaderInputKind, LoaderInputSpec, LoaderStore, LoaderStoreError,
	RouteLoader, RouteLoaderError, canonical_loader_inputs, loader_cache_id,
};
pub use crate::router::{NavigationType, navigate, navigate_named, navigate_or_reload};
pub use crate::{NavigateError, RouterHandle, use_router};

// ============================================================================
// API and Server Functions
// ============================================================================

pub use crate::api::{ApiModel, ApiQuerySet, Filter, FilterOp};
pub use crate::server_fn::{
	ServerFn, ServerFnError, ServerFnErrorKind, ServerFnErrorPayload, ServerFnFieldError,
};
pub use crate::server_mutation::{
	FormServerMutation, FormServerMutationBuilder, MutationDispatchOutcome, ServerMutation,
	ServerMutationBuilder, use_server_mutation,
};

// ============================================================================
// Authentication and Security
// ============================================================================

pub use crate::auth::{AuthData, AuthError, AuthState, auth_state};
pub use crate::csrf::{CsrfManager, get_csrf_token};

// ============================================================================
// SSR and Hydration
// ============================================================================

pub use crate::hydration::{
	HydrationContext, HydrationError, hydrate, init_hydration_state, is_hydration_complete,
	on_hydration_complete,
};

#[cfg(wasm)]
pub use crate::hydration::mark_hydration_complete;
pub use crate::ssr::SsrState;
#[cfg(native)]
pub use crate::ssr::{SsrChunk, SsrOptions, SsrRenderer, SsrRouteOutput, SsrStream};

// ============================================================================
// I18n
// ============================================================================

#[cfg(feature = "i18n")]
pub use crate::i18n::{
	I18nContext, I18nError, I18nStateError, LazyString, MessageCatalog, TranslatedText,
	TranslationContext, TranslationGuard, locale, provide_i18n_context, set_locale, tn, tnp, tp,
	tr, use_i18n_context, with_i18n_context,
};

// ============================================================================
// Static File URL Resolver
// ============================================================================

pub use crate::static_resolver::{
	component_stylesheet_url, init_static_resolver, is_initialized, resolve_static,
};
pub use crate::style::{
	ClassList, ClassToken, CssAngle, CssColor, CssInteger, CssLength, CssLengthPercentage,
	CssNumber, CssPercentage, CssTime, CssValueError, StyleVars,
};

// ============================================================================
// Forms (native only)
// ============================================================================

pub use crate::client_form::{ClientFormChoice, ClientFormChoiceSource};
pub use crate::form_state::{
	CollectionItem, CollectionItemKey, CollectionState, CustomWidgetContext, CustomWidgetRawValue,
	FieldError, FieldPathState, FieldState, FocusError, FormAction, FormCollectionRuntimeSource,
	FormEvent, FormRuntimeSource, FormState, FormSubscription, FormValidationError,
	FormWidgetAdapter, FormWidgetError, FormWidgetValueKind, NoDeps, ResetOnDeps, RevalidateOn,
	UseFormAsyncSubmitOutcome, UseFormBuilder, UseFormReturn, UseFormSubmitOutcome, use_form,
	use_form_action,
};

#[cfg(native)]
pub use crate::form::{FormBinding, FormComponent};

#[cfg(native)]
pub use reinhardt_forms::{
	Widget,
	wasm_compat::{FieldMetadata, FormMetadata},
};

// ============================================================================
// Macros
// ============================================================================

pub use crate::ClientForm;
pub use crate::ClientFormChoices;
pub use crate::client_page;
pub use crate::component;
pub use crate::form;
pub use crate::head;
pub use crate::layout;
pub use crate::page;
pub use crate::page_props;
pub use crate::style;
pub use crate::style_def;
#[cfg(feature = "i18n")]
pub use crate::t;
pub use crate::wasm_server_api;

// ============================================================================
// WASM-specific utilities
// ============================================================================

/// Spawn a local async task (WASM only).
///
/// This is a convenience re-export from `wasm_bindgen_futures`.
///
/// Deprecated in favor of [`spawn_task`], which is cross-target (a no-op on
/// native) and does not require a `#[cfg(wasm)]` guard at the call site.
#[cfg(wasm)]
#[deprecated(note = "use spawn_task from reinhardt_pages::prelude instead")]
pub use wasm_bindgen_futures::spawn_local;
