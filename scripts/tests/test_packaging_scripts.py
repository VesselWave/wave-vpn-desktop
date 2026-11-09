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

    # B-09: removal must not leave a running GUI that offers to connect
    # through a helper that no longer exists.
    def test_u2404_b09_removal_closes_running_wave_windows(self):
        self.authorize()
        self.connect()
        result, calls = self.run_hook('preremove.sh', 'remove')
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIn('wave-vpn lock-disable', calls)
        self.assertIn('wave-vpn service-uninstall', calls)
        self.assertIn('pkill -x wave-vpn-gui', calls)
        # B-13: the saved preferences survive; say so instead of implying
        # Network Lock is gone for good.
        self.assertIn('~/.config/wave-vpn', result.stdout)

    def test_upgrade_keeps_everything(self):
        self.authorize()
        self.connect()
        for argument in ('upgrade', '1'):
            result, calls = self.run_hook('preremove.sh', argument)
            self.assertEqual(0, result.returncode, result.stderr)
            self.assertEqual([], calls)


class NeedrestartOptOut(unittest.TestCase):
    """R-01: Ubuntu's needrestart restarts services whose executable was
    replaced. After an upgrade while connected that restart took the active
    tunnel down; the broker restarts itself once idle instead."""

    CONF = ROOT / 'packaging' / 'needrestart-wave-vpn.pl'
    DST = '/etc/needrestart/conf.d/wave-vpn.conf'

    def test_r01_package_ships_the_needrestart_opt_out_as_config(self):
        package = (ROOT / 'scripts' / 'package.sh').read_text()
        line = next((l for l in package.splitlines() if self.DST in l), '')
        self.assertIn('needrestart-wave-vpn.pl', line)
        self.assertIn('type: config', line, 'admins may edit it; keep their changes')

    @unittest.skipUnless(subprocess.run(['sh', '-c', 'command -v perl'], capture_output=True).returncode == 0, 'perl missing')
    def test_r01_opt_out_never_restarts_the_networking_service(self):
        probe = (
            'our %nrconf; -r $ARGV[0] or die "missing"; do $ARGV[0]; die "parse: $@" if $@; '  # needrestart's own check
            'for my $unit ("wave-vpn-service.service", "wave-vpn-lock.service", "ssh.service") { '
            'my $v = "unset"; for my $re (keys %{$nrconf{override_rc}}) { $v = $nrconf{override_rc}{$re} if $unit =~ /$re/ } '
            'print "$unit=$v\n" }'
        )
        result = subprocess.run(['perl', '-e', probe, str(self.CONF)], capture_output=True, text=True)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertEqual(
            ['wave-vpn-service.service=0', 'wave-vpn-lock.service=0', 'ssh.service=unset'],
            result.stdout.split(),
        )


if __name__ == '__main__':
    unittest.main()
