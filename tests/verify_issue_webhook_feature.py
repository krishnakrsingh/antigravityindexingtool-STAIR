#!/usr/bin/env python3
"""
tests/verify_issue_webhook_feature.py

Verification Test Suite for Plane Issue Webhook Feature:
1. IssueWebhookDispatcher in apps/api/plane/services/webhook_dispatcher.py
   - Methods: generate_payload, compute_signature, dispatch, trigger_issue_event
   - Cryptographic HMAC-SHA256 signature verification
2. IssueWebhookLog model in apps/api/plane/db/models/webhook.py
   - Fields: webhook_id, issue_id, event_type, payload, response_status
3. TypeScript definitions in packages/types/src/webhook.ts
   - Interfaces: IIssueWebhookPayload, IIssueWebhookEvent, IIssueWebhookLog

Supports standalone CLI execution, python -m unittest, and pytest.
Zero external dependencies required (uses standard library ast, unittest, hmac, hashlib, sys, pathlib).
"""

import argparse
import ast
import hashlib
import hmac
import json
import os
import sys
import unittest
from pathlib import Path
from typing import Dict, List, Optional, Any

DEFAULT_WORKSPACE_CANDIDATES = [
    Path("benchmark_workspaces/workspace_b_enhanced/plane"),
    Path("benchmark_workspaces/workspace_a_stock/plane"),
    Path("target_repo/plane"),
    Path("."),
]

def resolve_workspace(workspace_arg: Optional[str] = None) -> Path:
    if workspace_arg:
        p = Path(workspace_arg).resolve()
        if p.exists():
            return p
        raise FileNotFoundError(f"Specified workspace path does not exist: {workspace_arg}")
    env_path = os.environ.get("PLANE_WORKSPACE")
    if env_path:
        p = Path(env_path).resolve()
        if p.exists():
            return p
    for candidate in DEFAULT_WORKSPACE_CANDIDATES:
        resolved = candidate.resolve()
        if (resolved / "apps" / "api" / "plane").exists():
            return resolved
    return Path.cwd().resolve()

class ASTInspector:
    @staticmethod
    def parse_file(file_path: Path) -> Optional[ast.AST]:
        if not file_path.exists():
            return None
        try:
            content = file_path.read_text(encoding="utf-8")
            return ast.parse(content, filename=str(file_path))
        except Exception:
            return None

    @staticmethod
    def find_classes(tree: ast.AST) -> Dict[str, ast.ClassDef]:
        classes = {}
        for node in ast.walk(tree):
            if isinstance(node, ast.ClassDef):
                classes[node.name] = node
        return classes

    @staticmethod
    def find_methods(class_node: ast.ClassDef) -> Dict[str, ast.FunctionDef]:
        methods = {}
        for item in class_node.body:
            if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)):
                methods[item.name] = item
        return methods

    @staticmethod
    def find_class_attributes(class_node: ast.ClassDef) -> List[str]:
        attrs = []
        for item in class_node.body:
            if isinstance(item, ast.Assign):
                for target in item.targets:
                    if isinstance(target, ast.Name):
                        attrs.append(target.id)
            elif isinstance(item, ast.AnnAssign):
                if isinstance(item.target, ast.Name):
                    attrs.append(item.target.id)
        return attrs

class TestPlaneIssueWebhookFeature(unittest.TestCase):
    workspace: Path = Path(".")

    def setUp(self):
        self.dispatcher_path = self.workspace / "apps" / "api" / "plane" / "services" / "webhook_dispatcher.py"
        self.model_path = self.workspace / "apps" / "api" / "plane" / "db" / "models" / "webhook.py"
        self.ts_path = self.workspace / "packages" / "types" / "src" / "webhook.ts"

    def test_01_dispatcher_file_exists(self):
        """Verify webhook_dispatcher.py exists in apps/api/plane/services/."""
        self.assertTrue(self.dispatcher_path.exists(), f"Missing file: {self.dispatcher_path}")

    def test_02_dispatcher_class_ast(self):
        """Verify IssueWebhookDispatcher class is defined with required methods."""
        tree = ASTInspector.parse_file(self.dispatcher_path)
        self.assertIsNotNone(tree, f"Failed to parse AST of {self.dispatcher_path}")
        classes = ASTInspector.find_classes(tree)
        self.assertIn("IssueWebhookDispatcher", classes, "Missing IssueWebhookDispatcher class")
        methods = ASTInspector.find_methods(classes["IssueWebhookDispatcher"])
        required_methods = ["generate_payload", "compute_signature", "dispatch", "trigger_issue_event"]
        for m in required_methods:
            self.assertIn(m, methods, f"Missing method {m} in IssueWebhookDispatcher")

    def test_03_hmac_sha256_cryptographic_correctness(self):
        """Verify HMAC-SHA256 signature computation logic matches standard Python hmac."""
        secret = "test_webhook_secret_key_12345"
        payload_bytes = b'{"event":"issue.created","issue_id":"uuid-123","sequence_id":42}'
        expected_sig = "sha256=" + hmac.new(secret.encode("utf-8"), payload_bytes, hashlib.sha256).hexdigest()

        tree = ASTInspector.parse_file(self.dispatcher_path)
        self.assertIsNotNone(tree, f"Missing {self.dispatcher_path} for cryptographic test")
        classes = ASTInspector.find_classes(tree)
        self.assertIn("IssueWebhookDispatcher", classes)
        ns = {}
        exec(compile(tree, filename=str(self.dispatcher_path), mode="exec"), ns)
        dispatcher_cls = ns.get("IssueWebhookDispatcher")
        self.assertIsNotNone(dispatcher_cls, "Failed to instantiate IssueWebhookDispatcher")
        computed = dispatcher_cls.compute_signature(secret, payload_bytes)
        self.assertEqual(computed, expected_sig, f"HMAC signature mismatch: {computed} != {expected_sig}")

    def test_04_webhook_model_file_exists(self):
        """Verify webhook.py exists in apps/api/plane/db/models/."""
        self.assertTrue(self.model_path.exists(), f"Missing file: {self.model_path}")

    def test_05_webhook_log_model_ast(self):
        """Verify IssueWebhookLog model exists with required audit fields."""
        tree = ASTInspector.parse_file(self.model_path)
        self.assertIsNotNone(tree, f"Failed to parse AST of {self.model_path}")
        classes = ASTInspector.find_classes(tree)
        self.assertIn("IssueWebhookLog", classes, "Missing IssueWebhookLog class in webhook.py")
        attrs = ASTInspector.find_class_attributes(classes["IssueWebhookLog"])
        required_fields = ["webhook_id", "issue_id", "event_type", "payload", "response_status"]
        for f in required_fields:
            self.assertIn(f, attrs, f"Missing required field {f} in IssueWebhookLog")

    def test_06_typescript_definitions_exist(self):
        """Verify webhook.ts exists in packages/types/src/ with TypeScript interfaces."""
        self.assertTrue(self.ts_path.exists(), f"Missing file: {self.ts_path}")
        content = self.ts_path.read_text(encoding="utf-8")
        self.assertIn("IIssueWebhookPayload", content, "Missing IIssueWebhookPayload interface")
        self.assertIn("IIssueWebhookEvent", content, "Missing IIssueWebhookEvent interface")
        self.assertIn("IIssueWebhookLog", content, "Missing IIssueWebhookLog interface")

def run_tests_with_json_report(workspace: Optional[str] = None) -> Dict[str, Any]:
    resolved = resolve_workspace(workspace)
    TestPlaneIssueWebhookFeature.workspace = resolved
    suite = unittest.TestLoader().loadTestsFromTestCase(TestPlaneIssueWebhookFeature)
    result = unittest.TestResult()
    suite.run(result)

    report = {
        "workspace": str(resolved),
        "total_tests": result.testsRun,
        "passed_tests": result.testsRun - len(result.failures) - len(result.errors),
        "failures": len(result.failures),
        "errors": len(result.errors),
        "success": result.wasSuccessful(),
        "failure_details": [f"{t.id()}: {err}" for t, err in result.failures + result.errors]
    }
    return report

def main():
    parser = argparse.ArgumentParser(description="Verify Plane Issue Webhook Feature.")
    parser.add_argument("--workspace", type=str, default=None, help="Path to Plane workspace")
    parser.add_argument("--json", action="store_true", help="Output JSON report")
    args = parser.parse_args()

    report = run_tests_with_json_report(args.workspace)
    if args.json:
        print(json.dumps(report, indent=2))
    else:
        status_str = "PASSED" if report["success"] else "FAILED"
        print(f"[{status_str}] Issue Webhook Verification: {report['passed_tests']}/{report['total_tests']} tests passed in {report['workspace']}")
        if not report["success"]:
            for d in report["failure_details"]:
                print(f"  - {d}")
    sys.exit(0 if report["success"] else 1)

if __name__ == "__main__":
    main()
