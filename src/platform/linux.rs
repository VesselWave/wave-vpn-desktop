// SPDX-License-Identifier: GPL-3.0-or-later
//! Independently implemented controller for Linux WireGuard tools and nftables.
use crate::{
    bootstrap::Exemption,
    firewall,
    options::Options,
    transport::{parse_stats, LockInput, PersistentLock as LockConfig, Request, Stats},
    TunnelProfile,
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::os::unix::process::CommandExt;
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
use zeroize::Zeroize;
const DIR: &str = "/run/wave-vpn";
const CONFIG: &str = "/run/wave-vpn/wavevpn0.conf";
const IFACE: &str = "wavevpn0";
const LOCK_CONFIG: &str = "/etc/wave-vpn/lock.json";
/// Exemptions of the lock applied most recently, reused when the lock is
/// re-applied without new input (cleanup, Recover).
const SESSION_BOOTSTRAP: &str = "/run/wave-vpn/bootstrap.json";
const UNIT: &str = "/etc/systemd/system/wave-vpn-lock.service";
const MARK: &str = "51820";
const RULE: &str = "18500";
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Recovery {
    dns: String,
    rules: Vec<(String, String, String)>,
    forward: Option<(String, String)>,
    endpoint_route: Option<(String, String)>,
    src_mark: Option<String>,
    families: Vec<String>,
    resolver_swapped: bool,
    resolver_content: Option<String>,
}
fn root() -> Result<()> {
    ensure!(
        cfg!(target_os = "linux"),
        "Networking is supported on Linux"
    );
    ensure!(
        nix::unistd::Uid::effective().is_root(),
        "Administrator privileges required"
    );
    Ok(())
}
fn private_dir(path: &str) -> Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    match std::fs::create_dir(path) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    let m = std::fs::symlink_metadata(path)?;
    ensure!(
        m.is_dir() && m.uid() == 0 && m.mode() & 0o022 == 0,
        "Unsafe directory {path}"
    );
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}
fn exclusive() -> Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    root()?;
    private_dir(DIR)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .read(true)
        .write(true)
        .open(format!("{DIR}/worker.lock"))?;
    fs2::FileExt::try_lock_exclusive(&file).context("Another networking operation is running")?;
    Ok(file)
}
fn atomic(path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
    let path = path.as_ref();
    let mut f = tempfile::NamedTempFile::new_in(path.parent().context("Missing parent")?)?;
    f.write_all(bytes)?;
    f.as_file().sync_all()?;
    f.persist(path)?;
    Ok(())
}
fn trusted(path: &Path) -> Result<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let p = std::fs::canonicalize(path)?;
    for a in p.ancestors() {
        let m = std::fs::metadata(a)?;
        ensure!(
            m.uid() == 0 && m.mode() & 0o022 == 0,
            "System executables must be root-owned and not group/other writable"
        );
    }
    Ok(p)
}
fn tool(name: &str) -> Result<PathBuf> {
    for dir in ["/usr/sbin", "/usr/bin", "/sbin", "/bin"] {
        let p = Path::new(dir).join(name);
        if p.is_file() {
            return trusted(&p);
        }
    }
    bail!("Missing system tool: {name}")
}
fn run(name: &str, args: &[&str], input: Option<&[u8]>) -> Result<String> {
    let r = crate::process::output(
        Command::new(tool(name)?)
            .arg0(name)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LC_ALL", "C")
            .args(args),
        input,
        Duration::from_secs(30),
    )?;
    ensure!(
        r.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&r.stderr).trim()
    );
    Ok(String::from_utf8_lossy(&r.stdout).into_owned())
}
fn read_private(path: &str) -> Result<Option<Vec<u8>>> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.uid() == 0 && meta.mode() & 0o077 == 0,
        "Unsafe private state file: {path}"
    );
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65536, "Private state file too large");
    Ok(Some(bytes))
}
fn lock_config() -> Result<Option<LockConfig>> {
    read_private(LOCK_CONFIG)?
        .map(|b| LockConfig::parse(&b))
        .transpose()
}
fn save_lock_config(config: &LockConfig) -> Result<()> {
    atomic(LOCK_CONFIG, &serde_json::to_vec(config)?)
}
/// Explicit exemptions win; otherwise those of the lock applied last in this
/// boot, then the persistent configuration's. Invalid stored entries are
/// dropped, which only blocks more.
fn exemptions(explicit: &[Exemption]) -> Vec<Exemption> {
    if !explicit.is_empty() {
        return explicit.to_vec();
    }
    if let Ok(Some(bytes)) = read_private(SESSION_BOOTSTRAP) {
        if let Ok(mut list) = serde_json::from_slice::<Vec<Exemption>>(&bytes) {
            list.retain(|e| e.validate().is_ok());
            list.truncate(crate::bootstrap::MAX_EXEMPTIONS);
            return list;
        }
    }
    lock_config()
        .ok()
        .flatten()
        .map(|c| c.bootstrap)
        .unwrap_or_default()
}
fn persistent() -> Result<bool> {
    Ok(lock_config()?.is_some_and(|c| c.enabled))
}
pub fn lock_status() -> Result<serde_json::Value> {
    Ok(
        serde_json::json!({"event":"lock","installed":Path::new(UNIT).is_file(),"enabled":persistent()?}),
    )
}
fn family(cidr: &str) -> &'static str {
    if cidr.contains(':') {
        "-6"
    } else {
        "-4"
    }
}
fn owned_firewall() -> Result<bool> {
    // A failed nft query is unknown, never proof that protection is off.
    let tables: serde_json::Value =
        serde_json::from_str(&run("nft", &["-j", "list", "tables"], None)?)?;
    let entries = tables["nftables"]
        .as_array()
        .context("Invalid firewall status")?;
    if !entries
        .iter()
        .any(|v| v["table"]["family"] == "inet" && v["table"]["name"] == "wave_vpn")
    {
        return Ok(false);
    }
    let table: serde_json::Value = serde_json::from_str(&run(
        "nft",
        &["-j", "list", "table", "inet", "wave_vpn"],
        None,
    )?)?;
    crate::protection::table_enabled(&table)?;
    Ok(true)
}
fn actual_protection() -> Result<bool> {
    if !owned_firewall()? {
        return Ok(false);
    }
    let table = serde_json::from_str(&run(
        "nft",
        &["-j", "list", "table", "inet", "wave_vpn"],
        None,
    )?)?;
    crate::protection::table_enabled(&table)
}
pub fn network_status() -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    root()?;
    let mut status = crate::protection::Status {
        enabled: actual_protection().ok(),
        persistent: persistent()?,
        ..Default::default()
    };
    status.recovery_needed = read_private(&format!("{DIR}/recovery.json"))?.is_some()
        || Path::new(CONFIG).exists()
        || run("ip", &["link", "show", "dev", IFACE], None).is_ok();
    if let Ok(lock) = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(format!("{DIR}/worker.lock"))
    {
        status.worker_active = fs2::FileExt::try_lock_exclusive(&lock).is_err();
    }
    println!(
        "{}",
        serde_json::json!({"event":"network_status", "status":status})
    );
    Ok(())
}
/// Replace the table in one nft transaction, so a rotated exemption set never
/// leaves a window without the lock.
fn set_firewall(
    o: &Options,
    p: Option<&TunnelProfile>,
    locked: bool,
    bootstrap: &[Exemption],
) -> Result<()> {
    let mut batch = String::new();
    if owned_firewall()? {
        batch.push_str("delete table inet wave_vpn\n");
    }
    batch.push_str(&firewall::policy(o, p, locked, bootstrap)?);
    run("nft", &["-f", "-"], Some(batch.as_bytes()))?;
    atomic(SESSION_BOOTSTRAP, &serde_json::to_vec(bootstrap)?)?;
    publish_protection(locked)?;
    Ok(())
}
fn remove_firewall() -> Result<()> {
    if owned_firewall()? {
        run("nft", &["delete", "table", "inet", "wave_vpn"], None)?;
    }
    publish_protection(false)?;
    Ok(())
}
fn publish_protection(enabled: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let path = "/run/wave-vpn-protection.json";
    atomic(
        path,
        &serde_json::to_vec(&serde_json::json!({"enabled":enabled}))?,
    )?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))?;
    Ok(())
}
fn save_recovery(s: &Recovery) -> Result<()> {
    atomic(format!("{DIR}/recovery.json"), &serde_json::to_vec(s)?)
}
fn cache_services(o: &Options) -> Result<()> {
    for name in firewall::names(o, "dns_services") {
        run("systemctl", &["try-restart", "--", name], None)?;
    }
    Ok(())
}
fn configure(r: &Request, touched: &mut bool) -> Result<()> {
    let o = &r.options;
    let p = &r.profile;
    o.validate()?;
    p.config()?;
    ensure!(
        !Path::new(CONFIG).exists(),
        "Use Recover to clean up the previous connection"
    );
    ensure!(
        run("ip", &["link", "show", "dev", IFACE], None).is_err(),
        "The Wave interface is already in use"
    );
    for f in ["-4", "-6"] {
        ensure!(
            run("ip", &[f, "route", "show", "table", MARK], None)
                .unwrap_or_default()
                .trim()
                .is_empty(),
            "Routing table {MARK} is already in use"
        );
        ensure!(
            !run("ip", &[f, "rule", "show"], None)?
                .lines()
                .any(|l| [RULE, "18520", "18521"]
                    .iter()
                    .any(|p| l.starts_with(&format!("{p}:")))),
            "Wave routing priority is already in use"
        );
    }
    for name in ["wg", "ip", "nft"] {
        tool(name)?;
    }
    if r.checks.is_some() && !o.on("verify") {
        // Missing verification tooling is setup failure, not an endless network retry.
        tool("getent")?;
    }
    let resolved = run("resolvectl", &["status"], None).is_ok();
    // Ubuntu's resolvconf may be a compatibility symlink to resolvectl. Its
    // presence alone does not provide a resolver when resolved is unavailable.
    let resolvconf = tool("resolvconf").ok();
    let compat = resolvconf
        .as_ref()
        .is_some_and(|p| p.file_name().is_some_and(|n| n == "resolvectl"));
    let dns = match o.get("dns_mode") {
        "systemd-resolved" => {
            ensure!(resolved, "systemd-resolved is not running");
            "resolved"
        }
        "resolvconf" => {
            ensure!(
                !compat || resolved,
                "The resolvconf compatibility command requires systemd-resolved"
            );
            "resolvconf"
        }
        "resolv.conf" => "file",
        _ => {
            if resolved {
                "resolved"
            } else if resolvconf.is_some() && !compat {
                "resolvconf"
            } else {
                "file"
            }
        }
    };
    if dns == "resolvconf" {
        tool("resolvconf")?;
    }
    let mut s = Recovery {
        dns: dns.into(),
        ..Default::default()
    };
    save_recovery(&s)?;
    *touched = true;
    let mut text = p
        .config_with(o)?
        .lines()
        .filter(|l| {
            !["DNS = ", "Address = ", "MTU = "]
                .iter()
                .any(|prefix| l.starts_with(prefix))
        })
        .collect::<Vec<_>>()
        .join("\n");
    text = text.replace("[Interface]", "[Interface]\nFwMark = 51820");
    atomic(CONFIG, text.as_bytes())?;
    text.zeroize();
    set_firewall(
        o,
        Some(p),
        o.on("netlock") || persistent()?,
        &exemptions(&r.bootstrap),
    )?;
    println!(
        "{}",
        serde_json::json!({"event":"protection","enabled":o.on("netlock")||persistent()?})
    );
    if !o.get("entry_interface").is_empty() {
        // A host route pins only the encrypted endpoint, leaving all other routing intact.
        let ep = p.endpoint.to_string();
        let f = family(&ep);
        let name = o.get("entry_interface");
        let existing = run("ip", &[f, "route", "show", "exact", &ep], None)?;
        ensure!(
            existing.trim().is_empty(),
            "An endpoint host route already exists; clear the entry-interface option to use it"
        );
        let raw = run("ip", &["-j", f, "route", "get", &ep, "oif", name], None)?;
        let routes: serde_json::Value = serde_json::from_str(&raw)?;
        let gateway = routes[0]["gateway"].as_str();
        s.endpoint_route = Some((ep.clone(), name.into()));
        save_recovery(&s)?;
        let mut args = vec![f, "route", "add", &ep];
        if let Some(g) = gateway {
            args.extend(["via", g]);
        }
        args.extend(["dev", name]);
        run("ip", &args, None)?;
    }
    run(
        "ip",
        &["link", "add", "dev", IFACE, "type", "wireguard"],
        None,
    )?;
    run("wg", &["setconf", IFACE, CONFIG], None)?;
    for address in &p.addresses {
        run(
            "ip",
            &[family(address), "address", "add", address, "dev", IFACE],
            None,
        )?;
    }
    run(
        "ip",
        &["link", "set", "dev", IFACE, "mtu", o.get("mtu"), "up"],
        None,
    )?;
    s.src_mark = Some(
        std::fs::read_to_string("/proc/sys/net/ipv4/conf/all/src_valid_mark")?
            .trim()
            .into(),
    );
    save_recovery(&s)?;
    std::fs::write("/proc/sys/net/ipv4/conf/all/src_valid_mark", "1")?;
    for (f, v6, default) in [("-4", false, "0.0.0.0/0"), ("-6", true, "::/0")] {
        if !p.addresses.iter().any(|a| a.contains(':') == v6) {
            continue;
        }
        s.families.push(f.into());
        save_recovery(&s)?;
        run(
            "ip",
            &[f, "route", "add", default, "dev", IFACE, "table", MARK],
            None,
        )?;
        run(
            "ip",
            &[
                f,
                "rule",
                "add",
                "priority",
                "18520",
                "lookup",
                "main",
                "suppress_prefixlength",
                "0",
            ],
            None,
        )?;
        run(
            "ip",
            &[
                f, "rule", "add", "priority", "18521", "not", "fwmark", MARK, "lookup", MARK,
            ],
            None,
        )?;
    }
    if dns == "resolved" {
        let mut args = vec!["dns", IFACE];
        args.extend(p.dns.iter().map(String::as_str));
        run("resolvectl", &args, None)?;
        run("resolvectl", &["domain", IFACE, "~."], None)?;
        run("resolvectl", &["default-route", IFACE, "yes"], None)?;
    } else if dns == "resolvconf" {
        let text = p
            .dns
            .iter()
            .map(|s| format!("nameserver {s}\n"))
            .collect::<String>();
        run(
            "resolvconf",
            &["-a", IFACE, "-m", "0", "-x"],
            Some(text.as_bytes()),
        )?;
    } else {
        trusted(Path::new("/etc"))?;
        let backup = "/etc/resolv.conf.wave-vpn-backup";
        ensure!(
            std::fs::symlink_metadata(backup).is_err(),
            "Resolver backup already exists; use Recover first"
        );
        let text = p
            .dns
            .iter()
            .map(|ip| format!("nameserver {ip}\n"))
            .collect::<String>();
        s.resolver_swapped = true;
        s.resolver_content = Some(text.clone());
        save_recovery(&s)?;
        std::fs::rename("/etc/resolv.conf", backup)?;
        atomic("/etc/resolv.conf", text.as_bytes())?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions("/etc/resolv.conf", std::fs::Permissions::from_mode(0o644))?;
    }
    for route in o
        .get("routes")
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let (cidr, mode) = route.split_once(',').unwrap();
        let cidr = cidr.trim();
        let table = if mode.trim() == "in" { MARK } else { "main" };
        s.rules
            .push((family(cidr).into(), cidr.into(), table.into()));
        save_recovery(&s)?;
        run(
            "ip",
            &[
                family(cidr),
                "rule",
                "add",
                "priority",
                RULE,
                "to",
                cidr,
                "lookup",
                table,
            ],
            None,
        )?;
    }
    if !o.get("forward_interfaces").is_empty() {
        let v4 = std::fs::read_to_string("/proc/sys/net/ipv4/ip_forward")?;
        let v6 = std::fs::read_to_string("/proc/sys/net/ipv6/conf/all/forwarding")?;
        s.forward = Some((v4.trim().into(), v6.trim().into()));
        save_recovery(&s)?;
        std::fs::write("/proc/sys/net/ipv4/ip_forward", "1")?;
        std::fs::write("/proc/sys/net/ipv6/conf/all/forwarding", "1")?;
    }
    cache_services(o)?;
    Ok(())
}
fn cleanup(o: &Options, retain: bool, bootstrap: &[Exemption]) -> Result<()> {
    let configured_lock = lock_config()?;
    let mut errors = vec![];
    if let Some(bytes) = read_private(&format!("{DIR}/recovery.json"))? {
        let s: Recovery = serde_json::from_slice(&bytes)?;
        if s.dns == "resolved" {
            let _ = run("resolvectl", &["revert", IFACE], None);
        } else if s.dns == "resolvconf" {
            if let Err(e) = run("resolvconf", &["-d", IFACE], None) {
                errors.push(e.to_string());
            }
        }
        if s.resolver_swapped
            && std::fs::symlink_metadata("/etc/resolv.conf.wave-vpn-backup").is_ok()
        {
            let unchanged = std::fs::symlink_metadata("/etc/resolv.conf")
                .map(|m| m.is_file())
                .unwrap_or(false)
                && std::fs::read_to_string("/etc/resolv.conf").ok() == s.resolver_content;
            // A crash between rename and replacement leaves no current file;
            // restoring the original is safe in that specific state.
            let missing = std::fs::symlink_metadata("/etc/resolv.conf")
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound);
            if unchanged || missing {
                if let Err(e) =
                    std::fs::rename("/etc/resolv.conf.wave-vpn-backup", "/etc/resolv.conf")
                {
                    errors.push(e.to_string());
                }
            } else {
                errors.push("Resolver changed outside Wave; original backup retained for manual conflict resolution".into());
            }
        }
        if let Some(mark) = s.src_mark {
            if let Err(e) = std::fs::write("/proc/sys/net/ipv4/conf/all/src_valid_mark", mark) {
                errors.push(e.to_string());
            }
        }
        for f in s.families {
            let _ = run(
                "ip",
                &[
                    &f,
                    "rule",
                    "delete",
                    "priority",
                    "18520",
                    "lookup",
                    "main",
                    "suppress_prefixlength",
                    "0",
                ],
                None,
            );
            let _ = run(
                "ip",
                &[
                    &f, "rule", "delete", "priority", "18521", "not", "fwmark", MARK, "lookup",
                    MARK,
                ],
                None,
            );
            let default = if f == "-6" { "::/0" } else { "0.0.0.0/0" };
            let _ = run(
                "ip",
                &[&f, "route", "delete", default, "dev", IFACE, "table", MARK],
                None,
            );
        }
        for (f, cidr, table) in s.rules {
            let _ = run(
                "ip",
                &[
                    &f, "rule", "delete", "priority", RULE, "to", &cidr, "lookup", &table,
                ],
                None,
            );
        }
        if let Some((v4, v6)) = s.forward {
            for (path, value) in [
                ("/proc/sys/net/ipv4/ip_forward", v4),
                ("/proc/sys/net/ipv6/conf/all/forwarding", v6),
            ] {
                if let Err(e) = std::fs::write(path, value) {
                    errors.push(e.to_string());
                }
            }
        }
        if let Some((ep, dev)) = s.endpoint_route {
            let _ = run(
                "ip",
                &[family(&ep), "route", "delete", &ep, "dev", &dev],
                None,
            );
        }
    }
    if Path::new(CONFIG).is_file() && run("ip", &["link", "show", "dev", IFACE], None).is_ok() {
        if let Err(e) = run("ip", &["link", "delete", "dev", IFACE], None) {
            errors.push(e.to_string());
        }
    }
    for f in ["-4", "-6"] {
        let rules = run("ip", &[f, "rule", "show"], None)?;
        if rules.lines().any(|line| {
            [RULE, "18520", "18521"]
                .iter()
                .any(|priority| line.starts_with(&format!("{priority}:")))
        }) {
            errors.push(format!("Wave routing rules remain for {f}"));
        }
        let routes = run("ip", &[f, "route", "show", "table", MARK], None).unwrap_or_default();
        if routes
            .lines()
            .any(|line| line.contains(&format!("dev {IFACE}")))
        {
            errors.push(format!("Wave routes remain for {f}"));
        }
    }
    // Retain the kill switch and recovery record if restoring networking failed.
    ensure!(
        errors.is_empty(),
        "Recovery incomplete: {}",
        errors.join("; ")
    );
    if let Some(c) = configured_lock.filter(|c| c.enabled) {
        let list = if bootstrap.is_empty() {
            c.bootstrap.clone()
        } else {
            bootstrap.to_vec()
        };
        set_firewall(&c.options, None, true, &list)?;
    } else if retain {
        set_firewall(o, None, true, &exemptions(bootstrap))?;
    } else {
        remove_firewall()?;
    }
    println!(
        "{}",
        serde_json::json!({"event":"protection","enabled":retain||persistent()?})
    );
    for p in [CONFIG, &format!("{DIR}/recovery.json")] {
        if Path::new(p).exists() {
            std::fs::remove_file(p)?;
        }
    }
    cache_services(o)?;
    Ok(())
}
fn stats() -> Result<Stats> {
    let mut raw = run("wg", &["show", IFACE, "dump"], None)?;
    let result = parse_stats(&raw);
    raw.zeroize();
    result
}
fn verify_routes(r: &Request) -> Result<()> {
    if !r.options.on("verify_routes") {
        return Ok(());
    }
    for (ip, v6) in [("1.1.1.1", false), ("2606:4700:4700::1111", true)] {
        if !r.profile.addresses.iter().any(|a| a.contains(':') == v6) {
            continue;
        }
        let route = run("ip", &[family(ip), "route", "get", ip], None)?;
        ensure!(
            route.contains(&format!("dev {IFACE}")),
            "Public route does not use the tunnel"
        );
    }
    Ok(())
}
fn publish_checks(results: &crate::checks::Results) {
    println!(
        "{}",
        serde_json::json!({"event":"checks", "results":results})
    );
}
/// Provider endpoints reached from a tunnel address, plus the system resolver.
struct LiveProbe<'a> {
    check: &'a crate::checks::ProviderCheck,
    timeout: Duration,
}
impl crate::checks::Probe for LiveProbe<'_> {
    fn tunnel(&mut self, local: std::net::IpAddr) -> Result<serde_json::Value> {
        let client = self.check.client(local, self.timeout)?;
        self.check.fetch(&client, "tun")
    }
    fn resolve(&mut self, host: &str) {
        // The system resolver must see the random challenge. An NXDOMAIN can
        // still be correct: the provider receipt, not getent's exit code, proves it.
        if let Ok(getent) = tool("getent") {
            let _ = crate::process::output(
                Command::new(getent)
                    .args(["ahosts", host])
                    .env_clear()
                    .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin"),
                None,
                self.timeout,
            );
        }
    }
    fn dns(&mut self, local: std::net::IpAddr) -> Result<serde_json::Value> {
        let client = self.check.client(local, self.timeout)?;
        self.check.fetch(&client, "dns")
    }
}
fn tunnel_addresses(r: &Request) -> Result<Vec<std::net::IpAddr>> {
    r.profile
        .addresses
        .iter()
        .map(|a| {
            a.split('/')
                .next()
                .context("Tunnel address missing")?
                .parse()
                .context("Invalid tunnel address")
        })
        .collect()
}
/// One verification pass. Runs only right after the first handshake.
/// `results` is shared by every pass of this connection, so an observed
/// failure stays visible (and fatal) whatever a later pass sees.
fn verify_tunnel(
    r: &Request,
    stop: &AtomicBool,
    results: &mut crate::checks::Results,
) -> Result<crate::checks::Verdict> {
    use crate::checks::Verdict;
    if results.failure().is_some() {
        return Ok(Verdict::Failed("Tunnel verification failed"));
    }
    if verify_routes(r).is_err() {
        return Ok(Verdict::Failed("Public route does not use the tunnel"));
    }
    if r.options.on("verify") {
        return Ok(Verdict::Skipped);
    }
    let timeout = Duration::from_secs(r.options.number("http_timeout").min(5));
    let locals = tunnel_addresses(r)?;
    let stopped = || stop.load(Ordering::Relaxed);
    if let Some(check) = &r.checks {
        let token = format!("{:032x}", rand::random::<u128>());
        let mut probe = LiveProbe { check, timeout };
        return Ok(crate::checks::verify_once(
            check,
            &locals,
            &mut probe,
            &token,
            results,
            &mut publish_checks,
            &stopped,
        ));
    }
    // Generic/local peers have no provider challenge; never label DNS verified.
    for ip in locals {
        if stopped() {
            return Ok(Verdict::Stopped);
        }
        let reached = reqwest::blocking::Client::builder()
            .no_proxy()
            .local_address(ip)
            .timeout(timeout)
            .build()?
            .get("https://airvpn.org/")
            .send()
            .and_then(|r| r.error_for_status());
        if reached.is_err() {
            return Ok(Verdict::Unavailable);
        }
    }
    publish_checks(results);
    Ok(Verdict::Verified)
}
/// Sleep in short steps so Disconnect is not delayed by a verification pause.
fn pause(duration: Duration, stop: &AtomicBool) {
    let end = Instant::now() + duration;
    while !stop.load(Ordering::Relaxed) && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(100).min(end - Instant::now()));
    }
}
fn monitor(r: &mut Request, stop: &AtomicBool, changes: &mpsc::Receiver<bool>) -> Result<()> {
    let start = Instant::now();
    let mut connected = false;
    let mut stalled = false;
    let mut schedule = crate::checks::Schedule::default();
    let mut last_wall = std::time::SystemTime::now();
    let mut last_tick = Instant::now();
    // Generate traffic even when keepalive is disabled, to initiate the handshake.
    for dns in &r.profile.dns {
        if let Ok(ip) = dns.parse::<std::net::IpAddr>() {
            let bind = if ip.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
            if let Ok(socket) = std::net::UdpSocket::bind(bind) {
                let _ = socket.send_to(&[0; 12], std::net::SocketAddr::new(ip, 53));
            }
        }
    }
    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        for enabled in changes.try_iter() {
            r.options.set("netlock", enabled.to_string());
            // Turning off an explicitly persistent lock must turn off its boot
            // policy too; ordinary session changes do not install a service.
            if Path::new(LOCK_CONFIG).exists() {
                save_lock_config(&LockConfig {
                    options: r.options.clone(),
                    enabled,
                    bootstrap: exemptions(&r.bootstrap),
                })?;
            }
            set_firewall(
                &r.options,
                Some(&r.profile),
                enabled,
                &exemptions(&r.bootstrap),
            )?;
            publish_status()?;
            println!(
                "{}",
                serde_json::json!({"event":"protection","enabled":enabled})
            );
        }
        let wall = std::time::SystemTime::now();
        if connected
            && crate::lifecycle::resumed(
                wall.duration_since(last_wall).unwrap_or_default(),
                last_tick.elapsed(),
            )
        {
            bail!("System resumed or networking paused; reconnect required");
        }
        let s = stats()?;
        match schedule.tick(start.elapsed(), s.handshake > 0) {
            crate::checks::Due::Verify => {
                use crate::checks::Verdict;
                let mut results = crate::checks::Results::default();
                publish_checks(&results);
                let verdict = crate::checks::verify_at_connect(
                    r.options.number("verify_retries"),
                    &mut || {
                        verify_tunnel(r, stop, &mut results).unwrap_or_else(|e| {
                            eprintln!("Tunnel verification: {e}");
                            Verdict::Failed("Tunnel verification failed")
                        })
                    },
                    &mut |d| pause(d, stop),
                    &|| stop.load(Ordering::Relaxed),
                );
                if let Verdict::Failed(reason) = verdict {
                    // Terminal even if Disconnect raced it: the session ends,
                    // cleanup runs and the failure is reported as not retryable.
                    debug_assert!(verdict.stops_connection());
                    return Err(crate::checks::VerificationFailed(reason).into());
                }
                if verdict == Verdict::Stopped {
                    return Ok(());
                }
                if verdict == Verdict::Unavailable {
                    // A check-service outage is not a leak. Disconnecting here
                    // would make every client reconnect at once.
                    eprintln!(
                        "Warning: AirVPN verification service unavailable; connection kept unverified"
                    );
                }
                if stop.load(Ordering::Relaxed) {
                    return Ok(());
                }
                connected = true;
                println!("{}", serde_json::json!({"event":"connected"}));
            }
            // Local routing table only; provider checks never repeat.
            crate::checks::Due::Routes => verify_routes(r)?,
            crate::checks::Due::Idle => (),
        }
        if !connected {
            ensure!(
                start.elapsed().as_secs() < r.options.number("handshake_first"),
                "First WireGuard handshake timed out"
            );
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let age = now.saturating_sub(s.handshake);
            let timeout = r.options.number("handshake_connected");
            let next_stalled = age >= (timeout * 4 / 5).max(1);
            if next_stalled != stalled {
                stalled = next_stalled;
                println!(
                    "{}",
                    serde_json::json!({"event":"state","state":if stalled {"stalled"} else {"connected"}})
                );
            }
            ensure!(age <= timeout, "WireGuard handshake expired");
        }
        println!(
            "{}",
            serde_json::json!({"event":"stats","rx":s.rx,"tx":s.tx})
        );
        std::io::stdout().flush()?;
        // Exclude our own bounded verification work from suspend detection.
        last_wall = std::time::SystemTime::now();
        last_tick = Instant::now();
        std::thread::sleep(Duration::from_secs(1));
    }
}
// Read the actual descriptor without buffered prefetch, so a following control
// command remains available. A client cannot hold the global mutex forever by
// sending a partial JSON document or leaving its options pipe open.
fn read_input(line: bool) -> Result<Vec<u8>> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let descriptor = unsafe { libc::dup(libc::STDIN_FILENO) };
    ensure!(descriptor >= 0, "Unable to read controller input");
    let mut file = unsafe { File::from_raw_fd(descriptor) };
    let start = Instant::now();
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        ensure!(
            start.elapsed() < Duration::from_secs(10),
            "Controller input timed out"
        );
        let mut poll = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut poll, 1, 100) };
        if ready < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e.into());
        }
        if ready == 0 {
            continue;
        }
        let count = file.read(&mut buffer[..if line { 1 } else { 4096 }])?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        ensure!(bytes.len() <= 65536, "Controller input too large");
        if line && buffer[0] == b'\n' {
            break;
        }
    }
    Ok(bytes)
}
pub fn worker() -> Result<()> {
    let _guard = exclusive()?;
    let mut bytes = read_input(true)?;
    let parsed = serde_json::from_slice::<Request>(&bytes);
    bytes.zeroize();
    let mut r = parsed?;
    r.options.validate()?;
    r.profile.apply_options(&r.options)?;
    if let Some(check) = &r.checks {
        check.validate()?;
    }
    crate::bootstrap::validate_exemptions(&r.bootstrap)?;
    // Keep the boot-persistent lock current with the latest bootstrap.
    if let Some(mut c) = lock_config()? {
        if !r.bootstrap.is_empty() && c.bootstrap != r.bootstrap {
            c.bootstrap = r.bootstrap.clone();
            save_lock_config(&c)?;
        }
    }
    // The global operation lock proves there is no live worker. Repair only
    // our journaled resources before a new attempt, keeping confirmed protection.
    if read_private(&format!("{DIR}/recovery.json"))?.is_some() {
        cleanup(
            &r.options,
            actual_protection()? || persistent()?,
            &r.bootstrap,
        )?;
    }
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(sig, stop.clone())?;
    }
    let eof = stop.clone();
    let (sender, changes) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let input = std::io::stdin();
        let mut input = input.lock();
        loop {
            let mut line = String::new();
            if input.by_ref().take(32).read_line(&mut line).is_err() {
                break;
            }
            let enabled = match line.as_str() {
                "lock-enable\n" => true,
                "lock-disable\n" => false,
                // EOF, disconnect, switch and unknown input stop the worker.
                // Protection follows the last explicitly confirmed lock choice.
                _ => break,
            };
            if sender.send(enabled).is_err() {
                break;
            }
        }
        eof.store(true, Ordering::Relaxed);
    });
    let mut touched = false;
    let configured = configure(&r, &mut touched);
    let retryable = configured.is_ok();
    let result = configured.and_then(|_| monitor(&mut r, &stop, &changes));
    if touched {
        if let Err(error) = cleanup(&r.options, r.options.on("netlock"), &r.bootstrap) {
            println!(
                "{}",
                serde_json::json!({"event":"failure", "retryable":false, "category":"recovery"})
            );
            return Err(error);
        }
    }
    if let Err(error) = &result {
        println!("{}", crate::transport::failure_event(retryable, error));
    }
    result
}
pub fn recover() -> Result<()> {
    let _guard = exclusive()?;
    let retain = actual_protection()?;
    cleanup(&Options::default(), retain, &[])?;
    println!(
        "Tunnel resources restored; Network Lock {}",
        if retain || persistent()? {
            "retained"
        } else {
            "off"
        }
    );
    Ok(())
}
pub fn lock_action(action: &str) -> Result<()> {
    let _guard = exclusive()?;
    if action == "lock-restore" {
        if let Some(c) = lock_config()?.filter(|c| c.enabled) {
            set_firewall(&c.options, None, true, &c.bootstrap)?;
        }
        publish_status()?;
        return Ok(());
    }
    let mut bytes = vec![];
    if !std::io::stdin().is_terminal() {
        bytes = read_input(false)?;
    }
    let LockInput {
        options: o,
        bootstrap,
    } = LockInput::parse(&bytes)?;
    match action {
        "lock-install" => {
            ensure!(
                Path::new("/run/systemd/system").is_dir(),
                "Boot-persistent lock installation requires systemd; ordinary connections do not"
            );
            ensure!(
                !Path::new(UNIT).exists(),
                "Persistent lock already installed"
            );
            private_dir("/etc/wave-vpn")?;
            std::fs::create_dir_all("/usr/local/libexec")?;
            trusted(Path::new("/usr/local/libexec"))?;
            atomic(
                "/usr/local/libexec/wave-vpn-service",
                &std::fs::read(std::env::current_exe()?)?,
            )?;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                "/usr/local/libexec/wave-vpn-service",
                std::fs::Permissions::from_mode(0o755),
            )?;
            save_lock_config(&LockConfig {
                options: o,
                enabled: false,
                bootstrap,
            })?;
            atomic(UNIT,b"[Unit]\nDescription=Wave VPN persistent network lock\nDefaultDependencies=no\nAfter=local-fs.target\nBefore=network-pre.target\nWants=network-pre.target\n[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart=/usr/local/libexec/wave-vpn-service lock-restore\n[Install]\nWantedBy=multi-user.target\n")?;
            run("systemctl", &["daemon-reload"], None)?;
            run("systemctl", &["enable", "wave-vpn-lock.service"], None)?;
        }
        "lock-enable" => {
            set_firewall(&o, None, true, &exemptions(&bootstrap))?;
            println!(
                "{}",
                serde_json::json!({"event":"protection","enabled":true})
            );
            if !Path::new(UNIT).exists() {
                publish_status()?;
                println!("{}", lock_status()?);
                return Ok(());
            }
            save_lock_config(&LockConfig {
                options: o,
                enabled: true,
                bootstrap: exemptions(&bootstrap),
            })?;
        }
        "lock-disable" => {
            if Path::new(LOCK_CONFIG).exists() {
                save_lock_config(&LockConfig {
                    options: o.clone(),
                    enabled: false,
                    bootstrap: exemptions(&bootstrap),
                })?;
            }
            cleanup(&o, false, &bootstrap)?;
        }
        "lock-update" => {
            // Follow a bootstrap rotation. Never turns a lock on or off: only
            // the exemptions of the active and boot-persistent locks change,
            // each replaced atomically. Unsafe input was refused above.
            if let Some(mut c) = lock_config()? {
                c.bootstrap = bootstrap.clone();
                save_lock_config(&c)?;
            }
            if let Some(c) = lock_config()?.filter(|c| c.enabled) {
                set_firewall(&c.options, None, true, &c.bootstrap)?;
            } else if actual_protection()? {
                set_firewall(&o, None, true, &bootstrap)?;
            }
            println!(
                "{}",
                serde_json::json!({"event":"protection","enabled":actual_protection()?})
            );
        }
        "lock-uninstall" => {
            remove_firewall()?;
            run(
                "systemctl",
                &["disable", "--now", "wave-vpn-lock.service"],
                None,
            )?;
            for p in [UNIT, LOCK_CONFIG, "/usr/local/libexec/wave-vpn-service"] {
                if Path::new(p).exists() {
                    std::fs::remove_file(p)?;
                }
            }
            run("systemctl", &["daemon-reload"], None)?;
        }
        _ => bail!("Unknown lock action"),
    }
    publish_status()?;
    println!("{}", lock_status()?);
    Ok(())
}
fn publish_status() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let p = "/run/wave-vpn-lock-status.json";
    atomic(p, &serde_json::to_vec(&lock_status()?)?)?;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o644))?;
    Ok(())
}
