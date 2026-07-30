# Roadmap

This file records implementation evidence separately from the final release gate. A checked item
means the named behavior has focused tests; it does not imply that Review or release is complete.

## Foundation

- [x] Rust library and CLI scaffold with Rust 1.97.1 pinned (MSRV 1.95)
- [x] `merman-core` 0.8.0-alpha.3 is the sole parser and typed semantic source
- [x] No Selkie fork and no copied `beautiful-mermaid` implementation
- [x] Typed errors and validated charset/color/dimension options
- [x] Runtime state and build output ignored by Git

## Canvas And Output

- [x] Extended grapheme segmentation and Unicode display width
- [x] Explicit continuation-cell ownership for width-2 graphemes
- [x] Atomic multi-cell writes and checked overflow diagnostics
- [x] Directional stroke merging, closed corners, and deterministic overwrite priority
- [x] ASCII-safe structural glyph selection
- [x] ANSI16 and TrueColor roles that preserve plain geometry after SGR removal
- [x] Global output row, column, cell, and byte limits
- [x] Bounded structured serializer writer

## Semantic Adapters

- [x] Exhaustive dispatch for all 28 `RenderSemanticModel` variants
- [x] Inventory test for all 33 pinned render parser IDs
- [x] 29 English/Chinese fixture families, including the ZenUML transform
- [x] `merman-ascii` geometry or summary previews for its 14 typed model families
- [x] Lossless flowchart canonical output for supported, unsupported, and parallel-edge topologies
- [x] Readable previews plus canonical typed semantic models for all 28 variants
- [x] Delegated conformance round-trips every typed field, including Sequence actor links and Flowchart click metadata
- [x] Deterministic Unicode and ASCII conformance for every fixture
- [x] TD/TB/BT/LR/RL, self-loop, cycle, disconnected, nested, parallel, and dense graph tests

## Public API And CLI

- [x] One source-to-text API plus parsed-model and charset convenience APIs
- [x] Bounded file/stdin reads before parsing
- [x] Unicode, ASCII, ANSI16, TrueColor, width, and height CLI options
- [x] Parse/render/option-I/O exit code separation
- [x] Propagated stdout and flush failures
- [x] End-to-end CLI rendering for every Mermaid-source fixture in both charsets

## Release Gate

- [ ] Independent immutable Review passes with no findings
- [ ] Every review-sourced Issue is resolved by a higher-numbered passing Review
- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] `cargo package --locked` from a clean committed worktree
- [ ] Task and plan ledgers are completed only after all previous gates
- [ ] Reviewed work is committed with Conventional Commits

The Task remains open until every release-gate checkbox is supported by current repository
evidence. Passing a narrower test command never closes the Task.
