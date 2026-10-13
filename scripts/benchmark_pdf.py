"""Compare the PyPI and Rust-backed OpenDataLoader PDF converters."""

import argparse
import importlib.metadata
import json
import math
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from collections.abc import Iterator, Sequence
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SAMPLE_DIR = REPO_ROOT / "data" / "stg"
DEFAULT_TARGET_GB = 1.0
FILES_PER_DIRECTORY = 500


def parse_args() -> argparse.Namespace:
    """Parse benchmark options and worker settings.

    Returns:
        Parsed command-line options.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sample-dir", type=Path, default=DEFAULT_SAMPLE_DIR)
    parser.add_argument(
        "--target-gb",
        type=float,
        default=DEFAULT_TARGET_GB,
        help="minimum copied PDF input size in decimal gigabytes (default: 1)",
    )
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--batch-size", type=int, default=500)
    parser.add_argument("--temp-dir", type=Path, default=Path("/var/tmp"))
    parser.add_argument(
        "--worker-backend",
        choices=("pypi", "rust"),
        help=argparse.SUPPRESS,
    )
    parser.add_argument("--worker-input", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--worker-output", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--worker-repeats", type=int, help=argparse.SUPPRESS)
    parser.add_argument("--worker-batch-size", type=int, help=argparse.SUPPRESS)
    return parser.parse_args()


def copy_inputs(sample_dir: Path, input_dir: Path, target_bytes: int) -> tuple[int, int]:
    """Copy sample PDFs until the input corpus reaches the requested size.

    Args:
        sample_dir: Directory containing the source PDF samples.
        input_dir: Destination directory for unique PDF copies.
        target_bytes: Minimum total size of the copied PDFs.

    Returns:
        A tuple containing the number of copies and their total byte size.

    Raises:
        ValueError: If the sample directory contains no PDF files.
    """
    samples = sorted(path for path in sample_dir.rglob("*.pdf") if path.is_file())
    if not samples:
        raise ValueError(f"no PDF samples found in {sample_dir}")

    copied_count = 0
    copied_bytes = 0
    while copied_bytes < target_bytes:
        for sample in samples:
            directory = input_dir / f"{copied_count // FILES_PER_DIRECTORY:05d}"
            directory.mkdir(exist_ok=True)
            destination = directory / f"{copied_count:08d}-{sample.stem}.pdf"
            shutil.copyfile(sample, destination)
            copied_count += 1
            copied_bytes += sample.stat().st_size
            if copied_bytes >= target_bytes:
                break
    return copied_count, copied_bytes


def input_batches(input_dir: Path, batch_size: int) -> Iterator[list[str]]:
    """Yield sorted input path batches without loading the full corpus."""
    batch = []
    for directory in sorted(input_dir.iterdir()):
        for path in sorted(directory.glob("*.pdf")):
            batch.append(str(path))
            if len(batch) == batch_size:
                yield batch
                batch = []
    if batch:
        yield batch


def clear_directory(directory: Path) -> None:
    """Remove conversion outputs while keeping their directory available."""
    for path in directory.iterdir():
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()


def check_cache_flush_access() -> None:
    """Require root or noninteractive sudo before the benchmark starts."""
    cache_file = Path("/proc/sys/vm/drop_caches")
    if not cache_file.is_file():
        raise RuntimeError(f"Linux cache control is unavailable: {cache_file}")
    if os.access(cache_file, os.W_OK):
        return
    sudo = shutil.which("sudo")
    if (
        sudo is None
        or subprocess.run(
            [sudo, "-n", "true"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        ).returncode
    ):
        raise RuntimeError(
            "cache flushing requires write access to /proc/sys/vm/drop_caches "
            "or passwordless sudo"
        )


def flush_filesystem_caches() -> None:
    """Sync writes and drop Linux page, inode, and directory-entry caches."""
    cache_file = Path("/proc/sys/vm/drop_caches")
    subprocess.run(["sync"], check=True)
    if os.access(cache_file, os.W_OK):
        cache_file.write_text("3\n", encoding="ascii")
    else:
        subprocess.run(
            ["sudo", "-n", "tee", str(cache_file)],
            input="3\n",
            text=True,
            stdout=subprocess.DEVNULL,
            check=True,
        )


def convert_batch(backend: str, paths: list[str], output_dir: Path) -> None:
    """Convert one batch with the selected package."""
    if backend == "pypi":
        import opendataloader_pdf

        opendataloader_pdf.convert(
            input_path=paths,
            output_dir=str(output_dir),
            format="json,markdown",
            image_output="off",
            quiet=True,
        )
    else:
        import opendataloader

        opendataloader.convert(
            input_path=paths,
            output_dir=output_dir,
            format=["json", "markdown"],
            parallel=True,
            image_output_enabled=False,
        )


def run_worker(
    backend: str,
    input_dir: Path,
    output_dir: Path,
    repeats: int,
    batch_size: int,
) -> None:
    """Run repeated conversions and print timing statistics."""
    output_dir.mkdir()
    package_name = "opendataloader-pdf" if backend == "pypi" else "opendataloader"
    package_version = importlib.metadata.version(package_name)
    conversion_times = []

    for _ in range(repeats):
        clear_directory(output_dir)
        flush_filesystem_caches()
        elapsed = 0.0
        for paths in input_batches(input_dir, batch_size):
            started = time.perf_counter()
            convert_batch(backend, paths, output_dir)
            elapsed += time.perf_counter() - started
            clear_directory(output_dir)
        conversion_times.append(elapsed)

    stats = {
        "backend": backend,
        "parallel": backend == "rust",
        "package_version": package_version,
        "pdf_count_per_pass": sum(1 for _ in input_dir.rglob("*.pdf")),
        "repeats": repeats,
        "conversion_seconds": conversion_times,
        "mean_seconds": statistics.mean(conversion_times),
        "stddev_seconds": statistics.stdev(conversion_times) if len(conversion_times) > 1 else 0.0,
        "min_seconds": min(conversion_times),
        "max_seconds": max(conversion_times),
    }
    stats["pdfs_per_second"] = stats["pdf_count_per_pass"] / stats["mean_seconds"]
    print(json.dumps(stats, indent=2))


def check_capacity(
    temp_parent: Path,
    target_bytes: int,
    sample_dir: Path,
    batch_size: int,
) -> tuple[int, int, int]:
    """Reject a run before copying if its input corpus will not fit."""
    samples = sorted(path for path in sample_dir.rglob("*.pdf") if path.is_file())
    if not samples:
        raise ValueError(f"no PDF samples found in {sample_dir}")
    cycle_bytes = sum(path.stat().st_size for path in samples)
    cycles = math.ceil(target_bytes / cycle_bytes)
    copies = cycles * len(samples)
    required_bytes = cycles * cycle_bytes
    usage = shutil.disk_usage(temp_parent)
    reserve_bytes = 1_000_000_000
    if required_bytes + reserve_bytes > usage.free:
        raise ValueError(
            f"the copied input PDFs need about {required_bytes:,} bytes, but "
            f"{temp_parent} has {usage.free:,} bytes free; reserve at least "
            f"{reserve_bytes:,} bytes for environments and batch outputs"
        )

    stats = os.statvfs(temp_parent)
    reserve_inodes = 20_000 + 2 * batch_size
    if stats.f_favail and copies + reserve_inodes > stats.f_favail:
        raise ValueError(
            f"the run needs {copies:,} PDF files plus about {reserve_inodes:,} "
            f"temporary files, but {temp_parent} has {stats.f_favail:,} free inodes"
        )
    return len(samples), copies, required_bytes


def run_command(
    command: Sequence[str | Path], *, cwd: Path | None = None, env: dict[str, str] | None = None
) -> None:
    """Run a command and stop with its error status on failure."""
    print("+", " ".join(map(str, command)), flush=True)
    subprocess.run(command, check=True, cwd=cwd, env=env)


def venv_python(venv_dir: Path) -> Path:
    """Return the Python executable path inside a virtual environment."""
    return venv_dir / "bin" / "python"


def run_benchmark(args: argparse.Namespace) -> None:
    """Prepare isolated environments, copy inputs, and run both backends."""
    sample_dir = args.sample_dir.resolve()
    if (
        not math.isfinite(args.target_gb)
        or args.target_gb <= 0
        or args.repeats < 1
        or args.batch_size < 1
    ):
        raise ValueError("target gigabytes, repeats, and batch size must be positive")
    target_bytes = int(args.target_gb * 1_000_000_000)
    if shutil.which("uv") is None:
        raise RuntimeError("uv is required to create isolated benchmark environments")
    if shutil.which("java") is None:
        raise RuntimeError("Java is required by the PyPI OpenDataLoader PDF package")
    if sys.platform != "linux":
        raise RuntimeError("this benchmark supports Linux only")
    check_cache_flush_access()

    temp_parent = args.temp_dir.resolve()
    sample_count, expected_copies, expected_bytes = check_capacity(
        temp_parent, target_bytes, sample_dir, args.batch_size
    )
    print(
        f"Samples: {sample_count}; copied PDFs: about {expected_copies}; "
        f"target size: {args.target_gb:g} GB; estimated input size: "
        f"{expected_bytes:,} bytes; repeats: {args.repeats}; "
        f"batch size: {args.batch_size}",
        flush=True,
    )

    with tempfile.TemporaryDirectory(prefix="opendataloader-benchmark-", dir=temp_parent) as name:
        work_dir = Path(name)
        input_dir = work_dir / "inputs"
        input_dir.mkdir()
        copied_count, copied_bytes = copy_inputs(sample_dir, input_dir, target_bytes)
        print(f"Created {copied_count} PDF copies ({copied_bytes:,} bytes).", flush=True)

        upstream_env = work_dir / "venv-pypi"
        rust_env = work_dir / "venv-rust"
        run_command(["uv", "venv", "--python", sys.executable, upstream_env])
        run_command(
            ["uv", "pip", "install", "--python", venv_python(upstream_env), "opendataloader-pdf"]
        )
        run_command(["uv", "venv", "--python", sys.executable, rust_env])
        rust_wheels = sorted((REPO_ROOT / "dist").glob("opendataloader-*.whl"))
        if len(rust_wheels) != 1:
            raise RuntimeError(
                f"expected one ready opendataloader wheel in dist, found {len(rust_wheels)}"
            )
        run_command(
            [
                "uv",
                "pip",
                "install",
                "--python",
                venv_python(rust_env),
                "--no-deps",
                "--no-editable",
                rust_wheels[0],
            ]
        )

        for backend, python, env in (
            ("pypi", venv_python(upstream_env), None),
            ("rust", venv_python(rust_env), None),
        ):
            output_dir = work_dir / f"output-{backend}"
            command = [
                python,
                Path(__file__).resolve(),
                "--worker-backend",
                backend,
                "--worker-input",
                input_dir,
                "--worker-output",
                output_dir,
                "--worker-repeats",
                str(args.repeats),
                "--worker-batch-size",
                str(args.batch_size),
            ]
            child_env = os.environ.copy()
            if env:
                child_env.update(env)
            run_command(command, cwd=REPO_ROOT, env=child_env)


def main() -> None:
    """Run the benchmark or its isolated worker process."""
    args = parse_args()
    try:
        if args.worker_backend:
            run_worker(
                args.worker_backend,
                args.worker_input,
                args.worker_output,
                args.worker_repeats,
                args.worker_batch_size,
            )
        else:
            run_benchmark(args)
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"benchmark failed: {error}") from error


if __name__ == "__main__":
    main()
