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
all runtime overrides. Toggling any fact or pressing Reset immediately re-runs
the engine in the currently selected sequential/parallel mode, updating the
results without requiring **Run engine**. If you edit the `=...` line, source
defaults update while intentional checkbox overrides remain in effect until reset.

Opening another file or editing the current source resets the results.
Changing a fact toggle immediately starts a fresh run. Input and mode controls
are disabled while a run is in progress.

### Reasoning graph — explanation-first view

After running the engine, open **Reasoning graph** and select a queried symbol.
The default view now shows **why the engine returned that answer** instead of
drawing every recursive search attempt:

```text
Given fact A  -->  Applied rule A => B  -->  Derived B  -->  Query B = TRUE
```

- **Colour-coded, fixed-position cards** identify initial facts, applied
  rules, OR constraints, proven literals, the query result, and explanatory
  notices. Cards contain a readable title and short description.
- **Arrows point from evidence to its consequence.** A rule above its
  conclusion follows its own horizontal row before entering the destination
  card from the top; a rule below enters it from the bottom. Direct neighbors
  connect left-to-right. An obstacle-aware detour is used only when another
  card blocks a route. The selected node's incoming and outgoing links are
  highlighted; their meanings are listed in the inspector.
- **Click a card** to inspect its normalized rule, original rule and source
  line, premise evaluation, signed literal, proof origin, related causal event
  IDs, connections, and underlying trace event. You can click its linked
  neighbours to navigate the explanation.
- **The inspector stays within the window**, wraps long rules and errors, and
  scrolls vertically independently from the diagram.
- Nodes **cannot be dragged, repositioned, or edited**. The graph can be
  panned with scrollbars and zoomed between 70% and 145%.
- A contradictory result has **separate positive and negative proof paths**.
  If the engine falls back to False without a negative proof, an explanatory
  notice explicitly states that default False is not a proven negation.
  Unresolved OR constraints are linked with a **dashed, non-proof** arrow.
- The canvas is an explanation extracted from causal trace dependencies.
  Exploratory, unsuccessful search attempts are intentionally omitted from
  the default graph, but remain in the full trace held by the run result.

The layout still uses the already-present **petgraph** library, with cycle-safe
layering. It is computed once per query/run and reused on later UI frames.
Runtime fact toggles regenerate the explanation with the new initial facts.

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
    app.rs           workspace tabs, UI composition and controls
    document.rs      file loading, saving, and source text
    facts.rs         file-derived fact defaults and runtime overrides
    runner.rs        background solver execution
    views.rs         scrolling editor, fact controls, and results
    graph/
      mod.rs         graph workspace exports
      model.rs       causal proof extraction from the engine trace
      layout.rs      petgraph deterministic proof-stage layout
      view.rs        read-only evidence canvas and query selector
      details.rs     wrapped, scrollable proof inspector
```

The GUI renders focused read-only proof explanations but does not yet support
loading trace files, a timeline, or step-by-step playback. The solver runs inside
a background worker on an owned source snapshot, keeping its borrowed AST
lifetimes out of the UI state. `engine::RunReport::execution` contains full
owned graph/rewrite/trace data for future features.
