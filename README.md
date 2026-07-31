# mermansi

A production-quality pure Rust terminal renderer library and CLI for [Mermaid](https://mermaid.js.org/)
diagrams.

`mermansi` renders Mermaid diagrams as deterministic ASCII or Unicode text for terminals,
CLIs, logs, markdown previews, and documentation. It uses [`merman-core`](https://crates.io/crates/merman-core)
0.8.0-alpha.3 as the single strict Mermaid parser and semantic source, reuses
[`merman-ascii`](https://crates.io/crates/merman-ascii) 0.8.0-alpha.3 for families where that
crate satisfies the contract, and implements structured terminal adapters for every remaining
family.

## Features

- **All 28 `RenderSemanticModel` variants supported** — 33 pinned render parser IDs and 29
  fixture families (including the ZenUML-to-Sequence transform) are checked at runtime.
- **One public source-to-text API** — `render_source(source, &options)` accepts Mermaid or a raw
  JSON object/array.
- **ASCII and Unicode output** — switchable charset with display-column-aware layout.
- **Optional ANSI color roles** — geometry-independent color that never breaks alignment.
- **Bounded allocations** — typed errors when source, canvas, or output limits are exceeded.
- **Deterministic output** — identical input always produces byte-identical output.
- **International labels** — English, Chinese (CJK wide), combining marks, and emoji all
  preserve geometry.
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

Raw JSON objects and arrays use the same entry point and render as canonical structured text.
The exact `flowchart-v2` parser-id header is normalized to the public `flowchart` grammar header,
then parsed by `merman-core` through its known-type API.

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
| Class | Partial geometry + canonical model | merman-ascii + mermansi |
| Er | Partial geometry + canonical model | merman-ascii + mermansi |
| Packet | Full geometry + canonical model | merman-ascii + mermansi |
| TreeView | Full geometry + canonical model | merman-ascii + mermansi |
| XyChart | Partial geometry + canonical model | merman-ascii + mermansi |
| Mindmap | Summary + canonical model | merman-ascii + mermansi |
| Gantt | Summary + canonical model | merman-ascii + mermansi |
| GitGraph | Summary + canonical model | merman-ascii + mermansi |
| Journey | Summary + canonical model | merman-ascii + mermansi |
| Kanban | Summary + canonical model | merman-ascii + mermansi |
| Timeline | Summary + canonical model | merman-ascii + mermansi |
| ZenUML | Partial sequence geometry + canonical model | merman-ascii + mermansi |
| Json | Canonical structured text | mermansi adapter |
| Architecture | Structured text | mermansi adapter |
| C4 | Structured text | mermansi adapter |
| Pie | Structured text | mermansi adapter |
| Requirement | Structured text | mermansi adapter |
| Sankey | Structured text | mermansi adapter |
| Radar | Structured text | mermansi adapter |
| Info | Structured text | mermansi adapter |
| Treemap | Structured text | mermansi adapter |
| Block | Structured text | mermansi adapter |
| QuadrantChart | Structured text | mermansi adapter |
| Ishikawa | Structured text | mermansi adapter |
| EventModeling | Structured text | mermansi adapter |
| Venn | Structured text | mermansi adapter |

See `docs/SUPPORT_MATRIX.md` for the full machine-checkable support matrix.

## Deterministic output guarantee

`mermansi` guarantees that rendering the same Mermaid source text with the same options
always produces byte-identical output. This is verified by the test suite.

## License

MIT OR Apache-2.0
