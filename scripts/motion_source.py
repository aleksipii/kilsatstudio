"""Read and write the MotionScript `source` of a .kilsat project.

Koeta Studio has one YAML formatter, `motion_script::format` in Rust. Python
generators use it through the `format_script` example instead of keeping a
second emitter, and read YAML back through it because the standard library
has no YAML parser. Legacy one-line JSON sources are still read directly.

Set KOETA_FORMAT_SCRIPT to a built `format_script` binary to skip `cargo run`.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
from functools import lru_cache
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


@lru_cache(maxsize=1)
def _tool() -> tuple[str, ...]:
    explicit = os.environ.get("KOETA_FORMAT_SCRIPT")
    if explicit:
        return (explicit,)
    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("MotionScript YAML needs cargo or KOETA_FORMAT_SCRIPT=<format_script binary>")
    subprocess.run([cargo, "build", "-q", "-p", "motion-script", "--example", "format_script"],
                   cwd=ROOT, check=True)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    binary = target / "debug" / "examples" / "format_script"
    if not binary.is_file():
        raise RuntimeError(f"format_script was not built at {binary}")
    return (str(binary),)


def _run(flag: str, text: str) -> str:
    result = subprocess.run([*_tool(), flag], input=text.encode("utf-8"),
                            capture_output=True, check=False)
    if result.returncode != 0:
        raise ValueError(f"format_script {flag}: {result.stderr.decode('utf-8', 'replace').strip()}")
    return result.stdout.decode("utf-8")


def dumps(script: dict, header: str | None = None, sections: dict[str, str] | None = None) -> str:
    """Canonical MotionScript YAML for a screenplay given as plain data.

    `header` becomes the file's opening comment and `sections` maps top-level
    keys to a comment written above that section. Values go to the formatter as
    JSON, which is valid YAML flow syntax, so comments survive formatting.
    """
    sections = sections or {}
    lines = [f"# {line}".rstrip() for line in (header or "").splitlines()]
    if lines:
        lines.append("")
    for key, value in script.items():
        if key in sections:
            lines.extend(f"# {line}".rstrip() for line in sections[key].splitlines())
        lines.append(f"{json.dumps(key, ensure_ascii=False)}: "
                     f"{json.dumps(value, ensure_ascii=False, allow_nan=False)}")
    return _run("--stdin", "\n".join(lines) + "\n")


def loads(source: str) -> dict:
    """Screenplay data from MotionScript YAML (or a legacy JSON source)."""
    try:
        return json.loads(source)
    except json.JSONDecodeError:
        return json.loads(_run("--to-json", source))
