# mermansi

A production-quality pure Rust terminal renderer library and CLI for [Mermaid](https://mermaid.js.org/)
diagrams.

`mermansi` renders Mermaid diagrams as deterministic ASCII or Unicode text for terminals,
CLIs, logs, markdown previews, and documentation. It uses [`merman-core`](https://crates.io/crates/merman-core)
0.8.0-alpha.3 as the single strict Mermaid parser and semantic source, reuses
[`merman-ascii`](https://crates.io/crates/merman-ascii) 0.8.0-alpha.3 for families where that
crate satisfies the contract, and implements native terminal adapters for every remaining
family.

## Features

- **All 28 `RenderSemanticModel` variants supported** — 33 pinned render parser IDs and 29
  fixture families (including the ZenUML-to-Sequence transform) are checked in 232 concise
  rendering combinations: English/Chinese, ASCII/Unicode, and 100/120 display columns.
- **One public source-to-text API** — `render_source(source, &options)` accepts Mermaid or a raw
  JSON object/array.
- **Complete or concise output** — keep the canonical semantic model for lossless inspection, or
  request only the readable preview with a structured fallback.
- **ASCII and Unicode output** — switchable charset with display-column-aware layout.
- **Optional ANSI color roles** — geometry-independent color that never breaks alignment.
- **Bounded allocations** — typed errors when source, canvas, or output limits are exceeded.
- **Deterministic output** — identical input always produces byte-identical output.
- **International labels** — English, Chinese (CJK wide), combining marks, and emoji all
  preserve geometry.
- **Terminal chart geometry** — Pie, Radar, QuadrantChart, and Venn render genuine
  deterministic canvas geometry (closed circles, radial spokes, Cartesian plotting areas,
  overlapping set outlines) rather than structured-text-only output.
- **Native flow and hierarchy geometry** — Sankey, Treemap, Ishikawa, Event Modeling, and Info
  render connected flow routes, proportional nested rectangles, fishbones, frame/data graphs,
  and compact information cards instead of tables or indentation outlines.
- **Native schedule and protocol geometry** — Gantt renders proportional dated task lanes and
  Packet renders closed proportional 32-bit field rows, including cross-row fields.
- **Native graph and UML geometry** — GitGraph renders branch/commit/merge lanes; Class renders
  compartment boxes and connected UML relationships; Block cycles use deterministic routed ports.
- **No Mermaid parser bundled** — uses `merman-core` as the single source of truth.

## Install

```sh
cargo install mermansi
```

## CLI usage

```sh
# Render from stdin
echo 'flowchart TD
  A --> B' | mermansi

# Render from a file
mermansi --file diagram.mmd

# ASCII charset
mermansi --file diagram.mmd --ascii

# With ANSI color
mermansi --file diagram.mmd --color

# Geometry only (recommended for terminal previews and recordings)
mermansi --file diagram.mmd --concise
```

### CLI flags

| Flag | Description |
|---|---|
| `--file <PATH>` | Read Mermaid source from a file (default: stdin) |
| `--ascii` | Use ASCII charset (default: Unicode) |
| `--unicode` | Use Unicode charset (default) |
| `--color` | Enable ANSI color roles |
| `--truecolor` | Enable 24-bit ANSI color roles |
| `--no-color` | Disable ANSI color (default) |
| `--concise` | Emit terminal geometry only |
| `--complete` | Emit geometry plus the canonical model (default) |
| `--width <N>` | Maximum output width in terminal columns |
| `--height <N>` | Maximum output height in terminal rows |
| `--version` | Print version and exit |
| `--help` | Print help and exit |

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Parse error |
| 2 | Render error |
| 3 | Invalid options or I/O error |

## Library API

```rust
use mermansi::{render_source, MermansiOptions};

let mermaid_text = r#"
flowchart TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Do thing]
    B -->|No| D[End]
"#;

let output = render_source(mermaid_text, &MermansiOptions::unicode()).unwrap();
println!("{output}");
```

Presentation surfaces can omit the duplicate canonical model while retaining a structured
fallback for families without a readable preview:

```rust
use mermansi::{MermansiOptions, OutputMode, render_source};

let options = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);
let output = render_source(mermaid_text, &options).unwrap();
```

Raw JSON objects and arrays use the same entry point and render as connected box-tree geometry.
The exact `flowchart-v2` parser-id header is normalized to the public `flowchart` grammar header,
then parsed by `merman-core` through its known-type API.

### Syntax boundaries

`rack-beta` is not a Mermaid 11.16 diagram family and is intentionally rejected; use
`architecture-beta` or `block-beta` for rack/infrastructure views. Field-definition diagrams use
the supported `packet-beta` family, whose strict syntax requires quoted labels such as
`0-3: "Version"`. Both nested deployment Flowcharts and `C4Deployment` are supported. Runnable
examples live under `tests/scenarios/`; the exact parser inventory and alternatives are documented
in `docs/SUPPORT_MATRIX.md`.

### Convenience functions

```rust
use mermansi::{render_source_ascii, render_source_unicode};

let ascii_output = render_source_ascii(mermaid_text).unwrap();
let unicode_output = render_source_unicode(mermaid_text).unwrap();
```

## Support matrix

| Family | Support level | Renderer |
|---|---|---|
| Flowchart / graph | Geometry preview + canonical model | merman-ascii + mermansi |
| Sequence | Full geometry + canonical model | merman-ascii + mermansi |
| State | Partial geometry + canonical model | merman-ascii + mermansi |
| Class | Compartment/UML relationship geometry + canonical model | mermansi adapter |
| Er | Entity compartments and explicit cardinality routes + canonical model | mermansi adapter |
| Packet | Proportional closed bit-field geometry + canonical model | mermansi adapter |
| TreeView | Layered box-tree geometry + canonical model | mermansi adapter |
| XyChart | Partial geometry + canonical model | merman-ascii + mermansi |
| Mindmap | Layered box-tree geometry + canonical model | mermansi adapter |
| Gantt | Proportional dated task-lane geometry + canonical model | mermansi adapter |
| GitGraph | Connected branch/commit/merge geometry + canonical model | mermansi adapter |
| Journey | Connected scored task-path geometry + canonical model | mermansi adapter |
| Kanban | Nested board geometry + canonical model | mermansi adapter |
| Timeline | Connected period/event geometry + canonical model | mermansi adapter |
| ZenUML | Partial sequence geometry + canonical model | merman-ascii + mermansi |
| Json | Layered box-tree geometry + canonical model | mermansi adapter |
| Architecture | Nested box geometry + canonical model | mermansi adapter |
| C4 | Nested box geometry + canonical model | mermansi adapter |
| Pie | Circular sector geometry + legend | mermansi adapter |
| Requirement | Directional box geometry + canonical model | mermansi adapter |
| Sankey | Weighted connected flow geometry + canonical model | mermansi adapter |
| Radar | Radial spoke/graticule geometry + legend | mermansi adapter |
| Info | Closed information-card geometry + canonical model | mermansi adapter |
| Treemap | Proportional nested-rectangle geometry + canonical model | mermansi adapter |
| Block | Nested box and deterministic cycle geometry + canonical model | mermansi adapter |
| QuadrantChart | Cartesian quadrant geometry + legend | mermansi adapter |
| Ishikawa | Connected fishbone geometry + canonical model | mermansi adapter |
| EventModeling | Connected frame/data-box geometry + canonical model | mermansi adapter |
| Venn | Overlapping set-circle geometry + legend | mermansi adapter |

See `docs/SUPPORT_MATRIX.md` for the full machine-checkable support matrix.

## Deterministic output guarantee

`mermansi` guarantees that rendering the same Mermaid source text with the same options
always produces byte-identical output. This is verified by the test suite.

## License

MIT OR Apache-2.0
