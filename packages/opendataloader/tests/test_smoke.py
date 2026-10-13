import json
from pathlib import Path
from tempfile import TemporaryDirectory

from opendataloader import convert

PDF = Path(__file__).resolve().parents[3] / "data/pdf/lorem.pdf"


def test_e2e():
    with TemporaryDirectory() as temporary_directory:
        root = Path(temporary_directory)

        single_output = root / "single"
        assert convert(PDF, single_output, format=["json"]) is None
        json_output = single_output / "lorem.json"
        assert json_output.is_file()
        document = json.loads(json_output.read_text())
        assert document["file name"] == "lorem.pdf"
        assert document["number of pages"] == 1

        batch_output = root / "batch"
        assert convert([PDF, PDF], batch_output, format=["json", "markdown"]) is None
        batch_json = batch_output / "lorem.json"
        batch_markdown = batch_output / "lorem.md"
        assert batch_json.is_file()
        assert json.loads(batch_json.read_text())["file name"] == "lorem.pdf"
        assert batch_markdown.is_file()
        assert batch_markdown.is_file()

        try:
            convert(root / "missing.pdf", root / "missing", format=["json"])
        except FileNotFoundError:
            pass
        else:
            raise AssertionError("missing PDF was accepted")

        try:
            convert(root, root / "directory", format=["json"])
        except FileNotFoundError:
            pass
        else:
            raise AssertionError("directory was accepted")
