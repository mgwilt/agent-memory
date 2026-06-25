from __future__ import annotations

import unittest

import check_rust_coverage as gate


class CoverageGateTest(unittest.TestCase):
    def test_parse_changed_lines_scopes_to_production_rust(self) -> None:
        diff = """diff --git a/crates/nestor-core/src/lib.rs b/crates/nestor-core/src/lib.rs
--- a/crates/nestor-core/src/lib.rs
+++ b/crates/nestor-core/src/lib.rs
@@ -1,0 +1,3 @@
+// just a comment
+let covered = true;
+pub use crate::chunk::Chunk;
diff --git a/crates/nestor-core/tests/example.rs b/crates/nestor-core/tests/example.rs
--- a/crates/nestor-core/tests/example.rs
+++ b/crates/nestor-core/tests/example.rs
@@ -1,0 +1 @@
+let ignored = true;
"""

        changed = gate.parse_changed_lines(diff)

        self.assertEqual(
            changed,
            [
                gate.ChangedLine("crates/nestor-core/src/lib.rs", 1, "// just a comment"),
                gate.ChangedLine("crates/nestor-core/src/lib.rs", 2, "let covered = true;"),
                gate.ChangedLine(
                    "crates/nestor-core/src/lib.rs", 3, "pub use crate::chunk::Chunk;"
                ),
            ],
        )

    def test_comments_and_imports_are_not_executable_candidates(self) -> None:
        self.assertFalse(gate.is_potentially_executable_source_line("// comment"))
        self.assertFalse(gate.is_potentially_executable_source_line("pub use crate::chunk::Chunk;"))
        self.assertFalse(gate.is_potentially_executable_source_line("#[derive(Debug)]"))
        self.assertTrue(gate.is_potentially_executable_source_line("let covered = true;"))

    def test_small_changes_allow_one_uncovered_line(self) -> None:
        results = [
            gate.LineResult("crates/nestor-core/src/lib.rs", 10, True),
            gate.LineResult("crates/nestor-core/src/lib.rs", 11, False),
        ]

        self.assertEqual(gate.changed_line_failures(results), [])

    def test_small_changes_fail_with_more_than_one_uncovered_line(self) -> None:
        results = [
            gate.LineResult("crates/nestor-core/src/lib.rs", 10, True),
            gate.LineResult("crates/nestor-core/src/lib.rs", 11, False),
            gate.LineResult("crates/nestor-core/src/lib.rs", 12, False),
        ]

        failures = gate.changed_line_failures(results)

        self.assertEqual(len(failures), 1)
        self.assertIn("at most 1 uncovered line", failures[0])

    def test_large_changes_require_ninety_percent_changed_line_coverage(self) -> None:
        passing = [
            gate.LineResult("crates/nestor-core/src/lib.rs", line, line != 10)
            for line in range(1, 11)
        ]
        failing = [
            gate.LineResult("crates/nestor-core/src/lib.rs", line, line not in {9, 10})
            for line in range(1, 11)
        ]

        self.assertEqual(gate.changed_line_failures(passing), [])
        self.assertEqual(len(gate.changed_line_failures(failing)), 1)

    def test_missing_coverage_file_is_reported(self) -> None:
        changed = [
            gate.ChangedLine("crates/nestor-core/src/lib.rs", 10, "let missing = true;"),
        ]

        results, missing = gate.executable_line_results(changed, {})

        self.assertEqual(results, [])
        self.assertEqual(
            missing,
            ["crates/nestor-core/src/lib.rs (changed executable-looking lines: 10)"],
        )

    def test_workspace_ratchet_fails_on_metric_drop_or_uncovered_line_growth(self) -> None:
        baseline = gate.MetricTotals(
            lines_count=100,
            lines_covered=90,
            regions_count=100,
            regions_covered=90,
            functions_count=100,
            functions_covered=90,
        )
        current = gate.MetricTotals(
            lines_count=110,
            lines_covered=88,
            regions_count=100,
            regions_covered=89,
            functions_count=100,
            functions_covered=90,
        )

        failures = gate.workspace_ratchet_failures(current, baseline)

        self.assertEqual(len(failures), 3)
        self.assertIn("line coverage dropped", failures[0])
        self.assertIn("region coverage dropped", failures[1])
        self.assertIn("uncovered executable lines increased", failures[2])


if __name__ == "__main__":
    unittest.main()
