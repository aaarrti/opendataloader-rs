# OpenDataLoader PDF for Rust

OpenDataLoader PDF for Rust is a rewrite of the local PDF-to-JSON and
PDF-to-Markdown flow from [OpenDataLoader PDF](https://github.com/opendataloader-project/opendataloader-pdf).
It converts local PDF files and writes the selected outputs to a directory.

The Rust implementation focuses on local processing. It does not provide
hybrid or remote processing, OCR, or machine learning model support.

## Convert one PDF

From the repository root, use Cargo to convert a PDF to JSON and Markdown:

```sh
cargo run -p opendataloder_cli -- \
  --input-paths data/pdf/lorem.pdf \
  --out-dir out \
  --json \
  --markdown
```

Add `--image` to write extracted images as PNG files. The command requires at
least one of `--json` or `--markdown`.

## Convert multiple PDFs in parallel

Pass each input file to `--input-paths` and add `--parallel` to process the
files concurrently:

```sh
cargo run -p opendataloder_cli -- \
  --input-paths data/pdf/lorem.pdf data/pdf/empty.pdf \
  --out-dir out \
  --json \
  --markdown \
  --parallel
```

The CLI accepts PDF file paths. It does not traverse directories. When you omit
`--parallel`, it processes the files sequentially.

## Output

The CLI writes the selected JSON and Markdown outputs to the directory named by
`--out-dir`. Use `--image` to also write extracted images as PNG files.

## Attribution

This repository reimplements part of the
[OpenDataLoader PDF project](https://github.com/opendataloader-project/opendataloader-pdf)
in Rust. The upstream project provides the original PDF extraction behavior
and reference implementation.
