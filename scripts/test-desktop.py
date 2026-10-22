#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Synthetic rendering on X11 or nested GNOME Wayland; no credentials/network changes.
Run in a disposable GNOME VM under dbus-run-session, with DISPLAY=:1.
"""
import argparse
import json
import stat
import os
from pathlib import Path
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary', type=Path)
parser.add_argument('--wayland', action='store_true')
parser.add_argument('--interact', action='store_true', help='X11 copy/save interaction; requires xdotool and xclip')
parser.add_argument('--captures', type=Path, required=True)
args = parser.parse_args()
args.captures.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='wave-desktop-smoke-') as directory:
    base = Path(directory)
    env = dict(os.environ)
    for key, folder in [('XDG_CONFIG_HOME','config'), ('XDG_DATA_HOME','data'), ('XDG_RUNTIME_DIR','run')]:
        path = base / folder
        path.mkdir(mode=0o700)
        env[key] = str(path)
    compositor = None
    compositor_log = (base / 'compositor.log').open('wb')
    try:
        if args.wayland:
            compositor = subprocess.Popen(['gnome-shell','--nested','--wayland','--wayland-display=wave-smoke','--no-x11'], env=env, stdout=compositor_log, stderr=compositor_log)
            for _ in range(100):
                if (base / 'run/wave-smoke').exists():
                    break
                assert compositor.poll() is None, 'Nested GNOME compositor exited'
                time.sleep(.1)
            else:
                raise RuntimeError('Wayland socket unavailable')
            env.pop('DISPLAY', None)
            env['WAYLAND_DISPLAY'] = 'wave-smoke'
        for scene in ['diagnostics', 'unknown', 'connected']:
            output = args.captures / ('wayland' if args.wayland else 'x11')
            output.mkdir(exist_ok=True)
            target = output / ('new-' + scene + '.png')
            target.unlink(missing_ok=True)
            scene_env = dict(env, WAVE_PREVIEW='1', WAVE_PREVIEW_SCENE=scene, WAVE_CAPTURE_DIR=str(output))
            with (base / 'app.log').open('wb') as log:
                proc = subprocess.Popen([str(args.binary.resolve())], env=scene_env, stdout=log, stderr=log)
                try:
                    for _ in range(150):
                        assert proc.poll() is None, 'GUI exited before capture'
                        if target.exists() and target.stat().st_size > 1000:
                            break
                        time.sleep(.1)
                    else:
                        raise RuntimeError('GUI did not capture ' + scene)
                    if args.interact and not args.wayland and scene == 'diagnostics':
                        def xdo(*command):
                            return subprocess.run(['xdotool',*map(str,command)], env=scene_env, capture_output=True, text=True, check=True).stdout
                        window = xdo('search','--onlyvisible','--pid',proc.pid).splitlines()[-1]
                        geometry = dict(line.split('=',1) for line in xdo('getwindowgeometry','--shell',window).splitlines())
                        height = int(geometry['HEIGHT'])
                        xdo('windowactivate','--sync',window)
                        xdo('mousemove','--window',window,260,height-38,'click',1)
                        time.sleep(.3)
                        copied = subprocess.run(['xclip','-selection','clipboard','-o'], env=scene_env, capture_output=True, check=True, timeout=5).stdout
                        preview = json.loads(copied)
                        assert preview['schema'] == 2 and 'redaction' in preview
                        report_path = base / 'reviewed-report.json'
                        xdo('mousemove','--window',window,450,height-75,'click',1)
                        xdo('key','--clearmodifiers','ctrl+a')
                        xdo('type','--clearmodifiers','--delay',1,'--',report_path)
                        xdo('mousemove','--window',window,350,height-38,'click',1)
                        for _ in range(50):
                            if report_path.exists():
                                break
                            time.sleep(.1)
                        assert report_path.read_bytes() == copied, 'Saved report differs from preview/copied report'
                        assert stat.S_IMODE(report_path.stat().st_mode) == 0o600
                        print('PASS GUI Copy report and Save report export exactly the preview with private permissions', flush=True)
                    print('PASS ' + ('GNOME Wayland' if args.wayland else 'GNOME X11') + ': ' + scene + ' rendered', flush=True)
                finally:
                    proc.terminate()
                    proc.wait(timeout=10)
    finally:
        if compositor is not None:
            compositor.terminate()
            compositor.wait(timeout=15)
        compositor_log.close()
