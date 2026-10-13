"""MCP server for OpenDataLoader PDF."""

import tempfile
from pathlib import Path

import opendataloader
from mcp.server.mcpserver import MCPServer

mcp = MCPServer("opendataloader")


@mcp.tool()
def convert_pdf(
    input_path: str,
    ormat: str = "markdown",
) -> str:
    """Convert a PDF file to the specified format.

    Args:
        input_path: Path to the input PDF file.
        format: Output format. Values: json, text, html, markdown,
            markdown-with-html, markdown-with-images. Default: markdown.

    Returns:
        The converted content as text.
    """
    input_file = Path(input_path).expanduser().resolve()
    if not input_file.is_file():
        raise FileNotFoundError(f"Input file not found: {input_path}")

    # Determine output file extension from format
    ext_map = {
        "json": ".json",
        "markdown": ".md",
    }
    if format not in ext_map:
        raise ValueError(
            f"Unsupported format: {format!r}. " f"Supported formats: {', '.join(ext_map)}"
        )
    ext = ext_map[format]  # pyright: ignore[reportArgumentType]

    with tempfile.TemporaryDirectory() as tmp_dir:
        opendataloader.convert(input_path, tmp_dir, format)  # pyright: ignore[reportArgumentType]

        # Find and read the output file
        stem = input_file.stem
        output_file = Path(tmp_dir) / f"{stem}{ext}"

        if not output_file.is_file():
            files = [f for f in Path(tmp_dir).iterdir() if f.is_file()]
            if not files:
                raise RuntimeError("Conversion completed but no output file was generated.")
            matching_ext = sorted(f for f in files if f.suffix == ext)
            if not matching_ext:
                raise RuntimeError(
                    f"Conversion completed but no '{ext}' output file was generated."
                )
            output_file = matching_ext[0]

        return output_file.read_text(encoding="utf-8")


def main():
    """Run the MCP server."""
    mcp.run()


if __name__ == "__main__":
    main()
