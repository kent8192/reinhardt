# reinhardt-i18n

Internationalization and localization support for Reinhardt, inspired by Django's i18n framework.

## Overview

Framework for translating applications into multiple languages with Django-style gettext API.

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.25", features = ["i18n"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-rc.25", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-rc.25", features = ["full"] }      # All features
```

Then import i18n features:

```rust
use reinhardt::i18n::{gettext, ngettext, pgettext};
use reinhardt::i18n::{activate, deactivate, MessageCatalog};
```

**Note:** I18n features are included in the `standard` and `full` feature presets.

## Features

### Implemented ✓

#### Message Translation

- **Simple translation** (`gettext`): Basic message translation with fallback support
- **Lazy translation** (`gettext_lazy`): Deferred translation evaluation for compile-time definitions
- **Fallback mechanism**: Automatic fallback to default locale when translation is missing

#### Plural Forms Support

- **Plural translation** (`ngettext`): Language-aware plural form handling
- **Lazy plural translation** (`ngettext_lazy`): Deferred plural translation evaluation
- **Plural form rules**: Configurable plural form index calculation based on count

#### Context-Aware Translations

- **Contextual translation** (`pgettext`): Disambiguate translations with context (e.g., "File" as menu vs. verb)
- **Contextual plural translation** (`npgettext`): Context-aware plural form handling

#### Message Catalog Management

- **MessageCatalog**: In-memory storage for translations per locale
- **Simple translations**: Key-value translation pairs
- **Plural translations**: Multiple plural forms per message
- **Contextual translations**: Context-based message disambiguation
- **Contextual plural translations**: Combined context and plural support

#### Locale Management

- **Locale activation** (`activate`): Set active locale with associated catalog
- **Locale deactivation** (`deactivate`): Revert to default English locale
- **Locale query** (`get_locale`): Retrieve currently active locale
- **Thread-local state**: Per-thread translation state using `thread_local!` with `RefCell` (not a global `RwLock`; each OS thread maintains its own active locale independently)

#### Lazy Evaluation

- **LazyString**: Deferred translation evaluation
- **Display trait**: Automatic evaluation when displayed
- **String conversion**: Seamless conversion to String type
- **Plural support**: Lazy evaluation for plural translations
- **Context support**: Lazy evaluation for contextual translations
