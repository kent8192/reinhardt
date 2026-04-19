+++
title = "Tutorial 4: Authentication and Permissions"
weight = 50

[extra]
sidebar_weight = 60
+++

# Tutorial 4: Authentication and Permissions

Protect your API with authentication and permission controls.

## Authentication

Reinhardt provides authentication:

```rust
use reinhardt::prelude::*;

async fn login(username: &str, password: &str) -> Option<User> {
    // Authenticate user
    authenticate(username, password).await
}
```

## Permission System

Reinhardt provides standard permission implementations that you can use out of the box.

### Using Standard Permissions (Recommended)

Reinhardt includes common permission classes:

```rust
use reinhardt::prelude::*;
use reinhardt::{IsAuthenticated, AllowAny};

// Use the standard IsAuthenticated permission
let permissions = vec![
    Box::new(IsAuthenticated) as Box<dyn Permission>
];
```

**Available Standard Permissions:**
- `AllowAny` - Allows access to any user (authenticated or not)
- `IsAuthenticated` - Only allows access to authenticated users
- `IsAdminUser` - Only allows access to admin users

### Custom Permission Implementation (Advanced)

For custom authorization logic, you can implement the `Permission` trait:

```rust
use reinhardt::prelude::*;
use async_trait::async_trait;

pub struct CustomPermission;

#[async_trait]
impl Permission for CustomPermission {
    async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
        // Your custom authorization logic
        context.is_authenticated && context.user.map_or(false, |u| u.is_active)
    }
}
```

## PermissionContext

The `PermissionContext` provides request information for permission checks:

```rust
pub struct PermissionContext<'a> {
    pub request: &'a reinhardt_http::Request,
    pub is_authenticated: bool,
    pub is_admin: bool,
    pub is_active: bool,
    pub user: Option<Box<dyn User>>,
}
```

## Standard Permission Classes

Reinhardt provides the following standard permission classes:

### AllowAny

Allows access to any user (authenticated or not). This is the default permission:

```rust
use reinhardt::AllowAny;

let permission = Box::new(AllowAny) as Box<dyn Permission>;
```

### IsAuthenticated

Only authenticated users can access:

```rust
use reinhardt::IsAuthenticated;

let permission = Box::new(IsAuthenticated) as Box<dyn Permission>;
```

**Implementation Reference:**
```rust
// You don't need to implement this - use IsAuthenticated directly
pub struct IsAuthenticated;

#[async_trait]
impl Permission for IsAuthenticated {
    async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
        context.is_authenticated
    }
}
```

### IsAdminUser

Admin-only permission:

```rust
use reinhardt::IsAdminUser;

let permission = Box::new(IsAdminUser) as Box<dyn Permission>;
```

## Custom Permissions

Create custom permissions for specific requirements:

```rust
use reinhardt::prelude::*;
use async_trait::async_trait;

pub struct IsOwnerOrReadOnly;

#[async_trait]
impl Permission for IsOwnerOrReadOnly {
    async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
        // Read permissions for any request
        if context.request.method() == Method::GET {
            return true;
        }

        // Write permissions only for authenticated users
        if !context.is_authenticated {
            return false;
        }

        // Additional ownership check would go here
        // For example, check if user owns the resource
        true
    }
}
```

## Using Permissions with ViewSets

Apply permissions to ViewSets using `ModelViewSetHandler`:

```rust
use reinhardt::prelude::*;
use reinhardt::IsAuthenticated;
use std::sync::Arc;

let handler = ModelViewSetHandler::<Snippet>::new()
    .add_permission(Arc::new(IsAuthenticated));
```

## Object-Level Permissions

Check permissions for specific objects:

```rust
use reinhardt::prelude::*;
use async_trait::async_trait;

pub struct IsOwner;

#[async_trait]
impl Permission for IsOwner {
    async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
        // Allow all GET requests
        if context.request.method() == Method::GET {
            return true;
        }

        // For write operations, check ownership
        if let Some(user) = context.user {
            // Extract object ID from path and check ownership
            // This is a simplified example
            true
        } else {
            false
        }
    }
}
```

## Permission Combinations

Combine multiple permissions:

```rust
use reinhardt::prelude::*;
use async_trait::async_trait;

pub struct IsAuthenticatedAndActive;

#[async_trait]
impl Permission for IsAuthenticatedAndActive {
    async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
        if !context.is_authenticated {
            return false;
        }

        if let Some(user) = context.user {
            user.is_active
        } else {
            false
        }
    }
}
```

## Complete Example

Full authentication and permission workflow using standard and custom permissions:

```rust
use reinhardt::prelude::*;
use reinhardt::IsAuthenticated;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use reinhardt::Method;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snippet {
    id: i64,
    title: String,
    code: String,
    owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnippetSerializer {
    id: i64,
    title: String,
    code: String,
    owner: String,
}

// Example 1: Using standard permission with ModelViewSetHandler
let handler_with_standard = ModelViewSetHandler::<Snippet>::new()
    .add_permission(Arc::new(IsAuthenticated));

// Example 2: Custom permission for more complex logic
pub struct IsOwnerOrReadOnly;

#[async_trait]
impl Permission for IsOwnerOrReadOnly {
    async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
        // Read operations are allowed for everyone
        if matches!(context.request.method(), &Method::GET | &Method::HEAD | &Method::OPTIONS) {
            return true;
        }

        // Write operations require authentication
        if let Some(user) = context.user {
            // In a real app, check if user owns the snippet
            user.is_authenticated()
        } else {
            false
        }
    }
}

// Create handler with custom permission
let handler_with_custom = ModelViewSetHandler::<Snippet>::new()
    .add_permission(Arc::new(IsOwnerOrReadOnly));
```

## Group-Based Permissions

Reinhardt supports group-based permission management through `GroupManager`.
Users can be assigned to groups, and each group can have its own set of permissions.

### Setting Up GroupManager

Register a global `GroupManager` at application startup:

```rust
use reinhardt_auth::{GroupManager, register_group_manager};
use reinhardt_auth::group_management::CreateGroupData;
use std::sync::Arc;

async fn setup_groups() {
    let mut manager = GroupManager::new();

    // Create groups
    let editors = manager.create_group(CreateGroupData {
        name: "editors".to_string(),
        description: Some("Content editors".to_string()),
    }).await.unwrap();

    // Assign permissions to groups
    manager.add_group_permission(
        &editors.id.to_string(), "blog.add_post"
    ).await.unwrap();
    manager.add_group_permission(
        &editors.id.to_string(), "blog.edit_post"
    ).await.unwrap();

    // Register globally — PermissionsMixin will use this automatically
    register_group_manager(Arc::new(manager));
}
```

### How Group Permissions Work

Once a `GroupManager` is registered, `PermissionsMixin::get_group_permissions()`
automatically resolves permissions for the user's groups:

```rust
use reinhardt_auth::PermissionsMixin;

// User belongs to "editors" group
let user = get_current_user().await;

// Automatically includes group permissions
assert!(user.has_perm("blog.add_post"));    // from "editors" group
assert!(user.has_perm("blog.edit_post"));   // from "editors" group

// get_all_permissions() merges direct + group permissions
let all = user.get_all_permissions();
```

The resolution flow:
1. `has_perm()` calls `get_all_permissions()`
2. `get_all_permissions()` merges `get_user_permissions()` (direct) and `get_group_permissions()` (from groups)
3. `get_group_permissions()` looks up the global `GroupManager` and resolves permissions for each group name
4. Superusers bypass all checks and always return `true`

## User Model with Database Integration

When combining `#[user]` with `#[model]`, the user macro automatically injects
`ManyToManyField` relationships for structured database queries:

```rust
use reinhardt::prelude::*;
use reinhardt_auth::Argon2Hasher;

// What you write:
#[user(hasher = Argon2Hasher, username_field = "username", full = true)]
#[model(app_label = "auth", table_name = "auth_user")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[field(primary_key = true)]
    pub id: Uuid,
    #[field(max_length = 150, unique = true)]
    pub username: String,
    // ... other fields ...
    pub user_permissions: Vec<String>,  // PermissionsMixin cache
    pub groups: Vec<String>,            // PermissionsMixin cache
}

// The macro automatically:
// 1. Marks Vec<String> fields with #[field(skip = true)] (excluded from DB)
// 2. Injects ManyToManyField<User, AuthPermission> for permission relationships
// 3. Injects ManyToManyField<User, Group> for group relationships
// 4. Generates BaseUser, FullUser, PermissionsMixin, AuthIdentity impls
```

The `Vec<String>` fields serve as in-memory caches for `PermissionsMixin`,
while `ManyToManyField` relationships handle structured ORM queries.

## Summary

In this tutorial, you learned:

1. Basic authentication with the `reinhardt-auth` crate
2. Implementing custom permissions with the `Permission` trait
3. Using `PermissionContext` for permission checks
4. Built-in permission classes
5. Object-level permissions
6. Combining multiple permissions
7. Applying permissions to ViewSets
8. Group-based permissions with `GroupManager`
9. Database-backed user models with `#[user]` + `#[model]`

Next tutorial: [Tutorial 5: Relationships and Hyperlinked APIs](../5-relationships-and-hyperlinked-apis/)
