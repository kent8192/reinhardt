+++
title = "Part 4: Users and Authentication"
description = "Add the users app, session-backed authentication server functions, and a nav shell that reflects login state."
weight = 40

[extra]
sidebar_weight = 40
+++

# Part 4: Users and Authentication

The poll app can list, display, and vote. Now add accounts: a `users` app, a minimal `User` model, login/register/logout/current-user server functions, session middleware, auth routes, and a shared navigation bar.

This is the first basis tutorial part that uses dependency injection directly. Keep the DI surface small here. The REST tutorial's dependency-injection part goes deeper into providers, scopes, caching, and type keys.

## Create the Users App

Generate the app:

```bash
reinhardt-admin startapp users --template pages
```

`startapp` updates `src/config/apps.rs` for you. Check that `users` was added next to `polls`, but do not hand-edit this file unless you created the app directory manually:

```rust
use reinhardt::installed_apps;

installed_apps! {
    polls: "polls",
    users: "users",
}
```

## Define the User Model

Open `src/apps/users/models.rs`. The example uses a minimal user model, not `full = true`:

```rust
use chrono::{DateTime, Utc};
use reinhardt::prelude::*;
use reinhardt::user;
use serde::{Deserialize, Serialize};

#[user(hasher = reinhardt::Argon2Hasher, username_field = "username", manager = false)]
#[model(app_label = "users", table_name = "users")]
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct User {
    #[field(primary_key = true)]
    pub id: i64,

    #[field(max_length = 150, unique = true)]
    pub username: String,

    #[field(max_length = 255, skip_info = true)]
    pub password_hash: Option<String>,

    #[field(default = true)]
    pub is_active: bool,

    #[field(default = false, skip_info = true)]
    pub is_superuser: bool,

    #[field(include_in_new = false, skip_info = true)]
    pub last_login: Option<DateTime<Utc>>,

    #[field(auto_now_add = true, skip_info = true)]
    pub created_at: DateTime<Utc>,
}
```

`manager = false` opts out of the generated user manager because this tutorial keeps a project-local manager that owns password hashing, uniqueness checks, and persistence.

## Add the Auth User Manager

The manager is server-only. It stores a `DatabaseConnection` and is registered through a keyed injectable provider:

```rust
#[derive(Clone)]
pub struct AuthUserManager {
    db: DatabaseConnection,
}

#[injectable_key]
pub struct AuthUserManagerKey;

#[injectable(scope = "transient")]
async fn auth_user_manager_factory(
    #[inject] db: DatabaseConnection,
) -> FactoryOutput<AuthUserManagerKey, AuthUserManager> {
    FactoryOutput::new(AuthUserManager { db })
}
```

`FactoryOutput<AuthUserManagerKey, AuthUserManager>` registers the manager under an explicit key. Later, server functions ask for `Depends<AuthUserManagerKey, AuthUserManager>` so this provider is selected even if another provider returns the same value type.

The manager implements `BaseUserManager<User>`:

```rust
#[async_trait]
impl BaseUserManager<User> for AuthUserManager {
    async fn create_user(
        &mut self,
        username: &str,
        password: Option<&str>,
        extra: HashMap<String, Value>,
    ) -> Result<User, Error> {
        let new_user = self.build_user(username, password, &extra).await?;
        User::objects()
            .create_with_conn(&self.db, &new_user)
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }
}
```

`build_user` trims usernames, rejects empty or overlong names, checks uniqueness, enforces password length, and calls `set_password`. Keep that logic in the manager so server functions stay focused on request/session flow.

## Share Auth DTOs

`#[model]` generates `UserInfo` from the fields that are not marked `skip_info`; here that means `id`, `username`, and `is_active`. Server functions and WASM tests import it from the model module:

```rust
use crate::apps::users::models::UserInfo;
```

The login/register request DTOs still live in `src/shared/types.rs`:

```rust
#[dto]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    #[validate(length(
        min = 1,
        max = 150,
        message = "Username must be between 1 and 150 characters"
    ))]
    pub username: String,

    #[validate(length(min = 1, message = "Password must not be empty"))]
    pub password: String,
}
```

`RegisterRequest` follows the same pattern and adds `password_confirmation`. The example validates password equality with a helper instead of relying on validator crate field-name matching:

```rust
#[cfg(server)]
impl RegisterRequest {
    pub fn validate_passwords_match(&self) -> Result<(), &'static str> {
        if self.password == self.password_confirmation {
            Ok(())
        } else {
            Err("Passwords do not match")
        }
    }
}
```

## Add Auth Server Functions

Create `src/apps/users/server_fn.rs`. Login validates the request, checks the password, and rotates the session:

```rust
use std::result::Result;

#[server_fn]
pub async fn login(
    username: String,
    password: String,
    #[inject] _db: DatabaseConnection,
    #[inject] session: SessionData,
    #[inject] store: Depends<SessionStoreKey, Arc<SessionStore>>,
) -> Result<UserInfo, ServerFnError> {
    let mut session = session;

    let request = LoginRequest { username, password };
    request
        .validate()
        .map_err(|e| ServerFnError::application(format!("Validation failed: {}", e)))?;

    let manager = User::objects();
    let user = manager
        .filter(User::field_username().eq(request.username.trim().to_string()))
        .first()
        .await
        .map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?
        .ok_or_else(|| ServerFnError::server(401, "Invalid credentials"))?;

    let valid = user
        .check_password(&request.password)
        .map_err(|e| ServerFnError::application(format!("Password check failed: {}", e)))?;

    if !valid {
        return Err(ServerFnError::server(401, "Invalid credentials"));
    }

    if !user.is_active() {
        return Err(ServerFnError::server(403, "User account is inactive"));
    }

    session
        .login(&store, user.id())
        .map_err(|e| ServerFnError::application(format!("Session error: {}", e)))?;

    Ok(UserInfo::from(user))
}
```

Register creates the user through the injected `AuthUserManager` and then logs the new account in:

```rust
#[server_fn]
pub async fn register(
    username: String,
    password: String,
    password_confirmation: String,
    #[inject] user_manager: Depends<AuthUserManagerKey, AuthUserManager>,
    #[inject] session: SessionData,
    #[inject] store: Depends<SessionStoreKey, Arc<SessionStore>>,
) -> Result<UserInfo, ServerFnError> {
    let mut session = session;

    let request = RegisterRequest {
        username,
        password,
        password_confirmation,
    };

    request
        .validate()
        .map_err(|e| ServerFnError::application(format!("Validation failed: {}", e)))?;
    request
        .validate_passwords_match()
        .map_err(ServerFnError::application)?;

    let mut user_manager: AuthUserManager = (*user_manager).clone();
    let saved = user_manager
        .create_user(
            request.username.trim(),
            Some(&request.password),
            HashMap::new(),
        )
        .await
        .map_err(|e| ServerFnError::application(e.to_string()))?;

    session
        .login(&store, saved.id())
        .map_err(|e| ServerFnError::application(format!("Session error: {}", e)))?;

    Ok(UserInfo::from(saved))
}
```

Add logout and current-user lookup:

```rust
use std::result::Result;

#[server_fn]
pub async fn logout(
    #[inject] session: SessionData,
    #[inject] store: Depends<SessionStoreKey, Arc<SessionStore>>,
) -> Result<(), ServerFnError> {
    let mut session = session;

    session.logout(&store);
    Ok(())
}

#[server_fn]
pub async fn current_user(
    #[inject] _db: DatabaseConnection,
    #[inject] session: SessionData,
) -> Result<Option<UserInfo>, ServerFnError> {
    let Some(user_id) = session.get::<i64>(USER_ID_SESSION_KEY) else {
        return Ok(None);
    };

    let user = User::objects()
        .filter(User::field_id().eq(user_id))
        .first()
        .await
        .map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?;

    Ok(user.map(UserInfo::from))
}
```

## Register Auth Routes

The users app follows the same target-gated route surface as polls. `src/apps/users/urls.rs` aggregates the split router modules:

```rust
#[cfg(client)]
pub mod client_router;

#[cfg(client)]
pub use client_router::{client_url_patterns, reverse};

#[cfg(server)]
pub mod server_router;

#[cfg(server)]
pub use server_router::server_url_patterns;
```

Client routes live in `src/apps/users/urls/client_router.rs`:

```rust
use crate::apps::users::client::components;
use reinhardt::ClientRouter;

pub fn client_url_patterns() -> ClientRouter {
    ClientRouter::new()
        .component(components::login_page::login_page)
        .component(components::logout_page::logout_page)
        .component(components::signup_page::signup_page)
}

pub fn reverse(name: &str, params: &[(&str, &str)]) -> String {
    client_url_patterns()
        .reverse(name, params)
        .unwrap_or_else(|error| panic!("failed to reverse users client route `{name}`: {error}"))
}
```

`client_router.rs` is client-only. Server builds register only the server-function markers below.


Server routes register server-function markers in `src/apps/users/urls/server_router.rs`:

```rust
use crate::apps::users::server_fn::{current_user, login, logout, register};
use reinhardt::ServerRouter;
use reinhardt::pages::server_fn::ServerFnRouterExt;

pub fn server_url_patterns() -> ServerRouter {
    ServerRouter::new()
        .server_fn(login::marker)
        .server_fn(logout::marker)
        .server_fn(register::marker)
        .server_fn(current_user::marker)
}
```

App-local route-backed components in `src/apps/users/client/components/` wrap the users forms with the shared nav:

```rust
use reinhardt::pages::component;
use reinhardt::pages::component::Page;

use crate::client::components::nav::with_nav;

#[component("/login/", "login")]
pub fn login_page() -> Page {
    with_nav(super::login_form())
}
```

Mount the users app in `src/config/urls.rs` on both targets:

```rust
use crate::apps::{polls::urls as polls_urls, users::urls as users_urls};

#[cfg(server)]
let router = router.server(|s| {
    s.mount("/", polls_urls::server_url_patterns())
        .mount("/", users_urls::server_url_patterns())
});
```

```rust
#[cfg(client)]
let router = router
    .mount_unified(
        "/",
        UnifiedRouter::new().client(|_| polls_urls::client_url_patterns()),
    )
    .mount_unified(
        "/",
        UnifiedRouter::new().client(|_| users_urls::client_url_patterns()),
    );
```

## Add Session Middleware

The session middleware owns the `SessionStore` DI registration. Add it once in `src/config/urls.rs`:

```rust
#[cfg(server)]
fn create_session_middleware() -> SessionMiddleware {
    let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(1_209_600))
        .with_http_only(true)
        .with_same_site("Lax".to_string())
        .with_path("/".to_string());
    SessionMiddleware::new(config)
}
```

Then attach it to the project router followed by the tutorial's account-validating middleware:

```rust
use crate::config::session_auth::TutorialSessionAuthMiddleware;

#[cfg(server)]
let router = {
    let session_middleware = create_session_middleware();
    let session_store = session_middleware.store_arc();
    router
        .with_middleware(session_middleware)
        .with_middleware(TutorialSessionAuthMiddleware::new(session_store))
};
```

`SessionMiddleware` provides `SessionData` and `Depends<SessionStoreKey, Arc<SessionStore>>`. `TutorialSessionAuthMiddleware` resolves the stored user ID against the current `User` record before publishing the `AuthState` required by `CurrentUser<User>`.

## Build the Auth Pages

The users client module hosts forms:

```rust
pub mod components;
```

The login form binds directly to the `login` server function:

```rust
pub fn login_form() -> Page {
    let login_form = form! {
        name: LoginForm,
        server_fn: login,
        method: Post,
        redirect_on_success: "/",
        fields: {
            username: CharField {
                label: "Username",
                placeholder: "your-username",
                max_length: 150,
                class: "form-control",
            }
            password: PasswordField {
                label: "Password",
                placeholder: "Enter your password",
                class: "form-control",
            }
        }
    };
```

The signup form uses `register`; the logout form uses `logout` with no fields. The generated `#[server_fn]` client stubs handle the HTTP call and CSRF header.

Each route-backed auth component uses the component macro and wraps its form body in the shared nav:

```rust
use reinhardt::pages::component;
use reinhardt::pages::component::Page;

use crate::client::components::nav::with_nav;

#[component("/login/", "login")]
pub fn login_page() -> Page {
    with_nav(super::login_form())
}

#[component("/logout/", "logout")]
pub fn logout_page() -> Page {
    with_nav(super::logout_form())
}

#[component("/signup/", "signup")]
pub fn signup_page() -> Page {
    with_nav(super::signup_form())
}
```

## Show Login State in the Nav

The nav calls `current_user()` as a reactive action:

```rust
pub fn nav_bar() -> Page {
    let load_user =
        use_action(|_: ()| async move { current_user().await.map_err(|e| e.to_string()) });
    load_user.dispatch(());
```

When a user is present, show their username and a logout link:

```rust
if load_user.is_pending() {
    div {
        class: "flex items-center gap-3",
        aria_busy: "true",
        span {
            class: "sr-only",
            "Checking sign-in status"
        }
    }
} else if let Some(Some(user)) = load_user.result() {
    div {
        class: "flex items-center gap-3",
        span {
            class: "text-sm text-muted",
            {
                format!("Signed in as {}", user.username)
            }
        }
        a {
            href: logout_href.clone(),
            class: "btn-secondary",
            "Logout"
        }
    }
}
```

When no user id is present in the session, `current_user()` returns `Ok(None)` and the nav falls through to its signed-out branch. That branch links to signup and login:

```rust
let login_href = users_routes::reverse("login", &[]);
let logout_href = users_routes::reverse("logout", &[]);
let signup_href = users_routes::reverse("signup", &[]);
```

## Migrate and Check

Generate and apply the users migration:

```bash
cargo make makemigrations
cargo make migrate
```

Run the app:

```bash
cargo make dev
```

Open `/signup/`, create an account, and confirm that the nav changes from Sign up/Login to `Signed in as <username>` plus Logout. Then sign out and sign back in at `/login/`.

## Checkpoint

Before continuing:

- `User` uses `#[user(..., manager = false)]` plus `#[model(...)]`.
- `AuthUserManager` is registered with a keyed `#[injectable(scope = "transient")]` provider.
- `login`, `register`, `logout`, and `current_user` are registered in the users server router.
- Session middleware is attached once in `src/config/urls.rs`.
- The nav resolves auth links through `users_routes::reverse(...)`.
