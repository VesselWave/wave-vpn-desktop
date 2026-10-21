// SPDX-License-Identifier: GPL-3.0-or-later
// Schema reference: AirVPN/Eddie Service.cs; see docs/PROVENANCE.md.
use crate::{validate_document, Bootstrap};
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::IpAddr};
use zeroize::Zeroize;

#[derive(Clone, Debug)]
pub struct Server {
    pub name: String,
    pub country: String,
    pub location: String,
    pub entries: Vec<IpAddr>,
    pub bandwidth: u64,
    pub capacity: u64,
    pub users: u32,
    pub users_max: u32,
    pub score_base: f64,
    pub warning_open: String,
    pub warning_closed: String,
}
impl Server {
    pub fn available(&self) -> bool {
        self.capacity > 0 && self.warning_closed.is_empty()
    }
    // Eddie SupportTools.getTrafficLoad: byte/s -> Mbit/s, accounting for both directions.
    pub fn load(&self) -> f64 {
        if self.capacity == 0 {
            100.0
        } else {
            (200.0 * (self.bandwidth as f64 * 8.0 / 1_000_000.0).floor() / self.capacity as f64)
                .floor()
        }
    }
    pub fn endpoint(&self, index: usize, ipv6: bool) -> Result<IpAddr> {
        self.entries
            .iter()
            .filter(|ip| ip.is_ipv6() == ipv6)
            .nth(index)
            .copied()
            .context("Server has no matching entry address")
    }
}
#[derive(Clone)]
pub struct Mode {
    pub port: u16,
    pub entry_index: usize,
}
#[derive(Clone)]
pub struct Manifest {
    pub servers: Vec<Server>,
    pub modes: Vec<Mode>,
    pub bootstrap: Bootstrap,
    pub refresh_minutes: u32,
    pub factors: [f64; 3],
}
impl Manifest {
    pub fn score(&self, server: &Server) -> f64 {
        if !server.available() || !server.warning_open.is_empty() {
            return f64::INFINITY;
        }
        server.score_base / self.factors[0]
            + server.load() / self.factors[1]
            + 100.0 * server.users as f64 / server.users_max.max(1) as f64 / self.factors[2]
    }
    /// Desktop ranking: Eddie's speed terms with Wave's presentation policy.
    /// Users have little weight; congestion above 60% receives a quadratic cost.
    /// Missing ping contributes zero until a measurement arrives.
    pub fn desktop_score(
        &self,
        server: &Server,
        ping: Option<u32>,
        capacity_factor: u64,
    ) -> Option<u32> {
        if !server.available() || !server.warning_open.is_empty() {
            return None;
        }
        let load = server.load();
        let users = if server.users_max == 0 {
            100.
        } else {
            (100. * server.users as f64 / server.users_max as f64).floor()
        };
        let score = server.score_base / self.factors[0]
            + load * self.factors[1]
            + users * self.factors[2] / 1000.
            + f64::from(ping.unwrap_or(0))
            + (load - 60.).max(0.).powi(2)
            - capacity_factor as f64 * (server.capacity as f64 / 1000.);
        Some(score.max(0.) as u32)
    }
    pub fn best(&self) -> Option<&Server> {
        self.servers
            .iter()
            .filter(|s| s.available())
            .min_by(|a, b| self.score(a).total_cmp(&self.score(b)))
    }
}
pub fn parse_manifest(text: &str) -> Result<Manifest> {
    validate_document(text, "manifest")?;
    let doc = roxmltree::Document::parse(text)?;
    let root = doc.root_element();
    let groups: HashMap<_, _> = root
        .descendants()
        .filter(|n| n.has_tag_name("servers_group"))
        .map(|n| (n.attribute("group").unwrap_or(""), n))
        .collect();
    let mut servers = Vec::new();
    for n in root.descendants().filter(|n| n.has_tag_name("server")) {
        let g = groups.get(n.attribute("group").unwrap_or(""));
        let attr = |name| {
            n.attribute(name)
                .or_else(|| g.and_then(|g| g.attribute(name)))
                .unwrap_or("")
        };
        servers.push(Server {
            name: attr("name").into(),
            country: attr("country_code").to_lowercase(),
            location: attr("location").into(),
            entries: attr("ips_entry")
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().parse())
                .collect::<Result<_, _>>()?,
            bandwidth: attr("bw").parse().unwrap_or(0),
            capacity: attr("bw_max").parse().unwrap_or(0),
            users: attr("users").parse().unwrap_or(0),
            users_max: attr("users_max").parse().unwrap_or(100),
            score_base: attr("scorebase").parse().unwrap_or(0.0),
            warning_open: attr("warning_open").into(),
            warning_closed: attr("warning_closed").into(),
        });
    }
    let mut modes = Vec::new();
    for n in root
        .descendants()
        .filter(|n| n.has_tag_name("mode") && n.attribute("type") == Some("wireguard"))
    {
        let port = n.attribute("port").unwrap_or("").parse()?;
        let entry_index = n.attribute("entry_index").unwrap_or("").parse()?;
        ensure!(port != 0 && entry_index < 16, "Invalid WireGuard mode");
        modes.push(Mode { port, entry_index });
    }
    let urls = root
        .descendants()
        .filter(|n| n.has_tag_name("url"))
        .filter_map(|n| n.attribute("address"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let bootstrap = Bootstrap {
        urls,
        modulus: root.attribute("auth_rsa_modulus").unwrap_or("").into(),
        exponent: root.attribute("auth_rsa_exponent").unwrap_or("").into(),
    };
    ensure!(
        !bootstrap.urls.is_empty()
            && !bootstrap.modulus.is_empty()
            && !bootstrap.exponent.is_empty(),
        "Missing bootstrap data"
    );
    let factors = ["speed_factor", "load_factor", "users_factor"].map(|name| {
        root.attribute(name)
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(1.0)
    });
    Ok(Manifest {
        servers,
        modes,
        bootstrap,
        factors,
        refresh_minutes: root
            .attribute("next_update")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(30)
            .clamp(1, 1440),
    })
}

#[derive(Clone)]
pub struct Device {
    pub name: String,
    private: String,
    preshared: String,
    addresses: Vec<String>,
    dns: Vec<String>,
}
impl Drop for Device {
    fn drop(&mut self) {
        self.private.zeroize();
        self.preshared.zeroize();
    }
}
#[derive(Clone)]
pub struct Account {
    pub login: String,
    pub expiration: String,
    public_key: String,
    pub devices: Vec<Device>,
}
pub fn parse_account(text: &str, login: &str) -> Result<Account> {
    validate_document(text, "user")?;
    let doc = roxmltree::Document::parse(text)?;
    let root = doc.root_element();
    ensure!(
        root.attribute("login") == Some(login),
        "AirVPN account identity mismatch"
    );
    let devices: Vec<_> = root
        .descendants()
        .filter(|n| n.has_tag_name("key"))
        .map(|n| {
            let a = |name| n.attribute(name).unwrap_or("").to_owned();
            Device {
                name: a("name"),
                private: a("wg_private_key"),
                preshared: a("wg_preshared"),
                addresses: [a("wg_ipv4"), a("wg_ipv6")]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect(),
                dns: [a("wg_dns_ipv4"), a("wg_dns_ipv6")]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect(),
            }
        })
        .collect();
    ensure!(!devices.is_empty(), "Account contains no devices");
    Ok(Account {
        login: login.into(),
        expiration: root.attribute("expirationdate").unwrap_or("").into(),
        public_key: root.attribute("wg_public_key").unwrap_or("").into(),
        devices,
    })
}

/// Strict structured input for the privileged worker. No raw scripts or profile directives.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TunnelProfile {
    pub private_key: String,
    pub preshared_key: String,
    pub public_key: String,
    pub addresses: Vec<String>,
    pub dns: Vec<String>,
    pub endpoint: IpAddr,
    pub port: u16,
}
impl Drop for TunnelProfile {
    fn drop(&mut self) {
        self.private_key.zeroize();
        self.preshared_key.zeroize();
    }
}
impl Account {
    pub fn profile(
        &self,
        server: &Server,
        device: &str,
        mode: &Mode,
        ipv6: bool,
    ) -> Result<TunnelProfile> {
        ensure!(server.available(), "Server unavailable");
        let d = if device.is_empty() {
            self.devices.first()
        } else {
            self.devices.iter().find(|d| d.name == device)
        }
        .context("Select an available device")?;
        let p = TunnelProfile {
            private_key: d.private.clone(),
            preshared_key: d.preshared.clone(),
            public_key: self.public_key.clone(),
            addresses: d.addresses.clone(),
            dns: d.dns.clone(),
            endpoint: server.endpoint(mode.entry_index, ipv6)?,
            port: mode.port,
        };
        p.config()?;
        Ok(p)
    }
}
impl TunnelProfile {
    pub fn apply_options(&mut self, options: &crate::options::Options) -> Result<()> {
        options.validate()?;
        self.addresses.retain(|a| {
            if a.contains(':') {
                options.get("ipv6") != "block"
            } else {
                options.get("ipv4") != "block"
            }
        });
        if !options.get("dns").is_empty() {
            self.dns = options
                .get("dns")
                .split(',')
                .map(|s| s.trim().to_owned())
                .collect();
        }
        self.dns.retain(|a| {
            if a.contains(':') {
                options.get("ipv6") != "block"
            } else {
                options.get("ipv4") != "block"
            }
        });
        ensure!(
            !self.addresses.is_empty() && !self.dns.is_empty(),
            "Selected IP modes leave no tunnel address or DNS server"
        );
        ensure!(
            options.get("ipv6") != "in" || self.addresses.iter().any(|a| a.contains(':')),
            "IPv6 mode requires an IPv6 tunnel address; use in-block for an IPv4-only device"
        );
        Ok(())
    }
    pub fn config_with(&self, options: &crate::options::Options) -> Result<String> {
        options.validate()?;
        let mut config = self
            .config()?
            .replace("MTU = 1320", &format!("MTU = {}", options.number("mtu")))
            .replace(
                "PersistentKeepalive = 15",
                &format!("PersistentKeepalive = {}", options.number("keepalive")),
            );
        let allowed = match (
            options.get("ipv4") == "block",
            options.get("ipv6") == "block" || !self.addresses.iter().any(|a| a.contains(':')),
        ) {
            (true, false) => "::/0",
            (false, true) => "0.0.0.0/0",
            _ => "0.0.0.0/0, ::/0",
        };
        config = config.replace(
            "AllowedIPs = 0.0.0.0/0, ::/0",
            &format!("AllowedIPs = {allowed}"),
        );
        Ok(config)
    }
    pub fn config(&self) -> Result<String> {
        for key in [&self.private_key, &self.preshared_key, &self.public_key] {
            ensure!(STANDARD.decode(key)?.len() == 32, "Invalid WireGuard key");
        }
        ensure!(
            !self.addresses.is_empty() && !self.dns.is_empty() && self.port != 0,
            "Incomplete WireGuard profile"
        );
        for address in &self.addresses {
            let (ip, prefix) = address.split_once('/').context("CIDR address required")?;
            let ip: IpAddr = ip.parse()?;
            ensure!(
                prefix.parse::<u8>()? <= if ip.is_ipv6() { 128 } else { 32 },
                "Invalid address prefix"
            );
        }
        for dns in &self.dns {
            let _: IpAddr = dns.parse()?;
        }
        let endpoint = match self.endpoint {
            IpAddr::V4(ip) => ip.to_string(),
            IpAddr::V6(ip) => format!("[{ip}]"),
        };
        Ok(format!("[Interface]\nPrivateKey = {}\nAddress = {}\nDNS = {}\nMTU = 1320\n\n[Peer]\nPublicKey = {}\nPresharedKey = {}\nEndpoint = {}:{}\nAllowedIPs = 0.0.0.0/0, ::/0\nPersistentKeepalive = 15\n",
            self.private_key, self.addresses.join(", "), self.dns.join(", "), self.public_key,
            self.preshared_key, endpoint, self.port))
    }
}
