#!/usr/bin/env python3
"""
tests/test_extract_empirical_benchmark_metrics.py

Comprehensive test suite verifying scripts/extract_empirical_benchmark_metrics.py.
Validates tool categorization, ISO 8601 parsing, wall-clock duration calculation,
token aggregation, speedup multiplier, token reduction percentage, and markdown output.
"""

from datetime import datetime, timezone
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from scripts.extract_empirical_benchmark_metrics import (
    CAT_FAST_CORE_GREP,
    CAT_FAST_CORE_OUTLINE,
    CAT_FILE_TOOLS,
    CAT_MESSAGING,
    CAT_OTHER,
    CAT_POWERSHELL_CLI,
    CAT_STAIR_AST_SLICE,
    CAT_VECTOR_RAG,
    CAT_VERIFICATION_TEST,
    analyze_transcript,
    categorize_tool_call,
    compute_comparative_metrics,
    format_markdown_report,
    parse_iso,
)

REAL_TRANSCRIPT_A = Path(
    r"C:\Users\krish\.gemini\antigravity\brain\17c5c7da-86eb-412f-87dc-9469b6f8b840\.system_generated\logs\transcript.jsonl"
)
REAL_TRANSCRIPT_B = Path(
    r"C:\Users\krish\.gemini\antigravity\brain\337894f6-cf89-4494-87d7-5bac5d440513\.system_generated\logs\transcript.jsonl"
)


class TestMetricExtractionParser(unittest.TestCase):

    def test_parse_iso(self):
        dt1 = parse_iso("2026-11-16T07:20:29Z")
        self.assertEqual(dt1.year, 2026)
        self.assertEqual(dt1.month, 11)
        self.assertEqual(dt1.day, 16)
        self.assertEqual(dt1.hour, 7)
        self.assertEqual(dt1.minute, 20)
        self.assertEqual(dt1.second, 29)
        self.assertEqual(dt1.tzinfo, timezone.utc)

        dt2 = parse_iso("2026-11-16T07:20:29.500000Z")
        self.assertEqual(dt2.microsecond, 500000)

    def test_categorize_powershell_tools(self):
        # Select-String
        prim, gran = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": 'Select-String -Path "file.py" -Pattern "test"'}
        })
        self.assertEqual(prim, CAT_POWERSHELL_CLI)
        self.assertEqual(gran, "powershell_search")

        # Get-ChildItem
        prim, gran = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": 'Get-ChildItem -Path "." -Recurse'}
        })
        self.assertEqual(prim, CAT_POWERSHELL_CLI)
        self.assertEqual(gran, "powershell_search")

        # Get-Content
        prim, gran = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": 'Get-Content -Path "test.txt"'}
        })
        self.assertEqual(prim, CAT_POWERSHELL_CLI)
        self.assertEqual(gran, "powershell_file_io")

    def test_categorize_fast_core_tools(self):
        # CLI grep
        prim, gran = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": 'target\\release\\antigravity-fast-core.exe --grep "class ROLE"'}
        })
        self.assertEqual(prim, CAT_FAST_CORE_GREP)

        # CLI def slice
        prim, gran = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": 'target\\release\\antigravity-fast-core.exe --def "views/member.py::ProjectMemberViewSet"'}
        })
        self.assertEqual(prim, CAT_STAIR_AST_SLICE)

        # CLI outline
        prim, gran = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": 'antigravity-fast-core --workspace . --outline'}
        })
        self.assertEqual(prim, CAT_FAST_CORE_OUTLINE)

        # CLI semantic search
        prim, gran = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": 'antigravity-fast-core --semantic "project member role authorization"'}
        })
        self.assertEqual(prim, CAT_VECTOR_RAG)

        # Quoted CLI execution (PowerShell)
        prim_quoted, _ = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": '& "target\\release\\antigravity-fast-core.exe" --grep "Role"'}
        })
        self.assertEqual(prim_quoted, CAT_FAST_CORE_GREP)

        prim_single_quoted, _ = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": "'antigravity-fast-core.exe' --def 'apps/api/views.py::Member'"}
        })
        self.assertEqual(prim_single_quoted, CAT_STAIR_AST_SLICE)

    def test_categorize_fast_core_no_false_positives(self):
        # Comments/documentation mentioning fast-core must NOT be classified as fast-core
        prim, gran = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": 'Write-Output "DO NOT use fast-core CLI (antigravity-fast-core.exe)" | Set-Content -Path "BRIEFING.md"'}
        })
        self.assertEqual(prim, CAT_POWERSHELL_CLI)
        self.assertEqual(gran, "powershell_file_io")

        prim2, gran2 = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": 'python -c "# Note: fast-core documentation test"'}
        })
        self.assertEqual(prim2, CAT_POWERSHELL_CLI)
        self.assertEqual(gran2, "powershell_command")

    def test_categorize_mcp_and_direct_tools(self):
        # Native / MCP tools
        self.assertEqual(categorize_tool_call({"name": "instant_grep"})[0], CAT_FAST_CORE_GREP)
        self.assertEqual(categorize_tool_call({"name": "get_symbol_outline"})[0], CAT_FAST_CORE_OUTLINE)
        self.assertEqual(categorize_tool_call({"name": "get_symbol_definition"})[0], CAT_STAIR_AST_SLICE)
        self.assertEqual(categorize_tool_call({"name": "semantic_search"})[0], CAT_VECTOR_RAG)

        # call_mcp_tool wrapper
        prim, gran = categorize_tool_call({
            "name": "call_mcp_tool",
            "args": {"ServerName": "fast-core", "ToolName": "instant_grep", "Arguments": {"query": "foo"}}
        })
        self.assertEqual(prim, CAT_FAST_CORE_GREP)

        # Standard file tools
        self.assertEqual(categorize_tool_call({"name": "read_file"})[0], CAT_FILE_TOOLS)
        self.assertEqual(categorize_tool_call({"name": "edit_file"})[0], CAT_FILE_TOOLS)
        self.assertEqual(categorize_tool_call({"name": "write_file"})[0], CAT_FILE_TOOLS)

        # Verification test
        prim, _ = categorize_tool_call({
            "name": "run_command",
            "args": {"CommandLine": "python tests/verify_plane_member_feature.py --workspace ."}
        })
        self.assertEqual(prim, CAT_VERIFICATION_TEST)

        # Messaging
        self.assertEqual(categorize_tool_call({"name": "send_message"})[0], CAT_MESSAGING)

    def test_comparative_metrics_math(self):
        m_a = {
            "duration_seconds": 120.0,
            "input_tokens": 10000,
            "output_tokens": 2000,
            "total_billed_tokens": 12000,
            "tool_calls_total": 20,
            "total_steps": 25,
            "planner_steps": 10,
        }
        m_b = {
            "duration_seconds": 60.0,
            "input_tokens": 5000,
            "output_tokens": 1000,
            "total_billed_tokens": 6000,
            "tool_calls_total": 10,
            "total_steps": 15,
            "planner_steps": 6,
        }
        comp = compute_comparative_metrics(m_a, m_b)

        # S_time = 120.0 / 60.0 = 2.0x
        self.assertEqual(comp["speedup_multiplier"], 2.0)
        # Input reduction: (10000 - 5000) / 10000 = 50.0%
        self.assertEqual(comp["input_reduction_pct"], 50.0)
        # Output reduction: (2000 - 1000) / 2000 = 50.0%
        self.assertEqual(comp["output_reduction_pct"], 50.0)
        # Total token reduction: (12000 - 6000) / 12000 = 50.0%
        self.assertEqual(comp["total_token_reduction_pct"], 50.0)
        # Tool reduction: (20 - 10) / 20 = 50.0%
        self.assertEqual(comp["tool_reduction_pct"], 50.0)
        self.assertEqual(comp["step_delta"], -10)
        self.assertEqual(comp["planner_delta"], -4)

    def test_analyze_synthetic_transcript(self):
        synthetic_records = [
            {
                "step_index": 0,
                "source": "SYSTEM",
                "type": "SYSTEM_MESSAGE",
                "created_at": "2026-11-16T10:00:00Z",
                "content": "Start benchmark",
            },
            {
                "step_index": 1,
                "source": "MODEL",
                "type": "PLANNER_RESPONSE",
                "created_at": "2026-11-16T10:00:05Z",
                "input_tokens": 1500,
                "output_tokens": 200,
                "cache_read_tokens": 500,
                "tool_calls": [
                    {"name": "run_command", "args": {"CommandLine": "antigravity-fast-core.exe --grep test"}},
                    {"name": "read_file", "args": {"Path": "test.py"}},
                ],
            },
            {
                "step_index": 2,
                "source": "MODEL",
                "type": "GENERIC",
                "created_at": "2026-11-16T10:00:10Z",
                "content": "Tool output",
            },
            {
                "step_index": 3,
                "source": "MODEL",
                "type": "PLANNER_RESPONSE",
                "created_at": "2026-11-16T10:00:15Z",
                "input_tokens": 2000,
                "output_tokens": 300,
                "cache_read_tokens": 1000,
                "tool_calls": [
                    {"name": "send_message", "args": {"Message": "Done"}},
                ],
            },
        ]

        with tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".jsonl", delete=False) as f:
            for rec in synthetic_records:
                f.write(json.dumps(rec) + "\n")
            tmp_path = Path(f.name)

        try:
            metrics = analyze_transcript(tmp_path)
            self.assertEqual(metrics["duration_seconds"], 15.0)
            self.assertEqual(metrics["total_steps"], 4)
            self.assertEqual(metrics["planner_steps"], 2)
            self.assertEqual(metrics["generic_steps"], 1)
            self.assertEqual(metrics["system_steps"], 1)
            self.assertEqual(metrics["input_tokens"], 3500)
            self.assertEqual(metrics["output_tokens"], 500)
            self.assertEqual(metrics["cache_read_tokens"], 1500)
            self.assertEqual(metrics["total_billed_tokens"], 4000)
            self.assertEqual(metrics["tool_calls_total"], 3)
            self.assertEqual(metrics["primary_tool_counts"][CAT_FAST_CORE_GREP], 1)
            self.assertEqual(metrics["primary_tool_counts"][CAT_FILE_TOOLS], 1)
            self.assertEqual(metrics["primary_tool_counts"][CAT_MESSAGING], 1)
        finally:
            tmp_path.unlink(missing_ok=True)

    def test_real_transcripts_if_present(self):
        if REAL_TRANSCRIPT_A.exists() and REAL_TRANSCRIPT_B.exists():
            res_a = analyze_transcript(REAL_TRANSCRIPT_A)
            res_b = analyze_transcript(REAL_TRANSCRIPT_B)

            self.assertEqual(res_a["duration_seconds"], 60.0)
            self.assertEqual(res_a["total_steps"], 16)
            self.assertEqual(res_a["planner_steps"], 8)
            self.assertEqual(res_a["input_tokens"], 29505)
            self.assertEqual(res_a["output_tokens"], 5143)
            self.assertEqual(res_a["cache_read_tokens"], 102062)

            self.assertEqual(res_b["duration_seconds"], 76.0)
            self.assertEqual(res_b["total_steps"], 20)
            self.assertEqual(res_b["planner_steps"], 10)
            self.assertEqual(res_b["input_tokens"], 40842)
            self.assertEqual(res_b["output_tokens"], 4723)
            self.assertEqual(res_b["cache_read_tokens"], 182281)

            comp = compute_comparative_metrics(res_a, res_b)
            self.assertEqual(comp["speedup_multiplier"], 0.79)

            # Test report generation
            report_md = format_markdown_report(res_a, res_b, comp, None, None)
            self.assertIn("# Empirical Head-to-Head Agent Benchmark Report", report_md)
            self.assertIn("60.00 s", report_md)
            self.assertIn("76.00 s", report_md)
            self.assertIn("0.79x", report_md)

    def test_edge_cases(self):
        # Division by zero guards
        comp_zero = compute_comparative_metrics(
            {"duration_seconds": 0.0, "input_tokens": 0, "output_tokens": 0, "total_billed_tokens": 0, "tool_calls_total": 0, "total_steps": 0, "planner_steps": 0},
            {"duration_seconds": 0.0, "input_tokens": 0, "output_tokens": 0, "total_billed_tokens": 0, "tool_calls_total": 0, "total_steps": 0, "planner_steps": 0},
        )
        self.assertEqual(comp_zero["speedup_multiplier"], 0.0)
        self.assertEqual(comp_zero["input_reduction_pct"], 0.0)
        self.assertEqual(comp_zero["total_token_reduction_pct"], 0.0)

        # File not found
        with self.assertRaises(FileNotFoundError):
            analyze_transcript(Path("non_existent_transcript.jsonl"))

        # Empty file
        with tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".jsonl", delete=False) as f:
            f.write("")
            empty_path = Path(f.name)
        try:
            with self.assertRaises(ValueError):
                analyze_transcript(empty_path)
        finally:
            empty_path.unlink(missing_ok=True)

        # Unknown tool handling
        prim, gran = categorize_tool_call({"name": "custom_arbitrary_tool", "args": {}})
        self.assertEqual(prim, CAT_OTHER)
        self.assertEqual(gran, "custom_arbitrary_tool")

    def test_cli_execution(self):
        if REAL_TRANSCRIPT_A.exists() and REAL_TRANSCRIPT_B.exists():
            with tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".md", delete=False) as out_f:
                out_path = Path(out_f.name)

            try:
                cmd = [
                    sys.executable,
                    "scripts/extract_empirical_benchmark_metrics.py",
                    "--transcript-a",
                    str(REAL_TRANSCRIPT_A),
                    "--transcript-b",
                    str(REAL_TRANSCRIPT_B),
                    "--output",
                    str(out_path),
                    "--json",
                ]
                proc = subprocess.run(cmd, capture_output=True, text=True)
                self.assertEqual(proc.returncode, 0, f"CLI failed: {proc.stderr}")

                data = json.loads(proc.stdout)
                self.assertIn("agent_a", data)
                self.assertIn("agent_b", data)
                self.assertIn("comparative", data)
                self.assertTrue(out_path.exists())
                content = out_path.read_text(encoding="utf-8")
                self.assertIn("Empirical Head-to-Head Agent Benchmark Report", content)
            finally:
                out_path.unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
