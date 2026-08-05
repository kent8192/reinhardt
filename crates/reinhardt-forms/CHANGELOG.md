# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.3.2...reinhardt-forms@v0.4.0-alpha.1) - 2026-07-21

### Changed

- [**breaking**] remove remaining dynamic error dependencies

### Fixed

- *(release)* restore develop prerelease lifecycle

### Maintenance

- migrate dependency policy checks to cargo-deny
- merge develop/0.4.0 into remove-anyhow branch

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
