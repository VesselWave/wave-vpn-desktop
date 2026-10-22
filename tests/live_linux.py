#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Opt-in authenticated Linux verification in a disposable VM.
Reads private worker requests on stdin. An isolated network/mount/PID namespace
gets internet through a temporary VM veth/NAT table; host VPN/DNS/firewall are
never replaced. No credentials, keys, account addresses or exit IPs are printed.
"""
import http.server
import ipaddress
import json
import os
from pathlib import Path
import queue
import signal
import socket
import subprocess
import sys
import threading
import time
import uuid


def run(*args, data='', check=True, timeout=20):
    result = subprocess.run(args, input=data, text=True, capture_output=True, timeout=timeout)
    if check and result.returncode:
        raise RuntimeError('System command failed: ' + args[0])
    return result


def main(binary, requests):
    run('mount', '--make-rprivate', '/')
    run('mount', '-t', 'tmpfs', '-o', 'mode=755', 'tmpfs', '/run')
    Path('/run/upper').mkdir()
    Path('/run/work').mkdir()
    run('mount', '-t', 'overlay', 'overlay', '-o', 'lowerdir=/etc,upperdir=/run/upper,workdir=/run/work', '/etc')
    # Break the resolver symlink only in the private /etc overlay.
    Path('/etc/resolv.conf').unlink(missing_ok=True)
    Path('/etc/resolv.conf').write_text('nameserver 1.1.1.1\n')
    baseline_resolver = Path('/etc/resolv.conf').read_bytes()
    if Path('/etc/wave-vpn').exists():
        run('mount', '-t', 'tmpfs', 'tmpfs', '/etc/wave-vpn')
    run('ip', 'link', 'set', 'lo', 'up')
    run('nft', '-f', '-', data='table inet unrelated { comment "live-test-preserved"; }\n')

    def underlay_http():
        return run('curl', '--noproxy', '*', '--interface', 'physical0', '--max-time', '2', '-fsS', 'http://198.19.254.1:18973/', check=False).returncode == 0

    def underlay_dns(tcp=False):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM if tcp else socket.SOCK_DGRAM) as sock:
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_BINDTODEVICE, b'physical0\0')
            sock.settimeout(1)
            try:
                sock.connect(('198.19.254.1', 53))
                if tcp:
                    return True
                sock.send(b'wave-live-probe')
                return sock.recv(128) == b'wave-live-probe'
            except OSError:
                return False

    def public_ip(v6=False, physical=False):
        address = '[2606:4700:4700::1111]' if v6 else '1.1.1.1'
        args = ['curl', '--noproxy', '*', '-6' if v6 else '-4', '--max-time', '15', '--fail', '--silent', '--resolve', 'one.one.one.one:443:' + address]
        if physical:
            args += ['--interface', 'physical0']
        r = run(*args, 'https://one.one.one.one/cdn-cgi/trace', check=False)
        if r.returncode:
            return None
        values = dict(line.split('=', 1) for line in r.stdout.splitlines() if '=' in line)
        try:
            return ipaddress.ip_address(values.get('ip', ''))
        except ValueError:
            return None

    assert underlay_http() and underlay_dns() and underlay_dns(True), 'Baseline observation listeners unavailable'
    baseline_ip = public_ip()
    assert baseline_ip, 'Baseline public IPv4 observation unavailable'
    print('PASS isolated internet uplink and physical HTTP/TCP+UDP DNS observation listeners', flush=True)

    def worker(request):
        proc = subprocess.Popen([binary, 'transport-worker'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        proc.stdin.write(json.dumps(request) + '\n')
        proc.stdin.flush()
        events = queue.Queue()
        def read():
            for line in proc.stdout:
                try:
                    events.put(json.loads(line))
                except ValueError:
                    pass
            events.put({'exited': True})
        threading.Thread(target=read, daemon=True).start()
        return proc, events

    def connected(proc, events):
        deadline = time.monotonic() + 120
        checks = None
        while time.monotonic() < deadline:
            event = events.get(timeout=120)
            if event.get('event') == 'checks':
                checks = event['results']
            if event.get('event') == 'connected':
                assert checks and checks['ipv4'] == 'verified' and checks['ipv6'] == 'verified' and checks['dns'] == 'verified', 'Provider tunnel/DNS challenges not verified'
                print('PASS app provider challenges: IPv4/IPv6 identity and system DNS receipt verified', flush=True)
                return
            if event.get('exited'):
                # Do not expose raw worker diagnostics containing private addresses.
                raise RuntimeError('Worker exited before connection; safe check results: ' + json.dumps(checks))
        raise RuntimeError('Connection deadline exceeded')

    def protection():
        assert not underlay_http(), 'Plaintext HTTP bypass escaped Network Lock'
        assert not underlay_dns() and not underlay_dns(True), 'Physical DNS bypass escaped Network Lock'
        assert json.loads(Path('/run/wave-vpn-protection.json').read_text())['enabled'], 'Protection state incorrect'
        assert 'live-test-preserved' in run('nft', 'list', 'table', 'inet', 'unrelated').stdout

    def disconnect(proc, command='disconnect\n'):
        proc.stdin.write(command)
        proc.stdin.flush()
        proc.stdin.close()
        assert proc.wait(timeout=20) == 0, 'Worker cleanup failed'
        assert 'wavevpn0' not in run('ip', 'link', 'show').stdout
        assert Path('/etc/resolv.conf').read_bytes() == baseline_resolver, 'Resolver not restored'
        protection()
        assert public_ip() is None, 'Public internet remains available after disconnect'

    def tunnel(request):
        exit4 = public_ip()
        assert exit4 and exit4.version == 4 and exit4 != baseline_ip, 'Public IPv4 did not change through VPN'
        if any(':' in a for a in request['profile']['addresses']):
            exit6 = public_ip(v6=True)
            assert exit6 and exit6.version == 6, 'IPv6 cannot traverse tunnel'
        for dns in request['profile']['dns']:
            answer = run('dig', '+time=3', '+tries=1', '+short', '@' + dns, 'example.com', 'A').stdout
            assert any(line and line[0].isdigit() for line in answer.splitlines()), 'Tunnel DNS query failed'
        protection()
        assert public_ip(physical=True) is None, 'Forced physical HTTPS bypass escaped lock'
        print('PASS authenticated tunnel: changed IPv4 exit, IPv6 exit, AirVPN DNS, blocked forced physical HTTP/HTTPS/DNS', flush=True)

    proc = None
    capture = None
    try:
        run(binary, 'lock-enable')
        protection()
        # No real credentials: an encrypted refusal proves bootstrap is reachable
        # with broken system DNS and an active lock, before any tunnel exists.
        candidates = [path for path in (Path(binary).parent / 'deps').glob('protocol-*') if path.is_file() and os.access(path, os.X_OK)]
        assert candidates, 'Build protocol integration tests before live verification'
        test_binary = max(candidates, key=lambda path: path.stat().st_mtime)
        Path('/etc/resolv.conf').write_text('nameserver 192.0.2.53\n')
        bootstrap = run(str(test_binary), '--exact', 'live_bootstrap_decrypts_an_explicit_account_refusal', '--ignored', timeout=40, check=False)
        assert bootstrap.returncode == 0, 'Bootstrap unavailable with broken DNS and Network Lock'
        Path('/etc/resolv.conf').write_bytes(baseline_resolver)
        print('PASS encrypted sign-in bootstrap remains reachable with broken DNS and Network Lock before connection', flush=True)
        capture = subprocess.Popen(['tcpdump', '-i', 'physical0', '-U', '-w', '/run/leaks.pcap', 'port 53 or port 853 or (tcp and host 1.1.1.1)'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(.5)
        if os.environ.get('WAVE_LIVE_NEGATIVE_ONLY') != '1':
            proc, events = worker(requests[0])
            connected(proc, events)
            tunnel(requests[0])
            disconnect(proc, 'switch\n')
            proc = None
            print('PASS first server stopped; public IPv4, physical HTTP and TCP/UDP DNS stay blocked during switch', flush=True)
            proc, events = worker(requests[1])
            connected(proc, events)
            tunnel(requests[1])
            disconnect(proc)
            proc = None
            run(binary, 'recover')
            protection()
            print('PASS second server Disconnect and Recover retain Network Lock and restore resolver resources', flush=True)
            proc, events = worker(requests[1])
            connected(proc, events)
            tunnel(requests[1])
            proc.kill()
            proc.wait(timeout=5)
            protection()
            run(binary, 'recover')
            protection()
            proc = None
            print('PASS real-tunnel worker SIGKILL and Recover retain protection', flush=True)
        # Real negative checks: wrong system DNS and an unavailable provider endpoint.
        for kind in ['wrong_dns', 'check_outage', 'partial_ipv6']:
            bad = json.loads(json.dumps(requests[1]))
            bad['options'].update({'verify_retries':'1', 'http_timeout':'2'})
            if kind == 'wrong_dns':
                bad['profile']['dns'] = ['192.0.2.53']
            elif kind == 'partial_ipv6':
                bad['checks']['exits'] = [ip for ip in bad['checks']['exits'] if ':' not in ip] + ['2001:db8::54']
            else:
                bad['checks']['exits'] = ['192.0.2.54', '2001:db8::54']
            proc, events = worker(bad)
            last = None
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                event = events.get(timeout=45)
                assert event.get('event') != 'connected', 'Failed provider checks accepted as connected'
                if event.get('event') == 'checks':
                    last = event['results']
                if event.get('exited'):
                    break
            assert proc.wait(timeout=10) != 0 and last is not None
            if kind == 'wrong_dns':
                assert last['dns'] == 'failed', 'Wrong system DNS was not detected'
            elif kind == 'partial_ipv6':
                assert last['ipv4'] == 'verified' and last['ipv6'] == 'unavailable', 'Partial IP-family failure was not distinguished'
            else:
                assert last['ipv4'] == 'unavailable', 'Provider outage was not distinguished'
            protection()
            proc = None
            print('PASS ' + kind + ': refused Connected; Network Lock and blocked physical DNS retained', flush=True)
        capture.send_signal(signal.SIGINT)
        capture.wait(timeout=5)
        capture = None
        packets = run('tcpdump', '-nn', '-r', '/run/leaks.pcap').stdout.strip()
        # Old baseline HTTPS peers can retransmit FIN after the lock is enabled.
        # Leakage is an outgoing physical packet, regardless of its source IP.
        # Ethernet source matches the isolated physical NIC even for spoofed IPs.
        local_mac = json.loads(run('ip', '-j', 'link', 'show', 'dev', 'physical0').stdout)[0]['address']
        outgoing = run('tcpdump', '-nn', '-r', '/run/leaks.pcap', 'ether', 'src', local_mac).stdout.strip()
        incoming_baseline = run('tcpdump', '-nn', '-r', '/run/leaks.pcap', 'ether', 'dst', local_mac, 'and', 'src', 'host', '1.1.1.1', 'and', 'src', 'port', '443').stdout.strip()
        print('Capture counts: total=' + str(len(packets.splitlines())) + ', outgoing=' + str(len(outgoing.splitlines())) + ', incoming baseline HTTPS=' + str(len(incoming_baseline.splitlines())), flush=True)
        assert not outgoing, 'Physical packet capture detected outgoing DNS/TLS bypass'
        print('PASS physical packet capture: no outgoing plaintext DNS/DoT or direct HTTPS probe escaped across live transitions', flush=True)
        run(binary, 'lock-disable')
        assert underlay_http() and underlay_dns() and underlay_dns(True), 'Explicit disable did not restore physical access'
        assert public_ip() == baseline_ip, 'Explicit disable did not restore baseline internet'
        print('PASS explicit lock-disable restores baseline public internet and physical DNS', flush=True)
    finally:
        if proc is not None and proc.poll() is None:
            proc.kill()
            proc.wait(timeout=5)
        if capture is not None:
            capture.terminate()
            capture.wait(timeout=5)
        # All worker resources are private to these namespaces.
        run(binary, 'lock-disable', check=False)


def parent(binary, requests):
    suffix = uuid.uuid4().hex[:8]
    namespace = 'wave-live-' + suffix
    uplink = 'wl' + suffix
    table = 'wave_live_' + suffix
    old_forward = Path('/proc/sys/net/ipv4/ip_forward').read_text()
    httpd = None
    sockets = []
    try:
        run('ip', 'netns', 'add', namespace)
        run('ip', 'link', 'add', uplink, 'type', 'veth', 'peer', 'name', 'physical0', 'netns', namespace)
        run('ip', 'address', 'add', '198.19.254.1/30', 'dev', uplink)
        run('ip', 'link', 'set', uplink, 'up')
        run('ip', '-n', namespace, 'address', 'add', '198.19.254.2/30', 'dev', 'physical0')
        run('ip', '-n', namespace, 'link', 'set', 'physical0', 'up')
        run('ip', '-n', namespace, 'route', 'add', 'default', 'via', '198.19.254.1')
        Path('/proc/sys/net/ipv4/ip_forward').write_text('1')
        run('nft', '-f', '-', data=f'table ip {table} {{\n chain postrouting {{\n type nat hook postrouting priority srcnat;\n ip saddr 198.19.254.2 masquerade\n }}\n}}\n')
        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'wave-private-listener')
            def log_message(self, *_):
                pass
        httpd = http.server.ThreadingHTTPServer(('198.19.254.1', 18973), Handler)
        threading.Thread(target=httpd.serve_forever, daemon=True).start()
        def echo(sock):
            while True:
                try:
                    data, address = sock.recvfrom(512)
                    sock.sendto(data, address)
                except OSError:
                    return
        udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        udp.bind(('198.19.254.1', 53))
        sockets.append(udp)
        threading.Thread(target=echo, args=(udp,), daemon=True).start()
        tcp = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        tcp.bind(('198.19.254.1', 53))
        tcp.listen()
        sockets.append(tcp)
        def accept():
            while True:
                try:
                    connection, _ = tcp.accept()
                    connection.close()
                except OSError:
                    return
        threading.Thread(target=accept, daemon=True).start()
        result = subprocess.run(['ip', 'netns', 'exec', namespace, 'unshare', '--mount', '--pid', '--fork', '--kill-child=KILL', '--mount-proc', sys.executable, str(Path(__file__).resolve()), binary, '--isolated'], input=json.dumps(requests), text=True, timeout=480)
        if result.returncode:
            raise RuntimeError('Isolated live test failed')
    finally:
        if httpd:
            httpd.shutdown()
        for sock in sockets:
            sock.close()
        run('ip', 'netns', 'delete', namespace, check=False)
        run('ip', 'link', 'delete', uplink, check=False)
        run('nft', 'delete', 'table', 'ip', table, check=False)
        Path('/proc/sys/net/ipv4/ip_forward').write_text(old_forward)


if __name__ == '__main__':
    assert os.geteuid() == 0, 'Run only as root inside a disposable Linux VM'
    assert os.environ.get('WAVE_LIVE_TEST_VM') == '1', 'Set WAVE_LIVE_TEST_VM=1 inside the disposable VM'
    binary = str(Path(sys.argv[1]).resolve())
    requests = json.load(sys.stdin)
    if '--isolated' in sys.argv:
        main(binary, requests)
    else:
        parent(binary, requests)
