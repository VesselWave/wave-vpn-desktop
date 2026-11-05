// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed nftables policy generator. No shell expressions or raw user rules.
use crate::{
    bootstrap::{validate_exemptions, Exemption},
    options::Options,
    TunnelProfile,
};
use anyhow::Result;
pub(crate) fn address(cidr: &str) -> String {
    format!(
        "{} daddr {cidr}",
        if cidr.contains(':') { "ip6" } else { "ip" }
    )
}
pub(crate) fn names<'a>(options: &'a Options, key: &str) -> impl Iterator<Item = &'a str> {
    options
        .get(key)
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
}
/// Embedded-definition endpoints: the client falls back to them after the
/// latest manifest's, so they remain the only exemptions that need no input.
pub fn embedded_exemptions() -> Vec<Exemption> {
    crate::Bootstrap::eddie().exemptions(&Default::default())
}
/// `bootstrap` holds the validated IP/port pairs of the latest manifest
/// bootstrap (`BootstrapStore::exemptions`). Anything unsafe or more than
/// `MAX_EXEMPTIONS` is refused: no hostname, range, DNS port or LAN address.
pub fn policy(
    options: &Options,
    profile: Option<&TunnelProfile>,
    locked: bool,
    bootstrap: &[Exemption],
) -> Result<String> {
    options.validate()?;
    validate_exemptions(bootstrap)?;
    if let Some(p) = profile {
        p.config()?;
    }
    let mut out=format!("table inet wave_vpn {{\n comment \"Managed by Wave VPN\";\n chain output {{\n type filter hook output priority -10; policy {};\noifname \"lo\" accept\n",if locked{"drop"}else{"accept"});
    if let Some(p) = profile {
        out.push_str(&format!(
            "{} udp dport {} accept\n",
            address(&p.endpoint.to_string()),
            p.port
        ));
    }
    // Only encrypted-bootstrap IP/port pairs can bypass the tunnel: the latest
    // validated manifest's and the embedded fallback's, TCP to that port only.
    let exemptions: std::collections::BTreeSet<Exemption> = embedded_exemptions()
        .into_iter()
        .chain(bootstrap.iter().copied())
        .collect();
    for e in exemptions {
        out.push_str(&format!(
            "{} tcp dport {} accept\n",
            address(&e.ip.to_string()),
            e.port
        ));
    }
    out.push_str("udp sport 68 udp dport 67 accept\nudp sport 546 udp dport 547 accept\nicmpv6 type { nd-router-solicit, nd-router-advert, nd-neighbor-solicit, nd-neighbor-advert } accept\n");
    let block6 = options.get("ipv6") == "block"
        || options.get("ipv6") == "in-block"
            && profile.is_some_and(|p| !p.addresses.iter().any(|a| a.contains(':')));
    if options.get("ipv4") == "block" {
        out.push_str("meta nfproto ipv4 drop\n");
    }
    if block6 {
        out.push_str("meta nfproto ipv6 drop\n");
    }
    out.push_str("oifname \"wavevpn0\" accept\n");
    if locked {
        // LAN/outside-route exceptions must not permit fallback DNS to escape.
        out.push_str("udp dport { 53, 853 } drop\ntcp dport { 53, 853 } drop\n");
    }
    let mut exceptions = names(options, "allowlist")
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if options.on("allow_lan") {
        exceptions.extend(
            [
                "10.0.0.0/8",
                "172.16.0.0/12",
                "192.168.0.0/16",
                "169.254.0.0/16",
                "fc00::/7",
                "fe80::/10",
            ]
            .map(str::to_owned),
        );
    }
    for route in options
        .get("routes")
        .split(';')
        .filter(|s| !s.trim().is_empty())
    {
        let (cidr, mode) = route.split_once(',').unwrap();
        if mode.trim() == "out" {
            exceptions.push(cidr.trim().into());
        }
    }
    for cidr in exceptions {
        out.push_str(&format!("{} accept\n", address(&cidr)));
    }
    if options.on("allow_ping") {
        out.push_str("icmp type { echo-request, echo-reply } accept\nicmpv6 type { echo-request, echo-reply } accept\n");
    }
    out.push_str(&format!(
        "}}\n chain input {{\n type filter hook input priority -10; policy {};\n",
        if locked && options.get("incoming") == "block" {
            "drop"
        } else {
            "accept"
        }
    ));
    out.push_str("iifname \"lo\" accept\nct state established,related accept\niifname \"wavevpn0\" accept\nudp sport 67 udp dport 68 accept\nudp sport 547 udp dport 546 accept\nicmpv6 type { nd-router-solicit, nd-router-advert, nd-neighbor-solicit, nd-neighbor-advert } accept\n");
    if options.on("allow_lan") {
        out.push_str("ip saddr { 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16 } accept\nip6 saddr { fc00::/7, fe80::/10 } accept\n");
    }
    for cidr in names(options, "allowlist") {
        out.push_str(&format!(
            "{} saddr {cidr} accept\n",
            if cidr.contains(':') { "ip6" } else { "ip" }
        ));
    }
    if options.on("allow_ping") {
        out.push_str("icmp type { echo-request, echo-reply } accept\nicmpv6 type { echo-request, echo-reply } accept\n");
    }
    out.push_str(&format!(
        "}}\n chain forward {{\n type filter hook forward priority -10; policy {};\n",
        if locked { "drop" } else { "accept" }
    ));
    if options.get("ipv4") == "block" {
        out.push_str("meta nfproto ipv4 drop\n");
    }
    if block6 {
        out.push_str("meta nfproto ipv6 drop\n");
    }
    for name in names(options, "forward_interfaces") {
        out.push_str(&format!("iifname \"{name}\" oifname \"wavevpn0\" accept\niifname \"wavevpn0\" oifname \"{name}\" ct state established,related accept\n"));
    }
    out.push_str(
        "}\n chain postrouting {\n type nat hook postrouting priority srcnat; policy accept;\n",
    );
    for name in names(options, "forward_interfaces") {
        out.push_str(&format!(
            "iifname \"{name}\" oifname \"wavevpn0\" masquerade\n"
        ));
    }
    out.push_str("}\n chain mark_in { type filter hook prerouting priority -150; policy accept; meta l4proto udp ct mark 51820 meta mark set ct mark; }\n chain mark_out { type filter hook postrouting priority -150; policy accept; meta l4proto udp meta mark 51820 ct mark set meta mark; }\n}\n");
    Ok(out)
}
