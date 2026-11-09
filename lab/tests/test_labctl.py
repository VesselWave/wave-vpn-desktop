# SPDX-License-Identifier: GPL-3.0-or-later
"""Offline tests for labctl and labhelper. Run: python3 -m unittest discover -s lab/tests -v

`ssh` is replaced by a stub on PATH that plays a fake Proxmox host and guest;
labhelper's QMP/QGA code is exercised against in-process fake Unix sockets.
"""
import base64
import importlib.machinery
import importlib.util
import json
import os
import shlex
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import unittest
import zlib
from pathlib import Path

LAB = Path(__file__).resolve().parents[1]


def load(name, path):
    loader = importlib.machinery.SourceFileLoader(name, str(path))
    spec = importlib.util.spec_from_loader(name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


labctl = load('labctl', LAB / 'labctl')
labhelper = load('labhelper', LAB / 'labhelper.py')

# The stub reads a scenario JSON and appends every call to a log.
FAKE_SSH = r'''#!/usr/bin/env python3
import base64, json, os, re, shlex, sys, zlib
scenario = json.load(open(os.environ['FAKE_SCENARIO']))
args = sys.argv[1:]
with open(os.environ['FAKE_LOG'], 'a') as log:
    log.write(json.dumps(args) + '\n')
target = next(a for a in args if '@' in a and not a.startswith('-'))
command = args[-1]
stdin = b'' if sys.stdin.isatty() else sys.stdin.buffer.read()
def done(out=b'', err=b'', rc=0):
    sys.stdout.buffer.write(out if isinstance(out, bytes) else out.encode())
    sys.stderr.buffer.write(err if isinstance(err, bytes) else err.encode())
    sys.exit(rc)
if target == scenario['host']:
    if command.startswith('python3 -c '):
        parts = shlex.split(command)
        op, vmid, rest = parts[3], parts[4], parts[5:]
        with open(os.environ['FAKE_LOG'], 'a') as log:
            log.write(json.dumps({'helper': op, 'vmid': vmid, 'args': rest,
                                  'stdin': base64.b64encode(stdin).decode()}) + '\n')
        if op == 'qga':
            cmd = json.loads(rest[0])['execute']
            if cmd == 'guest-ping':
                done('{}', rc=0 if scenario.get('agent', True) else 3)
            if cmd == 'guest-network-get-interfaces':
                done(json.dumps([{'name': 'lo', 'ip-addresses': [{'ip-address-type': 'ipv4', 'ip-address': '127.0.0.1'}]},
                                 {'name': 'wg0', 'ip-addresses': [{'ip-address-type': 'ipv4', 'ip-address': '10.9.0.2'}]},
                                 {'name': 'enp6s18', 'ip-addresses': [{'ip-address-type': 'ipv4', 'ip-address': scenario['guest_ip']}]}]))
        if op == 'read':
            if rest[0] == '/etc/ssh/ssh_host_ed25519_key.pub':
                done('ssh-ed25519 AAAAFAKEKEY root@guest\n')
            done(scenario.get('files', {}).get(rest[0], ''))
        if op == 'write':
            done(json.dumps({'written': len(stdin)}))
        if op == 'exec':
            argv = json.loads(rest[1])
            out = scenario.get('agent_out', 'from-agent\n')
            done(json.dumps({'exitcode': scenario.get('agent_rc', 0), 'out': base64.b64encode(out.encode()).decode(), 'err': ''}))
        if op == 'screendump':
            done(base64.b64decode(scenario['screen']))
        if op == 'qmp':
            done(json.dumps([None for _ in json.loads(rest[0])]))
        done(err='unknown helper op', rc=2)
    for pattern, reply in scenario.get('host_replies', []):
        if re.search(pattern, command):
            done(reply.get('out', ''), reply.get('err', ''), reply.get('rc', 0))
    done()
# guest ssh
if scenario.get('guest_ssh') == 'down':
    done(err='ssh: connect to host %s port 22: Operation timed out\n' % target.split('@')[1], rc=255)
if scenario.get('guest_ssh') == 'dropped':
    done(err='Connection to %s closed by remote host.\n' % target.split('@')[1], rc=255)
done(scenario.get('guest_out', 'from-ssh\n'), rc=scenario.get('guest_rc', 0))
'''

RESOURCES = [
    {'vmid': 101, 'name': 'other-guest', 'type': 'qemu', 'status': 'running', 'tags': ''},
    {'vmid': 113, 'name': 'wave-legacy', 'type': 'qemu', 'status': 'stopped', 'tags': ''},
    {'vmid': 300, 'name': 'wave-tpl-ubuntu2404', 'type': 'qemu', 'status': 'stopped', 'tags': 'wave-lab',
     'template': 1},
    {'vmid': 310, 'name': 'wave-a', 'type': 'qemu', 'status': 'running', 'tags': 'wave-lab'},
    {'vmid': 311, 'name': 'wave-b', 'type': 'qemu', 'status': 'running', 'tags': 'wave-lab;x'},
    {'vmid': 312, 'name': 'wave-c', 'type': 'qemu', 'status': 'stopped', 'tags': 'wave-lab'},
    {'vmid': 320, 'name': 'wave-untagged', 'type': 'qemu', 'status': 'stopped', 'tags': ''},
]


def png_bytes(width, height):
    ppm = b'P6\n# comment\n%d %d\n255\n' % (width, height) + bytes(range(3)) * (width * height)
    return labctl.ppm_to_png(ppm)


class StubbedHost(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.lab_dir = root / '.lab'
        (self.lab_dir / 'ssh').mkdir(parents=True)
        (self.lab_dir / 'ssh' / 'id_ed25519').write_text('fake')
        (self.lab_dir / 'pw').write_text('Secret-pw_1\n')
        self.config = self.lab_dir / 'config.json'
        self.config.write_text(json.dumps({
            'host': 'root@pve.invalid', 'ssh_key': 'ssh/id_ed25519', 'password_file': 'pw',
            'vmid_min': 300, 'vmid_max': 349, 'clone_vmid_min': 310, 'max_running': 2}))
        bindir = root / 'bin'
        bindir.mkdir()
        (bindir / 'ssh').write_text(FAKE_SSH.replace('/usr/bin/env python3', sys.executable))
        (bindir / 'ssh').chmod(0o755)
        self.log = root / 'calls.log'
        self.scenario_path = root / 'scenario.json'
        self.scenario = {
            'host': 'root@pve.invalid', 'guest_ip': '192.0.2.50',
            'screen': base64.b64encode(png_bytes(1280, 800)).decode(),
            'host_replies': [
                [r'^pvesh get /nodes/localhost/qemu ', {'out': json.dumps(RESOURCES)}],
                [r'^pvesh get /nodes/localhost/lxc ', {'out': json.dumps([{'vmid': 313, 'name': 'ct'}])}],
                [r'^qm status 310', {'out': 'status: running\n'}],
                [r'^qm status 312', {'out': 'status: stopped\n'}],
                [r'^qm config', {'out': 'smbios1: uuid=1234abcd-0000-1111-2222-333344445555\n'}],
            ]}
        self.env = dict(os.environ, PATH='%s%s%s' % (bindir, os.pathsep, os.environ['PATH']),
                        WAVE_LAB_CONFIG=str(self.config), FAKE_LOG=str(self.log),
                        FAKE_SCENARIO=str(self.scenario_path))

    def tearDown(self):
        self.tmp.cleanup()

    def labctl(self, *args, stdin=None):
        self.scenario_path.write_text(json.dumps(self.scenario))
        return subprocess.run([sys.executable, str(LAB / 'labctl')] + list(args), env=self.env,
                              input=stdin, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def calls(self):
        if not self.log.exists():
            return []
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def host_commands(self):
        return [c[-1] for c in self.calls() if isinstance(c, list) and 'root@pve.invalid' in c]

    def helper_calls(self, op=None):
        return [c for c in self.calls() if isinstance(c, dict) and (op is None or c['helper'] == op)]


class GuardTests(StubbedHost):
    def test_list_shows_only_tagged_vms_in_range(self):
        proc = self.labctl('list')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        out = proc.stdout.decode()
        self.assertIn('wave-tpl-ubuntu2404', out)
        self.assertIn('template', out)
        for absent in ('other-guest', 'wave-legacy', 'wave-untagged'):
            self.assertNotIn(absent, out)

    def test_refuses_foreign_and_untagged_vms(self):
        for ref in ('101', 'wave-legacy', 'wave-untagged'):
            proc = self.labctl('destroy', ref)
            self.assertEqual(proc.returncode, 1)
            self.assertIn(b'refusing to touch', proc.stderr)
        self.assertFalse([c for c in self.host_commands() if c.startswith('qm destroy')])

    def test_destroy_template_needs_flag(self):
        proc = self.labctl('destroy', 'wave-tpl-ubuntu2404')
        self.assertEqual(proc.returncode, 1)
        self.assertIn(b'--template', proc.stderr)

    def test_destroy_stops_then_purges(self):
        proc = self.labctl('destroy', 'wave-a')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        commands = self.host_commands()
        self.assertIn('qm stop 310', commands)
        self.assertIn('qm destroy 310 --purge 1', commands)
        self.assertLess(commands.index('qm stop 310'), commands.index('qm destroy 310 --purge 1'))

    def test_start_respects_running_limit(self):
        proc = self.labctl('start', 'wave-c')
        self.assertEqual(proc.returncode, 1)
        self.assertIn(b'limit is 2', proc.stderr)
        proc = self.labctl('start', 'wave-c', '--force')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn('qm start 312', self.host_commands())

    def test_clone_is_linked_tagged_and_snapshotted(self):
        proc = self.labctl('clone', 'wave-tpl-ubuntu2404', 'wave-new')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        commands = self.host_commands()
        self.assertIn('qm clone 300 314 --name wave-new --full 0', commands)
        sets = [c for c in commands if c.startswith('qm set 314')]
        self.assertEqual(len(sets), 1)
        self.assertIn('--tags wave-lab', sets[0])
        serial = base64.b64encode(b'wave-new').decode()
        self.assertIn('uuid=1234abcd-0000-1111-2222-333344445555,serial=%s,base64=1' % serial, sets[0])
        self.assertTrue(any(c.startswith('qm snapshot 314 clean-desktop') for c in commands))

    def test_clone_rejects_bad_names_and_non_templates(self):
        self.assertIn(b'name must start', self.labctl('clone', 'wave-tpl-ubuntu2404', 'Bad_Name').stderr)
        self.assertIn(b'not a template', self.labctl('clone', 'wave-a', 'wave-z').stderr)
        self.assertIn(b'already exists', self.labctl('clone', 'wave-tpl-ubuntu2404', 'wave-b').stderr)


class RunTests(StubbedHost):
    def test_run_over_ssh_pins_agent_host_key(self):
        proc = self.labctl('run', 'wave-a', '--', 'echo', 'hi')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout, b'from-ssh\n')
        guest = [c for c in self.calls() if isinstance(c, list) and 'wave-lab@192.0.2.50' in c]
        self.assertEqual(len(guest), 1)
        self.assertEqual(guest[0][-1], "bash -c 'echo hi'")
        known = (self.lab_dir / 'state' / '310.known_hosts').read_text()
        self.assertEqual(known, '192.0.2.50 ssh-ed25519 AAAAFAKEKEY\n')
        self.assertIn('StrictHostKeyChecking=yes', guest[0])

    def test_run_root_uses_sudo(self):
        self.labctl('run', 'wave-a', '--root', 'id -u')
        guest = [c for c in self.calls() if isinstance(c, list) and 'wave-lab@192.0.2.50' in c]
        self.assertEqual(guest[-1][-1], "sudo -n bash -c 'id -u'")

    def test_falls_back_to_agent_when_ssh_unreachable(self):
        self.scenario['guest_ssh'] = 'down'
        self.scenario['agent_rc'] = 7
        proc = self.labctl('run', 'wave-a', '--json', 'nft list ruleset')
        self.assertEqual(proc.returncode, 7)
        result = json.loads(proc.stdout)
        self.assertEqual(result['via'], 'agent')
        self.assertEqual(result['stdout'], 'from-agent\n')
        execs = self.helper_calls('exec')
        self.assertEqual(json.loads(execs[0]['args'][1]),
                         ['/usr/sbin/runuser', '-l', 'wave-lab', '-c', 'nft list ruleset'])
        # SSH was retried once with a refreshed address before falling back.
        guest = [c for c in self.calls() if isinstance(c, list) and 'wave-lab@192.0.2.50' in c]
        self.assertEqual(len(guest), 2)

    def test_recent_failure_skips_ssh_until_it_expires(self):
        self.scenario['guest_ssh'] = 'down'
        self.labctl('run', 'wave-a', 'true')
        guest = lambda: [c for c in self.calls() if isinstance(c, list) and 'wave-lab@192.0.2.50' in c]
        self.assertEqual(len(guest()), 2)
        self.labctl('run', 'wave-a', 'true')
        self.assertEqual(len(guest()), 2)  # memo: straight to the agent
        self.labctl('run', 'wave-a', '--via', 'ssh', 'true')
        self.assertEqual(len(guest()), 4)  # explicit SSH ignores the memo
        state_file = self.lab_dir / 'state' / '310.json'
        state = json.loads(state_file.read_text())
        state['ssh_down_until'] = 0
        state_file.write_text(json.dumps(state))
        self.scenario['guest_ssh'] = 'up'
        self.assertEqual(self.labctl('run', 'wave-a', 'true').stdout, b'from-ssh\n')
        self.assertNotIn('ssh_down_until', json.loads(state_file.read_text()))

    def test_dropped_connection_is_not_rerun(self):
        self.scenario['guest_ssh'] = 'dropped'
        proc = self.labctl('run', 'wave-a', 'sudo nft add table inet x')
        self.assertEqual(proc.returncode, 255)
        self.assertFalse(self.helper_calls('exec'))

    def test_command_exit_255_is_not_treated_as_transport_failure(self):
        self.scenario['guest_rc'] = 255
        proc = self.labctl('run', 'wave-a', 'exit 255')
        self.assertEqual(proc.returncode, 255)
        self.assertFalse(self.helper_calls('exec'))

    def test_root_agent_and_session_environment(self):
        self.scenario['guest_ssh'] = 'down'
        self.labctl('run', 'wave-a', '--root', '--session', 'gsettings list-schemas')
        argv = json.loads(self.helper_calls('exec')[0]['args'][1])
        self.assertEqual(argv[:4], ['/usr/bin/env', 'HOME=/root', '/bin/bash', '-c'])
        self.assertIn('WAYLAND_DISPLAY=wayland-0', argv[4])
        self.assertIn('$(id -u wave-lab)', argv[4])

    def test_forced_agent_skips_ssh(self):
        proc = self.labctl('run', 'wave-a', '--via', 'agent', 'true')
        self.assertEqual(proc.returncode, 0)
        self.assertFalse([c for c in self.calls() if isinstance(c, list) and 'wave-lab@192.0.2.50' in c])

    def test_stdin_reaches_agent(self):
        self.scenario['guest_ssh'] = 'down'
        self.labctl('run', 'wave-a', '--stdin', 'cat', stdin=b'payload')
        self.assertEqual(base64.b64decode(self.helper_calls('exec')[0]['stdin']), b'payload')


class FileTests(StubbedHost):
    def test_push_agent_fallback_writes_and_chowns(self):
        self.scenario['guest_ssh'] = 'down'
        src = Path(self.tmp.name) / 'data.bin'
        src.write_bytes(b'\x00\x01binary')
        proc = self.labctl('push', 'wave-a', str(src), '/home/wave-lab/data.bin', '--mode', '0600')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        write = self.helper_calls('write')[0]
        self.assertEqual(write['args'], ['/home/wave-lab/data.bin'])
        self.assertEqual(base64.b64decode(write['stdin']), b'\x00\x01binary')
        fix = json.loads(self.helper_calls('exec')[0]['args'][1])
        self.assertIn('chown wave-lab: /home/wave-lab/data.bin', fix[-1])
        self.assertIn('chmod 0600', fix[-1])

    def test_pull_agent_fallback(self):
        self.scenario['guest_ssh'] = 'down'
        self.scenario['files'] = {'/var/log/x': 'log-line\n'}
        dest = Path(self.tmp.name) / 'x'
        proc = self.labctl('pull', 'wave-a', '/var/log/x', str(dest))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(dest.read_text(), 'log-line\n')

    def test_push_over_ssh_streams_stdin(self):
        src = Path(self.tmp.name) / 'f'
        src.write_text('abc')
        proc = self.labctl('push', 'wave-a', str(src), '/tmp/f', '--root')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        guest = [c for c in self.calls() if isinstance(c, list) and 'wave-lab@192.0.2.50' in c]
        self.assertEqual(guest[0][-1], "sudo -n bash -c 'cat > /tmp/f'")


class DisplayTests(StubbedHost):
    def test_screenshot_writes_png(self):
        out = Path(self.tmp.name) / 'shot.png'
        proc = self.labctl('screenshot', 'wave-a', str(out))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertTrue(out.read_bytes().startswith(b'\x89PNG'))
        self.assertIn(b'1280x800', proc.stdout)

    def test_screenshot_converts_ppm(self):
        self.scenario['screen'] = base64.b64encode(b'P6 2 1 255\n' + b'\xff\x00\x00\x00\xff\x00').decode()
        out = Path(self.tmp.name) / 'shot.png'
        self.assertEqual(self.labctl('screenshot', 'wave-a', str(out)).returncode, 0)
        data = out.read_bytes()
        self.assertEqual(labctl.image_size(data), (2, 1))
        idat = data.index(b'IDAT')
        length = int.from_bytes(data[idat - 4:idat], 'big')
        self.assertEqual(zlib.decompress(data[idat + 4:idat + 4 + length]), b'\x00\xff\x00\x00\x00\xff\x00')

    def test_click_scales_to_absolute_axes(self):
        proc = self.labctl('click', 'wave-a', '1279', '400', '--button', 'right')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        commands = json.loads(self.helper_calls('qmp')[0]['args'][0])
        move = commands[0]['arguments']['events']
        self.assertEqual(move[0]['data'], {'axis': 'x', 'value': 0x7FFF})
        self.assertEqual(move[1]['data'], {'axis': 'y', 'value': round(400 * 0x7FFF / 799)})
        buttons = [c['arguments']['events'][0]['data'] for c in commands[1:] if c['execute'] == 'input-send-event']
        self.assertEqual(buttons, [{'down': True, 'button': 'right'}, {'down': False, 'button': 'right'}])

    def test_click_outside_screen_fails(self):
        proc = self.labctl('click', 'wave-a', '1280', '0')
        self.assertEqual(proc.returncode, 1)
        self.assertIn(b'outside screen', proc.stderr)

    def test_type_and_key(self):
        self.labctl('type', 'wave-a', 'Hi!')
        self.labctl('key', 'wave-a', 'ctrl-alt-t', 'enter')
        typed, keys = [json.loads(c['args'][0]) for c in self.helper_calls('qmp')]
        sent = [[k['data'] for k in c['arguments']['keys']] for c in typed if c['execute'] == 'send-key']
        self.assertEqual(sent, [['shift', 'h'], ['i'], ['shift', '1']])
        sent = [[k['data'] for k in c['arguments']['keys']] for c in keys if c['execute'] == 'send-key']
        self.assertEqual(sent, [['ctrl', 'alt', 't'], ['ret']])


class PureFunctionTests(unittest.TestCase):
    def test_combo_keys(self):
        self.assertEqual([k['data'] for k in labctl.combo_keys('super')], ['meta_l'])
        self.assertEqual([k['data'] for k in labctl.combo_keys('-')], ['minus'])
        self.assertEqual([k['data'] for k in labctl.combo_keys('ctrl-shift-+')], ['ctrl', 'shift', 'equal'])

    def test_type_rejects_unmappable(self):
        with self.assertRaises(labctl.LabError):
            labctl.type_commands('é')

    def test_helper_command_roundtrip(self):
        command = labctl.helper_command('qga', 310, '{"execute": "guest-ping"}')
        parts = shlex.split(command)
        self.assertEqual(parts[:2], ['python3', '-c'])
        self.assertEqual(parts[3:], ['qga', '310', '{"execute": "guest-ping"}'])
        packed = parts[2].split('"')[1]
        self.assertEqual(zlib.decompress(base64.b64decode(packed)), (LAB / 'labhelper.py').read_bytes())

    def test_config_search_finds_nearest_private_dir(self):
        with tempfile.TemporaryDirectory() as tmp:
            os.makedirs(os.path.join(tmp, '.lab'))
            path = os.path.join(tmp, '.lab', 'config.json')
            with open(path, 'w') as handle:
                json.dump({'host': 'h', 'ssh_key': 'k', 'vmid_min': 1, 'vmid_max': 2}, handle)
            old = os.getcwd()
            env = os.environ.pop('WAVE_LAB_CONFIG', None)
            try:
                os.makedirs(os.path.join(tmp, 'a', 'b'))
                os.chdir(os.path.join(tmp, 'a', 'b'))
                self.assertEqual(str(labctl.find_config()), os.path.realpath(path))
            finally:
                os.chdir(old)
                if env is not None:
                    os.environ['WAVE_LAB_CONFIG'] = env


class SourceTarballTests(unittest.TestCase):
    def test_git_archive_excludes_untracked_and_git_metadata(self):
        import tarfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp) / 'repo'
            repo.mkdir()
            run = lambda *a: subprocess.run(['git', '-C', str(repo)] + list(a), check=True,
                                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            run('init', '-q')
            (repo / 'Cargo.toml').write_text('version = "1.0.0"\n')
            (repo / '.gitignore').write_text('.env\n')
            run('add', 'Cargo.toml', '.gitignore')
            run('-c', 'user.name=t', '-c', 'user.email=t@example.invalid', 'commit', '-qm', 'x')
            (repo / '.env').write_text('SECRET=1')
            (repo / 'untracked.txt').write_text('x')
            (repo / 'Cargo.toml').write_text('version = "1.0.1"\n')
            for worktree in (False, True):
                dest = Path(tmp) / ('src-%s.tar.gz' % worktree)
                meta = labctl.source_tarball(repo, 'HEAD', worktree, dest)
                with tarfile.open(dest) as tar:
                    names = set(tar.getnames())
                    content = tar.extractfile('Cargo.toml').read()
                self.assertEqual(names, {'Cargo.toml', '.gitignore'})
                self.assertFalse(any(n.startswith('.git/') for n in names))
                self.assertEqual(content, b'version = "1.0.1"\n' if worktree else b'version = "1.0.0"\n')
                self.assertEqual(meta['revision'].endswith('-dirty'), worktree)
                self.assertGreater(meta['source_date_epoch'], 0)


class FakeQemu(threading.Thread):
    """One-connection fake QMP/QGA server."""

    def __init__(self, path, kind, guest_files=None):
        super().__init__(daemon=True)
        self.kind = kind
        self.files = guest_files if guest_files is not None else {}
        self.server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.server.bind(path)
        self.server.listen(1)
        self.received = []

    def run(self):
        conn, _ = self.server.accept()
        stream = conn.makefile('rwb')
        if self.kind == 'qmp':
            stream.write(b'{"QMP": {"version": {}}}\r\n')
            stream.flush()
        handles = {}
        buf = b''
        decoder = json.JSONDecoder()
        while True:
            byte = stream.read(1)
            if not byte:
                return
            if byte == b'\xff':
                continue
            buf = (buf + byte).lstrip()
            try:
                msg, _ = decoder.raw_decode(buf.decode())
            except ValueError:
                continue
            buf = b''
            self.received.append(msg)
            name, args = msg['execute'], msg.get('arguments', {})
            reply = {'return': {}}
            if name == 'guest-sync-delimited':
                stream.write(b'\xff')
                reply = {'return': args['id']}
            elif name == 'query-status':
                stream.write(b'{"event": "NOISE"}\r\n')
                reply = {'return': {'status': 'running'}}
            elif name == 'guest-exec':
                reply = {'return': {'pid': 42}}
            elif name == 'guest-exec-status':
                reply = {'return': {'exited': True, 'exitcode': 3,
                                    'out-data': base64.b64encode(b'out').decode()}}
            elif name == 'guest-file-open':
                handles[1] = [args['path'], bytearray(self.files.get(args['path'], b'')), 0]
                reply = {'return': 1}
            elif name == 'guest-file-read':
                path, data, pos = handles[args['handle']]
                part = bytes(data[pos:pos + args['count']])
                handles[args['handle']][2] += len(part)
                reply = {'return': {'count': len(part), 'buf-b64': base64.b64encode(part).decode(),
                                    'eof': pos + len(part) >= len(data)}}
            elif name == 'guest-file-write':
                handles[args['handle']][1] += base64.b64decode(args['buf-b64'])
                reply = {'return': {'count': 1, 'eof': False}}
            elif name == 'guest-file-close':
                path, data, _ = handles.pop(args['handle'])
                self.files[path] = bytes(data)
            elif name == 'bogus':
                reply = {'error': {'class': 'CommandNotFound', 'desc': 'no bogus'}}
            stream.write(json.dumps(reply).encode() + b'\r\n')
            stream.flush()


class HelperProtocolTests(unittest.TestCase):
    def setUp(self):
        # Short path: Unix socket paths are limited to ~104 bytes on macOS.
        self.dir = tempfile.mkdtemp(prefix='wl', dir='/tmp')
        self.env = dict(os.environ, WAVE_LAB_RUN_DIR=self.dir)

    def tearDown(self):
        for name in os.listdir(self.dir):
            os.unlink(os.path.join(self.dir, name))
        os.rmdir(self.dir)

    def helper(self, server, *args, stdin=b''):
        server.start()
        proc = subprocess.run([sys.executable, str(LAB / 'labhelper.py')] + list(args), env=self.env,
                              input=stdin, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20)
        server.join(5)
        return proc

    def test_qmp_skips_events_and_negotiates(self):
        server = FakeQemu(os.path.join(self.dir, '310.qmp'), 'qmp')
        proc = self.helper(server, 'qmp', '310', json.dumps([{'execute': 'query-status'}]))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(json.loads(proc.stdout), [{'status': 'running'}])
        self.assertEqual([m['execute'] for m in server.received], ['qmp_capabilities', 'query-status'])

    def test_qmp_error_is_reported(self):
        server = FakeQemu(os.path.join(self.dir, '310.qmp'), 'qmp')
        proc = self.helper(server, 'qmp', '310', json.dumps([{'execute': 'bogus'}]))
        self.assertEqual(proc.returncode, 3)
        self.assertIn(b'no bogus', proc.stderr)

    def test_qga_exec_returns_exit_code_and_output(self):
        server = FakeQemu(os.path.join(self.dir, '310.qga'), 'qga')
        proc = self.helper(server, 'exec', '310', '30', json.dumps(['/bin/true', 'a']), stdin=b'in')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        result = json.loads(proc.stdout)
        self.assertEqual(result['exitcode'], 3)
        self.assertEqual(base64.b64decode(result['out']), b'out')
        exec_msg = [m for m in server.received if m['execute'] == 'guest-exec'][0]['arguments']
        self.assertEqual(exec_msg['path'], '/bin/true')
        self.assertEqual(exec_msg['arg'], ['a'])
        self.assertEqual(base64.b64decode(exec_msg['input-data']), b'in')

    def test_qga_file_roundtrip_in_chunks(self):
        files = {}
        payload = os.urandom(labhelper.CHUNK * 2 + 17)
        server = FakeQemu(os.path.join(self.dir, '310.qga'), 'qga', files)
        proc = self.helper(server, 'write', '310', '/tmp/blob', stdin=payload)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(files['/tmp/blob'], payload)
        writes = [m for m in server.received if m['execute'] == 'guest-file-write']
        self.assertEqual(len(writes), 3)
        os.unlink(os.path.join(self.dir, '310.qga'))
        server = FakeQemu(os.path.join(self.dir, '310.qga'), 'qga', files)
        proc = self.helper(server, 'read', '310', '/tmp/blob')
        self.assertEqual(proc.stdout, payload)

    def test_missing_socket(self):
        proc = subprocess.run([sys.executable, str(LAB / 'labhelper.py'), 'qga', '399', '{"execute":"x"}'],
                              env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(proc.returncode, 3)
        self.assertIn(b'is the VM running', proc.stderr)


if __name__ == '__main__':
    unittest.main()
