#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Opt-in <=5 location sample; private JSON stdin, public aggregate output only.
Uses existing live_linux namespace/NAT cleanup. No check-service or download traffic.
TCP443 x3 and ICMP x5 to each profile's tunnel DNS; ICMP refusal is not tunnel failure.
"""
import json
import os
from pathlib import Path
import re
import socket
import sys
import time
import live_linux as lab


def main(rows):
    lab.run('mount', '--make-rprivate', '/')
    lab.run('mount', '-t', 'tmpfs', '-o', 'mode=700', 'tmpfs', '/run')
    lab.run('ip', 'link', 'set', 'lo', 'up')
    for row in rows:
        p = row['request']['profile']
        tcp = []
        for _ in range(3):
            start = time.monotonic()
            try:
                with socket.create_connection((row['entry'], 443), timeout=2):
                    tcp.append(round((time.monotonic()-start)*1000, 1))
            except OSError:
                pass
        result = {k: row[k] for k in ('name', 'country', 'load', 'capacity', 'users', 'users_max', 'base', 'factors')}
        result['tcp_ms'] = tcp
        try:
            lab.run('ip', 'link', 'add', 'quality0', 'type', 'wireguard')
            path = Path('/run/quality.conf')
            path.write_text('[Interface]\nPrivateKey = '+p['private_key']+'\n[Peer]\nPublicKey = '+p['public_key']+'\nPresharedKey = '+p['preshared_key']+'\nEndpoint = '+p['endpoint']+':'+str(p['port'])+'\nAllowedIPs = 0.0.0.0/0, ::/0\n')
            path.chmod(0o600)
            lab.run('wg', 'setconf', 'quality0', str(path))
            path.unlink()
            for address in p['addresses']:
                lab.run('ip', 'address', 'add', address, 'dev', 'quality0')
            lab.run('ip', 'link', 'set', 'quality0', 'up')
            dns = next(d for d in p['dns'] if ':' not in d)
            lab.run('ip', 'route', 'add', dns+'/32', 'dev', 'quality0')
            start = time.monotonic()
            # Initial packet initiates the WireGuard handshake; no provider check API.
            lab.run('ping', '-n', '-c', '1', '-W', '2', dns, check=False)
            handshook = False
            for _ in range(16):
                raw = lab.run('wg', 'show', 'quality0', 'latest-handshakes').stdout.split()
                if raw and int(raw[-1]) > 0:
                    handshook = True
                    break
                time.sleep(.25)
            result['handshake'] = handshook
            result['handshake_observed_s'] = round(time.monotonic()-start, 2)
            if handshook:
                ping = lab.run('ping', '-n', '-c', '5', '-i', '.25', '-W', '1', dns, check=False, timeout=9).stdout
                rtts = [float(x) for x in re.findall(r'time[=<]([0-9.]+) ms', ping)]
                result['tunnel_icmp_ms'] = rtts
                result['icmp_loss_percent'] = 100-20*len(rtts)
                result['icmp_spread_ms'] = round(max(rtts)-min(rtts),2) if rtts else None
        finally:
            lab.run('ip', 'link', 'delete', 'quality0', check=False)
        print(json.dumps(result), flush=True)


if __name__ == '__main__':
    assert os.geteuid() == 0 and os.environ.get('WAVE_LIVE_TEST_VM') == '1'
    rows = json.load(sys.stdin)
    assert 0 < len(rows) <= 5
    if '--isolated' in sys.argv:
        main(rows)
    else:
        # Reuse the tested isolation/cleanup orchestration, invoking this child.
        lab.__file__ = __file__
        lab.parent(sys.argv[1], rows)
