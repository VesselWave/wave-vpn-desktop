#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Reboot an explicitly selected disposable Lima VM; check installed helper startup.
Refuses active tunnel, protection, or pending recovery. Does not enable boot lock.
"""
import argparse
import json
import subprocess
import time
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--vm', required=True)
args = parser.parse_args()
def guest(*command, input=None):
    return subprocess.run(['limactl','shell',args.vm,'--',*command], capture_output=True, text=True, timeout=15, input=input)
def status():
    result = guest('/usr/local/bin/wave-vpn','service-client','network-status')
    assert result.returncode == 0, 'Installed helper unavailable'
    return next(json.loads(line)['status'] for line in result.stdout.splitlines() if json.loads(line).get('event') == 'network_status')
before = status()
assert before['enabled'] is False and not before['worker_active'] and not before['recovery_needed'], 'Disconnect, recover and explicitly unlock the disposable VM before reboot'
boot = guest('cat','/proc/sys/kernel/random/boot_id').stdout.strip()
guest('sudo','systemctl','reboot')
print('Disposable VM reboot requested; waiting for the new boot and helper socket', flush=True)
deadline = time.monotonic() + 120
while time.monotonic() < deadline:
    time.sleep(3)
    try:
        result = guest('cat','/proc/sys/kernel/random/boot_id')
        if result.returncode == 0 and result.stdout.strip() != boot:
            after = status()
            assert after['enabled'] is False and not after['worker_active'] and not after['recovery_needed']
            print('PASS actual reboot: new boot ID, authorized helper returns without password prompt; protection correctly reported off', flush=True)
            break
    except (subprocess.TimeoutExpired, AssertionError, StopIteration):
        pass
else:
    raise RuntimeError('VM reboot/helper deadline exceeded')
