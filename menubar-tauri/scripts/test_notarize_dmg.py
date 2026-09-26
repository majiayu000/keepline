"""Exercise notarization acceptance, retry and failure gates without Apple services."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("notarize-dmg.sh")
MOCK = r"""
import json, os, pathlib, shutil, sys
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
root = pathlib.Path(os.environ["MOCK_ROOT"])
case = os.environ["MOCK_CASE"]
with (root / "calls.jsonl").open("a") as log:
    log.write(json.dumps([name, *args]) + "\n")
if name == "hdiutil":
    if args[0] == "attach" and case != "missing_app":
        (pathlib.Path(args[args.index("-mountpoint") + 1]) / "Fixture.app").mkdir()
    elif args[0] == "detach":
        shutil.rmtree(pathlib.Path(args[1]) / "Fixture.app", ignore_errors=True)
elif name == "codesign" and case == "bad_app":
    sys.exit(1)
elif name == "xcrun":
    if args[:2] == ["notarytool", "submit"]:
        if case == "submit_failure":
            sys.exit(1)
        print(json.dumps({"status": "Rejected" if case == "rejected" else "Accepted"}))
    elif args[:2] == ["stapler", "staple"]:
        counter = root / "attempts"
        count = int(counter.read_text()) + 1 if counter.exists() else 1
        counter.write_text(str(count))
        if case == "exhausted" or (case == "retry" and count < 2):
            sys.exit(65)
elif name == "spctl" and case == "bad_dmg" and "open" in args:
    sys.exit(3)
"""

class NotarizationTests(unittest.TestCase):
    def run_case(self, case):
        with tempfile.TemporaryDirectory(prefix="keepline-notary-test-") as tmp:
            root = Path(tmp)
            binary_dir = root / "bin"
            binary_dir.mkdir()
            for name in ["hdiutil", "codesign", "spctl", "xcrun", "sleep"]:
                path = binary_dir / name
                path.write_text("#!" + sys.executable + "\n" + MOCK)
                path.chmod(0o755)
            key = root / "fixture-key"
            key.write_text("not-a-real-credential")
            dmg = root / "Fixture App.dmg"
            dmg.touch()
            env = dict(os.environ, PATH=str(binary_dir) + os.pathsep + os.environ["PATH"],
                       APPLE_API_KEY_PATH=str(key), APPLE_API_KEY="fixture-key-id",
                       APPLE_API_ISSUER="fixture-issuer", RUNNER_TEMP=str(root),
                       MOCK_ROOT=str(root), MOCK_CASE=case)
            result = subprocess.run(["bash", str(SCRIPT), str(dmg)], env=env,
                                    capture_output=True, text=True, timeout=15)
            calls = [json.loads(line) for line in (root / "calls.jsonl").read_text().splitlines()]
            self.assertFalse(list(root.glob("keepline-dmg.*")), "mount temporary directory leaked")
            self.assertFalse(list(root.glob("keepline-notary.*")), "notary result leaked")
            return result, calls

    def test_accepted(self):
        result, calls = self.run_case("accepted")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(sum(c[:3] == ["xcrun", "stapler", "staple"] for c in calls), 1)
        self.assertEqual(calls[-1][0], "spctl")

    def test_ticket_retry(self):
        result, calls = self.run_case("retry")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(sum(c[:3] == ["xcrun", "stapler", "staple"] for c in calls), 2)

    def test_rejection_blocks_stapling(self):
        result, calls = self.run_case("rejected")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(c[:3] == ["xcrun", "stapler", "staple"] for c in calls))

    def test_submit_failure_blocks_stapling(self):
        result, calls = self.run_case("submit_failure")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(c[:3] == ["xcrun", "stapler", "staple"] for c in calls))

    def test_retry_is_bounded(self):
        result, calls = self.run_case("exhausted")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(sum(c[:3] == ["xcrun", "stapler", "staple"] for c in calls), 6)

    def test_bad_app_blocks_submission(self):
        result, calls = self.run_case("bad_app")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(c[:3] == ["xcrun", "notarytool", "submit"] for c in calls))

    def test_missing_app_blocks_submission(self):
        result, calls = self.run_case("missing_app")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(c[:3] == ["xcrun", "notarytool", "submit"] for c in calls))

    def test_gatekeeper_rejection_fails(self):
        result, _ = self.run_case("bad_dmg")
        self.assertNotEqual(result.returncode, 0)

if __name__ == "__main__":
    unittest.main()
