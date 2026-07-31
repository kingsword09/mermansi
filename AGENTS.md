# AGENTS.md — mermansi

## Overview

`mermansi` is a production-quality pure Rust terminal renderer library and CLI for Mermaid.
It does **not** contain a Mermaid parser. It uses [`merman-core`](https://crates.io/crates/merman-core)
0.8.0-alpha.3 as the single strict parser and semantic source, and reuses
[`merman-ascii`](https://crates.io/crates/merman-ascii) 0.8.0-alpha.3 where that crate
satisfies the rendering contract. Adapters for families not covered by `merman-ascii` are
implemented in this crate.

## Architecture boundaries

```
Mermaid source text
    │
    ▼
┌──────────────────┐
│  Parser layer    │  merman-core::Engine  (no modification, no fork)
└──────────────────┘
    │
    ▼
RenderSemanticModel   (28 variants: the authoritative semantic inventory)
    │
    ▼
┌──────────────────────────────┐
│  Adapter dispatch            │  src/adapters/mod.rs
│  ├─ merman-ascii reuse       │  Class, Er, Flowchart, Gantt, GitGraph,
│  │                           │  Packet, Sequence, State, XyChart, ZenUML
│  └─ mermansi native           │  Json, Architecture, C4, Journey, Kanban,
│     terminal adapters         │  Pie, Requirement, Sankey, Timeline, Radar,
│                               │  Info, Treemap, Block, QuadrantChart,
│                               │  Ishikawa, EventModeling, Mindmap,
│                               │  TreeView, Venn
└──────────────────────────────┘
    │
    ▼
┌──────────────────┐
│  Canvas          │  src/canvas.rs  (display-column-aware, bounded)
│  + ANSI roles    │  src/ansi.rs    (optional, geometry-independent)
└──────────────────┘
    │
    ▼
Deterministic ASCII / Unicode text output
```

### Rules

1. **No Mermaid parser is copied, rewritten, or forked.** `merman-core` is the sole parser.
2. **No fork of Selkie (the `merman` project).** We depend on published crates.
3. **`beautiful-mermaid` is MIT-licensed algorithmic prior art only.** It informs rendering
   approaches (box shapes, tree outlines) but is never treated as a byte-level oracle. No code
   is copied from it.
4. **Every successfully parsed built-in family must produce deterministic nonempty output.**
   No family may silently drop parsed semantic entities, relationships, labels, endpoints,
   markers, hierarchy, or chart values.
5. **Terminal-native geometry where meaningful; structured terminal representation where
   exact browser geometry is not meaningful.** Summary output is explicitly labeled as such —
   never disguised as full geometric parity.
6. **Optional ANSI roles do not affect geometry.** ANSI escape sequences add color only.
7. **All allocations are bounded and return typed errors.** Source, row, column, cell,
   routing, and output sizes are capped.
8. **Output is deterministic.** Parsing the same input twice yields byte-identical output.
9. **Workflow stages are isolated.** In `<aicode-stage:plan>`, inspect only what is necessary and
   return a plan without modifying files. In `<aicode-stage:implement>`, implement and run focused
   checks, but never run Git commands, stage files, create commits, resolve Issues, or mark the Task
   complete. Configured verification, independent Review, staging, commits, Issue transitions, and
   Task completion belong exclusively to the built-in `issue-loop` host after implementation.
10. **Immutable plans are historical evidence, not current instructions.** Never copy a staging or
    commit step from an earlier iteration plan into a Plan or Implement stage.
11. **Report durable work accurately.** While an active Issue still owns uncommitted repository
    changes, the Implement report must use `changed: true` even when the current invocation only
    verifies changes produced by an earlier interrupted iteration.
12. **Return the schema requested by the current stage.** A Review must return the requested Review
    object with `verdict`, `summary`, `findings`, and `resolutions`; never return or reuse Plan JSON.

## Modules

| Module | Responsibility |
|---|---|
| `src/lib.rs` | Public API, re-exports |
| `src/error.rs` | Typed errors |
| `src/input.rs` | Source classification, raw JSON decoding, and merman-core integration |
| `src/options.rs` | Render options (charset, color mode, dimensions) |
| `src/canvas.rs` | Display-column-aware bounded canvas with Unicode width |
| `src/ansi.rs` | Optional ANSI roles |
| `src/output.rs` | Canonical structured output and final output bounds |
| `src/adapters/mod.rs` | Dispatch by `RenderSemanticModel` variant |
| `src/adapters/*.rs` | Per-family terminal geometry and structured adapters |
| `src/adapters/chart_primitives.rs` | Shared bounded chart primitives (circle, line, sector) for Pie/Radar/QuadrantChart/Venn |
| `src/bin/mermansi.rs` | CLI binary |
| `tests/support_matrix.rs` | Executable conformance test |
| `tests/semantic_integrity.rs` | Label preservation, box closure, direction tests |
| `tests/bounds.rs` | Bounded allocation and determinism |
| `tests/cli.rs` | End-to-end CLI conformance and exit codes |
| `tests/fixtures/` | English and Chinese .mmd fixtures per family |

## Development principles

KISS, YAGNI, DRY, SOLID.

## License

- **mermansi** (this crate): MIT OR Apache-2.0
- **merman-core**: MIT OR Apache-2.0 (used as a dependency; unmodified)
- **merman-ascii**: MIT OR Apache-2.0 (used as a dependency; unmodified)
- **beautiful-mermaid**: MIT (algorithmic prior art reference only; no code copied)

## Toolchain

- `rust-version = "1.95"` (set by `merman-core`/`merman-ascii` `rust-version` field)
- Pinned via `rust-toolchain.toml` to `1.97.1` (satisfies MSRV).
