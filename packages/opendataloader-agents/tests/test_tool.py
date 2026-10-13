import asyncio
import json
from pathlib import Path

import pytest
from agents import Agent

from opendataloader_agents import parse_pdf

PDF = Path(__file__).resolve().parents[3] / "data/pdf/lorem.pdf"


def invoke(input_path: str, format: str) -> str:
    return asyncio.run(
        parse_pdf.on_invoke_tool(None, json.dumps({"input_path": input_path, "format": format}))
    )


def test_tool_metadata():
    agent = Agent(name="reader", instructions="Read PDFs.", tools=[parse_pdf])

    assert parse_pdf.name == "parse_pdf"
    assert parse_pdf.params_json_schema["properties"]["format"]["enum"] == ["json", "markdown"]
    assert parse_pdf in agent.tools


@pytest.mark.parametrize("format, suffix", [("json", ".json"), ("markdown", ".md")])
def test_tool_returns_existing_conversion_output(monkeypatch, tmp_path, format, suffix):
    expected = '{"source": "native"}' if format == "json" else "# native\n"

    def convert(input_path, output_dir, format):
        Path(output_dir, f"{Path(input_path).stem}{suffix}").write_text(expected, encoding="utf-8")

    monkeypatch.setattr("opendataloader.convert", convert)

    assert invoke(str(PDF), format) == expected


@pytest.mark.parametrize("format", ["text", "", "HTML"])
def test_tool_rejects_unsupported_format(format):
    with pytest.raises(ValueError, match="format"):
        parse_pdf.on_invoke_tool
        invoke(str(PDF), format)


@pytest.mark.parametrize("input_path", ["missing.pdf", str(Path(__file__).parent)])
def test_tool_rejects_invalid_path(input_path):
    with pytest.raises(FileNotFoundError, match="Input file not found"):
        invoke(input_path, "json")
