// SPDX-License-Identifier: GPL-3.0-or-later
//! Independently defined user options; values are validated before privileged use.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    General,
    Network,
    Wireguard,
    Lock,
    Advanced,
}
impl Section {
    pub const ALL: [Self; 2] = [Self::General, Self::Lock];
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Network => "Network",
            Self::Wireguard => "WireGuard",
            Self::Lock => "Network lock",
            Self::Advanced => "Advanced",
        }
    }
}
#[derive(Clone, Copy)]
pub enum Input {
    Toggle,
    Text,
    Number,
    Choice(&'static [&'static str]),
}
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub group: &'static str,
    pub section: Section,
    pub input: Input,
    pub default: &'static str,
}
macro_rules! f {
    ($k:literal,$l:literal,$h:literal,$g:literal,$s:ident,$i:expr,$d:literal) => {
        Field {
            key: $k,
            label: $l,
            hint: $h,
            group: $g,
            section: Section::$s,
            input: $i,
            default: $d,
        }
    };
}
pub const FIELDS: &[Field] = &[
    f!(
        "autoconnect",
        "Connect at startup",
        "Connect automatically after signing in",
        "Server preferences",
        General,
        Input::Toggle,
        "false"
    ),
    f!(
        "startlast",
        "Start with last server",
        "Reuse the last successful server",
        "Server preferences",
        General,
        Input::Toggle,
        "false"
    ),
    f!(
        "locklast",
        "Lock to server during session",
        "Reconnect to the same server",
        "Server preferences",
        General,
        Input::Toggle,
        "false"
    ),
    f!(
        "show_errors",
        "Show error messages",
        "Display errors above the current view",
        "Display",
        General,
        Input::Toggle,
        "true"
    ),
    f!(
        "units",
        "Speed and transfer unit",
        "",
        "Display",
        General,
        Input::Choice(&["bytes", "bits"]),
        "bytes"
    ),
    f!(
        "iec",
        "Binary units",
        "KiB/MiB instead of KB/MB",
        "Display",
        General,
        Input::Toggle,
        "false"
    ),
    f!(
        "theme",
        "Appearance",
        "Follow the desktop or choose a theme",
        "Display",
        General,
        Input::Choice(&["system", "dark", "light"]),
        "system"
    ),
    f!(
        "ipv6",
        "IPv6 mode",
        "in: tunnel; in-block: tunnel when available; block: no IPv6",
        "Network",
        Network,
        Input::Choice(&["in", "in-block", "block"]),
        "in"
    ),
    f!(
        "ipv4",
        "IPv4 mode",
        "",
        "Network",
        Network,
        Input::Choice(&["in", "block"]),
        "in"
    ),
    f!(
        "entry_interface",
        "Entry interface",
        "Empty = automatic route selection",
        "Network",
        Network,
        Input::Text,
        ""
    ),
    f!(
        "dns",
        "Custom DNS",
        "Comma-separated IP addresses; empty = AirVPN",
        "Network",
        Network,
        Input::Text,
        ""
    ),
    f!(
        "dns_mode",
        "DNS mode",
        "auto detects systemd-resolved",
        "DNS configuration",
        Network,
        Input::Choice(&["auto", "resolvconf", "systemd-resolved", "resolv.conf"]),
        "auto"
    ),
    f!(
        "dns_services",
        "DNS cache services",
        "Comma-separated service names to restart after DNS changes",
        "DNS configuration",
        Network,
        Input::Text,
        ""
    ),
    f!(
        "routes",
        "Routes",
        "CIDR,in or CIDR,out; separate routes with semicolons",
        "Custom routes",
        Network,
        Input::Text,
        ""
    ),
    f!(
        "countries_allow",
        "Allowed countries",
        "Empty = all; comma-separated country codes",
        "Country filters",
        Network,
        Input::Text,
        ""
    ),
    f!(
        "countries_deny",
        "Denied countries",
        "Empty = none",
        "Country filters",
        Network,
        Input::Text,
        ""
    ),
    f!(
        "mtu",
        "Interface MTU",
        "",
        "WireGuard",
        Wireguard,
        Input::Number,
        "1320"
    ),
    f!(
        "keepalive",
        "Persistent keepalive",
        "Seconds; 0 disables",
        "WireGuard",
        Wireguard,
        Input::Number,
        "15"
    ),
    f!(
        "handshake_first",
        "Handshake timeout, first",
        "Seconds",
        "WireGuard",
        Wireguard,
        Input::Number,
        "10"
    ),
    f!(
        "handshake_connected",
        "Handshake timeout, connected",
        "Seconds",
        "WireGuard",
        Wireguard,
        Input::Number,
        "200"
    ),
    f!(
        "netlock",
        "Network lock",
        "Block traffic outside the tunnel",
        "Connection",
        Lock,
        Input::Toggle,
        // Off until the user opts in: an enabled lock also blocks the internet
        // while disconnected, which surprised people on first use.
        "false"
    ),
    f!(
        "allow_lan",
        "Allow LAN traffic",
        "Allow local private networks outside the tunnel",
        "Connection",
        Lock,
        Input::Toggle,
        "false"
    ),
    f!(
        "reconnect",
        "Auto-reconnect",
        "Retry after handshake failure",
        "Connection",
        Lock,
        Input::Toggle,
        "true"
    ),
    f!(
        "verify",
        "Skip tunnel verification",
        "Connecting always waits for the first handshake",
        "Connection",
        Lock,
        Input::Toggle,
        "false"
    ),
    f!(
        "incoming",
        "Incoming policy",
        "",
        "Lock policy",
        Lock,
        Input::Choice(&["block", "allow"]),
        "block"
    ),
    f!(
        "allow_ping",
        "Allow ICMP ping",
        "Allow ping on physical interfaces",
        "Lock policy",
        Lock,
        Input::Toggle,
        "false"
    ),
    f!(
        "allowlist",
        "Allowlist IPs",
        "IP addresses or CIDRs; no routing change",
        "Allowlist",
        Lock,
        Input::Text,
        ""
    ),
    f!(
        "forward_interfaces",
        "Forward interfaces",
        "Comma-separated local interfaces routed through the tunnel",
        "Local forward",
        Lock,
        Input::Text,
        ""
    ),
    f!(
        "pinger_timeout",
        "Pinger timeout",
        "Seconds",
        "Tuning",
        Advanced,
        Input::Number,
        "3"
    ),
    f!(
        "refresh",
        "Manifest refresh interval",
        "Seconds",
        "Tuning",
        Advanced,
        Input::Number,
        "1800"
    ),
    f!(
        "penalty",
        "Penalty on error",
        "Seconds before retrying a failed server",
        "Tuning",
        Advanced,
        Input::Number,
        "30"
    ),
    f!(
        "http_timeout",
        "HTTP timeout",
        "Seconds",
        "Tuning",
        Advanced,
        Input::Number,
        "10"
    ),
    f!(
        "verify_retries",
        "Verification retries",
        "",
        "Tuning",
        Advanced,
        Input::Number,
        "3"
    ),
    f!(
        "capacity_factor",
        "Capacity factor",
        "0 = off; prefer higher-capacity servers",
        "Tuning",
        Advanced,
        Input::Number,
        "0"
    ),
    f!(
        "port",
        "Force port",
        "Empty = advertised default",
        "WireGuard mode",
        Advanced,
        Input::Text,
        ""
    ),
    f!(
        "log_file",
        "File logging",
        "Per-user diagnostic log; no credentials or keys",
        "Logging",
        Advanced,
        Input::Toggle,
        "false"
    ),
    f!(
        "log_path",
        "Log file path",
        "Empty = Wave config directory / wave.log",
        "Logging",
        Advanced,
        Input::Text,
        ""
    ),
    f!(
        "debug",
        "Debug logging",
        "Include diagnostic entries",
        "Logging",
        Advanced,
        Input::Toggle,
        "false"
    ),
    f!(
        "verify_routes",
        "Verify routes after connect",
        "Check that public traffic uses the tunnel",
        "Verification",
        Advanced,
        Input::Toggle,
        "true"
    ),
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct Options(pub BTreeMap<String, String>);
impl Default for Options {
    fn default() -> Self {
        Self(
            FIELDS
                .iter()
                .map(|f| (f.key.into(), f.default.into()))
                .collect(),
        )
    }
}
/// The deliberately small desktop settings surface. Other fields remain for
/// worker/CLI compatibility, but cannot silently carry old bypass settings forward.
pub fn desktop_field(key: &str) -> bool {
    matches!(
        key,
        "startlast" | "units" | "theme" | "reconnect" | "allow_lan"
    )
}
impl Options {
    pub fn desktop(&self) -> Self {
        let mut out = Self::default();
        out.set("netlock", self.get("netlock").into());
        for field in FIELDS.iter().filter(|f| desktop_field(f.key)) {
            out.set(field.key, self.get(field.key).into());
        }
        out
    }

    pub fn get(&self, key: &str) -> &str {
        self.0
            .get(key)
            .map(String::as_str)
            .or_else(|| FIELDS.iter().find(|f| f.key == key).map(|f| f.default))
            .unwrap_or("")
    }
    pub fn on(&self, key: &str) -> bool {
        self.get(key) == "true"
    }
    pub fn number(&self, key: &str) -> u64 {
        self.get(key).parse().unwrap_or(0)
    }
    pub fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.0.keys().all(|k| FIELDS.iter().any(|f| f.key == k)),
            "Unknown option"
        );
        for field in FIELDS {
            let value = self.get(field.key);
            ensure!(value.len() <= 4096, "{} is too long", field.label);
            ensure!(
                !value.contains(['\n', '\r', '\0']),
                "{} must be one line",
                field.label
            );
            match field.input {
                Input::Toggle => {
                    ensure!(matches!(value, "true" | "false"), "Invalid {}", field.label)
                }
                Input::Choice(choices) => {
                    ensure!(choices.contains(&value), "Invalid {}", field.label)
                }
                Input::Number => {
                    let n = value
                        .parse::<u64>()
                        .with_context(|| format!("Invalid {}", field.label))?;
                    let range = match field.key {
                        "mtu" => 576..=9000,
                        "keepalive" => 0..=65535,
                        "capacity_factor" => 0..=1000,
                        "refresh" => 300..=86400,
                        "penalty" => 0..=3600,
                        _ => 1..=3600,
                    };
                    ensure!(range.contains(&n), "{} is out of range", field.label);
                }
                Input::Text => (),
            }
        }
        if !self.get("port").is_empty() {
            ensure!(self.get("port").parse::<u16>()? > 0, "Invalid port");
        }
        for key in ["entry_interface", "forward_interfaces", "dns_services"] {
            for name in self
                .get(key)
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                ensure!(
                    name.len() <= 64
                        && !name.starts_with('-')
                        && name
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || "_.:@-".contains(c)),
                    "Invalid {key}"
                );
            }
        }
        for key in ["entry_interface", "forward_interfaces"] {
            ensure!(
                key != "entry_interface" || !self.get(key).contains(','),
                "Select one entry interface"
            );
            for name in self
                .get(key)
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                ensure!(
                    name.len() <= 15 && name != "wavevpn0" && name != "lo",
                    "Invalid network interface"
                );
            }
        }
        for address in self
            .get("dns")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let _: std::net::IpAddr = address.parse()?;
        }
        for address in self
            .get("allowlist")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            validate_cidr(address)?;
        }
        for route in self
            .get("routes")
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let (cidr, mode) = route
                .split_once(',')
                .context("Route format: CIDR,in or CIDR,out")?;
            validate_cidr(cidr.trim())?;
            ensure!(matches!(mode.trim(), "in" | "out"), "Invalid route mode");
        }
        for key in ["countries_allow", "countries_deny"] {
            ensure!(
                self.get(key)
                    .split(',')
                    .map(str::trim)
                    .all(|s| s.is_empty()
                        || s.len() == 2 && s.bytes().all(|b| b.is_ascii_alphabetic())),
                "Invalid country code"
            );
        }
        ensure!(
            self.get("ipv4") != "block" || self.get("ipv6") != "block",
            "At least one IP family must be tunneled"
        );
        Ok(())
    }
}
pub fn validate_cidr(text: &str) -> Result<()> {
    let (host, prefix) = text
        .split_once('/')
        .map(|(h, p)| (h, Some(p)))
        .unwrap_or((text, None));
    let ip: std::net::IpAddr = host.parse()?;
    if let Some(prefix) = prefix {
        ensure!(
            prefix.parse::<u8>()? <= if ip.is_ipv6() { 128 } else { 32 },
            "Invalid prefix"
        );
    }
    Ok(())
}
