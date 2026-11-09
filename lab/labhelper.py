# SPDX-License-Identifier: GPL-3.0-or-later
"""Proxmox host-side helper for labctl (stdlib only, no installation).

labctl sends this file over SSH and runs it with ``python3 -c``. It talks to a
guest's QEMU monitor (QMP) and QEMU guest agent (QGA) Unix sockets, so it keeps
working when the guest network is blocked. Usage:

    labhelper.py qmp VMID JSON_LIST        run QMP commands, print JSON results
    labhelper.py screendump VMID           write a PNG (or PPM) to stdout
    labhelper.py qga VMID JSON [TIMEOUT]   run one guest-agent command
    labhelper.py exec VMID TIMEOUT JSON_ARGV   guest-exec; stdin is input
    labhelper.py read VMID PATH            guest file -> stdout
    labhelper.py write VMID PATH           stdin -> guest file
    labhelper.py serial VMID SECONDS [TEXT [REGEX]]  send TEXT, read until REGEX
"""
import base64
import json
import os
import random
import re
import socket
import sys
import tempfile
import time

RUN_DIR = os.environ.get('WAVE_LAB_RUN_DIR', '/var/run/qemu-server')
CHUNK = 48 * 1024


class HelperError(Exception):
    pass


class JsonSocket:
    """Newline-agnostic JSON stream over a Unix socket."""

    def __init__(self, path, timeout):
        if not os.path.exists(path):
            raise HelperError('socket not found: %s (is the VM running?)' % path)
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(timeout)
        self.sock.connect(path)
        self.buf = b''
        self.decoder = json.JSONDecoder()

    def settimeout(self, timeout):
        self.sock.settimeout(timeout)

    def send(self, obj):
        self.sock.sendall(json.dumps(obj).encode() + b'\n')

    def discard_until(self, marker):
        while True:
            idx = self.buf.find(marker)
            if idx >= 0:
                self.buf = self.buf[idx + len(marker):]
                return
            self.buf = b''
            self._fill()

    def _fill(self):
        data = self.sock.recv(65536)
        if not data:
            raise HelperError('socket closed by QEMU')
        self.buf += data

    def recv(self):
        while True:
            text = self.buf.lstrip(b' \r\n\t\xff')
            if text:
                try:
                    obj, end = self.decoder.raw_decode(text.decode('utf-8', 'replace'))
                except ValueError:
                    pass
                else:
                    consumed = len(text.decode('utf-8', 'replace')[:end].encode('utf-8', 'replace'))
                    self.buf = text[consumed:]
                    return obj
            self._fill()

    def close(self):
        self.sock.close()


class Qmp:
    def __init__(self, vmid, timeout=30):
        self.js = JsonSocket(os.path.join(RUN_DIR, '%s.qmp' % vmid), timeout)
        greeting = self.js.recv()
        if 'QMP' not in greeting:
            raise HelperError('unexpected QMP greeting')
        self.command('qmp_capabilities')

    def command(self, name, arguments=None):
        msg = {'execute': name}
        if arguments:
            msg['arguments'] = arguments
        self.js.send(msg)
        while True:
            reply = self.js.recv()
            if 'event' in reply:
                continue
            if 'error' in reply:
                raise HelperError('%s: %s' % (name, reply['error'].get('desc', reply['error'])))
            return reply.get('return')

    def close(self):
        self.js.close()


class Qga:
    def __init__(self, vmid, timeout=10):
        self.js = JsonSocket(os.path.join(RUN_DIR, '%s.qga' % vmid), timeout)
        token = random.randint(1, 2 ** 31)
        # guest-sync-delimited flushes stale output: the reply follows a 0xFF byte.
        self.js.sock.sendall(b'\xff')
        self.js.send({'execute': 'guest-sync-delimited', 'arguments': {'id': token}})
        try:
            while True:
                self.js.discard_until(b'\xff')
                reply = self.js.recv()
                if reply.get('return') == token:
                    break
        except socket.timeout:
            raise HelperError('guest agent did not answer (not running or not installed?)')

    def command(self, name, arguments=None, timeout=None):
        msg = {'execute': name}
        if arguments is not None:
            msg['arguments'] = arguments
        if timeout is not None:
            self.js.settimeout(timeout)
        self.js.send(msg)
        reply = self.js.recv()
        if 'error' in reply:
            raise HelperError('%s: %s' % (name, reply['error'].get('desc', reply['error'])))
        return reply.get('return')

    def close(self):
        self.js.close()


def op_qmp(vmid, commands_json):
    commands = json.loads(commands_json)
    qmp = Qmp(vmid)
    results = []
    try:
        for cmd in commands:
            if cmd.get('execute') == 'sleep':
                time.sleep(float(cmd.get('seconds', 0.05)))
                results.append(None)
                continue
            results.append(qmp.command(cmd['execute'], cmd.get('arguments')))
    finally:
        qmp.close()
    print(json.dumps(results))


def op_screendump(vmid):
    fd, path = tempfile.mkstemp(prefix='wave-lab-screen-', suffix='.img', dir='/tmp')
    os.close(fd)
    os.unlink(path)  # QEMU creates the file itself.
    qmp = Qmp(vmid)
    try:
        try:
            qmp.command('screendump', {'filename': path, 'format': 'png'})
        except HelperError:
            # Older QEMU: PPM only; labctl converts it.
            qmp.command('screendump', {'filename': path})
        with open(path, 'rb') as handle:
            data = handle.read()
    finally:
        qmp.close()
        try:
            os.unlink(path)
        except OSError:
            pass
    sys.stdout.buffer.write(data)


def op_qga(vmid, command_json, timeout='10'):
    cmd = json.loads(command_json)
    qga = Qga(vmid, float(timeout))
    try:
        print(json.dumps(qga.command(cmd['execute'], cmd.get('arguments'))))
    finally:
        qga.close()


def op_exec(vmid, timeout, argv_json):
    argv = json.loads(argv_json)
    deadline = time.time() + float(timeout)
    stdin = sys.stdin.buffer.read() if not sys.stdin.isatty() else b''
    qga = Qga(vmid)
    try:
        args = {'path': argv[0], 'arg': argv[1:], 'capture-output': True}
        if stdin:
            args['input-data'] = base64.b64encode(stdin).decode()
        pid = qga.command('guest-exec', args)['pid']
        delay = 0.05
        while True:
            status = qga.command('guest-exec-status', {'pid': pid})
            if status.get('exited'):
                break
            if time.time() > deadline:
                try:
                    qga.command('guest-exec', {'path': '/bin/kill', 'arg': ['-KILL', str(pid)]})
                except HelperError:
                    pass
                print(json.dumps({'exitcode': 124, 'timeout': True, 'pid': pid,
                                  'out': status.get('out-data', ''), 'err': status.get('err-data', '')}))
                return
            time.sleep(delay)
            delay = min(delay * 2, 0.5)
    finally:
        qga.close()
    code = status.get('exitcode')
    if code is None and 'signal' in status:
        code = 128 + int(status['signal'])
    print(json.dumps({'exitcode': code, 'out': status.get('out-data', ''),
                      'err': status.get('err-data', ''),
                      'truncated': bool(status.get('out-truncated') or status.get('err-truncated'))}))


def op_read(vmid, path):
    qga = Qga(vmid, 30)
    try:
        handle = qga.command('guest-file-open', {'path': path, 'mode': 'rb'})
        try:
            while True:
                part = qga.command('guest-file-read', {'handle': handle, 'count': CHUNK})
                if part.get('count'):
                    sys.stdout.buffer.write(base64.b64decode(part['buf-b64']))
                if part.get('eof') or not part.get('count'):
                    break
        finally:
            qga.command('guest-file-close', {'handle': handle})
    finally:
        qga.close()


def op_write(vmid, path):
    data = sys.stdin.buffer.read()
    qga = Qga(vmid, 30)
    try:
        handle = qga.command('guest-file-open', {'path': path, 'mode': 'wb'})
        try:
            for start in range(0, len(data), CHUNK):
                part = data[start:start + CHUNK]
                qga.command('guest-file-write', {'handle': handle,
                                                 'buf-b64': base64.b64encode(part).decode()})
            qga.command('guest-file-flush', {'handle': handle})
        finally:
            qga.command('guest-file-close', {'handle': handle})
    finally:
        qga.close()
    print(json.dumps({'written': len(data)}))


def op_serial(vmid, seconds, text='', until=''):
    path = os.path.join(RUN_DIR, '%s.serial0' % vmid)
    if not os.path.exists(path):
        raise HelperError('no serial0 socket for %s' % vmid)
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.settimeout(5)
    sock.connect(path)
    try:
        if text:
            sock.sendall(text.encode().decode('unicode_escape').encode('latin-1'))
        end = time.time() + float(seconds)
        seen = b''
        while True:
            left = end - time.time()
            if left <= 0:
                break
            sock.settimeout(left)
            try:
                data = sock.recv(65536)
            except socket.timeout:
                break
            if not data:
                break
            sys.stdout.buffer.write(data)
            sys.stdout.buffer.flush()
            if until:
                seen = (seen + data)[-65536:]
                if re.search(until.encode(), seen):
                    break
    finally:
        sock.close()


OPS = {'qmp': op_qmp, 'screendump': op_screendump, 'qga': op_qga, 'exec': op_exec,
       'read': op_read, 'write': op_write, 'serial': op_serial}


def main(argv):
    if len(argv) < 3 or argv[1] not in OPS or not argv[2].isdigit():
        sys.stderr.write(__doc__)
        return 2
    try:
        OPS[argv[1]](*argv[2:])
    except (HelperError, OSError, ValueError, KeyError, TypeError) as exc:
        sys.stderr.write('labhelper: %s\n' % exc)
        return 3
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
