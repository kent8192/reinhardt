# reinhardt-commands Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-commands` parses management commands, generates projects and apps,
loads templates and plugins, collects static assets, reloads development
processes, and emits project introspection. Ordinary trusted local developer
power is not automatically reportable. Names, templates, archives, plugin
artifacts, static files, generated paths, watcher events, and introspection
inputs nevertheless become untrusted when they cross a safe command boundary.

## Security Invariants

- Child processes use structured program and argument vectors. Filenames,
  template values, watcher events, and file contents never become shell syntax,
  command fragments, environment assignments, or option injection.
- Project and app names, templates, static inputs, and generated files are
  validated and confined to their configured project, template, source, and
  output roots after normalization and symlink resolution. They cannot select
  arbitrary host paths or overwrite files outside their intended destination.
- Archive, plugin, and template extraction validates entry names, types, sizes,
  links, and destination paths before writing. Traversal, absolute paths,
  symlink or hard-link escape, and overwrite outside the extraction root fail
  safely.
- `collectstatic` preserves source and destination confinement when copying or
  linking. Symlinks are created only when their resolved target and destination
  remain within the authorized static roots; unsafe links are rejected rather
  than copied or followed.
- Reload builds and executes only structured, validated commands. File names,
  paths, and changed file contents cannot inject commands into rebuild, restart,
  logging, or browser-reload paths.
- Introspection output and errors omit secrets, credentials, signed URLs,
  private connection details, and other sensitive settings, including nested or
  backend-derived values.
- Generated secrets use a cryptographically secure operating-system random
  source with sufficient entropy for their purpose. Predictable randomness,
  timestamps, identifiers, or formatting alone are not secret generation.

## Reportable Findings

Report injection through a safe command boundary, path or extraction escape,
unsafe static symlink handling, reload injection, secret disclosure through
introspection, or predictable generated secrets. Explicit local shell use and
other documented trusted developer operations are out of scope unless a safe
command API lets untrusted input cross into them.
