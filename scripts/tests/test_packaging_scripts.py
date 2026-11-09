#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Offline tests of the package install/remove hooks in packaging/scripts.

Each hook runs under a temporary root (WAVE_PACKAGE_TEST_ROOT) with stub
wave-vpn, systemctl and pkill commands that only record their arguments.
Nothing touches the real system. Run:
python3 -m unittest discover -s scripts/tests -v
"""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / 'packaging' / 'scripts'
STUB = '#!/bin/sh\necho "{name} $*" >> "{log}"\n'


class Hooks(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name) / 'root'
        self.stubs = Path(self.tmp.name) / 'stubs'
        self.log = Path(self.tmp.name) / 'calls.log'
        self.log.touch()
        for path in ('run/systemd/system', 'usr/bin', 'etc/wave-vpn', 'etc/systemd/system', 'usr/local/libexec'):
            (self.root / path).mkdir(parents=True)
        self.stubs.mkdir()
        self.stub(self.root / 'usr/bin/wave-vpn', 'wave-vpn')
        for name in ('systemctl', 'pkill'):
            self.stub(self.stubs / name, name)

    def tearDown(self):
        self.tmp.cleanup()

    def stub(self, path, name):
        path.write_text(STUB.format(name=name, log=self.log))
        path.chmod(0o755)

    def run_hook(self, name, *args, env=None):
        environment = {
            'PATH': f'{self.stubs}:/usr/bin:/bin',
            'WAVE_PACKAGE_TEST_ROOT': str(self.root),
            **(env or {}),
        }
        result = subprocess.run(['sh', str(SCRIPTS / name), *args], env=environment,
                                capture_output=True, text=True, timeout=30)
        return result, self.log.read_text().splitlines()

    def authorize(self, uid=1000):
        (self.root / 'etc/wave-vpn/service.json').write_text('{"uid":%d}' % uid)

    def connect(self):
        (self.root / 'run/wave-vpn').mkdir(exist_ok=True)
        (self.root / 'run/wave-vpn/recovery.json').write_text('{}')
        (self.root / 'etc/systemd/system/wave-vpn-service.socket').write_text('')

    # B-01 (Ubuntu 24.04 report, D-08): an upgrade while connected must not
    # leave the old helper in place with only a terminal message.
    def test_u2404_b01_upgrade_while_connected_refreshes_the_helper(self):
        self.authorize()
        self.connect()
        result, calls = self.run_hook('postinstall.sh', 'configure', '0.1.3-1')
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIn('wave-vpn service-install --uid 1000', calls)
        self.assertNotIn('Disconnect, then run', result.stdout)

    def test_upgrade_while_disconnected_refreshes_the_helper(self):
        self.authorize(1234)
        result, calls = self.run_hook('postinstall.sh', 'configure', '0.1.3-1')
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIn('wave-vpn service-install --uid 1234', calls)

    def test_first_install_with_sudo_authorizes_the_sudo_user(self):
        result, calls = self.run_hook('postinstall.sh', 'configure', env={'SUDO_UID': '1000'})
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIn('wave-vpn service-install', calls)

    def test_first_install_without_sudo_sets_nothing_up(self):
        result, calls = self.run_hook('postinstall.sh', 'configure')
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertEqual([], [c for c in calls if c.startswith('wave-vpn')])


if __name__ == '__main__':
    unittest.main()
