# Support Matrix

This document is the authoritative support matrix for `mermansi`. Every row corresponds to a
`RenderSemanticModel` variant in `merman-core` 0.8.0-alpha.3. The executable conformance test in
`tests/support_matrix.rs` verifies each row at compile/run time.

## RenderSemanticModel variants

The authoritative semantic inventory from `merman-core::diagram::RenderSemanticModel`:

| # | Variant | Kind string |
|---|---------|-------------|
| 1 | `Json` | `json` |
| 2 | `Mindmap` | `mindmap` |
| 3 | `State` | `state` |
| 4 | `Sequence` | `sequence` |
| 5 | `Flowchart` | `flowchart` |
| 6 | `Architecture` | `architecture` |
| 7 | `Class` | `class` |
| 8 | `C4` | `c4` |
| 9 | `Kanban` | `kanban` |
| 10 | `Gantt` | `gantt` |
| 11 | `Pie` | `pie` |
| 12 | `Packet` | `packet` |
| 13 | `Timeline` | `timeline` |
| 14 | `Journey` | `journey` |
| 15 | `Requirement` | `requirement` |
| 16 | `Sankey` | `sankey` |
| 17 | `Radar` | `radar` |
| 18 | `Info` | `info` |
| 19 | `Treemap` | `treemap` |
| 20 | `Block` | `block` |
| 21 | `Er` | `er` |
| 22 | `QuadrantChart` | `quadrantChart` |
| 23 | `XyChart` | `xychart` |
| 24 | `GitGraph` | `gitGraph` |
| 25 | `TreeView` | `treeView` |
| 26 | `Ishikawa` | `ishikawa` |
| 27 | `EventModeling` | `eventmodeling` |
| 28 | `Venn` | `venn` |

## Parser aliases

`merman-core` accepts multiple Mermaid headers for the same semantic family. The exact
`flowchart-v2` parser-id header is the one exception: mermansi normalizes that header to
`flowchart` and invokes merman-core's known-type API, so merman-core remains the sole Mermaid
parser.

| Canonical metadata id | Parser aliases / headers |
|---|---|
| `flowchart` | `flowchart-v2`, `flowchart`, `graph`, `flowchart-elk` |
| `state` | `stateDiagram-v2`, `stateDiagram` |
| `class` | `classDiagram`, `classDiagram-v2` |
| `er` | `erDiagram` |
| `sequence` | `sequenceDiagram` |
| `zenuml` | `zenuml` (transformed to `Sequence` render model) |
| `gitgraph` | `gitGraph` |
| `xychart` | `xychart-beta` |
| `packet` | `packet-beta` |
| `treeView` | `treeView-beta` |
| `ishikawa` | `ishikawa-beta`, `ishikawa` |
| `quadrantchart` | `quadrantChart` |
| `venn` | `venn-beta` |
| `block` | `block-beta` |
| `radar` | `radar-beta` |
| `treemap` | `treemap-beta` |
| `c4` | `C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, `C4Deployment` |
| `architecture` | `architecture-beta` |

## Support matrix

| Family | Renderer | Support level | English fixture | Chinese fixture | Status |
|---|---|---|---|---|---|
| Flowchart | merman-ascii + mermansi | Geometry preview + canonical model | `fixtures/flowchart.en.mmd` | `fixtures/flowchart.zh.mmd` | yes |
| Sequence | merman-ascii + mermansi | Full geometry + canonical model | `fixtures/sequence.en.mmd` | `fixtures/sequence.zh.mmd` | ✅ |
| State | merman-ascii + mermansi | Partial geometry + canonical model | `fixtures/state.en.mmd` | `fixtures/state.zh.mmd` | ✅ |
| Class | merman-ascii + mermansi | Partial geometry + canonical model | `fixtures/class.en.mmd` | `fixtures/class.zh.mmd` | ✅ |
| Er | merman-ascii + mermansi | Partial geometry + canonical model | `fixtures/er.en.mmd` | `fixtures/er.zh.mmd` | ✅ |
| Packet | merman-ascii + mermansi | Full geometry + canonical model | `fixtures/packet.en.mmd` | `fixtures/packet.zh.mmd` | ✅ |
| TreeView | merman-ascii + mermansi | Full geometry + canonical model | `fixtures/treeview.en.mmd` | `fixtures/treeview.zh.mmd` | ✅ |
| XyChart | merman-ascii + mermansi | Partial geometry + canonical model | `fixtures/xychart.en.mmd` | `fixtures/xychart.zh.mmd` | ✅ |
| Mindmap | merman-ascii + mermansi | Summary + canonical model | `fixtures/mindmap.en.mmd` | `fixtures/mindmap.zh.mmd` | ✅ |
| Gantt | merman-ascii + mermansi | Summary + canonical model | `fixtures/gantt.en.mmd` | `fixtures/gantt.zh.mmd` | ✅ |
| GitGraph | merman-ascii + mermansi | Summary + canonical model | `fixtures/gitgraph.en.mmd` | `fixtures/gitgraph.zh.mmd` | ✅ |
| Journey | merman-ascii + mermansi | Summary + canonical model | `fixtures/journey.en.mmd` | `fixtures/journey.zh.mmd` | ✅ |
| Kanban | merman-ascii + mermansi | Summary + canonical model | `fixtures/kanban.en.mmd` | `fixtures/kanban.zh.mmd` | ✅ |
| Timeline | merman-ascii + mermansi | Summary + canonical model | `fixtures/timeline.en.mmd` | `fixtures/timeline.zh.mmd` | ✅ |
| ZenUML | merman-ascii + mermansi | Partial sequence geometry + canonical model | `fixtures/zenuml.en.mmd` | `fixtures/zenuml.zh.mmd` | ✅ |
| Json | mermansi | Canonical structured text | `fixtures/json.en.mmd` | `fixtures/json.zh.mmd` | yes |
| Architecture | mermansi | Structured text | `fixtures/architecture.en.mmd` | `fixtures/architecture.zh.mmd` | ✅ |
| C4 | mermansi | Structured text | `fixtures/c4.en.mmd` | `fixtures/c4.zh.mmd` | ✅ |
| Pie | mermansi | Structured text | `fixtures/pie.en.mmd` | `fixtures/pie.zh.mmd` | ✅ |
| Requirement | mermansi | Structured text | `fixtures/requirement.en.mmd` | `fixtures/requirement.zh.mmd` | ✅ |
| Sankey | mermansi | Structured text | `fixtures/sankey.en.mmd` | `fixtures/sankey.zh.mmd` | ✅ |
| Radar | mermansi | Structured text | `fixtures/radar.en.mmd` | `fixtures/radar.zh.mmd` | ✅ |
| Info | mermansi | Structured text | `fixtures/info.en.mmd` | `fixtures/info.zh.mmd` | ✅ |
| Treemap | mermansi | Structured text | `fixtures/treemap.en.mmd` | `fixtures/treemap.zh.mmd` | ✅ |
| Block | mermansi | Structured text | `fixtures/block.en.mmd` | `fixtures/block.zh.mmd` | ✅ |
| QuadrantChart | mermansi | Structured text | `fixtures/quadrant.en.mmd` | `fixtures/quadrant.zh.mmd` | ✅ |
| Ishikawa | mermansi | Structured text | `fixtures/ishikawa.en.mmd` | `fixtures/ishikawa.zh.mmd` | ✅ |
| EventModeling | mermansi | Structured text | `fixtures/eventmodeling.en.mmd` | `fixtures/eventmodeling.zh.mmd` | ✅ |
| Venn | mermansi | Structured text | `fixtures/venn.en.mmd` | `fixtures/venn.zh.mmd` | ✅ |

## Output guarantee

Json fixtures contain raw JSON objects and exercise the same public `render_source` and CLI paths
as Mermaid fixtures; raw JSON arrays are supported by the same bounded decoder.

Every family produces deterministic nonempty output. Geometry rows use terminal-native boxes,
edges, and routing. Every adapter appends a canonical JSON semantic model to its readable preview,
preserving every typed field without claiming SVG-coordinate parity. Flowcharts emit an empty
preview plus that canonical representation only when the delegated geometric router reports an
unsupported topology. Self-loops and parallel edges use mermansi's bounded, display-column-aware
Canvas with a distinct routed lane and endpoint marker for every edge. Canvas node labels, edge
labels, and arrow markers use semantic ANSI roles on every grapheme owner; stripping those roles
is byte-identical to plain geometry. Pie and Sankey preview tables align columns by terminal
display width rather than Unicode scalar count.
