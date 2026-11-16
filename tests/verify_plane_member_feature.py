#!/usr/bin/env python3
"""
tests/verify_plane_member_feature.py

Verification Test Suite for Plane Member Feature:
1. ProjectMemberAuditLog in apps/api/plane/db/models/project.py
2. ProjectMemberRateThrottle in apps/api/plane/throttles/project.py
3. Role transition audit logging & @action def audit_logs in apps/api/plane/app/views/project/member.py

Supports standalone CLI execution, python -m unittest, and pytest.
Zero external dependencies required (uses standard library ast, unittest, sys, pathlib, argparse).
"""

import argparse
import ast
import json
import os
import sys
import unittest
from pathlib import Path
from typing import Dict, List, Optional, Any

# Default workspace candidates to search if not explicitly specified
DEFAULT_WORKSPACE_CANDIDATES = [
    Path("benchmark_workspaces/workspace_b_enhanced/plane"),
    Path("benchmark_workspaces/workspace_a_stock/plane"),
    Path("target_repo/plane"),
    Path("."),
]


def resolve_workspace(workspace_arg: Optional[str] = None) -> Path:
    """Resolve the target Plane workspace root path."""
    if workspace_arg:
        p = Path(workspace_arg).resolve()
        if p.exists():
            return p
        raise FileNotFoundError(f"Specified workspace path does not exist: {workspace_arg}")

    # Check environment variable
    env_path = os.environ.get("PLANE_WORKSPACE")
    if env_path:
        p = Path(env_path).resolve()
        if p.exists():
            return p

    # Probe default candidates
    for candidate in DEFAULT_WORKSPACE_CANDIDATES:
        resolved = candidate.resolve()
        if (resolved / "apps" / "api" / "plane").exists():
            return resolved

    # Fallback to current directory
    return Path.cwd().resolve()


class ASTInspector:
    """Helper to parse and inspect Python AST without executing or importing modules."""

    @staticmethod
    def parse_file(file_path: Path) -> ast.Module:
        if not file_path.is_file():
            raise FileNotFoundError(f"File not found: {file_path}")
        with open(file_path, "r", encoding="utf-8") as f:
            return ast.parse(f.read(), filename=str(file_path))

    @staticmethod
    def find_class(tree: ast.AST, class_name: str) -> Optional[ast.ClassDef]:
        for node in ast.walk(tree):
            if isinstance(node, ast.ClassDef) and node.name == class_name:
                return node
        return None

    @staticmethod
    def get_class_bases(node: ast.ClassDef) -> List[str]:
        bases = []
        for b in node.bases:
            if isinstance(b, ast.Name):
                bases.append(b.id)
            elif isinstance(b, ast.Attribute):
                bases.append(b.attr)
        return bases

    @staticmethod
    def get_assigned_names(class_node: ast.ClassDef) -> List[str]:
        names = []
        for n in class_node.body:
            if isinstance(n, ast.Assign):
                for target in n.targets:
                    if isinstance(target, ast.Name):
                        names.append(target.id)
                    elif isinstance(target, (ast.Tuple, ast.List)):
                        for elt in target.elts:
                            if isinstance(elt, ast.Name):
                                names.append(elt.id)
            elif isinstance(n, ast.AnnAssign):
                if isinstance(n.target, ast.Name):
                    names.append(n.target.id)
        return names

    @staticmethod
    def get_methods(class_node: ast.ClassDef) -> Dict[str, ast.FunctionDef]:
        return {
            n.name: n
            for n in class_node.body
            if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))
        }

    @staticmethod
    def get_decorator_names(func_node: ast.FunctionDef) -> List[str]:
        decs = []
        for d in func_node.decorator_list:
            if isinstance(d, ast.Call):
                f = d.func
                if isinstance(f, ast.Name):
                    decs.append(f.id)
                elif isinstance(f, ast.Attribute):
                    decs.append(f.attr)
            elif isinstance(d, ast.Name):
                decs.append(d.id)
            elif isinstance(d, ast.Attribute):
                decs.append(d.attr)
        return decs


class TestPlaneMemberFeature(unittest.TestCase):
    """Test suite verifying the Plane Member Feature components."""

    workspace: Path = Path.cwd()
    models_file: Path = Path.cwd() / "apps" / "api" / "plane" / "db" / "models" / "project.py"
    throttle_file: Path = Path.cwd() / "apps" / "api" / "plane" / "throttles" / "project.py"
    views_file: Path = Path.cwd() / "apps" / "api" / "plane" / "app" / "views" / "project" / "member.py"
    _configured: bool = False

    @classmethod
    def configure(cls, workspace_path: Path):
        cls.workspace = workspace_path
        cls.models_file = workspace_path / "apps" / "api" / "plane" / "db" / "models" / "project.py"
        cls.throttle_file = workspace_path / "apps" / "api" / "plane" / "throttles" / "project.py"
        cls.views_file = workspace_path / "apps" / "api" / "plane" / "app" / "views" / "project" / "member.py"
        cls._configured = True

    @classmethod
    def setUpClass(cls):
        if not cls._configured:
            cls.configure(resolve_workspace())

    # -------------------------------------------------------------
    # 1. ProjectMemberAuditLog Model Verification
    # -------------------------------------------------------------
    def test_01_model_file_exists(self):
        """Verify apps/api/plane/db/models/project.py exists and parses."""
        self.assertTrue(self.models_file.is_file(), f"Models file missing: {self.models_file}")
        try:
            tree = ASTInspector.parse_file(self.models_file)
            self.assertIsInstance(tree, ast.Module)
        except SyntaxError as e:
            self.fail(f"Syntax error in models file: {e}")

    def test_02_audit_log_class_defined(self):
        """Verify ProjectMemberAuditLog class is defined in project.py."""
        self.assertTrue(self.models_file.is_file(), f"Models file missing: {self.models_file}")
        tree = ASTInspector.parse_file(self.models_file)
        cls_node = ASTInspector.find_class(tree, "ProjectMemberAuditLog")
        self.assertIsNotNone(cls_node, "Class 'ProjectMemberAuditLog' not found in project.py")

    def test_03_audit_log_inheritance(self):
        """Verify ProjectMemberAuditLog inherits from ProjectBaseModel or BaseModel."""
        self.assertTrue(self.models_file.is_file(), f"Models file missing: {self.models_file}")
        tree = ASTInspector.parse_file(self.models_file)
        cls_node = ASTInspector.find_class(tree, "ProjectMemberAuditLog")
        self.assertIsNotNone(cls_node, "Class 'ProjectMemberAuditLog' not found in project.py")
        bases = ASTInspector.get_class_bases(cls_node)
        valid_bases = {"ProjectBaseModel", "BaseModel", "Model"}
        self.assertTrue(
            any(b in valid_bases for b in bases),
            f"ProjectMemberAuditLog must inherit from ProjectBaseModel or BaseModel; found bases: {bases}",
        )

    def test_04_audit_log_role_fields(self):
        """Verify ProjectMemberAuditLog defines role tracking fields (old_role, new_role)."""
        self.assertTrue(self.models_file.is_file(), f"Models file missing: {self.models_file}")
        tree = ASTInspector.parse_file(self.models_file)
        cls_node = ASTInspector.find_class(tree, "ProjectMemberAuditLog")
        self.assertIsNotNone(cls_node, "Class 'ProjectMemberAuditLog' not found in project.py")
        fields = ASTInspector.get_assigned_names(cls_node)
        has_old = any(f in fields for f in ["old_role", "previous_role", "old_value"])
        has_new = any(f in fields for f in ["new_role", "role", "new_value"])
        self.assertTrue(has_old, f"ProjectMemberAuditLog missing old_role field. Fields found: {fields}")
        self.assertTrue(has_new, f"ProjectMemberAuditLog missing new_role field. Fields found: {fields}")

    def test_05_audit_log_member_relation(self):
        """Verify ProjectMemberAuditLog defines relation to project_member or member."""
        self.assertTrue(self.models_file.is_file(), f"Models file missing: {self.models_file}")
        tree = ASTInspector.parse_file(self.models_file)
        cls_node = ASTInspector.find_class(tree, "ProjectMemberAuditLog")
        self.assertIsNotNone(cls_node, "Class 'ProjectMemberAuditLog' not found in project.py")
        fields = ASTInspector.get_assigned_names(cls_node)
        has_member = any(f in fields for f in ["project_member", "member"])
        self.assertTrue(has_member, f"ProjectMemberAuditLog missing member/project_member field. Fields: {fields}")

    def test_06_audit_log_meta(self):
        """Verify ProjectMemberAuditLog defines Meta with db_table."""
        self.assertTrue(self.models_file.is_file(), f"Models file missing: {self.models_file}")
        tree = ASTInspector.parse_file(self.models_file)
        cls_node = ASTInspector.find_class(tree, "ProjectMemberAuditLog")
        self.assertIsNotNone(cls_node, "Class 'ProjectMemberAuditLog' not found in project.py")
        meta_cls = next((n for n in cls_node.body if isinstance(n, ast.ClassDef) and n.name == "Meta"), None)
        self.assertIsNotNone(meta_cls, "ProjectMemberAuditLog missing inner 'Meta' class")
        meta_fields = ASTInspector.get_assigned_names(meta_cls)
        self.assertIn("db_table", meta_fields, "Meta class missing 'db_table' attribute")

    # -------------------------------------------------------------
    # 2. ProjectMemberRateThrottle Verification
    # -------------------------------------------------------------
    def test_07_throttle_file_exists(self):
        """Verify apps/api/plane/throttles/project.py exists and parses."""
        self.assertTrue(self.throttle_file.is_file(), f"Throttle file missing: {self.throttle_file}")
        try:
            tree = ASTInspector.parse_file(self.throttle_file)
            self.assertIsInstance(tree, ast.Module)
        except SyntaxError as e:
            self.fail(f"Syntax error in throttle file: {e}")

    def test_08_throttle_class_and_base(self):
        """Verify ProjectMemberRateThrottle inherits from SimpleRateThrottle."""
        self.assertTrue(self.throttle_file.is_file(), f"Throttle file missing: {self.throttle_file}")
        tree = ASTInspector.parse_file(self.throttle_file)
        cls_node = ASTInspector.find_class(tree, "ProjectMemberRateThrottle")
        self.assertIsNotNone(cls_node, "Class 'ProjectMemberRateThrottle' not found in throttles/project.py")
        bases = ASTInspector.get_class_bases(cls_node)
        valid_bases = {"SimpleRateThrottle", "BaseThrottle", "UserRateThrottle"}
        self.assertTrue(
            any(b in valid_bases for b in bases),
            f"ProjectMemberRateThrottle must inherit from SimpleRateThrottle; found: {bases}",
        )

    def test_09_throttle_scope_and_cache_key(self):
        """Verify ProjectMemberRateThrottle defines scope and get_cache_key."""
        self.assertTrue(self.throttle_file.is_file(), f"Throttle file missing: {self.throttle_file}")
        tree = ASTInspector.parse_file(self.throttle_file)
        cls_node = ASTInspector.find_class(tree, "ProjectMemberRateThrottle")
        self.assertIsNotNone(cls_node, "Class 'ProjectMemberRateThrottle' not found in throttles/project.py")
        fields = ASTInspector.get_assigned_names(cls_node)
        self.assertIn("scope", fields, "ProjectMemberRateThrottle missing 'scope' class attribute")
        methods = ASTInspector.get_methods(cls_node)
        self.assertIn("get_cache_key", methods, "ProjectMemberRateThrottle missing 'get_cache_key' method")

    # -------------------------------------------------------------
    # 3. View Role Transition & Custom Action Verification
    # -------------------------------------------------------------
    def test_10_view_file_exists(self):
        """Verify apps/api/plane/app/views/project/member.py exists and parses."""
        self.assertTrue(self.views_file.is_file(), f"View file missing: {self.views_file}")
        try:
            tree = ASTInspector.parse_file(self.views_file)
            self.assertIsInstance(tree, ast.Module)
        except SyntaxError as e:
            self.fail(f"Syntax error in view file: {e}")

    def test_11_role_transition_logging(self):
        """Verify role transition audit logging in partial_update."""
        self.assertTrue(self.views_file.is_file(), f"View file missing: {self.views_file}")
        tree = ASTInspector.parse_file(self.views_file)

        # 1. Verify import of ProjectMemberAuditLog
        imported_symbols = []
        for node in ast.walk(tree):
            if isinstance(node, (ast.ImportFrom, ast.Import)):
                for alias in node.names:
                    imported_symbols.append(alias.name)
        self.assertIn(
            "ProjectMemberAuditLog",
            imported_symbols,
            "ProjectMemberAuditLog is not imported in apps/api/plane/app/views/project/member.py",
        )

        # 2. Verify partial_update invokes audit log creation
        cls_node = ASTInspector.find_class(tree, "ProjectMemberViewSet")
        self.assertIsNotNone(cls_node, "Class 'ProjectMemberViewSet' not found in member.py")
        methods = ASTInspector.get_methods(cls_node)
        self.assertIn("partial_update", methods, "partial_update method not found in ProjectMemberViewSet")

        partial_func = methods["partial_update"]
        referenced_identifiers = []
        for n in ast.walk(partial_func):
            if isinstance(n, ast.Name):
                referenced_identifiers.append(n.id)
            elif isinstance(n, ast.Attribute):
                referenced_identifiers.append(n.attr)

        audit_logged = (
            "ProjectMemberAuditLog" in referenced_identifiers
            or "project_member_audit_logs" in referenced_identifiers
        )
        self.assertTrue(
            audit_logged,
            "partial_update does not invoke ProjectMemberAuditLog creation on role change",
        )

    def test_12_audit_logs_action_endpoint(self):
        """Verify @action def audit_logs endpoint is defined on ProjectMemberViewSet."""
        self.assertTrue(self.views_file.is_file(), f"View file missing: {self.views_file}")
        tree = ASTInspector.parse_file(self.views_file)
        cls_node = ASTInspector.find_class(tree, "ProjectMemberViewSet")
        self.assertIsNotNone(cls_node, "Class 'ProjectMemberViewSet' not found in member.py")
        methods = ASTInspector.get_methods(cls_node)
        self.assertIn(
            "audit_logs",
            methods,
            "Custom action method 'audit_logs' not found in ProjectMemberViewSet",
        )
        audit_func = methods["audit_logs"]
        decs = ASTInspector.get_decorator_names(audit_func)
        self.assertIn("action", decs, "Method 'audit_logs' is not decorated with @action")


def run_standalone(workspace_path: Path, verbose: bool = False, as_json: bool = False) -> int:
    """Run tests standalone and print report / emit JSON telemetry."""
    TestPlaneMemberFeature.configure(workspace_path)

    suite = unittest.TestLoader().loadTestsFromTestCase(TestPlaneMemberFeature)
    results_list = []
    passed = 0
    failed = 0

    if not as_json:
        print(f"\n{'=' * 70}")
        print(f"Plane Member Feature Verification Runner")
        print(f"Target Workspace: {workspace_path}")
        print(f"{'=' * 70}\n")

    for test in suite:
        test_id = test._testMethodName
        doc = getattr(test, test._testMethodName).__doc__ or test_id
        try:
            test.setUp()
            getattr(test, test._testMethodName)()
            test.tearDown()
            passed += 1
            results_list.append({"id": test_id, "description": doc.strip(), "status": "PASS", "error": None})
            if not as_json:
                print(f"  [PASS] {test_id}: {doc.strip()}")
        except Exception as e:
            failed += 1
            err_msg = str(e)
            results_list.append({"id": test_id, "description": doc.strip(), "status": "FAIL", "error": err_msg})
            if not as_json:
                print(f"  [FAIL] {test_id}: {doc.strip()}")
                if verbose:
                    print(f"         Error: {err_msg}")

    total = passed + failed
    success = failed == 0

    if not as_json:
        print(f"\n{'-' * 70}")
        print(f"Verification Summary: {passed}/{total} Passed | {failed} Failed")
        print(f"Result: {'SUCCESS' if success else 'FAILURE'}")
        print(f"{'=' * 70}\n")
    else:
        payload = {
            "workspace": str(workspace_path),
            "total": total,
            "passed": passed,
            "failed": failed,
            "success": success,
            "results": results_list,
        }
        print(json.dumps(payload, indent=2))

    return 0 if success else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Plane Member Feature Verification Suite")
    parser.add_argument(
        "workspace",
        nargs="?",
        default=None,
        help="Path to the Plane repository workspace (default: auto-detected)",
    )
    parser.add_argument(
        "--workspace",
        dest="workspace_flag",
        default=None,
        help="Path to the Plane repository workspace",
    )
    parser.add_argument("--json", action="store_true", help="Output results as JSON telemetry")
    parser.add_argument("--verbose", "-v", action="store_true", help="Verbose output with failure details")

    args = parser.parse_args()

    try:
        ws = resolve_workspace(args.workspace or args.workspace_flag)
    except Exception as exc:
        print(f"Error resolving workspace: {exc}", file=sys.stderr)
        sys.exit(2)

    exit_code = run_standalone(ws, verbose=args.verbose, as_json=args.json)
    sys.exit(exit_code)
