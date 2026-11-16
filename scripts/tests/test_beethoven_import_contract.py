"""Keep a corpus exception separate from generic CLI defaults and other movements."""
import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_beethoven_import as gate


class ContractScope(unittest.TestCase):
    def test_only_pinned_movement_i_receives_contract(self):
        payload = b"original fixture stand-in"
        digest = hashlib.sha256(payload).hexdigest()
        result = ({"exit_code": 1, "diagnostics_truncated": False}, b"", [])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for movement in range(1, 5):
                (root / f"Beethoven_Op.67_{movement}.musicxml").write_bytes(payload)
            with patch.object(gate, "HASHES", dict.fromkeys(range(1, 5), digest)), patch.object(gate, "archive_score_digest", return_value=digest), patch.object(gate, "run", return_value=result) as run:
                contract = Path("movement-i.json")
                rows = gate.check(Path("cli"), root, contract)
                self.assertEqual(len(rows), 4)
                self.assertEqual([call.args[2] for call in run.call_args_list], [contract, contract] + [None] * 6)
                run.reset_mock()
                gate.check(Path("cli"), root)
                self.assertEqual([call.args[2] for call in run.call_args_list], [None] * 8)
                run.reset_mock()
                with patch.object(gate, "verify_transcript") as verify:
                    failed = gate.check(Path("cli"), root, contract, compare_events=True)
                    verify.assert_not_called()
                    self.assertTrue(all(row["note_event_transcript"]["status"] == "not_run_import_failed" for row in failed))
                    run.return_value = ({"exit_code": 0, "diagnostics_truncated": False}, b"native", [])
                    verify.return_value = {"status": "equal"}
                    passed = gate.check(Path("cli"), root, contract, compare_events=True)
                    self.assertEqual(verify.call_count, 4)
                    self.assertEqual([call.args[3] for call in verify.call_args_list], [contract, None, None, None])
                    self.assertTrue(all(row["note_event_transcript"]["status"] == "equal" for row in passed))
                run.reset_mock()
                (root / "Beethoven_Op.67_1.musicxml").write_bytes(b"changed")
                with self.assertRaises(ValueError):
                    gate.check(Path("cli"), root, contract)
                run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
