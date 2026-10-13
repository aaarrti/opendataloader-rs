### Guidelines

- This project is for private use. Do not add backward-compatibility layers or
  fallbacks.
- This repository is a Rust rewrite of the local PDF-to-JSON/Markdown flow of
  OpenDataLoader PDF. Keep implementation and documentation scoped to local
  PDF conversion; do not add hybrid, remote, OCR, or ML processing.
- Keep attribution to the original OpenDataLoader PDF project in the README
  and root `LICENSE`.
- Keep the project license, upstream attribution, and third-party license
  texts and notices in the root `LICENSE`. Do not add separate `NOTICE` or
  `THIRD_PARTY` license files.
- For Python scripts, do not add a shebang or `from __future__` imports.
- Keep Python package `__init__.py` files limited to package metadata and public
  imports or re-exports. Put function, class, and business logic in
  responsibility-specific modules; do not implement it in `__init__.py`.
- Write Python docstrings in Google style. Document arguments, return values,
  and raised exceptions where they apply.
- Write Python tests with `pytest`, not `unittest`.
- Implement work in small, incremental changes.
- When a task changes multiple files, fan out sub-agents to parallelize the
  work.
- If a required investigation or build tool is missing, do not look for or use
  a workaround. Tell the user which tool is missing and ask them to install it.

### Repository layout

- `crates/` contains the Rust source code.
- `packages/` contains supplementary packages.
- `data/` contains PDF inputs and committed oracle fixtures.
- `docs/` contains project documentation; put specifications in `docs/specs/`.
- `README.md` documents supported usage and examples. `LICENSE` is the single
  file for project and third-party license terms and notices.
- `Cargo.toml` and `pyproject.toml` contain project configuration.
- Treat remaining Java files as reference or legacy material unless the task
  explicitly asks you to change them.

### Rust conventions

- Do not place an entire Rust implementation in one file. Split code into
  logically grouped modules and files with one clear responsibility each, and
  keep each module self-contained with a narrow interface to other modules.
- Keep each Rust unit test in the same source file as the function or type it
  tests, usually in that module's inline `#[cfg(test)] mod tests`; do not gather
  unit tests in a separate shared test file.
- Use `tracing` events and spans for Rust logging and instrumentation. Initialize
  `tracing_subscriber` at the executable or application entry point; library
  crates emit tracing events but do not install a global subscriber.
- Use `anyhow::Result<T>` for fallible Rust functions and add context to errors
  when it helps identify the failed operation.
- Do not call `.unwrap()` in Rust code, including tests. Propagate errors with
  `?`; make test failures explicit with assertions or returned errors.
- Use `clap` to define and parse command-line arguments.
- Use `rayon` for parallel or multithreaded Rust work. Do not create a separate
  thread pool or use raw threads for parallel processing.

### Code discovery

Use the codebase-memory MCP graph tools before grep, glob, or file search when
discovering code. Call `index_repository` first if the repository is not
indexed. Prefer `search_graph`, `trace_path`, `get_code_snippet`, `query_graph`,
and `get_architecture`; use `search_code` when those tools do not provide enough
detail. Use grep or file search for string literals and non-code files.

### Documentation

Use the Google developer documentation style skill when writing or editing
project documentation.

### Terminal tools

- Use Zsh for shell scripts and interactive shell commands.
- Use `rg` and `rg --files` for text and file searches.
- Use `z` from zoxide for directory navigation.
- Prefer Zsh built-ins over Bash compatibility constructs; use `zargs` instead
  of `xargs`.

### Platform support

- Target Linux only. Do not add or maintain Windows or macOS support unless a
  task explicitly requests it.

### CFR decompiler

- Run `just download-cfr` to download the pinned CFR release into
  `target/tools/`. The recipe verifies its SHA-256 before installing the JAR.
- Run `just decompile-cfr <input.jar> <output-dir>` to decompile a JAR. For
  example, use `just decompile-cfr path/to/dependency.jar target/decompiled`.
- Update the pinned release version and checksum together when upgrading CFR.
- Decompile the exact dependency JAR when its source JAR is unavailable; do not
  substitute a nearby release's sources.
