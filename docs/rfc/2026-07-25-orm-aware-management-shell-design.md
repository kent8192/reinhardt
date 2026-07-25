# ORM-Aware Management Shell

## Status

- Issue: [#5803](https://github.com/kent8192/reinhardt-web/issues/5803)
- Target branch: `develop/0.4.0`
- Change class: breaking management-command change
- Selected evaluator: embedded `evcxr`

## Summary

Replace the Rhai-backed `manage shell` implementation with a stateful Rust
evaluation context that loads the application crate, initializes its settings,
ORM connection, and dependency-injection context, and imports its installed
models. The interactive shell and `shell --command` use the same evaluator and
bootstrap path.

The new shell is available through an explicit `commands-shell` facade feature.
Projects that do not enable it receive an actionable non-zero error instead of
the current warning-and-success behavior. The obsolete `shell-rhai` feature and
Rhai dependency are removed.

## Goals

1. Evaluate normal, type-checked Rust against the application crate.
2. Preserve variables, functions, and type definitions between successful
   interactive inputs.
3. Support top-level asynchronous ORM and DI operations.
4. Expose the composed project settings, an ORM connection handle, and an
   application-level DI context through stable shell bindings.
5. Import every uniquely named `#[model]` type from `INSTALLED_APPS`.
6. Keep colliding model names available through fully qualified Rust paths.
7. Make `shell -c` execute exactly one snippet and return non-zero on any
   compilation, bootstrap, evaluation, or runtime failure.
8. Keep evaluator subprocesses, output workers, and database registrations
   scoped through RAII guards.
9. Keep evaluator artifacts outside the project directory and reuse Cargo's
   normal build cache where possible.

## Non-Goals

- Implementing `dbshell`.
- Connecting to a running production server process or reusing its in-memory
  DI state.
- Adding a remote shell, browser shell, Jupyter integration, or IDE protocol.
- Building a second query language or dynamically binding the ORM to Rhai.
- Adding completion behavior beyond what the selected `evcxr` version already
  provides.
- Matching Python interpreter startup latency.

## Design Decisions

### Use one `evcxr` evaluation context

Interactive input is evaluated in a single `evcxr::EvalContext`. Successful
definitions remain available to later inputs. An input that fails compilation
or returns an error is not committed to the context, so it does not invalidate
previous successful state.

`shell -c` creates the same context, runs the same dependency and bootstrap
prelude, evaluates one user snippet, prints its output and final expression,
and exits. There is no separate compile-and-run implementation for command
mode.

### Keep the feature opt-in

The `commands-shell` facade feature enables the `reinhardt-commands/shell`
feature and its `evcxr` and line-editing dependencies. Generated projects do
not enable it by default because the evaluator has a material dependency and
build-time cost.

The `shell-rhai` feature is removed. `shell` is redefined to mean the Rust
evaluator. When the feature is unavailable, selecting `manage shell` returns a
non-zero error that names `commands-shell` and shows the dependency feature
needed to enable it.

### Bootstrap inside the evaluator subprocess

An `evcxr` evaluation subprocess cannot safely reuse typed settings, database
handles, or DI state from the parent `manage` process. It loads the application
crate as a path dependency and calls the configured project settings factory
inside the evaluation process.

The bootstrap creates a hidden `ShellEnvironment<S>` that owns:

- the concrete project settings value;
- an RAII guard for the shell's global ORM registration;
- a copy-safe `DatabaseConnection` handle;
- an `Arc<InjectionContext>` built with the application provider registry.

The generated prelude exposes:

```rust
let settings = __reinhardt_shell.settings();
let db = __reinhardt_shell.database();
let di = __reinhardt_shell.di_context();
```

The hidden environment remains alive for the evaluation context's lifetime.
Dropping it restores any previous global ORM registration and releases the
shell-owned database lease.

### Discover model Rust paths at link time

Extend the existing link-time `ModelMetadata` emitted by `#[model]` with the
model's fully qualified Rust type path. The macro derives the path from
`module_path!()` and the declared type name; users do not maintain a second
model list for the shell.

`ShellConfig` supplies the labels from the project's generated
`InstalledApp::all_labels()` method. The import planner filters registered
models to those labels, groups them by Rust type name, and sorts all output for
deterministic diagnostics.

- A name that appears once is imported unqualified.
- A name that appears more than once is not imported.
- Every collision produces one startup warning listing the available fully
  qualified paths.

A collision is not a bootstrap failure.

## Architecture

### Shell command adapter

`ShellCommand` remains the built-in CLI command, but only parses its `command`
option and delegates to a shell driver in a focused
`reinhardt-commands/src/shell.rs` module. The driver owns evaluator creation,
prelude assembly, input handling, output forwarding, restart behavior, and
resource guards.

This removes evaluator details from the already large `builtin.rs` module.

### Project shell configuration

Generated projects add `config::shell::get_shell_config()`. A `ShellConfig`
contains:

- Cargo package name;
- Rust crate name;
- absolute manifest directory from `env!("CARGO_MANIFEST_DIR")`;
- installed application labels;
- the fully qualified project settings factory path;
- optional project-specific Rust prelude text.

The generated `manage.rs` uses a new entry point:

```rust
execute_from_command_line_with_settings_and_shell(
	get_settings(),
	get_shell_config(),
)
.await
```

Existing settings-only and registry entry points remain available. If one of
them dispatches `shell` without a `ShellConfig`, the command fails with the
configuration migration message.

### Runtime hook

The generated native `manage` entry point calls a facade-provided shell runtime
hook before creating the Tokio runtime or parsing management-command
arguments. With `commands-shell` disabled, the hook is a no-op. With it enabled,
the hook delegates to `evcxr::runtime_hook()` so evaluator subprocesses cannot
recursively execute the management CLI.

### Prelude layers

The evaluator applies prelude layers in this order:

1. Add the project package as a path dependency.
2. Import the Reinhardt prelude and shell support types.
3. Import uniquely named installed models.
4. Evaluate the settings factory and common shell bootstrap.
5. Bind `settings`, `db`, and `di`.
6. Evaluate the optional project-specific prelude.

The prompt is shown only after every layer succeeds.

### Input and output adapters

The interactive driver depends on small input and evaluator interfaces rather
than directly coupling control flow to a terminal. Production uses the line
editor and `evcxr`; tests use deterministic adapters.

Incomplete Rust input switches from `>>> ` to a continuation prompt and is
evaluated only after the snippet is complete. `exit` and `quit` are commands
only when there is no pending multiline snippet.

The `EvalContextOutputs` stdout and stderr receivers are continuously drained
by owned workers. Their guards stop and join the workers when the session ends,
preventing a full output channel from deadlocking evaluation.

## Execution Flow

### Interactive mode

1. Validate `ShellConfig` and the project manifest.
2. Create an evaluator and its output workers.
3. Apply dependency, import, bootstrap, and project prelude layers.
4. Print collision warnings and the startup banner.
5. Read a complete snippet.
6. Evaluate it in the existing context.
7. Print output and the final expression.
8. Repeat until `exit`, `quit`, or EOF.
9. Drop the session guard, evaluator, output workers, and shell environment.

Successful evaluation commits definitions to the context. Compilation and
ordinary evaluation errors leave the previous context intact.

### Command mode

1. Perform the same validation and bootstrap steps.
2. Evaluate the exact `--command` value once.
3. Forward stdout, stderr, and the final expression without echoing the source.
4. Return success only when bootstrap and evaluation both succeed.
5. Drop all session resources before returning.

The management binary maps every returned command error to a non-zero process
exit.

## Error and Interruption Semantics

### Startup failures

Missing shell configuration, invalid paths, dependency-resolution failures,
prelude compilation failures, settings errors, database failures, and DI
bootstrap failures prevent the prompt from opening. Diagnostics identify the
failing phase but do not print secrets or unsanitized database URLs.

### Recoverable evaluation failures

Compilation errors, type errors, and errors propagated with `?` are
recoverable in interactive mode. The failing input is displayed with the
evaluator diagnostic, prior successful state remains available, and the prompt
continues.

The same failures in command mode return a command error.

### Context-resetting failures

A user-code panic, evaluator subprocess exit, or interruption during evaluation
may leave evaluator-owned state inconsistent. Interactive mode discards that
context, creates a new one, reapplies every prelude layer, and reports that
user-defined state was cleared.

If rebootstrap fails, the shell exits with the rebootstrap error. Command mode
does not restart; it returns the original failure.

Ctrl+C while reading input clears the current input buffer and returns to the
primary prompt. Ctrl+C while evaluating interrupts the subprocess and follows
the context-reset path.

### Normal exit

EOF, `exit`, and `quit` return success after cleanup. EOF with an incomplete
multiline snippet emits one warning that the pending input was discarded.

Cleanup errors never replace an earlier startup or evaluation error. When no
earlier error exists, a cleanup failure is returned as the command error.

## Security and Artifact Handling

- The source passed through `--command` is never repeated in informational
  logs because it may contain credentials or personal data.
- Database diagnostics reuse credential-redaction behavior.
- Evaluator diagnostics may contain project-relative source paths, but
  framework-generated messages do not expose unrelated absolute paths.
- Temporary evaluator files use the operating-system temporary or cache
  directory, never the project tree.
- Session cleanup is represented by `Drop`-based guards. There is no required
  public `close`, `release`, or `cleanup` call.
- The shell executes arbitrary Rust with the invoking user's permissions. The
  documentation states this explicitly and does not present the shell as a
  sandbox.

## Testing Strategy

### Unit tests

Import-planner tests cover:

- filtering by installed application label;
- unique unqualified imports;
- collision exclusion and fully qualified warnings;
- deterministic ordering;
- empty registries and apps without models.

Session-driver tests use fake input and evaluator adapters to cover:

- state carried across successful inputs;
- multiline input;
- rollback of one failed input;
- EOF and explicit exit;
- input interruption;
- evaluator restart and rebootstrap after panic;
- non-masking cleanup failures.

Configuration and diagnostic tests cover invalid manifests, absent
`ShellConfig`, feature-disabled errors, source non-echoing, and credential
redaction.

### Real evaluator integration tests

A small Reinhardt fixture crate is loaded into a real `evcxr` context. Tests
verify:

- project path dependency resolution;
- project and model imports;
- top-level `await` and `?`;
- final-expression output;
- successful state reuse;
- compile-error propagation;
- settings, database, and DI bindings.

### Generated-project end-to-end tests

Generate a project in a temporary directory and use SQLite to verify:

- `shell -c` can create and query a real model;
- `settings` exposes the project's composed configuration;
- `di` resolves a registered provider;
- invalid Rust and failed ORM operations return non-zero;
- interactive adapter input can bind a query result and inspect it in a later
  input;
- two installed apps with the same model name require qualified paths.

Every fixture, database, and generated project is owned by a temporary-directory
guard and removed after the test.

## Documentation and Migration

Update:

- `reinhardt-commands` crate documentation and README;
- facade feature documentation for `commands-shell`;
- generated project templates;
- management-command documentation;
- the `develop/0.4.0` migration guide.

The migration guide states:

1. Remove `shell-rhai`.
2. Add `commands-shell` when the management shell is required.
3. Add `config::shell::get_shell_config()`.
4. Call the settings-and-shell command-line entry point.
5. Call the shell runtime hook at the beginning of native `manage`.

Documentation includes examples using `settings`, `db`, `di`, unqualified
unique models, and qualified colliding models. It also explains that cold start
may compile the project and evaluator support, while warm start can reuse Cargo
artifacts.

## Compatibility

Removing `shell-rhai` and changing the meaning of `shell` are intentional
breaking changes for the `0.4.0` development line. No compatibility adapter is
provided for Rhai snippets because the old evaluator exposed no meaningful ORM
or project API.

Projects that do not use `manage shell` may keep their existing command-line
entry point. Projects that enable `commands-shell` must supply `ShellConfig`;
the explicit failure for missing configuration prevents silent partial
behavior.
