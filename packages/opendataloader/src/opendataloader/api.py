"""Local PDF conversion through the bundled native library."""

import ctypes
import os
from functools import singledispatch
from pathlib import Path
from threading import RLock
from typing import Literal

_CHAR_POINTER = ctypes.c_char_p
_CHAR_POINTERS = ctypes.POINTER(_CHAR_POINTER)
_OPTION_JSON = 1
_OPTION_MARKDOWN = 2
_OPTION_IMAGES = 4
_OPTION_PARALLEL = 8
_STATUS_OK = 0
_STATUS_INVALID_ARGUMENT = 1
_STATUS_CONVERSION_ERROR = 2
_STATUS_PANIC = 3
_LIBRARY: ctypes.CDLL | None = None
_LIBRARY_LOCK = RLock()


type _OutputFormat = Literal["json", "markdown"]


@singledispatch
def _paths(input_path: object) -> list[Path]:
    raise TypeError("input_path must be a str, Path, or list of either")


@_paths.register
def _(input_path: str) -> list[Path]:
    return [Path(input_path)]


@_paths.register
def _(input_path: Path) -> list[Path]:
    return [input_path]


@_paths.register(list)
def _(input_paths: list[str | Path]) -> list[Path]:
    return [Path(input_path) for input_path in input_paths]


def _load_library() -> ctypes.CDLL:
    global _LIBRARY
    with _LIBRARY_LOCK:
        if _LIBRARY is not None:
            return _LIBRARY
        library_path = Path(os.environ.get("ODL_LIB_PATH", Path(__file__).with_name("libodl.so")))
        try:
            library = ctypes.CDLL(library_path)
        except OSError as error:
            raise OSError(f"unable to load native library {library_path}: {error}") from error
        library.odl_convert.argtypes = [_CHAR_POINTERS, ctypes.c_uint, _CHAR_POINTER, ctypes.c_uint]
        library.odl_convert.restype = ctypes.c_int
        library.odl_last_error.argtypes = []
        library.odl_last_error.restype = _CHAR_POINTER
        _LIBRARY = library
        return library


def _native_error(library: ctypes.CDLL) -> str:
    message = library.odl_last_error()
    return message.decode("utf-8", errors="replace") if message else "native conversion failed"


def convert(
    input_path: str | Path | list[str | Path],
    output_dir: str | Path,
    format: set[_OutputFormat] | list[_OutputFormat] | _OutputFormat | None = None,
    parallel: bool = True,
    image_output_enabled: bool = False,
) -> None:
    """Convert local PDF files to JSON or Markdown, with optional image files.

    Args:
        input_path: A PDF path or a list of PDF paths. Directory inputs are
            not supported.
        output_dir: Directory where the generated files are written. The
            directory is created if it does not exist.
        format: Output formats to write: ``"json"``, ``"markdown"``, or
            both. Defaults to JSON.
        parallel: Process multiple PDF files concurrently. Defaults to
            ``True``; a single PDF is processed serially.
        image_output_enabled: Write extracted images to files. Defaults to
            ``False``.

    Returns:
        ``None``. The conversion results are written to ``output_dir``.

    Raises:
        TypeError: If ``input_path`` is not a string, ``Path``, or list.
        ValueError: If no input paths are provided or ``format`` is empty or
            contains an unsupported value.
        FileNotFoundError: If an input path does not refer to a file.
        OSError: If the native library cannot be loaded or conversion fails.
        RuntimeError: If the native conversion reports a panic or an unknown
            status.
    """
    paths = _paths(input_path)
    if not paths:
        raise ValueError("input_path must contain at least one PDF path")
    for path in paths:
        if not path.is_file():
            raise FileNotFoundError(path)

    if format is None:
        formats = {"json", "markdown"}
    elif isinstance(format, str):
        formats = {format}
    else:
        formats = set(format)

    if not formats or not formats <= {"json", "markdown"}:
        raise ValueError("format must contain only 'json' and 'markdown'")
    destination = Path(output_dir)
    destination.mkdir(parents=True, exist_ok=True)
    mode = (
        (_OPTION_JSON if "json" in formats else 0)
        | (_OPTION_MARKDOWN if "markdown" in formats else 0)
        | (_OPTION_IMAGES if image_output_enabled else 0)
        | (_OPTION_PARALLEL if parallel else 0)
    )
    encoded_paths = [str(path).encode() for path in paths]
    native_paths = (_CHAR_POINTER * len(encoded_paths))(
        *(ctypes.c_char_p(path) for path in encoded_paths)
    )
    native_output = str(destination).encode()
    library = _load_library()
    with _LIBRARY_LOCK:
        status = library.odl_convert(native_paths, len(paths), native_output, mode)
        if status == _STATUS_OK:
            return
        detail = _native_error(library)
    if status == _STATUS_INVALID_ARGUMENT:
        raise ValueError(detail)
    if status == _STATUS_CONVERSION_ERROR:
        raise OSError(f"conversion failed for {paths}: {detail}")
    if status == _STATUS_PANIC:
        raise RuntimeError(f"native conversion panicked for {paths}: {detail}")
    raise RuntimeError(f"unknown native conversion status {status}: {detail}")
