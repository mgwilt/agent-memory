#!/usr/bin/env python3
"""Validate Rust coverage for modified production code."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

CHANGED_LINE_THRESHOLD = 90.0
SMALL_CHANGE_LINE_LIMIT = 10
SMALL_CHANGE_MAX_UNCOVERED = 1
WORKSPACE_MAX_PERCENT_DROP = 0.2
WORKSPACE_MAX_UNCOVERED_LINE_INCREASE = 5
CARGO_LLVM_COV_VERSION = "0.8.7"

DEFAULT_COVERAGE_JSON = Path("target/coverage/coverage.json")
DEFAULT_BASELINE = Path("coverage/baseline.json")
PRODUCTION_PATH_RE = re.compile(r"^crates/[^/]+/src/.+\.rs$")
PRODUCTION_PATHSPECS = (
    ":(glob)crates/*/src/*.rs",
    ":(glob)crates/*/src/**/*.rs",
)
HUNK_RE = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@")


class CoverageGateError(Exception):
    """Raised for expected coverage gate failures."""


@dataclass(frozen=True)
class ChangedLine:
    path: str
    line: int
    text: str


@dataclass(frozen=True)
class LineResult:
    path: str
    line: int
    covered: bool


@dataclass(frozen=True)
class MetricTotals:
    lines_count: int
    lines_covered: int
    regions_count: int
    regions_covered: int
    functions_count: int
    functions_covered: int

    @property
    def line_percent(self) -> float:
        return percent(self.lines_covered, self.lines_count)

    @property
    def region_percent(self) -> float:
        return percent(self.regions_covered, self.regions_count)

    @property
    def function_percent(self) -> float:
        return percent(self.functions_covered, self.functions_count)

    @property
    def uncovered_lines(self) -> int:
        return self.lines_count - self.lines_covered

    def to_json(self) -> dict[str, Any]:
        return {
            "lines": {
                "count": self.lines_count,
                "covered": self.lines_covered,
                "percent": round(self.line_percent, 4),
            },
            "regions": {
                "count": self.regions_count,
                "covered": self.regions_covered,
                "percent": round(self.region_percent, 4),
            },
            "functions": {
                "count": self.functions_count,
                "covered": self.functions_covered,
                "percent": round(self.function_percent, 4),
            },
            "uncovered_lines": self.uncovered_lines,
        }


@dataclass(frozen=True)
class FileCoverage:
    path: str
    lines: dict[int, bool]


def percent(covered: int, count: int) -> float:
    if count == 0:
        return 100.0
    return covered * 100.0 / count


def is_production_path(path: str) -> bool:
    return bool(PRODUCTION_PATH_RE.match(path))


def is_potentially_executable_source_line(text: str) -> bool:
    stripped = text.strip()
    if not stripped:
        return False
    if stripped.startswith(("//", "///", "//!", "/*", "*", "*/")):
        return False
    if stripped.startswith(("#[", "#![")):
        return False
    if re.fullmatch(r"[{}\[\](),;:.]+", stripped):
        return False
    if re.fullmatch(r"(pub\s+)?(use|mod)\b.*;?", stripped):
        return False
    return not bool(re.fullmatch(r"extern\s+crate\b.*;?", stripped))


def run_capture(command: list[str]) -> str:
    completed = subprocess.run(
        command,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout


def changed_diff_command(args: argparse.Namespace) -> list[str]:
    command = ["git", "diff", "--unified=0", "--no-ext-diff"]
    if args.staged:
        command.append("--cached")
    else:
        command.append(f"{args.base}...{args.head}")
    command.append("--")
    command.extend(PRODUCTION_PATHSPECS)
    return command


def parse_changed_lines(diff_text: str) -> list[ChangedLine]:
    changed: list[ChangedLine] = []
    current_path: str | None = None
    new_line: int | None = None

    for raw_line in diff_text.splitlines():
        if raw_line.startswith("+++ "):
            path = raw_line[4:]
            if path == "/dev/null":
                current_path = None
            elif path.startswith("b/"):
                current_path = path[2:]
            else:
                current_path = path
            if current_path is not None and not is_production_path(current_path):
                current_path = None
            continue

        hunk = HUNK_RE.match(raw_line)
        if hunk:
            new_line = int(hunk.group(1))
            continue

        if new_line is None:
            continue
        if raw_line.startswith("\\"):
            continue
        if raw_line.startswith("+") and not raw_line.startswith("+++"):
            if current_path is not None:
                changed.append(ChangedLine(current_path, new_line, raw_line[1:]))
            new_line += 1
        elif raw_line.startswith("-") and not raw_line.startswith("---"):
            continue
        else:
            new_line += 1

    return changed


def changed_production_lines(args: argparse.Namespace) -> list[ChangedLine]:
    try:
        diff_text = run_capture(changed_diff_command(args))
    except subprocess.CalledProcessError as exc:
        detail = exc.stderr.strip() or exc.stdout.strip() or str(exc)
        raise CoverageGateError(f"failed to read git diff for coverage gate: {detail}") from exc
    return parse_changed_lines(diff_text)


def ensure_cargo_llvm_cov() -> None:
    completed = subprocess.run(
        ["cargo", "llvm-cov", "--version"],
        capture_output=True,
        text=True,
    )
    if completed.returncode == 0:
        return

    raise CoverageGateError(
        "cargo-llvm-cov is required for the Rust coverage precommit gate.\n"
        "Install it with `cargo install cargo-llvm-cov --version "
        f"{CARGO_LLVM_COV_VERSION} --locked` and ensure the `llvm-tools-preview` "
        "rustup component is available."
    )


def run_coverage(output_path: Path) -> None:
    ensure_cargo_llvm_cov()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    command = [
        "cargo",
        "llvm-cov",
        "--workspace",
        "--all-targets",
        "--json",
        "--output-path",
        str(output_path),
    ]
    try:
        subprocess.run(command, check=True)
    except subprocess.CalledProcessError as exc:
        raise CoverageGateError(f"cargo llvm-cov failed with exit code {exc.returncode}") from exc


def repo_relative_path(filename: str, repo_root: Path) -> str:
    path = Path(filename)
    if path.is_absolute():
        try:
            return path.resolve(strict=False).relative_to(repo_root).as_posix()
        except ValueError:
            return path.as_posix()
    return path.as_posix()


def read_source_lines(repo_root: Path, path: str) -> list[str]:
    source_path = repo_root / path
    try:
        return source_path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        return []


def segment_parts(segment: list[Any]) -> tuple[int, int, bool, bool]:
    line = int(segment[0])
    count = int(segment[2])
    has_count = bool(segment[3])
    is_gap = bool(segment[5]) if len(segment) > 5 else False
    return line, count, has_count, is_gap


def mark_covered_lines(
    result: dict[int, bool],
    source_lines: list[str],
    start: int,
    end: int,
    count: int,
) -> None:
    if start <= 0 or end <= 0:
        return
    last_line = min(end, len(source_lines))
    for line_no in range(start, last_line + 1):
        if not is_potentially_executable_source_line(source_lines[line_no - 1]):
            continue
        result[line_no] = result.get(line_no, False) or count > 0


def line_coverage_from_segments(
    segments: list[list[Any]],
    source_lines: list[str],
) -> dict[int, bool]:
    result: dict[int, bool] = {}
    normalized = sorted(segments, key=lambda segment: (int(segment[0]), int(segment[1])))

    active_start: int | None = None
    active_count: int | None = None

    for segment in normalized:
        line, count, has_count, is_gap = segment_parts(segment)
        if active_start is not None and active_count is not None:
            end = line if line == active_start else line - 1
            mark_covered_lines(result, source_lines, active_start, end, active_count)

        if has_count and not is_gap:
            active_start = line
            active_count = count
        else:
            active_start = None
            active_count = None

    if active_start is not None and active_count is not None:
        mark_covered_lines(result, source_lines, active_start, len(source_lines), active_count)

    return result


def summary_count(summary: dict[str, Any], metric: str, field: str) -> int:
    return int(summary.get(metric, {}).get(field, 0))


def load_coverage(
    coverage_json: Path,
    repo_root: Path,
) -> tuple[dict[str, FileCoverage], MetricTotals]:
    try:
        payload = json.loads(coverage_json.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise CoverageGateError(f"coverage JSON was not created at {coverage_json}") from exc
    except json.JSONDecodeError as exc:
        raise CoverageGateError(f"coverage JSON is invalid: {exc}") from exc

    files: dict[str, FileCoverage] = {}
    totals = {
        "lines_count": 0,
        "lines_covered": 0,
        "regions_count": 0,
        "regions_covered": 0,
        "functions_count": 0,
        "functions_covered": 0,
    }

    for data in payload.get("data", []):
        for file_payload in data.get("files", []):
            path = repo_relative_path(str(file_payload.get("filename", "")), repo_root)
            if not is_production_path(path):
                continue

            source_lines = read_source_lines(repo_root, path)
            files[path] = FileCoverage(
                path=path,
                lines=line_coverage_from_segments(file_payload.get("segments", []), source_lines),
            )

            summary = file_payload.get("summary", {})
            totals["lines_count"] += summary_count(summary, "lines", "count")
            totals["lines_covered"] += summary_count(summary, "lines", "covered")
            totals["regions_count"] += summary_count(summary, "regions", "count")
            totals["regions_covered"] += summary_count(summary, "regions", "covered")
            totals["functions_count"] += summary_count(summary, "functions", "count")
            totals["functions_covered"] += summary_count(summary, "functions", "covered")

    return files, MetricTotals(**totals)


def executable_line_results(
    changed: list[ChangedLine],
    coverage: dict[str, FileCoverage],
) -> tuple[list[LineResult], list[str]]:
    results: list[LineResult] = []
    missing_files: dict[str, list[int]] = {}

    for changed_line in changed:
        if not is_potentially_executable_source_line(changed_line.text):
            continue

        file_coverage = coverage.get(changed_line.path)
        if file_coverage is None:
            missing_files.setdefault(changed_line.path, []).append(changed_line.line)
            continue

        if changed_line.line in file_coverage.lines:
            results.append(
                LineResult(
                    path=changed_line.path,
                    line=changed_line.line,
                    covered=file_coverage.lines[changed_line.line],
                )
            )

    missing = [
        f"{path} (changed executable-looking lines: {format_line_list(lines)})"
        for path, lines in sorted(missing_files.items())
    ]
    return results, missing


def format_line_list(lines: list[int]) -> str:
    unique = sorted(set(lines))
    if len(unique) <= 8:
        return ", ".join(str(line) for line in unique)
    return ", ".join(str(line) for line in unique[:8]) + ", ..."


def changed_line_failures(results: list[LineResult]) -> list[str]:
    if not results:
        return []

    uncovered = [result for result in results if not result.covered]
    covered_count = len(results) - len(uncovered)
    coverage_percent = percent(covered_count, len(results))

    if len(results) < SMALL_CHANGE_LINE_LIMIT:
        if len(uncovered) <= SMALL_CHANGE_MAX_UNCOVERED:
            return []
        threshold = (
            f"fewer than {SMALL_CHANGE_LINE_LIMIT} changed executable lines may have "
            f"at most {SMALL_CHANGE_MAX_UNCOVERED} uncovered line"
        )
    else:
        if coverage_percent + 1e-9 >= CHANGED_LINE_THRESHOLD:
            return []
        threshold = f"changed executable line coverage must be >= {CHANGED_LINE_THRESHOLD:.1f}%"

    lines = "\n".join(f"  {result.path}:{result.line}" for result in uncovered[:25])
    if len(uncovered) > 25:
        lines += f"\n  ... and {len(uncovered) - 25} more"

    return [
        (
            f"changed executable production line coverage is {coverage_percent:.2f}% "
            f"({covered_count}/{len(results)} covered); {threshold}.\n"
            f"Uncovered changed lines:\n{lines}"
        )
    ]


def read_baseline(path: Path) -> MetricTotals:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise CoverageGateError(
            f"coverage baseline is missing at {path}; generate it with "
            "`python3 scripts/check_rust_coverage.py --update-baseline`."
        ) from exc
    except json.JSONDecodeError as exc:
        raise CoverageGateError(f"coverage baseline is invalid JSON: {exc}") from exc

    totals = payload.get("totals", {})
    try:
        return MetricTotals(
            lines_count=int(totals["lines"]["count"]),
            lines_covered=int(totals["lines"]["covered"]),
            regions_count=int(totals["regions"]["count"]),
            regions_covered=int(totals["regions"]["covered"]),
            functions_count=int(totals["functions"]["count"]),
            functions_covered=int(totals["functions"]["covered"]),
        )
    except (KeyError, TypeError, ValueError) as exc:
        raise CoverageGateError(f"coverage baseline has an unsupported schema: {path}") from exc


def workspace_ratchet_failures(current: MetricTotals, baseline: MetricTotals) -> list[str]:
    failures: list[str] = []
    comparisons = (
        ("line", current.line_percent, baseline.line_percent),
        ("region", current.region_percent, baseline.region_percent),
        ("function", current.function_percent, baseline.function_percent),
    )
    for label, current_percent, baseline_percent in comparisons:
        drop = baseline_percent - current_percent
        if drop > WORKSPACE_MAX_PERCENT_DROP + 1e-9:
            failures.append(
                f"{label} coverage dropped by {drop:.2f} percentage points "
                f"({baseline_percent:.2f}% -> {current_percent:.2f}%), "
                f"exceeding the {WORKSPACE_MAX_PERCENT_DROP:.1f} point ratchet."
            )

    uncovered_increase = current.uncovered_lines - baseline.uncovered_lines
    if uncovered_increase > WORKSPACE_MAX_UNCOVERED_LINE_INCREASE:
        failures.append(
            f"uncovered executable lines increased by {uncovered_increase} "
            f"({baseline.uncovered_lines} -> {current.uncovered_lines}), exceeding the "
            f"{WORKSPACE_MAX_UNCOVERED_LINE_INCREASE} line ratchet."
        )

    return failures


def write_baseline(path: Path, totals: MetricTotals) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "schema_version": 1,
        "coverage_backend": "cargo llvm-cov",
        "cargo_llvm_cov_version": CARGO_LLVM_COV_VERSION,
        "coverage_command": (
            "cargo llvm-cov --workspace --all-targets --json "
            "--output-path target/coverage/coverage.json"
        ),
        "scope": "crates/*/src/**/*.rs",
        "thresholds": {
            "changed_line_percent": CHANGED_LINE_THRESHOLD,
            "small_change_line_limit": SMALL_CHANGE_LINE_LIMIT,
            "small_change_max_uncovered": SMALL_CHANGE_MAX_UNCOVERED,
            "workspace_max_percent_drop": WORKSPACE_MAX_PERCENT_DROP,
            "workspace_max_uncovered_line_increase": WORKSPACE_MAX_UNCOVERED_LINE_INCREASE,
        },
        "totals": totals.to_json(),
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate Rust coverage for changed production lines.",
    )
    parser.add_argument("--staged", action="store_true", help="check staged changes for precommit")
    parser.add_argument("--base", default="origin/main", help="base ref for commit-range checks")
    parser.add_argument("--head", default="HEAD", help="head ref for commit-range checks")
    parser.add_argument(
        "--coverage-json",
        type=Path,
        default=DEFAULT_COVERAGE_JSON,
        help="path for cargo llvm-cov JSON output",
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        default=DEFAULT_BASELINE,
        help="workspace coverage baseline file",
    )
    parser.add_argument(
        "--update-baseline",
        action="store_true",
        help="refresh the baseline from the current workspace coverage and exit",
    )
    parser.add_argument(
        "--use-existing-coverage",
        action="store_true",
        help="read an existing coverage JSON instead of running cargo llvm-cov",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    repo_root = Path.cwd().resolve()

    try:
        changed = changed_production_lines(args)
        executable_candidates = [
            line for line in changed if is_potentially_executable_source_line(line.text)
        ]

        if not args.update_baseline and not executable_candidates:
            print("coverage gate: no changed executable production Rust lines; skipping")
            return 0

        if not args.use_existing_coverage:
            run_coverage(args.coverage_json)

        coverage, current_totals = load_coverage(args.coverage_json, repo_root)

        if args.update_baseline:
            write_baseline(args.baseline, current_totals)
            print(f"coverage gate: wrote baseline to {args.baseline}")
            print(
                "coverage gate: "
                f"lines {current_totals.line_percent:.2f}%, "
                f"regions {current_totals.region_percent:.2f}%, "
                f"functions {current_totals.function_percent:.2f}%, "
                f"uncovered lines {current_totals.uncovered_lines}"
            )
            return 0

        results, missing = executable_line_results(changed, coverage)
        failures: list[str] = []
        if missing:
            failures.append(
                "coverage JSON did not include changed production files:\n  " + "\n  ".join(missing)
            )
        failures.extend(changed_line_failures(results))
        failures.extend(workspace_ratchet_failures(current_totals, read_baseline(args.baseline)))

        if failures:
            raise CoverageGateError("\n\n".join(failures))

        if results:
            covered_count = sum(1 for result in results if result.covered)
            print(
                "coverage gate: changed executable production lines "
                f"{percent(covered_count, len(results)):.2f}% covered "
                f"({covered_count}/{len(results)})"
            )
        else:
            print("coverage gate: no changed executable lines found in coverage data")
        print(
            "coverage gate: workspace ratchet "
            f"lines {current_totals.line_percent:.2f}%, "
            f"regions {current_totals.region_percent:.2f}%, "
            f"functions {current_totals.function_percent:.2f}%, "
            f"uncovered lines {current_totals.uncovered_lines}"
        )
        return 0
    except CoverageGateError as exc:
        print(f"coverage gate: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
