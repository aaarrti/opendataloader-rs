# PDF to JSON and Markdown behavior specification

This document defines the Java behavior to reproduce in the Rust extraction
engine. It is intended for an independent implementation team that may not
read or reuse the Java source. It covers local PDF input, structured
extraction, JSON and Markdown outputs, and image output used by those formats.

The eventual product has a command-line executable and a C shared library
consumed by Python. This document defines neither interface: the user owns that
contract. Python packaging and wrapper behavior are out of scope for the first
implementation round.

The implementation target is the default local Java pipeline in this checkout.
Hybrid or remote processing is excluded. The specification does not require
special handling for linearized PDFs or other layout optimization hints.

## Scope

Implement these behaviors:

- Open a local PDF and determine its page count and document metadata.
- Extract text, images, and layout information into page-ordered content.
- Infer paragraphs, headings, lists, tables, captions, and page headers or
  footers where the Java pipeline does so.
- Write the content as the JSON structure defined here.
- Write the content as Markdown using the formatting rules defined here.
- Extract image content to files for JSON and Markdown references.
- Represent embedded images as Base64 data URIs when requested. This is lower
  priority; the first implementation can emit a documented placeholder.

Do not implement hybrid backends, OCR services, HTML, plain text, PDF
modification, tagged-PDF output, PDF/UA validation, Node.js, Python, publishing,
or Java compatibility APIs as part of this specification. HTML markup embedded
inside Markdown is an optional Markdown rendering mode, not a separate HTML
output target.

## Java implementation map

Use these Java components as the source of truth when a behavior below needs
clarification:

| Concern | Reference source |
| --- | --- |
| PDF validation, parsing, extraction orchestration, cleanup, and output dispatch | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/processors/DocumentProcessor.java` |
| Local page content filtering and text cleanup | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/processors/ContentFilterProcessor.java` and `TextProcessor.java` |
| Semantic reconstruction | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/processors/` |
| Page-reading order | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/processors/readingorder/XYCutPlusPlusSorter.java` |
| Output node classes | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/entities/` and veraPDF WCAG algorithm entities |
| JSON root and metadata | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/json/JsonWriter.java` |
| JSON property names and serializer registration | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/json/JsonName.java` and `ObjectMapperHolder.java` |
| Per-node JSON shapes | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/json/serializers/` |
| Markdown conversion | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/markdown/MarkdownGenerator.java` |
| Markdown HTML mode | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/markdown/MarkdownHTMLGenerator.java` |
| Extracted image files | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/utils/ImagesUtils.java` |
| Embedded image data URIs | `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/utils/Base64ImageUtils.java` |
| Existing end-to-end and serializer behavior | `java/opendataloader-pdf-core/src/test/java/org/opendataloader/pdf/` |

## Rust implementation map

The Rust path is self-contained. It uses the Java implementation only to
generate the committed oracle files described in
[`data/oracle/manifest.toml`](../../data/oracle/manifest.toml). The
following table maps each local stage to this specification and its fixture
coverage.

| Stage | Rust entry point | Specification section | Oracle coverage |
| --- | --- | --- | --- |
| Parse and validate | `crates/opendataloader_core/src/parser.rs::parse_document` | [Input validation and document metadata](#input-validation-and-document-metadata) | `lorem`, `multipage`, `raster`, `invalid-input`, `password-protected` |
| Clean page chunks | `crates/opendataloader_core/src/cleanup.rs::normalize_page_chunks` | [Extraction stages](#extraction-stages) | `sanitization`, `invalid-chars` |
| Reconstruct semantics | `crates/opendataloader_core/src/semantics.rs::reconstruct_semantics` | [Supported parser and semantic elements](#supported-parser-and-semantic-elements) | `lorem`, `multipage`, `table` |
| Order and assign IDs | `crates/opendataloader_core/src/reading_order.rs::apply` | [Reading order](#reading-order) | `multipage` |
| Write external images | `crates/opendataloader_core/src/images.rs::write_external_images` | [Image output](#image-output) | `raster` |
| Serialize JSON | `crates/opendataloader_core/src/json.rs::serialize_document` | [JSON output](#json-output) | `lorem`, `multipage`, `table`, `raster` |
| Serialize Markdown | `crates/opendataloader_core/src/markdown.rs::serialize_markdown` | [Markdown output](#markdown-output) | `lorem`, `multipage`, `table`, `raster`, `sanitization` |
| Orchestrate conversion | `crates/opendataloader_core/src/lib.rs::convert` | [Errors and partial output](#errors-and-partial-output) | `raster` |

Rust tests read only committed PDFs and oracle outputs. They do not require
Java sources, Maven, generated JARs, decompilation output, or network access.

## Processing model

Treat the PDF as a sequence of pages. Preserve the original page count even
when page selection is used. Each selected page produces an ordered list of
content elements; skipped pages produce an empty list internally and contribute
no children to either output.

Use this logical pipeline:

1. Open and parse the PDF. Read page count and document information. Reject an
   unreadable document with a conversion error; do not silently produce an
   empty success result.
2. Extract page text, font information, colors, bounding boxes, image
   references, and drawing information needed for layout analysis.
3. Filter unwanted content such as out-of-page or tiny content according to
   the selected processing configuration. Preserve the Java defaults for the
   first parity implementation.
4. Reconstruct lines and paragraphs from text fragments. Preserve explicit
   line breaks only when the corresponding option is enabled.
5. Infer structured elements: headings and heading levels, lists and nesting,
   tables and cells, captions, and repeating headers or footers.
6. Assign element IDs in page order. Keep each element's page number and
   bounding box associated with the content through all transformations.
7. Sort final page content in reading order. The default reading-order mode is
   `xycut`.
8. Apply configured text sanitization and content filters before serialization.
9. Write requested JSON and/or Markdown. If image output is enabled, write
   referenced image files before writing either format.

## Input validation and document metadata

For each PDF, the Java preprocessing path checks that the temporary directory
is writable, validates the PDF magic bytes, resets per-document parser state,
opens the document, and parses its page chunks. The Rust parser can have a
different setup sequence, but it must distinguish these observable outcomes:

- Bytes without a PDF signature are invalid input, even when the path ends in
  `.pdf`.
- A file with a PDF signature but a truncated or corrupt body is still an
  invalid PDF. Preserve the parser error as the underlying cause.
- An encrypted PDF without a valid password is a password error, not a generic
  parse error. The eventual CLI/library contract will decide how that error is
  returned.
- Failure to read the file or create required image output is an I/O failure.
- Do not modify the source PDF.

The page count comes from the parsed document and includes unselected pages.
Document metadata comes from the PDF information dictionary. For author, title,
creation date, and modification date, use the corresponding XMP value only
when the document-information value is missing. Preserve the parser's date
string instead of normalizing it.

The Java preprocessor also configures optional-content visibility, font-program
parsing, marked-content handling, and automatic spacing between text pieces.
Initialize equivalent parser settings before extracting page chunks. If a
structure tree is explicitly requested but the document has none, continue via
the ordinary local path.

## Page content model

Keep parser chunks distinct from semantic output nodes until cleanup and layout
analysis are complete. A positioned content object should carry the following
data when available:

- A zero-based page index, converted to one-based at output time.
- A PDF user-space bounding rectangle `(left, bottom, right, top)`.
- Unicode text, glyph order, and character spacing for text content.
- Font name, font size, style, and color when the parser exposes them.
- Original parser order and all geometry needed for sorting and grouping.
- An image object reference or a page region that can be rendered as an image.
- A recognized structure ID and PDF/UA tag when present.
- Parent/child links for list, table, figure, and other semantic nodes.

Do not flatten a page to plain text before reconstruction. That discards the
spatial evidence needed to identify columns, sidebars, tables, lists, headings,
and captions.

## Extraction stages

The stages below reflect the current Java dependency order. The Rust
implementation can use different types and algorithms, but it must retain the
same information until each dependent stage has completed.

1. Parse page chunks, page boxes, document metadata, and table-border lines.
2. Per page, remove duplicate text and decoration images, discard null
   elements, filter tiny and out-of-page content, merge close text fragments,
   trim whitespace, collapse repeated spaces, split on whitespace where the
   processor requires it, replace undefined characters, and filter backgrounds.
3. If enabled, render pages to detect hidden text. This stage is disabled by
   default and runs sequentially in Java.
4. If configured, cluster tables across pages. The default selects the
   border-based table processor.
5. Per page, detect strikethrough or underlined text, reconstruct table
   borders, remove geometric line chunks from semantic content, detect special
   tables, and group text fragments into lines.
6. Across pages, identify repeated headers and footers and group lists.
7. Per page, group paragraphs, refine lists from text nodes, and detect
   headings.
8. Assign nonzero content IDs in page order.
9. Detect captions and link them to figure or table IDs when possible.
10. Across pages, join neighboring lists and tables, infer heading levels, and
    assign semantic levels.
11. Sort page content using the selected reading-order mode.
12. Apply configured sensitive-data rules when sanitization is enabled.

The Java implementation parallelizes some page-local stages and performs
cross-page stages sequentially. The Java thread-local/static-container design
is not a compatibility requirement. Preserve deterministic ID assignment,
stable page order, and cross-page dependencies instead.

### Reading order

The default `xycut` mode uses XY-Cut++ to partition a page by whitespace and
regions, then orders the resulting groups. The `off` mode retains parser object
order. JSON `kids` and Markdown must consume the same ordered page content. An
unknown Java reading-order value warns and falls back to `off`.

Reading order is a layout heuristic. A simple top-to-bottom coordinate sort is
not equivalent for multi-column pages, sidebars, or separated regions.

The Java implementation has an optional tagged-structure-tree path. Hybrid
processing is out of scope. A Rust implementation can add tagged-structure-tree
support as a separate parity step; the default untagged-PDF path is required.

Use these Java defaults for the first parity run. A public configuration API is
not specified here; these values define the no-options behavior only.

| Behavior | Default |
| --- | --- |
| JSON format | Enabled. |
| Markdown format | Disabled. |
| Page selection | All pages. |
| Reading order | `xycut`. |
| Table detection | Border-based. |
| Image output | External files. |
| Image format | PNG. |
| Image resolution | 144 DPI. |
| Image directory | `<PDF_BASE_NAME>_images` under the output directory. |
| Header/footer output | Excluded from JSON and Markdown. |
| Keep original line breaks | Disabled. |
| Strikethrough detection | Disabled. |
| Out-of-page content filter | Enabled. |
| Tiny-text filter | Enabled. |
| Hidden optional-content-group filter | Enabled. |
| Page-sized background-vector filter | Enabled. |
| Hidden-text detection | Disabled. |
| Sensitive-data sanitization | Disabled. |
| Undefined or unmapped characters | Replace with a space. |
| Automatic text-piece spacing ratio | `0.17 × font size`. |
| Worker count | One page-processing worker. |

Structured extraction remains enabled for both outputs. The default filter
configuration contains sensitive-data rules, but it applies none unless
sanitization is enabled. When enabled, the rules replace emails, phone-like
numbers, national identifiers, payment-card numbers, long digit sequences,
IPv4 and IPv6 addresses, MAC addresses, and URLs with fixed placeholders.

## Supported PDF inputs and page elements

### First-round input profile

Support local, parser-valid, ordinary digitally generated PDFs that contain
selectable text and common page geometry, optionally with embedded raster
images or vector drawing operators. This is the default first-round profile
and the most common PDF kind. It includes single-page and multipage PDFs,
primarily untagged PDFs, standard page sizes, rotated pages, and mixed
text/image pages when the parser can read their content. Tagged documents may
be parsed, but tagged-structure-tree extraction is off by default and is not
required for the first-round profile.

The Java path accepts parser-supported PDF versions; it does not enforce a
PDF-version allow-list. It selects a veraPDF WCAG 2.2 human-processing flavor
from the declared version: PDF 2.0 receives the PDF 2.0 flavor, and other
versions use the non-2.0 flavor. Do not describe this as PDF/A validation. Do
not promise every encrypted-PDF scheme; the inspected veraPDF reader path uses
the Standard Security Handler.

For planning and first-round conformance, treat ordinary PDF 1.x documents
(especially common PDF 1.4–1.7 output from office and publishing software) as
the target profile. PDF 2.0 and older parser-readable versions may work, but
they are not a separately tested compatibility target. Do not reject a file
only because it is linearized; no linearization-specific optimization or
behavior is required.

The first Rust round does not perform OCR or invoke any remote or local ML
model. Image-only scans therefore produce no recognized words. They may still
produce an image element if the parser exposes the page image. Do not generate
descriptions, captions, headings, or reading order with a model. Existing PDF
metadata or alt text can be retained only when the local parser exposes it.

Do not promise extraction of form values, annotations, digital signatures,
attachments, embedded files, or other interactive PDF features. The required
content is visible page content plus document metadata and geometry.

### Supported parser and semantic elements

The parser layer supplies page artifacts such as text chunks, image chunks,
line-art chunks, and table-border geometry. The local processor converts these
into the following useful output kinds when detected:

| Page/output kind | First-round behavior |
| --- | --- |
| Text chunks and text lines | Extract Unicode text, geometry, page number, font name, font size, and color when available. |
| Paragraphs | Group adjacent text lines into paragraph content and preserve the resulting geometry. |
| Headings | Detect heading candidates and infer levels 1–6 when evidence is available. |
| Lists and list items | Group ordered or unordered items and preserve nested child content. |
| Tables, rows, and cells | Reconstruct cells from borders and positioned text; preserve row/column order and spans. |
| Captions | Detect text associated with a figure or table and preserve a linked content ID when available. |
| Images and semantic pictures | Preserve image geometry; write image files and references according to [Image output](#image-output). |
| Formulas | Preserve formula/LaTeX content only when the parser or semantic stage supplies it. |
| Headers and footers | Detect repeated page regions; omit them by default and include them only when configured. |
| Vector/line-art chunks | Use geometry for layout and table detection; omit drawing-only chunks from JSON and Markdown. |
| Table of contents | Preserve only when supplied by a supported structure-tree path. The default untagged heuristic detector is disabled. |

The `type` values emitted by the current serializers are `heading`,
`paragraph`, `caption`, `formula`, `image`, `list`, `list item`, `table`,
`table row`, `table cell`, `toc`, `toc item`, `text chunk`, `text block`, and
`line`. Header and footer nodes use their semantic lower-case type. A semantic
picture is serialized as `image`; `picture` is not a separate JSON type.
Nested children use `kids` arrays or the parent-specific `list items`, `rows`,
`cells`, and `toc items` arrays. `line` is a serializer capability for an
intermediate line chunk, not an expected final page element: the normal
pipeline consumes/removes those chunks while constructing higher-level
content.

Emit a kind only when the local extraction path creates it. Do not manufacture
empty semantic objects to make every PDF appear structured. A PDF containing
only a scanned page may have an empty text tree and an image element; this is
not a reason to emit OCR text.

## JSON output

Write one UTF-8 JSON object per input PDF. Use pretty-printed JSON. Preserve
the Java property names exactly, including spaces and underscores.

The document object contains these fields in this order:

| Field | Type | Meaning |
| --- | --- | --- |
| `file name` | string | Input file name, without its parent directory. |
| `number of pages` | integer | Total page count in the PDF, not the selected-page count. |
| `author` | string or null | PDF author; use XMP creator when the PDF author is absent. |
| `title` | string or null | PDF title; use XMP title when the PDF title is absent. |
| `creation date` | string or null | PDF creation date; use XMP creation date when absent. |
| `modification date` | string or null | PDF modification date; use XMP modification date when absent. |
| `kids` | array | Flat list of serialized content elements in page and reading order. |

Write every root key shown above, including the four metadata keys whose
values may be null. The ordinary local pipeline does not emit a `hybrid`
field.

Each `kids` item is an object with a `type` discriminator. Common fields are:

| Field | Type | Meaning |
| --- | --- | --- |
| `type` | string | Element kind, such as `heading`, `paragraph`, `image`, or `table`. |
| `pdfua_tag` | string, optional | Source or inferred PDF/UA structure tag when available. |
| `id` | integer, optional | Nonzero recognized structure ID when available. |
| `level` | string, optional | Semantic level when the element has one. |
| `page number` | integer | One-based page number. |
| `bounding box` | four numbers | `[left, bottom, right, top]` in PDF page coordinates. |

Coordinates use the PDF page coordinate system. Do not reorder the box into
`[left, top, right, bottom]`. Numeric serialization follows the Java output's
three-decimal precision for measured layout values.

Round each serialized floating-point layout value to three decimal places
using decimal `HALF_UP` rounding. Do not round coordinates before geometric
analysis; rounding is an output-only operation. Integers such as IDs and page
numbers remain integers. The property order shown for the document root is
stable; consumers should otherwise treat JSON object property order as
non-semantic.

### Element kinds

The JSON writer omits drawing-only `LineArtChunk` elements. It omits semantic
header/footer elements unless `include-header-footer` is enabled. It serializes
the following content kinds when extraction produces them:

| `type` | Kind-specific fields and rules |
| --- | --- |
| `heading` | Common fields, integer `heading level`, `font`, `font size`, `content`; `text color` when available. |
| `paragraph` | Common fields, `font`, `font size`, `content`; `text color` when available. It is serialized by `SemanticTextNodeSerializer`, not the unused `ParagraphSerializer`. |
| `text chunk` | `content` plus common fields when a low-level extracted text chunk is retained. |
| `text block` | Common fields, `font`, `font size`, `text color`, and `content` when a text block is serialized directly. |
| `image` | Image fields described in [Image output](#image-output). |
| `image` (semantic picture) | Same `image` discriminator and image fields; retain semantic description/caption metadata only when the serializer has it. |
| `list` | Common fields, `numbering style`, `number of list items`, optional `previous list id` and `next list id`, and `list items`. |
| `list item` | Common fields, text information, and `kids` for item content. |
| `table` | Common fields, `number of rows`, `number of columns`, optional `previous table id` and `next table id`, and `rows`. A `text block` table representation instead uses `kids`. |
| `table row` | `type`, one-based `row number`, `id`, and `cells`. A cell spanning rows or columns appears only in its origin row and column. |
| `table cell` | Common fields, one-based `row number` and `column number`, `row span`, `column span`, optional `is header: true`, and `kids`. Line-art children are omitted. |
| `caption` | Common fields, text information, optional `linked content id`, and available metadata. |
| `formula` | Formula content in the source representation, typically LaTeX. |
| `header` or `footer` | Header/footer content and common geometry fields. These are filtered out by default. |

Semantic text-node serialization writes `font`, `font size`, and `content`
fields even when a value is null; `text color` is written only when available,
and `hidden_text` is written as `true` only for hidden text. A heading adds
`heading level`. Omit absent optional values unless the element serializer
explicitly writes null. Do not invent empty metadata or alt text.

The common `pdfua_tag` is inferred from output type when available: heading
uses `H1` through `H6` (or `H` when its level is unknown), paragraph uses `P`,
image uses `Figure`, formula uses `Formula`, caption uses `Caption`, list uses
`L`, list item uses `LI`, TOC uses `TOC`, TOC item uses `TOCI`, table uses
`Table`, and table cells use `TH` or `TD`. Low-level text, text block, line,
header, and footer nodes have no canonical tag in this mapper. Use the exact
spelling and case supplied by the Java constants.

Tables serialize rows and cells in their logical row/column order. A cell can
include `row number`, `column number`, content, and span information when
present in the extracted table model. Each cell appears once even when it spans
multiple rows or columns.

JSON elements are flattened into `kids`; they are not nested under page objects.
The page association comes from each element's one-based `page number`.

### JSON image fields

For every image, write `alt_source`: `original` when nonempty source alt text
exists, otherwise `missing` in the local no-model flow. Include `alt` only for
a nonempty description; never synthesize it. For external images, include
`source` with the relative image path when the corresponding file exists. For
embedded images, include `data` as a data URI and `format` when image bytes are
available; omit `source`.

If extraction or image writing fails for one image, keep the rest of the PDF
output. Do not emit a broken `source` or `data` value for that image. Preserve
the image element and its geometry if the JSON serializer can represent it
without image bytes.

The Java serializer can attach element metadata when present, including
`ai_score`, `source label`, `heading inference`, `tsr`, `caption`,
`regionlist resolution`, `word match`, `text source`, and
`stream ocr similarity`. These fields are conditional; the local default path
usually has no hybrid metadata.

The current Jackson setup has a compatibility quirk: `ParagraphSerializer`
exists but is not registered. Paragraph instances therefore serialize through
the registered semantic-text-node serializer and inherit its runtime type and
fields. Do not implement the unused paragraph-specific serializer as if it
were current output. Likewise, only serializers registered by the local
pipeline define observable output; a class in the source tree is not by itself
proof that the pipeline emits that node.

## Markdown output

Write UTF-8 Markdown in ascending page order, consuming the same final page
content and ordering as JSON. Do not add a document title or page heading
unless it exists in the extracted content. The default page separator is empty.
After each supported content element, write two line-feed characters (LF),
including after the last element. A configured page separator is written only
for selected pages and before that page's content; replace `%page-number%`
with the one-based page number. Pages not selected contribute no content or
separator. When a nonempty page separator is written, it is followed by the
same two-LF content separator before page content.

Render supported elements as follows:

| Element | Markdown representation |
| --- | --- |
| Heading | Prefix text with `#` repeated for the heading level clamped to 1–6, then one space. |
| Paragraph or text node | Emit the extracted text. |
| List | Emit the Java list marker (`- `) and item content. Preserve nested children recursively; do not infer numeric markers from an ordered-list label unless the content has that explicit form. |
| Table | Emit pipe-delimited rows. Treat the first row as the Markdown header and put a separator row after it. In the plain Markdown mode, represent merged cells using the Java empty-cell behavior. The optional HTML-in-Markdown mode emits HTML table markup to preserve merged row/column spans. |
| Formula | Emit `$$`, a line break, the formula text, a line break, and `$$`. |
| Picture or image | Emit `![ALT](<DESTINATION>)` when a usable file or data URI exists. |
| Header or footer | Emit only when `include-header-footer` is enabled. |
| Unsupported extraction object | Omit it. |

For Markdown text, convert null to an empty string, remove U+0000, then escape
ampersand, less-than, and greater-than characters in that order as `&amp;`,
`&lt;`, and `&gt;`. This behavior also escapes existing entities. When
`keep-line-breaks` is false, line breaks within table cells become spaces.
When it is true, table-cell breaks use the configured table-safe break
representation. Heading breaks are still normalized to spaces. Outside cells,
preserve or normalize line breaks according to the element writer.

For an image link to a file, use a relative destination under the image
directory and wrap the destination in angle brackets. Replace `<`, `>`,
backslash, carriage return, and line feed in the destination with spaces before
writing it. Use the extracted image description as alt text when available;
otherwise use an empty alt string.

The default file name is the input PDF base name with the `.md` extension. The
exact output-directory contract is intentionally unspecified here. The Java
writer dispatches only supported semantic nodes; low-level chunks and line-art
geometry do not become Markdown text merely because they appear in JSON.

## Image output

Run image extraction when JSON or Markdown output is requested and image output
is enabled. The default image mode is external. The default image directory is
`<PDF_BASE_NAME>_images` under the selected output directory; a caller-supplied
image directory overrides it.

Walk pages and their elements in order. Assign image indexes starting at 1.
Use the source PDF image object when one is available. If it is unavailable,
render and crop the image element's page bounding box at the configured image
resolution. Write PNG by default; support JPEG when selected. The Java
reference name pattern is `imageFile<INDEX>.<EXT>` in the per-PDF image
directory. The index is one-based, document-local, and increments in page
order and then final element order. PNG uses `.png`; JPEG uses `.jpeg`, matching
the configured format value.

The Markdown and JSON references must point to the same file. Keep the relative
path stable for the same output layout. Do not write an image file when image
output is off. Do not emit a file reference for a file that was not written.
For JSON, `source` identifies the relative path; for Markdown, use the
corresponding relative path as the link destination. Create the image directory
before writing the first image. A failure to create that directory is an
output I/O failure; a failure for one image after directory creation should
not invalidate unrelated extracted text.

### Embedded image priority

Base64 image output is lower priority than external files. The Java behavior
uses `data:<mime-type>;base64,<bytes>` for JSON and Markdown. It rejects
individual embedded images larger than 10 MiB; omit the failed image reference
without failing the document. A first Rust version may use a fixed placeholder
instead of encoding bytes, but it must not present that placeholder as a valid
data URI. Keep the option and output shape available for later completion.

## Errors and partial output

Treat input-open, parse, and document-level extraction failures as conversion
errors. Do not report success with fabricated empty content. A missing image or
image encoding failure is local to that image and must not discard successfully
extracted text or other images.

The Java JSON and Markdown writer methods catch output exceptions and log a
warning rather than propagating them from those methods. This is an
implementation detail, not a requirement to silently succeed in Rust. The
external CLI and C-library error/status contract is deliberately left to the
project owner. The Rust conversion core must retain enough error detail to let
those interfaces report parse, password, input I/O, output I/O, and
document-processing failures distinctly.

If both output formats are requested, derive both from the same extracted page
content and use identical text, page ordering, geometry, and image numbering.
Do not run independent extraction passes for JSON and Markdown.

## Reference fixtures

The commands in this section regenerate Java oracle data only. They are not
required to build or test the Rust implementation. Rust tests use the
committed files under `data/` and do not read Java build or decompilation
directories.

Use the committed `data/pdf/lorem.pdf` input with
`data/json/lorem.json` and `data/markdown/lorem.md`. The JSON fixture is
the Java output. The Markdown fixture has also been confirmed against the Java
CLI. From the repository root, regenerate both references with:

```shell
mvn -f java/pom.xml -pl opendataloader-pdf-cli -am package -Dmaven.test.skip=true
java -jar java/opendataloader-pdf-cli/target/opendataloader-pdf-cli-2.5.13-SNAPSHOT.jar \
  -f json,markdown -q -o target/java-reference data/pdf/lorem.pdf
diff -u data/json/lorem.json target/java-reference/lorem.json
diff -u data/markdown/lorem.md target/java-reference/lorem.md
```

Both `diff` commands currently succeed with no differences.

The `lorem.pdf` fixture exercises document metadata, one heading, and one
paragraph. It does not exercise image output, tables, lists, or page selection.

These fixtures are human-reviewable parity references, not a complete corpus.
Add targeted fixtures for images, tables, lists, headers/footers, formulas,
multiple pages, and malformed PDFs before claiming full parity.

The committed Markdown example records the Java output for the existing
one-page fixture, including the final two LF characters required by the Java
content separator. Compare decoded strings, not visual Markdown rendering, so
trailing spaces, escapes, and final line feeds remain testable. Compare JSON
semantically for object key order, but compare arrays, strings, numbers, nulls,
and omitted properties exactly.

### First-round conformance checklist

An implementation is ready for independent review when it can demonstrate
each item below against the source behavior and fixture corpus:

1. Reject non-PDF signatures and malformed or truncated parser input; preserve
   a distinct password-required/invalid-password failure.
2. Read the page count and metadata without modifying the source file. Apply
   XMP metadata fallback only where the corresponding PDF information value is
   absent.
3. Process ordinary selectable-text PDF 1.x pages locally without invoking
   OCR, a model, or a network service. Raster-only pages yield no recognized
   text.
4. Produce deterministic page and reading order, one-based output page
   numbers, stable IDs, and `[left,bottom,right,top]` boxes.
5. Apply the default filtering values from the table, then infer semantic
   paragraphs, headings, lists, tables, and captions only when evidence exists.
6. Serialize the JSON root, metadata keys, type strings, element children,
   optional fields, and image references as specified here.
7. Produce Markdown with the same extracted content and ordering, the Java
   escaping rules, semantic headings/lists/tables/images, and exact separator
   behavior.
8. Write external image output using deterministic names, supported formats,
   and consistent JSON/Markdown paths. An image-specific failure leaves other
   content usable.
9. Keep Base64 support explicitly lower priority. A placeholder must not be
   mistaken for a valid data URI.

The current `lorem` fixture verifies only a subset of these items. Do not treat
the checklist as completed by passing that fixture alone.

## Parser boundary and dependency evidence

The Java extraction code does not directly implement PDF tokenization. It
opens the veraPDF parser/model stack, obtains page artifacts, and then runs its
own filtering and semantic reconstruction. CFR decompilation of the exact
cached `wcag-validation-1.31.175.jar` shows that `GFSAPDFDocument` parses page
content streams and delegates each page's artifacts to `GFSAPage`. That page
adapter wraps only `TextChunk`, `ImageChunk`, and `LineArtChunk` as its public
page artifacts; other chunk types are not passed into this pipeline.

The exact cached `wcag-validation-1.31.175.jar` `ChunkParser` implementation
creates text chunks from PDF text operators with text, bounding boxes, font
name, size, weight, italic angle, baseline, fill color, and rotation data. It
creates image chunks for inline-image (`BI`) operators and image XObjects
invoked by `Do`, retaining image bounds and available XObject stream identity.
It creates line-art chunks for path geometry. The first Rust parser boundary
must therefore preserve these three primitive kinds and their geometry; the
later Java processors turn them into semantic text, table, and image output.
Raster page content does not become text unless text operators supply it: this
path contains no OCR stage.

The current Java dependency set observed in the checkout is useful for
identifying the behavior source. These Java libraries are not mandated Rust
dependencies:

| Java component | Observed version | Role in this flow |
| --- | --- | --- |
| veraPDF parser | `1.31.55` | Opens local PDFs and parses page content. |
| veraPDF PDF model | `1.31.13` | Represents PDF objects, pages, and content chunks. |
| veraPDF WCAG algorithms | `1.31.44` | Supplies page chunks, text grouping and layout abstractions used by extraction. |
| veraPDF WCAG validation | `1.31.175` | Supplies `GFSAPDFDocument` and page artifact adapters. The local flow is not PDF/A validation. |
| veraPDF validation model | `1.31.175` | Shared model types consumed by the parser and WCAG layer. |
| PDFBox | `3.0.4` | Image rendering, crop extraction, and PDF utility operations; it is not the primary semantic text extractor here. |
| Jackson | Project-resolved version | JSON serialization and custom serializers. |

Exact source JARs for the cached veraPDF versions listed above are not present
in the local Maven cache. Source JARs for nearby releases do not prove
exact-version behavior. CFR 0.152 was downloaded from its official GitHub
release and checksum-verified. The exact parser, PDF model, WCAG algorithm,
WCAG validation, and validation-model JARs were decompiled under the ignored
`target/decompiled/` directory. Use the Just recipes in the repository root to
repeat this optional oracle inspection. Do not add generated decompilation
output to Git or present uninspected transitive-library behavior as an exact
Rust contract.

## Java source map

These source files define the behavior this document summarizes:

- `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/processors/DocumentProcessor.java`
  — extraction pipeline, ordering, sanitization, and output dispatch.
- `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/json/JsonWriter.java`
  — document metadata, page flattening, and JSON output.
- `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/json/serializers/`
  — element-specific JSON fields.
- `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/markdown/MarkdownGenerator.java`
  — Markdown type dispatch and formatting.
- `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/utils/ImagesUtils.java`
  — image numbering, extraction, and files.
- `java/opendataloader-pdf-core/src/main/java/org/opendataloader/pdf/utils/Base64ImageUtils.java`
  — embedded image data URIs and size limit.
