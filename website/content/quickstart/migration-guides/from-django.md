+++
title = "Migrating from Django"
weight = 10
+++

# Migrating from Django

Guide for developers migrating from Django to Reinhardt.

## Table of Contents

- [MTV Pattern](#mtv-pattern)
- [URL Routing](#url-routing)
- [ORM Comparison](#orm-comparison)
- [Settings System](#settings-system)
- [Templates](#templates)

---

## MTV Pattern

### Django Structure

```
myapp/
├── models.py      # Model
├── views.py       # View (TemplateView, ListView, etc.)
├── urls.py        # URL configuration
├── forms.py       # Forms
└── templates/     # HTML templates
```

### Reinhardt Structure

```
myapp/
├── models/        # Model definitions
├── views/         # Handler implementations
├── routes.rs      # Router configuration
├── forms.rs       # Form validation
└── templates/     # HTML templates (if using template engine)
```

---

## URL Routing

### Django URLs

```python
# urls.py
from django.urls import path
from . import views

urlpatterns = [
    path('users/', views.UserListView.as_view(), name='user-list'),
    path('users/<int:id>/', views.UserDetailView.as_view(), name='user-detail'),
]
```

### Reinhardt Routes

```rust
// routes.rs
use reinhardt::ServerRouter;
use reinhardt::View;
use hyper::Method;

pub fn user_routes() -> ServerRouter {
    ServerRouter::new()
        .with_namespace("users")
        .view("/users/", UserListView)           // List View equivalent
        .view("/users/{id}/", UserDetailView)     // Detail View equivalent
}

// Equivalent to:
// GET /users/ → UserListView.dispatch(request, Action::list())
// GET /users/{id}/ → UserDetailView.dispatch(request, Action::retrieve())
```

### URL Reversal

#### Django

```python
from django.urls import reverse

url = reverse('user-detail', kwargs={'id': 123})
# Returns: /users/123/
```

#### Reinhardt

```rust
use reinhardt::ServerRouter;

let mut router = user_routes();
router.register_all_routes();

let url = router.reverse("users:user-detail", &[("id", "123")]).unwrap();
// Returns: /users/123/
```

---

## ORM Comparison

### Django ORM

```python
# QuerySet API
users = User.objects.filter(is_active=True).order_by('-created_at')

# Get single object
user = User.objects.get(id=123)

# Create
user = User.objects.create(username="alice")

# Update
user.email = "alice@example.com"
user.save()

# Delete
user.delete()
```

### Reinhardt ORM

```rust
use reinhardt::QuerySet;

// QuerySet API
let users = User::objects()
    .filter(user::is_active.eq(true))
    .order_by(user::created_at, Order::Desc)
    .all()
    .await?;

// Get single object
let user = User::objects().get(123).await?;

// Create
let user = User::new();
user.username = "alice".to_string();
user.save().await?;

// Update
let user = User::objects().get(123).await?;
user.email = "alice@example.com".to_string();
user.save().await?;

// Delete
user.delete().await?;
```

---

## Settings System

### Django Settings

```python
# settings.py
DEBUG = True
SECRET_KEY = 'your-secret-key'
DATABASES = {
    'default': {
        'ENGINE': 'django.db.backends.postgresql',
        'NAME': 'mydb',
    }
}
```

### Reinhardt Settings

Reinhardt uses a built-in settings system with multiple sources:

```rust
// config.rs
use reinhardt::{Settings, SettingsBuilder, EnvSource, DefaultSource};

pub fn get_settings() -> Settings {
    SettingsBuilder::new()
        .add_source(DefaultSource::new())
        .add_source(EnvSource::new().with_prefix("APP_"))
        .build()
        .expect("Failed to build settings")
        .into_typed()
        .expect("Failed to convert settings")
}

// main.rs
let settings = get_settings();
let debug = settings.get::<bool>("debug").unwrap_or(false);
let secret_key = settings.get::<String>("secret_key").unwrap();
let database_url = settings.get::<String>("database_url").unwrap();
```

---

## Templates

### Django Templates

```python
# views.py
from django.shortcuts import render

def user_list(request):
    users = User.objects.all()
    return render(request, 'users/list.html', {'users': users})
```

### Reinhardt with Template Engine

Reinhardt can integrate with template engines like Askama:

```rust
use askama::Template;

#[derive(Template)]
#[template(path = "users/list.html")]
struct UserListTemplate {
    users: Vec<User>,
}

async fn user_list() -> Response {
    let users = User::objects().all().await?;
    let template = UserListTemplate { users };
    Response::ok()
        .with_header("content-type", "text/html")
        .with_body(template.render().unwrap())
}
```

Or use JSON responses with a frontend framework:

```rust
async fn user_list() -> Response {
    let users = User::objects().all().await?;
    Response::ok().with_json(&users).unwrap()
}
```

---

## Class-Based Views

### Django Class-Based Views

```python
# views.py
from django.views import View

class UserListView(View):
    def get(self, request):
        users = User.objects.all()
        return render(request, 'users/list.html', {'users': users})
```

### Reinhardt Views

```rust
// views.rs
use reinhardt::{View, EndpointMetadata, Request, Response};

pub struct UserListView;

#[async_trait]
impl View for UserListView {
    async fn dispatch(&self, request: Request) -> Result<Response, Error> {
        let users = User::objects().all().await?;
        Ok(Response::ok().with_json(&users)?)
    }
}

// EndpointMetadata for route configuration
impl EndpointMetadata for UserListView {
    fn path() -> &'static str { "/users/" }
    fn method() -> Method { Method::GET }
    fn name() -> &'static str { "user-list" }
}
```

---

## Forms

### Django Forms

```python
# forms.py
from django import forms

class UserForm(forms.ModelForm):
    class Meta:
        model = User
        fields = ['username', 'email']
```

### Reinhardt Forms

```rust
// forms.rs
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Serialize, Validate)]
struct UserForm {
    #[validate(length(min = 1, max = 100))]
    username: String,

    #[validate(email)]
    email: String,
}

// In handler
use reinhardt::Json;

async fn create_user(Json(form): Json<UserForm>) -> Response {
    if let Err(errors) = form.validate() {
        return Response::bad_request()
            .with_json(&errors)
            .unwrap();
    }

    let user = User::new();
    user.username = form.username;
    user.email = form.email;
    user.save().await?;

    Response::created().with_json(&user).unwrap()
}
```

---

## Middleware

### Django Middleware

```python
# middleware.py
class CustomMiddleware:
    def __init__(self, get_response):
        self.get_response = get_response

    def __call__(self, request):
        # Process request
        response = self.get_response(request)
        # Process response
        return response
```

### Reinhardt Middleware

```rust
use async_trait::async_trait;
use reinhardt::{Handler, Middleware, Request, Response};

pub struct CustomMiddleware;

#[async_trait]
impl Middleware for CustomMiddleware {
    async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
        // Process request
        let response = next.handle(request).await?;
        // Process response
        Ok(response)
    }
}
```

---

## Migration Checklist

- [ ] Convert models to `#[model]` structs
- [ ] Convert views to handlers (`Handler` trait)
- [ ] Convert URL patterns to router configuration
- [ ] Convert forms to validation structs
- [ ] Set up environment-based configuration
- [ ] Configure database connection
- [ ] Add middleware stack
- [ ] Implement error handling
- [ ] Add tests

---

## See Also

- [Request API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Request.html)
- [Response API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Response.html)
- [Router API](https://docs.rs/reinhardt-urls/latest/reinhardt_urls/routers/struct.ServerRouter.html)
- [From Actix-web](../from-actix/)
- [From Axum](../from-axum/)
