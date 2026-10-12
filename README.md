# Expert System (Rust)

A backward-chaining expert-system engine with a native desktop GUI.

## Desktop GUI

```bash
cargo run --release --bin gui
```

The GUI opens a native window with an **Open .txt** file picker, a
**scrollable, editable** rules view, **Save / Save As**, a **Parallel** checkbox,
and a **Run engine** button. It displays each queried symbol as `True`,
`False`, `Undetermined`, `Contradiction`, or a query error. Invalid
source files display an error with the source line when possible.

Editing the rules marks the document **Modified**. **Save** writes the editor
text to its existing path (or asks for a path for an untitled document).
**Save As** chooses a new path. Failed or cancelled saves keep unsaved text
intact. Opening another file when the editor is modified asks for
confirmation before discarding those edits.

**Initial facts** are displayed as A-Z checkboxes. They default to the
source file's `=...` declaration. Uncheck a fact to disable it, or check
another symbol to enable it for the next run. Checkboxes affect **both
sequential and parallel runs** without changing the editor, the original
`=...` line, or the contents written by Save. **Reset to source** drops
all runtime overrides. If you edit the `=...` line, source defaults update
while intentional checkbox overrides remain in effect until reset.

Opening another file or editing the current source resets the results.
Changing a fact toggle also clears old results. Input and mode controls are
disabled while a run is in progress.

On Ubuntu, eframe/rfd may require the usual desktop development packages,
including `libgtk-3-dev`, `libxkbcommon-dev`, and `pkg-config`.
Rust 1.95+ is required for the project's current egui/eframe dependencies.

## Command-line interface

The CLI remains the default binary:

```bash
cargo run --release -- test.txt
cargo run --release -- test.txt --report reasoning.json
cargo run --release -- test.txt --report-pretty reasoning.json
```

## Code structure

```text
src/
  lib.rs             shared library module exports
  main.rs            existing CLI
  bin/gui.rs         native GUI executable
  engine.rs          UI-independent source -> owned query results adapter
  system.rs          parser, inference engine, sequential/parallel results
  trace.rs           execution trace types
  report.rs          report / graph serialization types
  gui/
    mod.rs           GUI exports
    app.rs           UI composition and controls
    document.rs      file loading, saving, and source text
    facts.rs         file-derived fact defaults and runtime overrides
    runner.rs        background solver execution
    views.rs         scrolling editor, fact controls, and results
```

This first GUI version does **not** generate or render graphs, or load traces.
The UI only consumes owned results from `engine::run_source`. The solver runs
inside the worker using an owned input snapshot, keeping its borrowed AST
lifetimes out of the UI state. Future trace/graph views can be added as
independent GUI modules without coupling inference and rendering.
