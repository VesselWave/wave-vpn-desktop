#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Offline forum/parity contracts for Wave VPN for Linux; failed contracts exit 1.

Runs every `forum_` Rust test (tests/forum_contracts.rs and the GUI cases in
tests/support/forum_gui_cases.rs) and checks that each named contract from
docs/qa/DESKTOP-BUG-INTAKE.md and docs/qa/ANDROID-0.1.5-PARITY.md ran and
passed. No account, VM, helper, socket or network is used; Cargo may download
build dependencies. Exit codes: 0 all passed, 1 a contract failed, 2 a
contract is missing or ignored, or the build failed.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
FILTER = 'forum_'
# Every contract the intake/parity documents point to. A rename or deletion
# must update this list, or the run reports it missing.
REQUIRED = (
    'forum_p01_refresh_failure_or_deadline_does_not_gate_cached_connect',
    'forum_p02_invalid_optional_bootstrap_keeps_valid_servers',
    'forum_d03_retained_network_lock_says_traffic_is_blocked',
    'forum_d03_turn_off_network_lock_from_overview_is_an_explicit_disable',
    'forum_d04_cancel_is_reachable_while_connecting_and_waiting_to_retry',
    'forum_d05_user_disconnect_does_not_penalize_the_server',
    'forum_d05_cancelled_connect_does_not_penalize_the_server',
    'forum_d05_failed_connection_still_penalizes_and_fails_over',
    'forum_d06_missing_helper_is_explained_instead_of_unknown_status',
    'forum_d06_helper_refusal_explains_the_failed_connection',
    'forum_d06_declined_administrator_prompt_is_explained',
    'forum_d06_missing_pkexec_is_explained',
    'forum_d07_target_matching_no_server_is_refused_with_its_name',
    'forum_d09_sign_in_deadline_settles_as_an_ordinary_failure',
    'forum_d09_user_cancel_settles_without_failure_or_penalty',
    'forum_d09_cancel_during_startup_restore_keeps_the_saved_account',
    'forum_d09_sign_in_cancel_stops_failover_alongside_the_session_flag',
    'forum_d09_sign_in_deadline_is_45_seconds_with_shared_copy',
    'forum_p08_state_and_secondary_text_meet_contrast_in_both_themes',
    'forum_p09_device_choice_is_hidden_for_a_single_device_account',
    'forum_d10_one_malformed_server_address_keeps_the_other_servers',
    'forum_d10_server_left_without_entries_refuses_connect_clearly',
    'forum_d11_manifest_without_servers_is_rejected_so_the_cache_survives',
    'forum_d11_manifest_without_wireguard_modes_is_rejected',
    'forum_d11_malformed_wireguard_mode_is_skipped_not_fatal',
    'forum_copy_error_banner_capitalizes_network_lock',
    # docs/qa/DISTRO-UBUNTU-2404.md (B-ids); package hooks are covered by
    # scripts/tests/test_packaging_scripts.py.
    'forum_u2404_b01_replaced_helper_executable_is_detected',
    'forum_u2404_b04_failure_banner_names_the_reason',
    'forum_u2404_b05_disconnect_and_recover_clear_a_stale_error',
    'forum_u2404_b06_leftover_tunnel_is_not_reported_as_blocking',
    'forum_u2404_b07_servers_button_does_not_reconnect_to_the_current_server',
    'forum_u2404_b09_removed_app_says_so_instead_of_offering_setup',
    'forum_u2404_b10_cli_and_gui_report_their_version',
    'forum_u2404_b10_gui_rejects_unknown_arguments',
    'forum_u2404_b11_background_events_wake_the_ui_without_fast_polling',
    'forum_u2404_b14_lost_network_lock_is_reported_and_restored',
    'forum_u2404_b14_saved_choice_says_the_lock_returns_on_connect',
    'forum_u2404_b14_lock_loss_needs_wave_state_and_a_missing_table',
    'forum_u2404_b19_second_launch_shows_the_first_window',
    'forum_u2404_b19_stale_socket_after_a_crash_is_replaced',
    'forum_u2404_b21_country_list_uses_names_plurals_and_name_order',
    'forum_u2404_b22_foreign_resolv_conf_is_reasserted',
    'forum_u2404_b22_recover_keeps_a_resolver_changed_outside_wave',
    'forum_u2404_b22_networkmanager_leaves_resolv_conf_alone_during_a_session',
    'forum_u2404_b22_repeated_rewrites_warn_once_per_minute',
)
RESULT = re.compile(r'^test (\S+) \.\.\. (ok|FAILED|ignored)\b')
STATUS = {'ok': 'passed', 'FAILED': 'failed', 'ignored': 'skipped'}


def collect(output):
    """Test cases from libtest's plain output, with the first panic line of failures."""
    cases, details, current = {}, {}, None
    for line in output.splitlines():
        match = RESULT.match(line)
        if match:
            cases[match.group(1)] = STATUS[match.group(2)]
            continue
        header = re.match(r'^---- (\S+) stdout ----$', line)
        if header:
            current = header.group(1)
            continue
        if current and current not in details and line.strip() and 'panicked at' not in line:
            details[current] = line.strip()[:500]
    return [{'id': name, 'status': status, 'detail': details.get(name, '') if status == 'failed' else ''}
            for name, status in sorted(cases.items())]


def missing(cases):
    present = {case['id'].rsplit('::', 1)[-1] for case in cases}
    return sorted(set(REQUIRED) - present)


def outcome(cases, returncode, absent=()):
    # Missing/ignored contracts and build failures are never a pass.
    if absent or not cases or any(c['status'] == 'skipped' for c in cases):
        return 2
    if any(c['status'] == 'failed' for c in cases):
        return 1
    return 2 if returncode else 0


def git(*args):
    try:
        return subprocess.check_output(['git', '-C', str(ROOT), *args], text=True).strip()
    except (OSError, subprocess.CalledProcessError):
        return ''


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
    parser.add_argument('--report', type=Path, default=target / 'forum/contracts.json')
    args = parser.parse_args(argv)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    log = args.report.with_suffix('.log')
    command = ['cargo', 'test', '--locked', '--all-targets', '--no-fail-fast', '--', FILTER]
    run = subprocess.run(command, cwd=ROOT, env=os.environ.copy(), capture_output=True, text=True)
    log.write_text(run.stdout + '\n' + run.stderr)
    cases = [c for c in collect(run.stdout) if c['id'].rsplit('::', 1)[-1].startswith(FILTER)]
    absent = missing(cases)
    code = outcome(cases, run.returncode, absent)
    report = {
        'schema': 1,
        'scope': 'offline Rust contracts for forum/Android-parity defects; no VM, helper or provider qualification',
        'source': git('rev-parse', 'HEAD'),
        'working_tree_dirty': bool(git('status', '--porcelain')),
        'command': command,
        'exit_code': code,
        'cargo_exit_code': run.returncode,
        'missing_contracts': absent,
        'counts': {state: sum(c['status'] == state for c in cases) for state in ('passed', 'failed', 'skipped')},
        'cases': cases,
    }
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: report[k] for k in ('scope', 'counts', 'missing_contracts', 'exit_code')}))
    print(f'Report: {args.report}\nBuild log: {log}')
    return code


if __name__ == '__main__':
    sys.exit(main())
