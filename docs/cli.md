# Local conversion CLI

The `opendataloader` command converts one or more local PDF files. It does not
traverse directories.

```text
opendataloader --input-paths INPUT.pdf [MORE.pdf ...] --out-dir OUTPUT_DIR \
  [--json | --markdown] [--image] [--parallel]
```

At least one of `--json` or `--markdown` is required. `--image` writes PNG
files for extracted images and requires an output format. `--parallel` enables
Rayon processing only when multiple PDFs are supplied; it is opt-in.

Generated files are written below `--out-dir`, using each input filename stem:
`NAME.json`, `NAME.md`, and `NAME_images/`. Invalid arguments and conversion
failures return a nonzero process status and include the relevant input path
when conversion has started.
