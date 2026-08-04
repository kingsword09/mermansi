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

## Syntax boundaries and user-facing alternatives

The support count is based on the pinned `merman-core` parser inventory, not on names that merely
look like Mermaid diagram headers.

| Requested diagram | Boundary | Supported representation |
|---|---|---|
| Rack diagram | `rack-beta` is not a Mermaid 11.16 or pinned `merman-core` family and is rejected as invalid Mermaid. | Use `architecture-beta` for connected infrastructure/service placement, or `block-beta` for a literal rack-like arrangement. |
| Field-definition diagram | This is the supported Packet family. Strict Packet syntax requires every field label after `:` to be quoted. | Use `packet-beta` with entries such as `0-3: "Version"`; `tests/scenarios/ipv4.packet.mmd` covers a five-word IPv4-style definition. |
| Deployment architecture | A nested `flowchart` is supported and rendered by mermansi's native bounded fallback when delegated geometry cannot preserve its groups. | `tests/scenarios/deployment.flowchart.mmd` preserves eight nodes, four subgraphs, and nine edges at 95 columns. |
| C4 deployment | `C4Deployment` is an alias of the supported C4 semantic family. | `tests/scenarios/deployment.c4.mmd` exercises nested deployment nodes, containers, a database container, and relationships. |

Mermansi does not add a private parser or relax strict Mermaid syntax for these boundaries. This
keeps parsing behavior aligned with `merman-core` and avoids accepting diagrams that Mermaid itself
does not define.

## Support matrix

| Family | Renderer | Support level | English fixture | Chinese fixture | Status |
|---|---|---|---|---|---|
| Flowchart | merman-ascii + mermansi | Geometry preview + canonical model | `fixtures/flowchart.en.mmd` | `fixtures/flowchart.zh.mmd` | yes |
| Sequence | merman-ascii + mermansi | Full geometry + canonical model | `fixtures/sequence.en.mmd` | `fixtures/sequence.zh.mmd` | ✅ |
| State | merman-ascii + mermansi | Partial geometry + canonical model | `fixtures/state.en.mmd` | `fixtures/state.zh.mmd` | ✅ |
| Class | mermansi | Compartment/UML relationship geometry + canonical model | `fixtures/class.en.mmd` | `fixtures/class.zh.mmd` | ✅ |
| Er | mermansi | Entity compartments and explicit cardinality routes + canonical model | `fixtures/er.en.mmd` | `fixtures/er.zh.mmd` | ✅ |
| Packet | mermansi | Proportional closed bit-field geometry + canonical model | `fixtures/packet.en.mmd` | `fixtures/packet.zh.mmd` | ✅ |
| TreeView | mermansi | Layered box-tree geometry + canonical model | `fixtures/treeview.en.mmd` | `fixtures/treeview.zh.mmd` | ✅ |
| XyChart | merman-ascii + mermansi | Partial geometry + canonical model | `fixtures/xychart.en.mmd` | `fixtures/xychart.zh.mmd` | ✅ |
| Mindmap | mermansi | Layered box-tree geometry + canonical model | `fixtures/mindmap.en.mmd` | `fixtures/mindmap.zh.mmd` | ✅ |
| Gantt | mermansi | Proportional dated task-lane geometry + canonical model | `fixtures/gantt.en.mmd` | `fixtures/gantt.zh.mmd` | ✅ |
| GitGraph | mermansi | Connected branch/commit/merge geometry + canonical model | `fixtures/gitgraph.en.mmd` | `fixtures/gitgraph.zh.mmd` | ✅ |
| Journey | mermansi | Connected scored task-path geometry + canonical model | `fixtures/journey.en.mmd` | `fixtures/journey.zh.mmd` | ✅ |
| Kanban | mermansi | Nested board geometry + canonical model | `fixtures/kanban.en.mmd` | `fixtures/kanban.zh.mmd` | ✅ |
| Timeline | mermansi | Connected period/event geometry + canonical model | `fixtures/timeline.en.mmd` | `fixtures/timeline.zh.mmd` | ✅ |
| ZenUML | merman-ascii + mermansi | Partial sequence geometry + canonical model | `fixtures/zenuml.en.mmd` | `fixtures/zenuml.zh.mmd` | ✅ |
| Json | mermansi | Layered box-tree geometry + canonical model | `fixtures/json.en.mmd` | `fixtures/json.zh.mmd` | yes |
| Architecture | mermansi | Nested box geometry + canonical model | `fixtures/architecture.en.mmd` | `fixtures/architecture.zh.mmd` | ✅ |
| C4 | mermansi | Nested box geometry + canonical model | `fixtures/c4.en.mmd` | `fixtures/c4.zh.mmd` | ✅ |
| Pie | mermansi | Circular sector geometry + legend | `fixtures/pie.en.mmd` | `fixtures/pie.zh.mmd` | ✅ |
| Requirement | mermansi | Directional box geometry + canonical model | `fixtures/requirement.en.mmd` | `fixtures/requirement.zh.mmd` | ✅ |
| Sankey | mermansi | Weighted connected flow geometry + canonical model | `fixtures/sankey.en.mmd` | `fixtures/sankey.zh.mmd` | ✅ |
| Radar | mermansi | Radial spoke/graticule geometry + legend | `fixtures/radar.en.mmd` | `fixtures/radar.zh.mmd` | ✅ |
| Info | mermansi | Closed information-card geometry + canonical model | `fixtures/info.en.mmd` | `fixtures/info.zh.mmd` | ✅ |
| Treemap | mermansi | Proportional nested-rectangle geometry + canonical model | `fixtures/treemap.en.mmd` | `fixtures/treemap.zh.mmd` | ✅ |
| Block | mermansi | Nested box and deterministic cycle geometry + canonical model | `fixtures/block.en.mmd` | `fixtures/block.zh.mmd` | ✅ |
| QuadrantChart | mermansi | Cartesian quadrant geometry + legend | `fixtures/quadrant.en.mmd` | `fixtures/quadrant.zh.mmd` | ✅ |
| Ishikawa | mermansi | Connected fishbone geometry + canonical model | `fixtures/ishikawa.en.mmd` | `fixtures/ishikawa.zh.mmd` | ✅ |
| EventModeling | mermansi | Connected frame/data-box geometry + canonical model | `fixtures/eventmodeling.en.mmd` | `fixtures/eventmodeling.zh.mmd` | ✅ |
| Venn | mermansi | Overlapping set-circle geometry + legend | `fixtures/venn.en.mmd` | `fixtures/venn.zh.mmd` | ✅ |

## Output guarantee

Json fixtures contain raw JSON objects and exercise the same public `render_source` and CLI paths
as Mermaid fixtures; raw JSON arrays are supported by the same bounded decoder and box-tree
adapter.

Every family produces deterministic nonempty output. Geometry rows use terminal-native boxes,
edges, and routing. Every adapter appends a canonical JSON semantic model to its readable preview,
preserving every typed field without claiming SVG-coordinate parity. Flowcharts never emit an empty
preview: nested groups, explicit node shapes, and delegated empty or unsupported output use
mermansi's native bounded geometry. Self-loops and parallel edges use mermansi's bounded,
display-column-aware Canvas with a distinct routed lane and endpoint marker for every edge. Canvas
node labels, edge labels, and arrow markers use semantic ANSI roles on every grapheme owner;
stripping those roles is byte-identical to plain geometry. Pie, Radar, QuadrantChart, and Venn render genuine
terminal chart geometry — closed circles, radial spokes, Cartesian plotting areas, and
overlapping set outlines — using shared bounded chart primitives (`chart_primitives`).
Sankey, Treemap, Ishikawa, EventModeling, and Info use bounded terminal-native flow, nested-area,
fishbone, frame/data, and card geometry. EventModeling preserves explicit source-frame references;
when the semantic model contains ordered frames without an explicit source edge, adjacent frames
remain connected in their parsed order.
Gantt, GitGraph, Packet, and Class now use bounded native timeline, branch-lane, bit-grid, and UML
geometry instead of delegated summaries or fractured multi-inheritance. Block reverse pairs share
one routed path, long cycle edges use deterministic outer ports, and unlabeled edge legends are not
duplicated below the geometry.

`tests/support_matrix.rs` executes every English and Chinese fixture in concise mode at
40/60/80/100/120 display columns in both ASCII and Unicode, for 580 attempted combinations.
Every family must render at 80/100/120 columns (348 required successes). At 40/60 columns an
adapter may instead return a deterministic typed `RenderLimit` when its minimum useful geometry
cannot fit. Every successful combination stays within its display-column bound, renders
deterministically with a minimum terminal-geometry signal, preserves quoted fixture labels, and
contains neither source/structured-text nor semantic-model fallback output. Complete-mode renders
are also compared with the parsed semantic model so typed entities, relationships, hierarchy, and
chart values remain lossless; preview dimensions do not rewrite arbitrary canonical JSON strings.

## ASG visual audit

The support matrix is necessary but cannot decide whether geometry is visually useful. The gallery
workflow records concise output through the real `mermansi` CLI, converts asciicast v3 recordings
with an externally supplied [ASG](https://github.com/kingsword09/asg) 2.0.2 binary, and creates an
inspectable HTML index. The script rejects other ASG versions so dimensions, font geometry, and
animation frame offsets cannot drift implicitly. Generated text, casts, SVGs, and optional PNG
previews stay under the ignored `.aicode/state/` directory during normal audits; it skips the
macOS `qlmanage` fallback because its square thumbnails can crop wide or tall diagrams. Set
`PUBLISH_README=1` to refresh the validated
34-SVG snapshot tracked in `docs/gallery/` and embedded by the repository README.

```sh
ASG_BIN=/absolute/path/to/asg/target/release/asg scripts/asg-gallery.sh

# Refresh README assets only after the 34-asset generation gate passes
PUBLISH_README=1 ASG_BIN=/absolute/path/to/asg/target/release/asg scripts/asg-gallery.sh
```

The default run uses a 95-column render budget and a 100-column terminal limit. Each static capture
uses only the columns its content needs. It produces 34 SVGs: all 29 supported fixture families,
the deployment Flowchart, C4Deployment, complex IPv4 Packet, rack Architecture alternative, and a
compact animated showcase. The showcase changes only between complete diagram frames; the static
assets remain the exhaustive audit surface. A successful file count is not a visual pass; every
SVG in the generated `index.html` must be inspected before a rendering Task is completed.
`tests/readme_gallery.rs` separately enforces exact parity between those family/scenario IDs, the
tracked SVG filenames, complete ASG SVG roots, and README image/source references.
