# reinhardt-admin Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-admin` serves a privileged administrative UI and its server
functions, generated clients, static assets, import/export facilities, and
WASM SPA. Admin routes, form values, record identifiers, files, and browser
state are attacker-controlled until the server validates them.

## Security Invariants

- Every privileged read, mutation, bulk action, import, export, and custom
  action enforces server-side model and operation permission. Applications
  requiring object or tenant isolation must perform a per-target authorization
  check before each operation; the current admin permission hooks are not
  object-aware. Client, WASM, and generated-client state is display state only.
- Cookie-authenticated mutations preserve CSRF protection. Security-sensitive
  identifiers, ownership, tenant, role, permission, credential, and read-only
  fields cannot be changed through forms, inline edits, or alternate requests
  unless the server explicitly authorizes that operation.
- Import and export validate formats and data, bound work, preserve the
  caller's authorized scope, and must neutralize spreadsheet formula prefixes
  before CSV/TSV values are opened by spreadsheet software. Protected
  applications must also apply the selected `ModelAdmin` field allowlist,
  read-only, ownership, and tenant checks to every imported record;
  `import_data` does not independently apply `create_record` mutation
  validation. CSV quoting alone does not prevent formula interpretation;
  rendered values use context-appropriate escaping.
- Static, uploaded, generated, and vendor asset paths remain confined to their
  configured asset roots. Deployments enabling remote executable or render-
  active vendor assets must provide verified integrity values before download
  or serving; an empty integrity value is an explicitly unverified bootstrap,
  not a trusted update.
- Native, WASM, generated-client, and direct server-function paths enforce
  equivalent permissions and never let a client-side route or UI check replace
  server authorization.

## Reportable Findings

Report admin authorization or CSRF bypass, protected-field mass assignment,
unsafe import/export or rendered values, asset confinement escape, unverified
remote active vendor assets, or weaker native/WASM/generated-client behavior.
