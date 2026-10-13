2026-10-06 — Completed F1. Built the Java CLI, generated JSON/Markdown oracle
outputs for baseline, multipage, table, raster/image, sanitization, and invalid
character PDFs, and recorded invalid-input/password outcomes. Added checksums,
provenance, and task coverage to `data/oracle/manifest.toml`.

2026-10-06 — Completed M1. Added the Rust document model, parser chunk types,
semantic element variants, image references, and typed conversion errors with
fixture-derived unit tests. `cargo test -p opendataloader_core` and
`cargo check --workspace` pass.

2026-10-06 — Completed P1. Added the local `lopdf` parser for PDF validation,
password classification, page count and geometry, Info metadata, text/image/
line-art chunks, plus fixture-backed tests for baseline, raster, invalid, and
password-protected PDFs. `cargo test --workspace` and `cargo check --workspace`
pass.

2026-10-06 — Completed C1. Added deterministic page-local cleanup for duplicate,
empty, tiny, out-of-page, page-background, whitespace, NUL, and U+FFFD content;
sanitization remains disabled by default. Added focused and invalid-character
fixture tests. `cargo test -p opendataloader_core` and `cargo check --workspace`
pass.

2026-10-06 — Completed S1. Added deterministic semantic reconstruction for
positioned text lines, paragraphs, font-size headings, and ordered/unordered
lists, and wired it into `parse_pdf`. Added focused semantic tests;
nested/cross-page list joining remains deferred because the current parser
boundary does not expose reliable indentation and line geometry. `cargo test
-p opendataloader_core` and `cargo check --workspace` pass.

2026-10-06 — Completed the observable portion of T1. Added border-grid table
reconstruction with ordered rows/cells, first-row headers, cell bounds, text
children, and line-art omission, plus a focused geometry-backed test. Full
table-fixture parity remains deferred because the current parser does not yet
preserve reliable text coordinates or line segments for the committed PDF.
`cargo test -p opendataloader_core` and `cargo check --workspace` pass.

2026-10-06 — Completed I1. Added semantic image elements, deterministic PNG
external-image writing from embedded PDF streams, JPEG passthrough support,
stable relative references, and isolated per-image write failures. Added a
raster-fixture test; `cargo test -p opendataloader_core` and `cargo check
--workspace` pass.

2026-10-06 — Completed M2. Verified the Rust core is split into focused
`model`, `parser`, `cleanup`, `semantics`, and `images` modules, with `lib.rs`
limited to module declarations, public exports, and thin parsing orchestration.
Preserved public symbols and behavior; `cargo test -p opendataloader_core`
(12 passed) and `cargo check --workspace` pass.

2026-10-06 — Completed R1. Added deterministic default reading-order sorting,
explicit parser-order mode, and recursive stable nonzero IDs in
`crates/opendataloader_core/src/reading_order.rs`; wired the default into
`parse_pdf` and added focused tests. Full XY-Cut++ parity remains deferred
until the parser preserves reliable per-chunk coordinates. `cargo test
-p opendataloader_core` (14 passed) and `cargo check --workspace` pass.

2026-10-06 — Completed M3. Moved the centralized Rust unit tests into the
`model`, `parser`, `cleanup`, `semantics`, and `images` modules, leaving
`lib.rs` without a centralized test module. `cargo fmt --all`, `cargo test
-p opendataloader_core` (14 passed), and `cargo check --workspace` pass.

2026-10-06 — Completed J1. Added serde_json serialization for the specified
document root and semantic element shapes, including mandatory metadata nulls,
canonical PDF/UA tags, rounded bounds, image references, nested lists/tables,
formulas, captions, and TOC nodes. Added focused serializer tests. `cargo fmt
--all -- --check`, `cargo check --workspace`, and `cargo test
-p opendataloader_core` (16 passed) pass; existing C-library warnings remain.

2026-10-06 — Completed D1. Added `serialize_markdown` with heading, text,
formula, recursive list, pipe-table, and external/embedded image rendering,
Java-compatible text escaping, destination sanitization, and exact two-LF
element separators. Added focused renderer tests; `cargo fmt --all -- --check`,
`cargo test -p opendataloader_core` (18 passed), and `cargo check --workspace`
pass. Existing C-library warnings remain; full output orchestration is deferred
to O1.

2026-10-06 — Completed O1. Connected `convert` to parse each local PDF once,
optionally write per-PDF PNG images, and write requested JSON and Markdown
files with contextual errors. Added an oracle-backed raster end-to-end test
that verifies shared image references and unchanged source input. `cargo fmt
--all`, `cargo check --workspace`, and `cargo test --workspace` pass; existing
C-library warnings remain.

2026-10-06 — Completed V1. Added checklist traceability to
`data/oracle/manifest.toml`, mapping all nine first-round conformance items
to Rust entry points, committed fixtures, focused tests, and concrete parity
blockers. Deferred gaps remain explicit for truncated/XMP fixtures, exact
XY-Cut++ parity, unsupported semantic inputs, isolated image-failure fixtures,
and Base64 images.

2026-10-06 — Completed X1. Added a Rust stage-to-specification and fixture map
to `docs/specs/pdf-json-markdown-reimplementation.md` and documented the
self-contained pipeline in `crates/opendataloader_core/src/lib.rs`. Verified
that Rust tests and builds use committed data and do not require Java
sources, Maven, generated JARs, or decompilation output.

2026-10-06 — Completed B1. Added core `ConversionOptions` with sequential
defaults and Rayon-backed processing for multiple PDFs when `parallel` is
enabled; single-PDF conversion remains serial. Added colocated tests comparing
serial and parallel JSON, Markdown, and image outputs plus enabled single-PDF
conversion. `cargo fmt --all`, `cargo test -p opendataloader_core` (20 passed),
and `cargo check --workspace` pass; existing C-library warnings remain.

2026-10-06 — Completed C2. Moved JSON, Markdown, and external-image output
switches into `ConversionOptions` alongside `parallel`; updated the core batch
conversion path, CLI caller, and colocated tests. `cargo fmt --all`,
`cargo test --workspace` (20 core tests passed), and `cargo check --workspace`
pass; existing C-library warnings remain.

2026-10-07 — Completed C3. Extracted the per-PDF conversion work into the
explicit `convert_one` function and reused it from sequential and Rayon batch
paths. `cargo fmt --all -- --check`, `cargo test --workspace` (20 tests
passed), and `cargo check --workspace` pass; existing C-library warnings
remain.

2026-10-07 — Completed CLI1. Implemented the local Clap CLI with required
input/output arguments, JSON/Markdown/image selection, opt-in parallel
processing, directory rejection, contextual conversion failures, and help and
project documentation. Added colocated CLI tests covering argument validation,
single/multiple inputs, output generation, image output, parallel selection,
and conversion errors. `cargo fmt --all -- --check`, `cargo test --workspace`,
`cargo check --workspace`, and `cargo run -p opendataloder_cli -- --help` pass;
existing C-library warnings remain.

2026-10-07 — Completed ABI1. Implemented the local batch C ABI over the core
conversion options, including a generated cbindgen header, stable JSON,
Markdown, image, and parallel option bits, stable status codes, UTF-8 and
pointer validation, thread-local last-error reporting, and panic containment.
Added colocated ABI tests, `docs/abi.md`, and a C compile/link smoke test.
`cargo test --workspace` and `cargo check --workspace` pass.

2026-10-07 — Completed PY1. Implemented the Python ctypes file-writing API
 over the native ABI with JSON as the default, JSON/Markdown selection,
 single or multiple local paths, directory rejection, output-directory
 creation, native load/status error mapping, and the documented
 `OPENDATALOADER_PDF_LIBRARY` override. Bundled the release native library in
 the wheel and verified source tests, Python compilation, wheel contents, and
 installed-package conversion without Java or network access.

2026-10-07 — Completed LINT1. Applied the five requested Clippy fixes in the
Rust parser and conversion orchestration. `cargo check --workspace` and strict
workspace Clippy pass. Core and CLI tests pass; the existing C ABI missing-input
assertion remains the separate TEST1 failure. Formatting remains blocked by an
unrelated pre-existing `markdown.rs` rustfmt difference.

2026-10-07 — Completed LINT2. Verified `packages/opendataloader/src/opendataloader/api.py`
already matches the configured Black formatting, so no source changes were
needed. `uv run black --check .`, `uv run ruff check .`, `uv run pytest` (1
passed), and `cargo check --workspace --locked` pass.

2026-10-07 — Completed TEST1. Preserved the full `anyhow` error chain when the
C ABI maps core conversion failures, so `odl_last_error()` includes the
failing input path while retaining output-directory details. The targeted C
ABI test, `cargo test --workspace --locked`, `cargo check --workspace
--locked`, and strict workspace Clippy pass.

2026-10-07 — Completed STD1. Added an exact JSON-structure and Markdown-byte
comparison harness for representative `data/stg` text/list, table, and
image/table PDFs. The test is ignored pending parser parity; a 98-case probe
found zero exact matches, with encoded-font text, missing coordinates, and
image extraction differences recorded in `data/oracle/manifest.toml`.

2026-10-07 — Completed STD1. Activated the regression harness for exactly ten
complete `data/stg` PDF/JSON/Markdown triplets, recorded their coverage in the
manifest, and made JSON/Markdown comparisons continue across all fixtures with
fixture-specific failure reporting. `cargo check --workspace --locked` and
strict workspace Clippy pass; the active regression and full workspace tests
report the expected ten-fixture parity mismatches for STD2.

2026-10-07 — Completed STD2. Ran the active ten-fixture regression: all ten
PDFs converted, and each reported JSON and Markdown mismatches. Diagnosed the
shared causes as embedded-font decoding, missing text coordinates causing
semantic/table geometry drift, and image/table structure differences. Added
fixture-specific evidence to `data/oracle/manifest.toml` and created the
scoped STD2-F1/F2/F3 fix tasks; production code and oracle outputs were not
changed.

2026-10-07 — Completed STD2-F1. Updated `parser.rs` to decode page text with
the active PDF font's ToUnicode mapping, retaining raw UTF-8 fallback and
operation order. Added an embedded Japanese-font parser test; all ten STD1
generated JSON text contents are free of parser-produced control characters.
The active ten-fixture regression still reports the documented geometry and
image/table mismatches for STD2-F2/F3. `cargo test -p opendataloader_core
--locked parser_` passes.

2026-10-07 — Completed STD2-F2. Updated `parser.rs` to track PDF text matrices,
line movement, font size, and graphics-state transforms (`cm`, `q`, `Q`) so
text chunks retain page-space bounds instead of full-page rectangles. Added a
fixture-backed positioned-text and non-table assertion. `cargo test -p
opendataloader_core --locked parser_` passes; the active ten-fixture regression
still reports the separately scoped image/table mismatches for STD2-F3.

2026-10-07 — Completed STD2-F3. Updated `parser.rs` to preserve transformed
line segments and recursively inspect Form XObjects for nested image
invocations. Updated `semantics.rs` to ignore page-background and filled
rectangle noise, group connected border lines, and reject one-cell false
tables. Table row/cell structures now match the table-heavy committed
fixtures; unsupported image artifacts and remaining semantic-content parity
gaps remain for STD3. Core tests pass; the active ten-fixture regression still
reports the documented JSON and Markdown mismatches.

2026-10-07 — Advanced STD3 with STD3-F1. Parser font metadata now resolves
 resource aliases through `BaseFont`, and semantic reconstruction no longer
 classifies long large-font body blocks as headings; added a focused regression
 test. Core unit tests and type checks pass, but the active ten-fixture gate
 still fails. Added STD3-F2 for tagged-PDF semantic/text segmentation and
 STD3-F3 for remaining inline-image extraction gaps; expected fixtures remain
 unchanged.

2026-10-07 — Completed STD3-F2. Parser now tracks marked-content `BMC`/`BDC`/
`EMC` scopes and `MCID` values, carries PDF tags onto text and image chunks,
and semantic reconstruction honors tagged headings, captions, paragraphs, and
list items while preserving tagged block boundaries. Added a focused tagged-
semantics test. `cargo test -p opendataloader_core --locked`, `cargo check
--workspace --locked`, and strict workspace Clippy pass; STD3-F3 and the
remaining full-fixture parity work are unchanged.

2026-10-07 — Completed STD3-F3 for supported inline streams. Image chunks now
retain raw 8-bit grayscale/RGB inline payloads from `BI` operations, and the
existing external-image writer emits them as PNG files while preserving
isolated per-image failures. Added a focused inline-image writer test.
`cargo test -p opendataloader_core --locked` passes all 25 non-regression tests
and `cargo check --workspace --locked` passes; the active ten-fixture STD3
regression still reports its documented JSON/Markdown mismatches, and filtered
inline encodings remain outside the current `lopdf` parser boundary.

2026-10-07 — Completed STD3-F4. Semantic line reconstruction no longer inserts
synthetic spaces between same-baseline parser chunks, groups tagged runs by
marked-content tag, and skips font-size heading inference for tagged text.
The active ten-fixture STD3 regression still fails on broader structural-tag
mapping and image-order parity; expected fixtures and comparisons remain
unchanged.

2026-10-08 — Advanced STD3 with a scoped parser fix. Page StructParents values
 are now dereferenced, ParentTree Kids number-tree nodes are traversed, subset
 font prefixes are removed from resolved names, and text-matrix scale is used
 for effective font sizes, bounds, and advances. Focused parser tests pass;
 the active ten-fixture regression still reports tagged semantic/table,
 image-order, and exact geometry mismatches.

2026-10-07 — Advanced STD3 with one scoped semantic fix. Border-table
reconstruction now ignores thin lines spanning a page edge, preventing page
frames from becoming fabricated tables; added a colocated regression test.
`cargo check --workspace --locked` passes and 26 non-regression workspace
tests pass. The active ten-fixture gate still reports JSON and Markdown
mismatches for all ten fixtures, so STD3 remains incomplete.

2026-10-07 — Advanced STD3 with parser-side PDF structure-tree role
resolution. ParentTree MCID mappings now carry roles such as Figure onto
parser chunks instead of retaining only broad marked-content P scopes; added
a focused parser test. `cargo check --workspace --locked` and strict
workspace Clippy pass; the active ten-fixture gate still reports JSON and
Markdown mismatches from remaining image inclusion/order and tagged table or
semantic reconstruction gaps, so STD3 remains incomplete.

2026-10-08 — Advanced STD3 with descendant-font ToUnicode decoding for Type0/CID
fonts in `parser.rs`. The focused embedded-font test passes and parser-produced
control-character text is gone from the regression output. `cargo check
--workspace --locked` passes; `cargo test --workspace --locked` still reports
the documented ten-fixture semantic/table/image mismatches, so STD3 remains
incomplete.

2026-10-08 — Advanced STD3 with a scoped semantic fix. Semantic text lines now
retain parser structure IDs, so adjacent tagged runs from different structure
elements remain separate paragraphs. Added a focused regression test. The
focused test, `cargo check --workspace --locked`, `cargo fmt --all -- --check`,
and `git diff --check` pass; the full ten-fixture regression still reports the
documented remaining mismatches, so STD3 remains incomplete.

2026-10-08 — Completed STD3-F5. ParentTree role resolution now walks through
leaf `Span` structure elements to the nearest meaningful parent role, while
preserving roles such as `Figure`; added a fixture-backed parser assertion for
paragraph and figure roles. Focused parser tests, `cargo check --workspace
--locked`, formatting, and `git diff --check` pass. The active ten-fixture
regression remains failing on separate paragraph-grouping and table-structure
mismatches, so STD3 remains incomplete.

2026-10-08 — Completed STD3-F6. Semantic reconstruction now combines same-tag,
same-baseline parser chunks even when their MCIDs differ, while vertically
distinct tagged runs remain separate. Added a colocated regression test.
`cargo fmt --all`, `cargo check --workspace --locked`, and the 29 passing
non-regression workspace tests pass; the ten-fixture regression still reports
JSON and Markdown mismatches for all ten fixtures, so STD3 remains incomplete.

2026-10-08 — Advanced STD3 with a scoped semantic fix. Tagged paragraph runs
now merge by shared PDF role despite differing MCIDs, while paragraph joins
require compatible left alignment so indented runs remain separate. All seven
semantic unit tests pass; the ten-fixture gate still reports the documented
broader table, image, metadata, and tagged-structure mismatches.

2026-10-13 — Completed STD3-F7. `page_bounds` now walks inherited `/Parent`
page-tree dictionaries with cycle detection, preserving explicit page
`MediaBox` values and returning a parse error when the chain has no valid box.
Added the regression PDF and Java JSON/Markdown oracles under
`data/oracle/mediabox/`, plus parser and `parallel=true` conversion tests.
Focused tests pass; unrelated image/metadata parity gaps remain documented
under STD3.
