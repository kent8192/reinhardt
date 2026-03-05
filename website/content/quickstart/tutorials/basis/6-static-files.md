+++
title = "Part 6: Static Files and Styling"
weight = 60

[extra]
sidebar_weight = 60
+++

# Part 6: Static Files and Styling

In this tutorial, we'll add CSS stylesheets and images to make our polls application look better using reinhardt-pages' approach to static file management.

## Understanding Static Files in reinhardt-pages

Static files are assets like CSS, JavaScript, images, and fonts that don't change during runtime. In reinhardt-pages applications, static files are managed differently from traditional server-rendered frameworks:

**Traditional Approach (Server-Side Rendering):**
```html
<!-- Server-rendered template -->
<link rel="stylesheet" href="{{ static('polls/css/style.css') }}">
```

**reinhardt-pages Approach (WASM):**
1. **CDN Resources**: External libraries loaded from CDNs
2. **Local Assets**: Static files copied to `dist/` during build
3. **Direct References**: Files referenced using one of three methods (see below)

## Static File Reference Methods

Reinhardt provides three ways to reference static files, each optimized for different use cases:

### 1. Compile-Time: `static_url!` Macro

For static paths known at compile time:

```rust
use reinhardt::static_url;

page!(|| {
	link {
		rel: "stylesheet",
		href: static_url!("css/polls.css")
		// Resolved at compile time to "/static/css/polls.css"
	}
})
```

**When to use**: Fixed asset paths that don't change at runtime

**Benefits**:
- Zero runtime overhead
- Compile-time path validation
- Optimal for CDN integration

### 2. Runtime: `resolve_static()` Function

For dynamic paths determined at runtime:

```rust
use reinhardt::pages::static_resolver::resolve_static;

// Example: User-selectable theme
let theme = user.get_theme(); // "dark" or "light"
let css_path = format!("css/{}.css", theme);

page!(|css_path: String| {
	link {
		rel: "stylesheet",
		href: resolve_static(&css_path)
		// Returns: "/static/css/dark.css"
	}
})(css_path)
```

**When to use**: Dynamic asset paths based on user input, state, or configuration

**Benefits**:
- Flexible runtime resolution
- Integrates with settings configuration
- Works with reactive state management

### 3. Server Template: `{{ static_url() }}`

For `index.html` and server-rendered templates:

```html
<!-- Template (pre-deployment) -->
<script src="{{ static_url('app.js') }}"></script>
<link rel="stylesheet" href="{{ static_url('css/main.css') }}">

<!-- Rendered (post-collectstatic with content hashing) -->
<script src="/static/app.abc123.js"></script>
<link rel="stylesheet" href="/static/css/main.def456.css">
```

**When to use**: HTML templates processed on the server before deployment

**Benefits**:
- Cache busting with content hashing
- Server-side path resolution
- Works with `collectstatic` workflow

### Choosing the Right Method

Use this decision tree:

| Question | Method |
|----------|--------|
| Path known at compile time? | `static_url!` macro |
| Path depends on runtime state/user input? | `resolve_static()` function |
| Reference in server-rendered HTML template? | `{{ static_url() }}` template function |

**Example Scenarios:**

```rust
// ✅ GOOD: Compile-time static reference
link {
	rel: "icon",
	href: static_url!("favicon.ico")
}

// ✅ GOOD: Runtime dynamic reference
let avatar_url = format!("avatars/{}.png", user.id);
img {
	src: resolve_static(&avatar_url)
}

// ❌ BAD: Hard-coded path (breaks with CDN or STATIC_URL changes)
link {
	rel: "stylesheet",
	href: "/static/css/polls.css"  // Don't do this!
}
```

**Note**: For `index.html`, use the template function approach:
```html
<!-- index.html -->
<script type="module">
	const jsUrl = '{{ static_url("examples_tutorial_basis.js") }}';
	const wasmUrl = '{{ static_url("examples_tutorial_basis_bg.wasm") }}';
	const { default: init } = await import(jsUrl);
	await init(wasmUrl);
</script>
```

---

## Using Static URLs in page! Macro

Now that you understand the three methods for static URL resolution, let's see how to use them in practice within `page!` macros.

### Basic Usage with resolve_static()

The `resolve_static()` function is the recommended way to reference static files in `page!` macros. It works at runtime and integrates with the static files configuration:

```rust
use reinhardt::pages::static_resolver::resolve_static;

page!(|| {
	div { class: "container",
		img {
			src: resolve_static("images/logo.png"),
			alt: "Polls App Logo",
			class: "logo"
		}
	}
})()
```

**Key Points:**
- Import `resolve_static` from `reinhardt::pages::static_resolver`
- Pass the relative path (without `/static/` prefix)
- The function returns the full URL: `/static/images/logo.png`
- With manifest, it returns cache-busted URLs: `/static/images/logo.abc123.png`

### Practical Examples

#### Displaying Images

**Static Image Path:**

For fixed images (like logos or icons), use a simple string:

```rust
page!(|| {
	div { class: "poll-header",
		img {
			src: resolve_static("images/poll-icon.svg"),
			alt: "Poll",
			class: "poll-icon w-16 h-16"
		}
		h1 { "Latest Polls" }
	}
})()
```

**Dynamic Image Path:**

For user-specific or data-driven images, construct the path at runtime:

```rust
page!(|user_id: i64| {
	// Construct path based on user ID
	let avatar_path = format!("images/avatars/user_{}.png", user_id);

	div { class: "user-profile",
		img {
			src: resolve_static(&avatar_path),
			alt: "User Avatar",
			class: "avatar rounded-full w-12 h-12"
		}
	}
})(user_id)
```

**Real-World Example - Poll Card:**

```rust
page!(|question: QuestionInfo| {
	div { class: "poll-card p-4 border rounded",
		// Poll icon
		img {
			src: resolve_static("images/poll-icon.svg"),
			alt: "Poll",
			class: "w-8 h-8 mb-2"
		}

		// Question text
		h2 { class: "text-xl font-bold",
			{ question.question_text }
		}

		// Vote button
		a {
			href: format!("/polls/{}/", question.id),
			class: "btn-primary mt-3",
			"Vote Now"
		}
	}
})(question)
```

#### Loading Stylesheets and Scripts

While stylesheets and scripts are typically loaded in `index.html`, you can also load them conditionally in `page!` macros:

```rust
page!(|enable_dark_mode: bool| {
	div {
		// Conditionally load dark mode stylesheet
		if enable_dark_mode {
			link {
				rel: "stylesheet",
				href: resolve_static("css/dark-theme.css")
			}
		}

		// Page content
		div { class: "content",
			"Page content here"
		}
	}
})(enable_dark_mode)
```

**Note:** For global stylesheets, prefer loading in `index.html` or using the `head!` macro (see below).

#### Dynamic Asset Selection

Select assets based on user state or application logic:

```rust
page!(|theme: String| {
	// Select theme-specific CSS
	let css_path = format!("css/{}.css", theme);

	div {
		link {
			rel: "stylesheet",
			href: resolve_static(&css_path)
			// Returns: "/static/css/dark.css" or "/static/css/light.css"
		}

		div { class: "themed-content",
			"Content styled by selected theme"
		}
	}
})(theme)
```

### Integration with head! Macro

For server-side rendering (SSR) and global assets, use `head!` macro with `resolve_static()`:

```rust
use reinhardt::pages::{head, page};
use reinhardt::pages::static_resolver::resolve_static;

let my_head = head!(|| {
	// Stylesheet
	link {
		rel: "stylesheet",
		href: resolve_static("css/polls.css")
	}

	// JavaScript
	script {
		src: resolve_static("js/analytics.js"),
		defer
	}

	// Favicon
	link {
		rel: "icon",
		href: resolve_static("images/favicon.ico")
	}
});

// Use with view
let view = page!(|| {
	div { class: "app",
		"App content"
	}
})();

// Render with SSR
let mut renderer = SsrRenderer::new();
let html = renderer.render_page_with_view_head(view, my_head);
```

**Benefits of head! macro:**
- SEO-friendly (assets loaded before page render)
- Optimal performance (stylesheets load before content)
- Clean separation of concerns

### Choosing the Right Approach

Use this decision guide to select the appropriate method:

| Scenario | Method | Example | Reason |
|----------|--------|---------|--------|
| Fixed asset path in `page!` | `resolve_static("path")` | `img { src: resolve_static("logo.png") }` | Simple, runtime resolution |
| Dynamic path based on state | `resolve_static(&format!(...))` | `resolve_static(&format!("user_{}.png", id))` | Flexible, data-driven |
| Global assets (CSS/JS) | `head!` with `resolve_static()` | `head!(|| { link { href: resolve_static("app.css") } })` | SEO, performance |
| Server template (index.html) | `{{ static_url("path") }}` | `<script src="{{ static_url('app.js') }}">` | Server-side processing |

**Best Practices:**
1. **Always use `resolve_static()`** - Never hardcode `/static/` URLs
2. **Initialize early** - Call `init_static_resolver()` at app startup
3. **Use manifest in production** - Enables cache busting with hashed filenames
4. **Prefer static paths** - Use string literals when possible for future optimizations

**Common Mistakes:**
```rust
// ❌ BAD: Hardcoded URL
img { src: "/static/images/logo.png" }

// ❌ BAD: Including /static/ prefix
img { src: resolve_static("/static/images/logo.png") }	// Results in /static//static/...

// ✅ GOOD: Relative path without prefix
img { src: resolve_static("images/logo.png") }
```

---


## Styling Options

reinhardt-pages applications support multiple styling approaches. This tutorial covers two main options:

### Option A: UnoCSS (Recommended)

**UnoCSS** is a modern utility-first CSS engine with instant, on-demand styling. It's recommended for new projects due to:
- **Zero build step**: Runtime-based CSS generation
- **Atomic CSS**: Minimal CSS output
- **Theme support**: Dark mode and custom themes out of the box
- **Type-safe**: IntelliSense support with IDE plugins

See [Setting Up UnoCSS](#setting-up-unocss-recommended) section below for implementation.



---

## Setting Up UnoCSS (Recommended)

UnoCSS provides instant, on-demand styling with zero build configuration. This is the recommended approach for production reinhardt-pages applications.

### Step 1: Update index.html

Replace Bootstrap CDN with UnoCSS runtime:

```html
<!DOCTYPE html>
<html lang="en">
<head>
	<meta charset="UTF-8">
	<meta name="viewport" content="width=device-width, initial-scale=1.0">
	<title>Polls App - Reinhardt Tutorial</title>

	<!-- UnoCSS Reset -->
	<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@unocss/reset/tailwind.min.css">

	<!-- UnoCSS Runtime -->
	<script>
	window.__unocss = {
		theme: {
			colors: {
				brand: {
					DEFAULT: '#4a90e2',
					hover: '#357abd',
				},
				success: '#28a745',
				danger: '#dc3545',
				warning: '#ffc107',
			},
		},
		shortcuts: [
			// Buttons
			['btn', 'inline-flex items-center px-4 py-2 rounded-full font-semibold transition-all'],
			['btn-primary', 'btn bg-brand text-white hover:bg-brand-hover'],
			['btn-secondary', 'btn bg-gray-200 text-gray-800 hover:bg-gray-300'],

			// Cards
			['card', 'bg-white rounded-2xl border border-gray-200 shadow-sm'],
			['card-body', 'p-6'],

			// Form
			['form-input', 'w-full px-4 py-3 border border-gray-300 rounded-xl focus:outline-none focus:ring-2 focus:ring-brand'],
			['form-check', 'p-4 border-2 border-gray-200 rounded-xl hover:border-brand hover:bg-blue-50 transition-all cursor-pointer'],

			// Spinner
			['spinner', 'animate-spin rounded-full border-2 border-gray-200 border-t-brand'],
		],
	};
	</script>
	<script src="https://cdn.jsdelivr.net/npm/@unocss/runtime"></script>
</head>
<body class="bg-gray-50 text-gray-900 antialiased">
	<div id="root">
		<div class="flex items-center justify-center min-h-screen">
			<div class="text-center">
				<div class="spinner w-12 h-12 mx-auto mb-4"></div>
				<p class="text-gray-600">Loading...</p>
			</div>
		</div>
	</div>
</body>
</html>
```

### Step 2: Update Component Styles

Replace Bootstrap classes with UnoCSS utilities:

```rust
// UnoCSS styling example
page!(|| {
	div { class: "max-w-4xl mx-auto px-4 mt-12",
		h1 { class: "text-3xl font-bold mb-6", "Polls" }
		button { class: "btn-primary", "Vote" }
	}
})()
```

### Step 3: Common UnoCSS Patterns

Use shortcuts for consistent styling:

```rust
// Question card
div {
	class: "card card-body",
	h1 { class: "text-2xl font-bold mb-4", "Question text" }
}

// Form with radio buttons
div { class: "space-y-3",
	for choice in &choices {
		label {
			class: "form-check",
			input { type: "radio", class: "mr-3" }
			span { "Choice text" }
		}
	}
}

// Submit button
button {
	class: "btn-primary mt-6 w-full",
	type: "submit",
	"Vote"
}

// Loading spinner
div { class: "flex justify-center py-12",
	div { class: "spinner w-8 h-8" }
}
```

**Benefits:**
- **Smaller CSS**: Only generates used utilities
- **Consistency**: Shortcuts ensure uniform styling
- **Dark mode**: Built-in support with `dark:` prefix
- **Responsive**: Easy breakpoints (`md:`, `lg:`, etc.)

For complete UnoCSS configuration examples, see [examples/examples-twitter/index.html](../../../examples/examples-twitter/index.html).

---

## Adding Custom CSS

To customize the appearance beyond Bootstrap's defaults, create a custom stylesheet.

### Step 1: Create Static Directory

```bash
mkdir -p static/css
mkdir -p static/images
```

### Step 2: Create Custom Stylesheet

Create `static/css/polls.css`:

```css
/* Custom polls styling (extends Bootstrap) */

/* Override Bootstrap defaults */
:root {
	--polls-primary: #4a90e2;
	--polls-primary-dark: #357abd;
	--polls-secondary: #6c757d;
	--polls-success: #28a745;
	--polls-danger: #dc3545;
}

/* Custom question card styling */
.question-card {
	background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
	color: white;
	border-radius: 15px;
	padding: 2rem;
	margin-bottom: 2rem;
	box-shadow: 0 10px 20px rgba(0,0,0,0.2);
}

.question-card h1 {
	font-weight: 700;
	margin-bottom: 1.5rem;
}

/* Custom form check styling */
.form-check-custom {
	background-color: #f8f9fa;
	border: 2px solid #e9ecef;
	border-radius: 10px;
	padding: 1rem;
	margin-bottom: 1rem;
	transition: all 0.3s ease;
}

.form-check-custom:hover {
	border-color: var(--polls-primary);
	background-color: #e7f3ff;
	transform: translateY(-2px);
	box-shadow: 0 4px 8px rgba(74, 144, 226, 0.2);
}

.form-check-custom input[type="radio"]:checked + label {
	color: var(--polls-primary);
	font-weight: 600;
}

/* Results visualization */
.result-bar-container {
	background-color: #e9ecef;
	border-radius: 10px;
	overflow: hidden;
	margin-bottom: 1rem;
}

.result-bar {
	background: linear-gradient(90deg, var(--polls-primary), var(--polls-primary-dark));
	color: white;
	padding: 0.75rem 1rem;
	font-weight: 600;
	transition: width 0.5s ease;
}

.result-percentage {
	float: right;
	font-size: 0.9rem;
}

/* Loading spinner customization */
.spinner-custom {
	width: 3rem;
	height: 3rem;
	border-width: 0.3rem;
}

/* Alert customization */
.alert-custom {
	border-radius: 10px;
	border: none;
	box-shadow: 0 2px 8px rgba(0,0,0,0.1);
}

/* Button enhancements */
.btn-custom-primary {
	background: linear-gradient(135deg, var(--polls-primary), var(--polls-primary-dark));
	border: none;
	padding: 0.75rem 2rem;
	font-weight: 600;
	transition: transform 0.2s ease, box-shadow 0.2s ease;
}

.btn-custom-primary:hover {
	transform: translateY(-2px);
	box-shadow: 0 6px 12px rgba(74, 144, 226, 0.3);
}

.btn-custom-primary:active {
	transform: translateY(0);
}

/* Responsive design */
@media (max-width: 768px) {
	.question-card {
		padding: 1.5rem;
	}

	.form-check-custom {
		padding: 0.75rem;
	}
}
```

### Step 3: Reference in index.html

Update `index.html` to include the custom stylesheet:

```html
<!DOCTYPE html>
<html lang="en">
<head>
	<meta charset="UTF-8">
	<meta name="viewport" content="width=device-width, initial-scale=1.0">
	<title>Polls App - Reinhardt Tutorial</title>

	<!-- Custom CSS (if needed) -->
	<link rel="stylesheet" href="/static/css/polls.css">
</head>
<body>
	<div id="root">
		<div class="flex items-center justify-center min-h-screen">
			<div class="text-center">
				<div class="spinner w-12 h-12 mx-auto mb-4"></div>
				<p class="text-gray-600">Loading...</p>
			</div>
		</div>
	</div>
</body>
</html>
```

### Step 4: Configure Static File Serving

The development server automatically serves static files from the `dist/static/` directory when running with `--with-pages`. Create a simple script to copy static files during build:

Add to `Makefile.toml` or your build script:

```bash
# Copy static files to dist directory
mkdir -p dist/static
cp -r static/* dist/static/
```

When you run `cargo make dev`, the static files will be available at `/static/` paths.

**Development workflow:**

```bash
# Build WASM and start development server
cargo make dev

# Or build only
cargo make wasm-build-dev
```

The static files middleware serves files from `dist/` directory, with SPA mode enabled by default for client-side routing support.

## Using Custom Styles in Components

Apply the custom classes in your components:

### Updated Index Page

```rust
// src/client/components/polls.rs
pub fn polls_index() -> View {
	// ... state management

	page!(|questions_list: Vec<QuestionInfo>, loading_state: bool, error_state: Option<String>| {
		div {
			class: "container mt-5",

			if let Some(ref err) = error_state {
				div {
					class: "alert alert-danger alert-custom",
					{err}
				}
			} else if loading_state {
				div {
					class: "text-center",
					div {
						class: "spinner-border text-primary spinner-custom",
						role: "status",
						span {
							class: "visually-hidden",
							"Loading..."
						}
					}
				}
			} else {
				div {
					h1 { class: "mb-4 text-center", "Latest Polls" }

					if questions_list.is_empty() {
						div {
							class: "alert alert-info alert-custom text-center",
							"No polls are available."
						}
					} else {
						div {
							class: "list-group",
							for question in &questions_list {
								a {
									href: format!("/polls/{}/", question.id),
									class: "list-group-item list-group-item-action",
									{&question.question_text}
								}
							}
						}
					}
				}
			}
		}
	})(questions_list, loading_state, error_state)
}
```

### Updated Detail Page with Custom Form Styling

```rust
pub fn polls_detail_page(question_id: i64) -> View {
	// ... state management and event handlers

	page!(|
		question_data: Option<QuestionInfo>,
		choices_data: Vec<ChoiceInfo>,
		form_error_state: Option<String>,
		voting_state: bool,
		selected: Option<i64>,
		handle_submit: impl Fn(web_sys::Event) + 'static,
		handle_choice_change: impl Fn(web_sys::Event) + 'static
	| {
		div {
			class: "container mt-5",

			if let Some(ref q) = question_data {
				// Question card with custom styling
				div {
					class: "question-card",
					h1 { {&q.question_text} }
				}

				if let Some(ref form_err) = form_error_state {
					div {
						class: "alert alert-warning alert-custom",
						{form_err}
					}
				}

				form {
					onsubmit: handle_submit,

					div {
						class: "mb-4",
						for choice in &choices_data {
							div {
								class: "form-check form-check-custom",
								input {
									class: "form-check-input",
									type: "radio",
									name: "choice",
									id: format!("choice{}", choice.id),
									value: choice.id.to_string(),
									onchange: handle_choice_change.clone(),
									checked: selected == Some(choice.id)
								}
								label {
									class: "form-check-label",
									for: format!("choice{}", choice.id),
									{&choice.choice_text}
								}
							}
						}
					}

					button {
						class: "btn btn-custom-primary",
						type: "submit",
						disabled: voting_state,
						if voting_state {
							"Voting..."
						} else {
							"Vote"
						}
					}

					" "
					a {
						href: format!("/polls/{}/results/", q.id),
						class: "btn btn-secondary",
						"View Results"
					}
				}
			}
		}
	})(
		question_data,
		choices_data,
		form_error_state,
		voting_state,
		selected,
		handle_submit,
		handle_choice_change
	)
}
```

### Updated Results Page with Progress Bars

```rust
pub fn polls_results_page(question_id: i64) -> View {
	// ... state management

	page!(|
		question_data: Option<QuestionInfo>,
		choices_data: Vec<ChoiceInfo>,
		total_votes: i32,
		loading_state: bool,
		error_state: Option<String>
	| {
		div {
			class: "container mt-5",

			if let Some(ref q) = question_data {
				// Question card
				div {
					class: "question-card",
					h1 { {&q.question_text} }
					p {
						class: "mb-0",
						"Total votes: " {total_votes.to_string()}
					}
				}

				// Results visualization
				div {
					class: "mt-4",
					for choice in &choices_data {
						let percentage = if total_votes > 0 {
							(choice.votes as f64 / total_votes as f64 * 100.0) as i32
						} else {
							0
						};

						div {
							class: "result-bar-container",
							div {
								class: "result-bar",
								style: format!("width: {}%", percentage),
								span { {&choice.choice_text} }
								span {
									class: "result-percentage",
									{format!("{}% ({} votes)", percentage, choice.votes)}
								}
							}
						}
					}
				}

				// Actions
				div {
					class: "mt-4",
					a {
						href: format!("/polls/{}/", q.id),
						class: "btn btn-primary",
						"Vote Again"
					}
					" "
					a {
						href: "/",
						class: "btn btn-secondary",
						"← Back to Polls"
					}
				}
			}
		}
	})(question_data, choices_data, total_votes, loading_state, error_state)
}
```

## Adding Images

To add images to your application:

### Step 1: Add Image Files

```bash
# Add a logo
cp /path/to/logo.png static/images/logo.png

# Add a background pattern
cp /path/to/pattern.svg static/images/pattern.svg
```

### Step 2: Reference in CSS

Update `static/css/polls.css`:

```css
/* Add background pattern */
body {
	background-image: url('/static/images/pattern.svg');
	background-repeat: repeat;
	background-size: 50px 50px;
}

/* Add logo to question card */
.question-card::before {
	content: '';
	display: block;
	width: 60px;
	height: 60px;
	background-image: url('/static/images/logo.png');
	background-size: contain;
	background-repeat: no-repeat;
	margin-bottom: 1rem;
}
```

### Step 3: Reference in Components (Alternative)

You can also reference images directly in components:

```rust
page!(|| {
	div {
		class: "text-center",
		img {
			src: "/static/images/logo.png",
			alt: "Polls App Logo",
			class: "img-fluid mb-4",
			style: "max-width: 200px"
		}
		h1 { "Welcome to Polls App" }
	}
})()
```

## Building for Production

When building for production, use the release build task with wasm-opt optimization.

### Step 1: Build for Production

```bash
# Build with optimizations
cargo make wasm-build-release

# Output will be in dist/
# - index.html (copied from project root)
# - *.wasm (optimized WASM bundle via wasm-opt -O3)
# - *.js (JS glue code generated by wasm-bindgen)
# - static/ (copied static assets)
```

### Step 2: Copy Static Assets

Ensure your static files are copied to dist:

```bash
# Copy static files to dist
mkdir -p dist/static
cp -r static/* dist/static/
```

For convenience, add this to your `Makefile.toml`:

```toml
[tasks.copy-static]
description = "Copy static files to dist"
script = '''
mkdir -p dist/static
if [ -d "static" ]; then
    cp -r static/* dist/static/
fi
'''
```

### Step 3: Deploy

The `dist/` directory contains all files needed for deployment:

```bash
# Deploy to static hosting (e.g., Netlify, Vercel, GitHub Pages)
cd dist
# Upload to your hosting service

# Or serve with a simple HTTP server
python -m http.server 8080
```

## Static File Organization Best Practices

### Recommended Directory Structure

```
project/
├── static/
│   ├── css/
│   │   ├── polls.css
│   │   └── admin.css
│   ├── images/
│   │   ├── logo.png
│   │   ├── favicon.ico
│   │   └── backgrounds/
│   │       └── pattern.svg
│   ├── fonts/
│   │   └── custom-font.woff2
│   └── icons/
│       └── sprite.svg
├── index.html
├── Makefile.toml
└── src/
    └── ...
```

### Namespacing by Feature

For larger applications, organize by feature:

```
static/
├── common/
│   ├── css/
│   │   └── base.css
│   └── images/
│       └── logo.png
├── polls/
│   ├── css/
│   │   └── polls.css
│   └── images/
│       └── poll-icon.svg
└── admin/
    ├── css/
    │   └── admin.css
    └── images/
        └── admin-icon.svg
```

Reference in `index.html`:

```html
<link rel="stylesheet" href="/static/common/css/base.css">
<link rel="stylesheet" href="/static/polls/css/polls.css">
```

## CDN Integration for Production

For better performance in production, serve static files from a CDN:

### Option 1: Use Existing CDNs

```html
<!-- Use popular CDN services -->
<link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css" rel="stylesheet">
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&display=swap" rel="stylesheet">
```

### Option 2: Deploy to Your Own CDN

Build your WASM and static files:

```bash
# Build optimized WASM
cargo make wasm-build-release

# Copy static assets
cp -r static/* dist/static/
```

After building, upload `dist/` to your CDN:

```bash
# Example using AWS S3
aws s3 sync dist/ s3://your-bucket/polls-app/ --delete

# Or Cloudflare R2
wrangler r2 object put polls-app --file dist/ --recursive
```

Update your `index.html` to use CDN URLs for assets if needed.

## Optimization Techniques

### 1. Image Optimization

Before adding images, optimize them:

```bash
# Install image optimization tools
brew install imageoptim-cli  # macOS
# or
sudo apt-get install imagemagick  # Linux

# Optimize PNG
imageoptim static/images/*.png

# Convert to WebP for better compression
convert static/images/logo.png static/images/logo.webp
```

Use WebP with fallback:

```html
<picture>
	<source srcset="/static/images/logo.webp" type="image/webp">
	<img src="/static/images/logo.png" alt="Logo">
</picture>
```

### 2. CSS Optimization

Use CSS purging to remove unused styles:

```bash
# Install PurgeCSS
npm install -g purgecss

# Purge unused CSS
purgecss --css static/css/polls.css --content src/**/*.rs dist/index.html --output static/css/
```

### 3. Font Loading Optimization

Use `font-display: swap` for custom fonts:

```css
@font-face {
	font-family: 'CustomFont';
	src: url('/static/fonts/custom-font.woff2') format('woff2');
	font-display: swap;  /* Show fallback font immediately */
}
```

### 4. Lazy Loading Images

For images below the fold:

```rust
img {
	src: "/static/images/large-image.jpg",
	loading: "lazy",  // Browser-native lazy loading
	alt: "Description"
}
```

## Collecting Static Files for Production

For production deployments, the `collectstatic` command gathers all static files from your apps and copies them to a single directory for efficient serving.

### What is collectstatic?

The `collectstatic` command:
- Scans all configured static file directories
- Copies files to a central `STATIC_ROOT` directory
- Resolves naming conflicts
- Prepares files for production web servers or CDNs

### Basic Usage

```bash
# Collect all static files to STATIC_ROOT
cargo make collectstatic

# Options:
# --clear: Clear existing files before collecting
# --no-input: Skip confirmation prompts
# --dry-run: Preview what would be collected without actually copying

# Production workflow (non-interactive)
cargo run --bin manage collectstatic --clear --no-input
```

### Configuration

Configure static file settings in your `settings/production.toml`:

```toml
[static]
# URL prefix for static files
static_url = "/static/"

# Directory where collectstatic outputs files
static_root = "./staticfiles"

# Directories to collect from
staticfiles_dirs = [
	"static",                        # Your custom static files
	"node_modules/@unocss/reset",    # UnoCSS reset CSS
]
```

**Configuration Options**:
- `static_url`: URL prefix for accessing static files (default: `/static/`)
- `static_root`: Absolute path to output directory (required for production)
- `staticfiles_dirs`: List of directories to collect from (optional)

### Production Workflow

A typical production deployment workflow:

**1. Development**: Serve static files directly

```bash
# WASM projects
cargo make dev

# Traditional projects
cargo make runserver
```

**2. Build for Production**:

```bash
# Build optimized WASM (for reinhardt-pages projects)
cargo make dev-release

# Collect static files
cargo run --bin manage collectstatic --no-input

# Output: All static files copied to ./staticfiles/
```

**3. Deploy**: Configure your web server to serve `staticfiles/` directory

**Nginx Configuration Example**:
```nginx
server {
	listen 80;
	server_name example.com;

	# Serve static files
	location /static/ {
		alias /path/to/your/app/staticfiles/;
		expires 1y;
		add_header Cache-Control "public, immutable";
	}

	# Proxy API requests to Reinhardt
	location / {
		proxy_pass http://127.0.0.1:8000;
	}
}
```

### Best Practices

**For CDN Deployment**:
- ✅ Use CDN for UnoCSS runtime in production (faster delivery, better caching)
- ✅ Upload `staticfiles/` to your CDN after running collectstatic
- ✅ Update `STATIC_URL` to point to CDN URL

**For Performance**:
- ✅ Version your static files (cache busting) - add version parameter to URLs
- ✅ Compress static files (gzip/brotli) - reduce bandwidth
- ✅ Set far-future cache headers for immutable files - reduce server requests
- ✅ Use WebP images with fallback - better compression than PNG/JPEG

**For Automation**:
- ✅ Run collectstatic in CI/CD pipeline before deployment
- ✅ Use `--no-input` flag in automated scripts
- ✅ Verify file count after collection

**Security**:
- ❌ Never commit `staticfiles/` to version control (add to `.gitignore`)
- ❌ Never serve `staticfiles/` from development server (use cargo make dev)

### Troubleshooting

**Issue**: "STATIC_ROOT setting is not configured"
```toml
# Solution: Add to settings/production.toml
[static]
static_root = "./staticfiles"
```

**Issue**: Files not found after collectstatic
```bash
# Check what was collected
cargo run --bin manage collectstatic --dry-run

# Verify STATIC_ROOT exists
ls -la ./staticfiles/
```

**Issue**: Naming conflicts between files
```bash
# collectstatic will warn about duplicate file names
# Resolution: Rename files or use namespaced directories
static/
├── app1/
│   └── style.css
└── app2/
	└── style.css  # Different namespace, no conflict
```

---

## Summary

In this tutorial, you learned:

- **Static File Management in reinhardt-pages**: Different from traditional server-rendered approaches
- **Bootstrap Integration**: Using CDN for common libraries
- **Custom CSS**: Creating and referencing custom stylesheets
- **Build Configuration**: Using `cargo make` for WASM builds and static file handling
- **Component Styling**: Applying custom styles in reinhardt-pages components
- **Image Assets**: Adding and optimizing images
- **Production Build**: Optimizing for production with wasm-opt
- **CDN Integration**: Serving static files from CDNs for better performance
- **Optimization Techniques**: Image optimization, CSS purging, font loading, lazy loading

**Key Differences from Traditional Approaches:**

| Aspect | Traditional (Server-Rendered) | reinhardt-pages |
|--------|-------------------|-----------------|
| Asset Reference | `{{ 'file.css'\|static }}` tag | Direct URL in `index.html` |
| Build Tool | `collectstatic` command | `cargo make wasm-build-*` tasks |
| Processing | Server-side collection | WASM bundling + wasm-pack |
| Deployment | Separate static file server | Single `dist/` directory |
| Optimization | Manual configuration | wasm-opt for WASM optimization |

Your polls app now has a clean, professional appearance with custom styling!

## What's Next?

In the final tutorial, we'll explore the Reinhardt admin interface and learn how to customize it for managing poll data. Note that the admin panel uses a different rendering approach, so concepts from this tutorial will be adapted accordingly.

Continue to [Part 7: Admin Customization](../7-admin-customization/).
