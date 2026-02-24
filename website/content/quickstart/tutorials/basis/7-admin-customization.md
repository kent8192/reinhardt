+++
title = "Part 7: Admin Customization"
weight = 70

[extra]
sidebar_weight = 70
+++

# Part 7: Admin Customization

In this tutorial, we'll explore the Reinhardt admin interface and learn how to customize it for managing poll data.

## Activating the Admin

The Reinhardt admin is a powerful, automatically-generated interface for managing your application's data. Let's enable it for our polls application.

Add the admin dependency if not already present:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web", features = ["admin"] }
```

## Creating an Admin User

First, create a superuser account that can access the admin interface using the `createsuperuser` command:

```bash
cargo run --bin manage createsuperuser
```

You'll be prompted to enter:
- Username
- Email address
- Password (entered twice for confirmation)

Example session:

```
$ cargo run --bin manage createsuperuser
Username: admin
Email address: admin@example.com
Password:
Password (again):
Superuser created successfully.
```

**Note**: The `createsuperuser` command handles password hashing automatically using secure algorithms (Argon2 by default).

## Registering Models with the Admin

Reinhardt provides two approaches for registering models with the admin panel.

### Approach A: Declarative Configuration with #[admin(...)] (Recommended)

The simplest way to configure admin for your models is using the `#[admin(...)]` attribute macro. This approach provides compile-time validation and keeps configuration close to the model definition.

**Example: Question Model with Admin Configuration**

```rust
// src/models.rs
use reinhardt::prelude::*;
use chrono::{DateTime, Utc};

#[admin(
    list_display = ["question_text", "pub_date", "was_published_recently"],
    list_filter = ["pub_date"],
    search_fields = ["question_text"],
    date_hierarchy = "pub_date",
    ordering = [("pub_date", desc)],
    list_per_page = 25
)]
#[model(app_label = "polls", table_name = "polls_question")]
pub struct Question {
    #[field(primary_key = true)]
    pub id: i64,

    #[field(max_length = 200)]
    pub question_text: String,

    #[field(auto_now_add = true)]
    pub pub_date: DateTime<Utc>,
}

impl Question {
    /// Check if this question was published recently (within the last day)
    pub fn was_published_recently(&self) -> bool {
        let now = Utc::now();
        let one_day_ago = now - chrono::Duration::days(1);
        self.pub_date >= one_day_ago && self.pub_date <= now
    }
}
```

**Example: Choice Model with Admin Configuration**

```rust
use reinhardt::db::associations::ForeignKeyField;

#[admin(
    list_display = ["choice_text", "votes", "question"],
    list_filter = ["question"],
    search_fields = ["choice_text"],
    ordering = [("votes", desc)]
)]
#[model(app_label = "polls", table_name = "polls_choice")]
pub struct Choice {
    #[field(primary_key = true)]
    pub id: i64,

    // ⚠️ IMPORTANT: related_name is REQUIRED for #[rel(foreign_key)]
    #[rel(foreign_key, related_name = "choices")]
    question: ForeignKeyField<Question>,

    #[field(max_length = 200)]
    pub choice_text: String,

    #[field(default = 0)]
    pub votes: i32,
}
```

**Available #[admin(...)] Options:**

**Display Options:**
- `list_display = ["field1", "field2", ...]` - Columns to show in list view
- `list_per_page = 25` - Number of items per page (default: 100)
- `list_editable = ["field1"]` - Fields editable directly in list view

**Filtering Options:**
- `list_filter = ["field1", "field2"]` - Add sidebar filters
- `date_hierarchy = "date_field"` - Add date drill-down navigation
- `search_fields = ["field1", "field2"]` - Enable search on specified fields

**Ordering Options:**
- `ordering = [("field", asc)]` - Default sort order (ascending)
- `ordering = [("field", desc)]` - Default sort order (descending)

**Field Options:**
- `readonly_fields = ["field1"]` - Non-editable fields in forms
- `exclude = ["field1"]` - Fields hidden in forms
- `fieldsets = [...]` - Grouped form fields (advanced)

**Benefits of #[admin(...)]:**

1. **Declarative**: Configuration is defined alongside the model
2. **Type-safe**: Compile-time validation of field names
3. **Less boilerplate**: No manual trait implementation needed
4. **Consistent**: Same syntax across all models
5. **Auto-registration**: Models are automatically registered with admin site

### Approach B: Manual Configuration with ModelAdmin Trait

For cases requiring complex logic, custom permissions, or dynamic configuration, you can manually implement the `ModelAdmin` trait.

**When to use manual implementation:**
- Complex permission logic that varies by user/context
- Custom queryset filtering based on request parameters
- Dynamic field configuration
- Custom actions beyond CRUD operations
- Advanced inline formset customization

**Example: Manual Admin Configuration**

Create `src/admin.rs`:

```rust
use reinhardt::prelude::*;
use reinhardt::admin::{AdminSite, ModelAdmin, AdminContext};
use crate::models::{Question, Choice};

pub struct QuestionAdmin;

impl ModelAdmin for QuestionAdmin {
    type Model = Question;

    fn list_display(&self) -> Vec<&str> {
        vec!["question_text", "pub_date", "was_published_recently"]
    }

    fn list_filter(&self) -> Vec<&str> {
        vec!["pub_date"]
    }

    fn search_fields(&self) -> Vec<&str> {
        vec!["question_text"]
    }

    // Advanced: Custom queryset filtering
    fn get_queryset(&self, ctx: &AdminContext) -> QuerySet<Self::Model> {
        let mut qs = Self::Model::objects();
        // Only show published questions to non-superusers
        if !ctx.user.is_superuser {
            qs = qs.filter(Self::Model::field_pub_date().lte(Utc::now()));
        }
        qs
    }

    // Advanced: Custom permission logic
    fn has_add_permission(&self, ctx: &AdminContext) -> bool {
        // Only staff in "editors" group can add questions
        ctx.user.is_staff && ctx.user.groups.contains(&"editors")
    }
}

pub struct ChoiceAdmin;

impl ModelAdmin for ChoiceAdmin {
    type Model = Choice;

    fn list_display(&self) -> Vec<&str> {
        vec!["choice_text", "question", "votes"]
    }

    fn list_filter(&self) -> Vec<&str> {
        vec!["question"]
    }
}

pub fn register_admin(site: &mut AdminSite) {
    site.register::<Question, QuestionAdmin>();
    site.register::<Choice, ChoiceAdmin>();
}
```

**Note**: When using `#[admin(...)]` on models, you don't need manual registration in `src/admin.rs`. The macro handles registration automatically.

Update `src/config/apps.rs` to include the admin:

```rust
use reinhardt::prelude::*;
use reinhardt::admin::AdminSite;

pub fn configure_admin() -> AdminSite {
    let mut admin_site = AdminSite::new("Polls Administration");
    admin::register_admin(&mut admin_site);
    admin_site
}
```

Register the admin routes in `src/config/urls.rs`:

```rust
use crate::config::apps::configure_admin;

pub fn url_patterns() -> UnifiedRouter {
    UnifiedRouter::new()
        .mount("/admin/", configure_admin().urls())
        // ... other routes ...
}
```

Then start the server:

```bash
cargo make runserver
```

Now visit `http://127.0.0.1:8000/admin/` and log in with your superuser credentials.

## Customizing the Admin Form

Let's customize how the Question admin form looks. Update `QuestionAdmin`:

```rust
impl ModelAdmin for QuestionAdmin {
    type Model = Question;

    fn fieldsets(&self) -> Vec<(&str, Vec<&str>)> {
        vec![
            ("Question Information", vec!["question_text"]),
            ("Date Information", vec!["pub_date"]),
        ]
    }

    fn list_display(&self) -> Vec<&str> {
        vec!["question_text", "pub_date", "was_published_recently"]
    }

    fn list_filter(&self) -> Vec<&str> {
        vec!["pub_date"]
    }

    fn search_fields(&self) -> Vec<&str> {
        vec!["question_text"]
    }

    fn readonly_fields(&self) -> Vec<&str> {
        vec!["was_published_recently"]
    }
}
```

The `fieldsets` option allows you to organize fields into groups with descriptive headers.

## Adding Related Objects

We can edit Choices directly on the Question admin page using inline editing. Update `QuestionAdmin`:

```rust
use reinhardt::prelude::*;
use reinhardt::admin::{AdminSite, ModelAdmin, TabularInline, StackedInline};

pub struct ChoiceInline;

impl TabularInline for ChoiceInline {
    type Model = Choice;
    type ParentModel = Question;

    fn fields(&self) -> Vec<&str> {
        vec!["choice_text", "votes"]
    }

    fn extra(&self) -> usize {
        3  // Show 3 extra empty forms
    }
}

impl ModelAdmin for QuestionAdmin {
    type Model = Question;

    fn inlines(&self) -> Vec<Box<dyn InlineAdmin>> {
        vec![Box::new(ChoiceInline)]
    }

    // ... rest of configuration ...
}
```

Now when editing a Question, you can add and edit Choices on the same page.

## Customizing the Change List

The change list is the page that displays all objects of a given type. Let's make it more useful:

```rust
impl ModelAdmin for QuestionAdmin {
    type Model = Question;

    fn list_display(&self) -> Vec<&str> {
        // Columns to display in the list
        vec!["question_text", "pub_date", "was_published_recently"]
    }

    fn list_filter(&self) -> Vec<&str> {
        // Add filters in the sidebar
        vec!["pub_date"]
    }

    fn search_fields(&self) -> Vec<&str> {
        // Enable search on these fields
        vec!["question_text"]
    }

    fn list_per_page(&self) -> usize {
        // Number of items per page
        25
    }

    fn date_hierarchy(&self) -> Option<&str> {
        // Add drill-down navigation by date
        Some("pub_date")
    }
}
```

### Customizing Display Methods

Let's improve how `was_published_recently` is displayed:

Update `src/models.rs`:

```rust
impl Question {
    /// Check if this question was published recently (within the last day)
    pub fn was_published_recently(&self) -> bool {
        let now = Utc::now();
        let one_day_ago = now - chrono::Duration::days(1);
        self.pub_date >= one_day_ago && self.pub_date <= now
    }

    /// Admin display for was_published_recently
    pub fn was_published_recently_display(&self) -> &str {
        if self.was_published_recently() {
            "Yes"
        } else {
            "No"
        }
    }
}
```

## Customizing Admin with Components

**Note**: reinhardt-pages uses a Component-based approach for admin customization, not Django-style templates.

### Admin Configuration with Code

Instead of template files, customize the admin panel using Rust code. Update your admin registration:

```rust
use reinhardt::admin::{AdminSite, ModelAdmin, AdminConfig};
use reinhardt::pages::component::View;
use reinhardt::pages::page;

pub struct QuestionAdmin;

impl ModelAdmin for QuestionAdmin {
    type Model = Question;

    fn list_display(&self) -> Vec<&str> {
        vec!["question_text", "pub_date", "was_published_recently"]
    }

    fn list_filter(&self) -> Vec<&str> {
        vec!["pub_date"]
    }

    fn search_fields(&self) -> Vec<&str> {
        vec!["question_text"]
    }

    // Custom admin panel configuration
    fn admin_config(&self) -> AdminConfig {
        AdminConfig::new()
            .site_title("Polls Administration")
            .site_header("Polls Admin Panel")
            .site_description("Manage your polls and choices")
    }
}
```

### Custom Admin Components

For advanced customization, create custom components:

```rust
use reinhardt::pages::admin::AdminPanel;
use reinhardt::pages::component::View;
use reinhardt::pages::page;

pub fn custom_admin_index() -> View {
    page!(|| {
        div {
            class: "admin-dashboard",
            div {
                class: "welcome-banner bg-blue-50 p-6 rounded-lg mb-6",
                h1 {
                    class: "text-2xl font-bold mb-2",
                    "Welcome to Polls Administration"
                }
                p {
                    class: "text-gray-700",
                    "Use the sidebar to manage polls and choices."
                }
            }
            // Render default admin panel
            { AdminPanel::default_view() }
        }
    })()
}
```

### Styling the Admin Panel

Use UnoCSS classes to style admin components:

```rust
impl ModelAdmin for QuestionAdmin {
    // ... other methods ...

    fn custom_list_styles(&self) -> &str {
        "card card-body shadow-lg"
    }

    fn custom_form_styles(&self) -> &str {
        "space-y-4 p-6 bg-white rounded-xl"
    }
}
```

**Key Differences from Template-Based Customization:**

| Aspect | Django Templates | reinhardt-pages |
|--------|------------------|-----------------|
| **Configuration** | HTML template files | Rust code with `AdminConfig` |
| **Styling** | Template syntax (`{% block %}`) | Component functions with `page!` |
| **Type Safety** | Runtime template errors | Compile-time type checking |
| **Reusability** | Template inheritance | Component composition |

### Why Component-Based?

- **Type safety** - Compiler catches errors at build time
- **Refactoring** - IDEs can help rename and restructure
- **Composition** - Easy to build complex UIs from simple components
- **Consistency** - Same patterns as the rest of your reinhardt-pages app

## Admin Actions

Admin actions allow you to perform bulk operations on selected items. Add a custom action:

```rust
impl ModelAdmin for QuestionAdmin {
    type Model = Question;

    fn actions(&self) -> Vec<Box<dyn AdminAction<Self::Model>>> {
        vec![
            Box::new(MakePublishedAction),
        ]
    }
}

pub struct MakePublishedAction;

impl AdminAction<Question> for MakePublishedAction {
    fn name(&self) -> &str {
        "make_published"
    }

    fn short_description(&self) -> &str {
        "Mark selected questions as published"
    }

    async fn perform(
        &self,
        conn: &DatabaseConnection,
        queryset: Vec<Question>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let count = queryset.len();

        for mut question in queryset {
            question.pub_date = Utc::now();
            question.save(conn).await?;
        }

        Ok(format!("Successfully updated {} question(s).", count))
    }
}
```

## Summary

In this tutorial, you learned:

- How to activate the Reinhardt admin interface
- How to create and use admin user accounts
- How to register models with the admin
- How to customize admin forms with fieldsets
- How to use inline editing for related objects
- How to customize the change list display
- How to add filters and search functionality
- How to customize admin templates
- How to create custom admin actions

The Reinhardt admin is a powerful tool for managing your application's data. With customization, it can be tailored to fit your exact needs.

## Congratulations!

You've completed the Reinhardt Basis Tutorial! You now know how to:

- Create a Reinhardt project and apps
- Define models and work with databases
- Create views and URL configurations
- Use templates to render HTML
- Process forms and handle user input
- Write tests for your application
- Serve static files (CSS, images)
- Use and customize the admin interface

## Where to Go From Here

Now that you've learned the basics, you might want to explore:

- **[REST API Tutorial](../rest/quickstart/)** - Learn to build REST APIs with Reinhardt
- **[Feature Flags Guide](/docs/feature-flags/)** - Optimize your build configuration
- **Individual Crate Documentation** - Deep dive into specific components
- **Production Deployment** - Learn about deploying Reinhardt applications

Happy coding with Reinhardt!