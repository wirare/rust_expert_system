# Expert System (Rust)

A backward-chaining expert-system engine with a native desktop GUI.

## Desktop GUI

```bash
cargo run --release --bin gui
```

The GUI opens a native window with an **Open .txt** file picker, an editable
rules view, a **Parallel** checkbox, and a **Run engine** button. It shows the
result for each queried symbol: `True`, `False`, `Undetermined`,
`Contradiction`, or an explicit query error. Invalid source files produce an
error message (with a source line when available), rather than misleading
logical results.

You can edit the text in the rules pane and run it again; those edits do
**not** overwrite the opened file. The next run uses the current editor
contents. Opening another file or editing the current source clears old
results. Input and mode controls are disabled while a run is in progress.

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
    document.rs      file loading and source text
    runner.rs        background solver execution
    views.rs         rules editor and query results
```

This first GUI version does **not** generate or render graphs, or load traces.
The UI only consumes owned results from `engine::run_source`. The solver runs
inside the worker using an owned input snapshot, keeping its borrowed AST
lifetimes out of the UI state. Future trace/graph views can be added as
independent GUI modules without coupling inference and rendering.
