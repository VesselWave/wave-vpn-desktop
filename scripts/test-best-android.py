#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Run the standalone quality tests against an Android checkout without editing it.
Usage: python3 scripts/test-best-android.py /path/to/android-checkout
Requires that checkout's Gradle dependencies already cached and Java 17.
"""
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

source = Path(sys.argv[1]).resolve()
repo = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='wave-best-android-') as temporary:
    target = Path(temporary)
    for name in ('core', 'gradle', 'protocol-data'):
        shutil.copytree(source / name, target / name,
                        ignore=shutil.ignore_patterns('build', '.gradle'))
    shutil.copy2(source / 'gradlew', target / 'gradlew')
    (target / 'build.gradle.kts').write_text(
        'plugins { alias(libs.plugins.kotlin.jvm) apply false }\n')
    settings = (source / 'settings.gradle.kts').read_text()
    assert 'include(":core", ":app")' in settings, 'Review changed Android settings'
    (target / 'settings.gradle.kts').write_text(
        settings.replace('include(":core", ":app")', 'include(":core")'))
    shutil.copy2(repo / 'tests/BestQualityTest.kt', target /
                 'core/src/test/kotlin/me/vwave/vpn/core/BestQualityTest.kt')
    subprocess.run(['./gradlew', ':core:test', '--tests', '*BestQualityTest',
                    '--offline', '--console=plain'], cwd=target, check=True)
    import xml.etree.ElementTree as ET
    report = ET.parse(target / 'core/build/test-results/test/TEST-me.vwave.vpn.core.BestQualityTest.xml')
    print(report.findtext('system-out', default=''))
