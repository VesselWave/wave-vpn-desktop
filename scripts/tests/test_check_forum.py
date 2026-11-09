# SPDX-License-Identifier: GPL-3.0-or-later
"""Unit tests for scripts/check-forum.py; run with
python3 -m unittest discover -s scripts/tests -v"""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('check_forum', Path(__file__).resolve().parents[1] / 'check-forum.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

OUTPUT = """
running 3 tests
test forum_gui_cases::forum_d05_user_disconnect_does_not_penalize_the_server ... FAILED
test forum_gui_cases::forum_p09_device_choice_is_hidden_for_a_single_device_account ... ok
test forum_d11_manifest_without_wireguard_modes_is_rejected ... ignored

failures:

---- forum_gui_cases::forum_d05_user_disconnect_does_not_penalize_the_server stdout ----

thread 'x' panicked at src/../tests/support/forum_gui_cases.rs:80:5:
a clean user Disconnect is not a server failure
note: run with `RUST_BACKTRACE=1`

test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 27 filtered out
"""


class ForumReportTest(unittest.TestCase):
    def test_parses_libtest_status_and_failure_detail(self):
        cases = module.collect(OUTPUT)
        by_id = {c['id']: c for c in cases}
        failed = by_id['forum_gui_cases::forum_d05_user_disconnect_does_not_penalize_the_server']
        self.assertEqual('failed', failed['status'])
        self.assertEqual('a clean user Disconnect is not a server failure', failed['detail'])
        self.assertEqual('passed', by_id['forum_gui_cases::forum_p09_device_choice_is_hidden_for_a_single_device_account']['status'])
        self.assertEqual('skipped', by_id['forum_d11_manifest_without_wireguard_modes_is_rejected']['status'])

    def test_missing_ignored_or_build_failure_cannot_pass(self):
        passed = [{'status': 'passed'}]
        self.assertEqual(2, module.outcome([], 0))
        self.assertEqual(2, module.outcome([{'status': 'skipped'}], 0))
        self.assertEqual(2, module.outcome(passed, 101))
        self.assertEqual(2, module.outcome(passed, 0, ['forum_p01']))
        self.assertEqual(1, module.outcome([{'status': 'failed'}], 101))
        self.assertEqual(0, module.outcome(passed, 0))

    def test_required_contracts_match_by_test_name_across_modules(self):
        cases = [{'id': 'forum_gui_cases::' + name, 'status': 'passed'} for name in module.REQUIRED]
        self.assertEqual([], module.missing(cases))
        self.assertEqual([module.REQUIRED[0]], module.missing(cases[1:]))

    def test_every_required_contract_exists_in_the_rust_sources(self):
        root = Path(__file__).resolve().parents[2]
        sources = (root / 'tests/forum_contracts.rs').read_text() + (root / 'tests/support/forum_gui_cases.rs').read_text()
        for name in module.REQUIRED:
            self.assertIn(f'fn {name}()', sources)


if __name__ == '__main__':
    unittest.main()
