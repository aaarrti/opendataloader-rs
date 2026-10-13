"""OpenDataLoader tools for the OpenAI Agents SDK."""

import tempfile
from pathlib import Path
from typing import Literal

import opendataloader
from agents import function_tool


@function_tool
def parse_pdf(input_path: str, format: Literal["json", "markdown"] = "json") -> str:
    """Convert one local PDF and return its JSON or Markdown content.

    Args:
        input_path: Path to a local PDF file.
        format: Output format, either ``"json"`` or ``"markdown"``.

    Returns:
        The generated UTF-8 JSON or Markdown content.

    Raises:
        FileNotFoundError: If ``input_path`` is not a file.
        ValueError: If ``format`` is unsupported.
        OSError: If local PDF conversion fails.
    """
    input_file = Path(input_path).expanduser().resolve()
    if not input_file.is_file():
        raise FileNotFoundError(f"Input file not found: {input_path}")
    if format not in {"json", "markdown"}:
        raise ValueError("format must be 'json' or 'markdown'")

    with tempfile.TemporaryDirectory() as temporary_directory:
        output_dir = Path(temporary_directory)
        opendataloader.convert(input_file, output_dir, format=format)
        output_file = output_dir / f"{input_file.stem}{'.json' if format == 'json' else '.md'}"
        if not output_file.is_file():
            raise OSError(f"conversion produced no {format} output for {input_path}")
        return output_file.read_text(encoding="utf-8")
