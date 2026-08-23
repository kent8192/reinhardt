# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.4.0-alpha.6...reinhardt-forms@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.3
- update version references to v0.3.4
- update version references to v0.3.5
- update version references to v0.3.6
- update version references to v0.3.7
- update version references to v0.3.8

### Fixed

- *(forms)* repair password and string key validation
- *(forms)* ignore ordering for multiple choices
- *(forms)* isolate prefixed form submissions
- *(forms)* preserve prefixed bound data
- *(forms)* preserve cleaned field semantics
- *(forms)* preserve cleaned field state after validation
- *(forms)* preserve choice value type distinctions
- *(forms)* redact sensitive bound values independently of widget

### Maintenance

- merge main into develop/0.4.0

### Testing

- *(forms)* raise coverage to 80%
- *(forms)* split coverage by component
- *(forms)* cover prefixed model choice submissions
- *(forms)* split advanced field metadata coverage
- *(forms)* cover redaction with custom widgets and later errors

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.4.0-alpha.3...reinhardt-forms@v0.4.0-alpha.6) - 2026-08-06

### Added

- *(forms)* [**breaking**] make generated model forms async

### Documentation

- *(forms)* document generated model forms
- *(forms)* correct facade and formset guidance
- update version references to v0.4.0-alpha.4
- update version references to v0.4.0-alpha.5
- *(release)* restore coherent alpha.3 references

### Fixed

- *(forms)* enforce model form persistence invariants
- *(forms)* align model form automatic defaults
- *(forms)* harden generated form persistence semantics
- *(forms)* close residual model form review gaps
- *(forms)* expose model validation errors
- *(forms)* retain fractional time precision
- *(forms)* synchronize replacement field values
- *(forms)* defer inline parent key validation
- *(forms)* make direct model saves insert explicitly
- *(forms)* preserve native model field values
- *(forms)* enforce generated model constraints
- *(forms)* preserve model form field contracts
- *(forms)* preserve specialized field constraints
- *(forms)* preserve exact generated constraints
- *(forms)* complete model-backed form submission
- *(forms)* harden native model form decoding
- *(forms)* preserve model form defaults
- *(forms)* harden model form input handling
- *(forms)* preserve untouched model controls
- *(forms)* validate model form submission boundaries
- *(forms)* validate inline and runtime model form state
- *(forms)* prevalidate inline foreign keys
- *(forms)* preserve model-backed form state
- *(forms)* preflight deferred child validators
- *(forms)* prevent duplicate MySQL form inserts
- *(forms)* preserve native range defaults
- *(forms)* prevent duplicate create retries
- *(forms)* defer uncertain generated keys
- *(forms)* synchronize defaults and persistence state
- *(forms)* synchronize transaction-backed form state
- *(forms)* preserve transactional retry semantics
- *(forms)* support trusted inline foreign keys
- *(forms)* preserve inline uncertain create state
- *(forms)* preserve model form control semantics
- *(forms)* preserve nested form retries
- *(forms)* validate inline formset retries
- *(forms)* align native form validation
- *(forms)* use serde-json for trusted fields
- *(forms)* preserve trusted non-editable model values
- *(forms)* address model form review feedback
- *(release)* break forms facade publish cycle
- *(release)* restore unpublished crates after partial release

### Testing

- *(forms)* cover uncertain insert persistence state

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.2...reinhardt-forms@v0.4.0-alpha.1) - 2026-07-21

### Changed

- [**breaking**] remove remaining dynamic error dependencies

### Fixed

- *(release)* restore develop prerelease lifecycle

### Maintenance

- migrate dependency policy checks to cargo-deny
- merge develop/0.4.0 into remove-anyhow branch
## [0.3.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.9...reinhardt-forms@v0.3.10) - 2026-08-22

### Maintenance

- update Cargo.toml dependencies

## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.8...reinhardt-forms@v0.3.9) - 2026-08-21

### Maintenance

- update Cargo.toml dependencies

## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.7...reinhardt-forms@v0.3.8) - 2026-08-16

### Maintenance

- update Cargo.toml dependencies

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.6...reinhardt-forms@v0.3.7) - 2026-08-12

### Fixed

- *(forms)* repair password and string key validation
- *(forms)* ignore ordering for multiple choices
- *(forms)* isolate prefixed form submissions
- *(forms)* preserve prefixed bound data
- *(forms)* preserve cleaned field semantics
- *(forms)* preserve cleaned field state after validation
- *(forms)* preserve choice value type distinctions
- *(forms)* redact sensitive bound values independently of widget

### Testing

- *(forms)* raise coverage to 80%
- *(forms)* split coverage by component
- *(forms)* cover prefixed model choice submissions
- *(forms)* split advanced field metadata coverage
- *(forms)* cover redaction with custom widgets and later errors

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.5...reinhardt-forms@v0.3.6) - 2026-08-04

### Maintenance

- update Cargo.toml dependencies

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.4...reinhardt-forms@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.3...reinhardt-forms@v0.3.4) - 2026-07-30

### Maintenance

- update Cargo.toml dependencies

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.2...reinhardt-forms@v0.3.3) - 2026-07-28

### Maintenance

- update Cargo.toml dependencies

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.1...reinhardt-forms@v0.3.2) - 2026-07-14

### Fixed

- *(ci)* allow intentional dependency-version duplicates

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.2.2...reinhardt-forms@v0.3.0) - 2026-06-28

Stable release of `reinhardt-forms` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Fixed

- *(todo-check)* clear public api audit markers

### Maintenance

- merge main into develop/0.3.0

## [0.2.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.2.1...reinhardt-forms@v0.2.2) - 2026-06-25

### Documentation

- update version references to v0.2.1

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.3...reinhardt-forms@v0.2.0) - 2026-06-11

Stable release of `reinhardt-forms` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Route form UI through `use_form` and form definitions; update typed field generics where server functions expect non-string values.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- *(ci)* recover develop release-plz prerelease

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-rc.30...reinhardt-forms@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-forms` as part of the
reinhardt-web 0.1.0 release. This crate provides Django-style form
handling and validation primitives — fields, widgets, validators,
and `ModelForm` — used by both the `form!` macro in
`reinhardt-pages` and direct server-side consumers.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Typed form fields** — `CharField`, `TextField`, `EmailField`,
  `IntegerField`, `DecimalField`, `BooleanField`, `DateField` /
  date-time variants, `FileField` / `ImageField`, and `SlugField`,
  each implementing the unified `FormFieldProperty` surface with
  `Debug` and `Clone` derives.
- **Widget library** — `TextInput`, `PasswordInput`, `Select`,
  `RadioSelect`, `MultiSelect`, `Textarea`, file widgets, and a
  `SelectDateWidget` whose year range is computed dynamically
  (no hard-coded years).
- **Built-in validators** — `UrlValidator`, `SlugValidator`,
  required / min-length / max-length / pattern, with regex
  caches behind `LazyLock` for the URL and email patterns.
- **Security defaults** — file-size limits on uploads, path
  traversal validation on file fields, HTML escaping in
  `Widget::render_html`, removal of SVG from default image
  extensions to prevent stored XSS, constant-time CSRF token
  comparison, and password plaintext-storage prevention in
  validator error sanitisation.
- **`ModelForm` integration** — typed bridge between
  `#[model]` types and form rendering / save, with explicit
  error handling on save (no panics).

### Notable Breaking Changes

`reinhardt-forms` itself ships no end-user breaking changes at
0.1.0; its surface stabilises around the `form!` macro in
`reinhardt-pages`. For the macro-level breaking changes that
affect form authoring (closure lifts, `Send + Sync` requirement,
unified validators), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
and the [reinhardt-pages-macros CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/crates/reinhardt-pages/macros/CHANGELOG.md).

### Migration Notes

- Replace inline regex-based validation with the cached
  `UrlValidator` / email validator constants; downstream code that
  recompiled these patterns per call now has a no-op upgrade path.
- For the workspace-wide migration narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
