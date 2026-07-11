from __future__ import annotations

import os
import shutil
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from benchmarks.dogfood.mcp_deterministic_runner import dogfood_fixture_parent, prepare_workspace


class DogfoodRunnerHelperTests(unittest.TestCase):
    def test_prepare_workspace_default_ignores_process_tmpdir(self) -> None:
        if os.name == "nt":
            self.skipTest("POSIX /tmp fixture root semantics do not apply on Windows")
        with TemporaryDirectory() as tmp:
            drifted_tmp = Path(tmp) / ".coding-tools" / "tmp"
            drifted_tmp.mkdir(parents=True)
            with patch.dict(os.environ, {"TMPDIR": str(drifted_tmp)}, clear=False):
                root, workspace = prepare_workspace()
            try:
                self.assertEqual(root.parent, dogfood_fixture_parent())
                self.assertTrue(root.name.startswith("coding-tools-mcp-dogfood-"))
                self.assertEqual(workspace, root / "workspace")
                self.assertFalse(str(root).startswith(str(drifted_tmp)))
            finally:
                shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()
