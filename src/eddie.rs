// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only Eddie profile import. See docs/PROVENANCE.md for the pinned schema.
use crate::{options::Options, MAX_DOCUMENT};
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use anyhow::{ensure, Context, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

const PAYLOAD: &[u8] = b"4af85e84255b077ad890dba297e811b7d016add1";
const EMPTY_PASSWORD: &str = "e6552ddf3ac5c8755a82870d91273a63eab0da1e";

pub struct Imported {
    pub login: String,
    pub password: String,
    pub options: Options,
    pub ignored: usize,
}
impl Drop for Imported {
    fn drop(&mut self) {
        self.login.zeroize();
        self.password.zeroize();
    }
}
/// Supports raw XML plus Eddie v1/v2 n (default password) and p (user password).
/// For s profiles the caller must resolve the keyring password first.
pub fn decode(bytes: &[u8], password: &str) -> Result<Imported> {
    ensure!(bytes.len() <= MAX_DOCUMENT, "Profile is too large");
    let plain = Zeroizing::new(
        if bytes.starts_with(b"<") || bytes.starts_with(b"\xef\xbb\xbf") {
            bytes.to_vec()
        } else {
            ensure!(bytes.len() >= 3, "Truncated Eddie profile");
            let header = &bytes[..3];
            ensure!(
                matches!(header, b"v1n" | b"v1p" | b"v1s" | b"v2n" | b"v2p" | b"v2s"),
                "Unsupported Eddie profile format"
            );
            if header[2] == b's' {
                ensure!(
                    !password.is_empty(),
                    "Unlock the system keyring to import this Eddie profile"
                );
            }
            let offset = if header[1] == b'2' { 67 } else { 3 };
            let data = bytes.get(offset..).context("Truncated Eddie profile")?;
            let password = if header[2] == b'n' {
                EMPTY_PASSWORD
            } else {
                password
            };
            ensure!(!password.is_empty(), "Enter the Eddie profile password");
            decrypt(data, password)?
        },
    );
    let text = std::str::from_utf8(&plain).context("Profile is not UTF-8 XML")?;
    let doc = roxmltree::Document::parse(text).context("Malformed Eddie profile XML")?;
    let root = doc.root_element();
    ensure!(root.has_tag_name("eddie"), "Expected an Eddie profile");
    let node = root
        .children()
        .find(|n| n.has_tag_name("options"))
        .context("Missing Eddie options")?;
    let mut out = Imported {
        login: String::new(),
        password: String::new(),
        options: Options::default(),
        ignored: 0,
    };
    let mut seen = std::collections::BTreeSet::new();
    for n in node.children().filter(|n| n.is_element()) {
        ensure!(
            n.has_tag_name("option"),
            "Unexpected profile option element"
        );
        let name = n.attribute("name").context("Missing option name")?;
        let value = n.attribute("value").context("Missing option value")?;
        ensure!(seen.insert(name), "Duplicate profile option");
        ensure!(value.len() <= 4096, "Profile option too large");
        match name {
            "login" => out.login = value.into(),
            "password" => out.password = value.into(),
            _ => {
                if let Some(key) = mapped(name) {
                    out.options.set(key, value.into());
                } else {
                    out.ignored += 1;
                }
            }
        }
    }
    ensure!(
        !out.login.contains(['\r', '\n', '\0']) && !out.password.contains('\0'),
        "Invalid account credentials"
    );
    out.options.validate()?;
    Ok(out)
}
/// Bounded, read-only import with the same Secret Service attribute used by Eddie.
pub fn read(path: &std::path::Path, password: &str) -> Result<Imported> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "Choose a regular Eddie profile file"
    );
    let mut bytes = Zeroizing::new(Vec::new());
    file.take((MAX_DOCUMENT + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= MAX_DOCUMENT, "Profile is too large");
    let key = if bytes.starts_with(b"v2s") {
        let id = bytes.get(3..67).context("Truncated Eddie profile")?;
        let id = std::str::from_utf8(id).context("Invalid Eddie profile identifier")?;
        Some(crate::credentials::eddie_password(id)?)
    } else if bytes.starts_with(b"v1s") {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid Eddie profile filename")?;
        Some(crate::credentials::eddie_password(name)?)
    } else {
        None
    };
    decode(
        &bytes,
        key.as_deref().map(String::as_str).unwrap_or(password),
    )
}
fn mapped(name: &str) -> Option<&'static str> {
    match name {
        "servers.startlast" => Some("startlast"),
        _ => None,
    }
}

fn decrypt(data: &[u8], password: &str) -> Result<Vec<u8>> {
    ensure!(data.starts_with(PAYLOAD), "Invalid Eddie encrypted payload");
    let salt_start = PAYLOAD.len() + usize::from(data.get(PAYLOAD.len()) == Some(&1));
    ensure!(
        data.len() >= salt_start + 16 + 16 + 16 + 32,
        "Truncated encrypted profile"
    );
    let mut crypt = Zeroizing::new([0u8; 32]);
    let mut auth = Zeroizing::new([0u8; 32]);
    let a = &data[salt_start..salt_start + 8];
    let b = &data[salt_start + 8..salt_start + 16];
    if salt_start > PAYLOAD.len() {
        pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), a, 10000, crypt.as_mut());
        pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), b, 10000, auth.as_mut());
    } else {
        pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password.as_bytes(), a, 10000, crypt.as_mut());
        pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password.as_bytes(), b, 10000, auth.as_mut());
    }
    let end = data.len() - 32;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(auth.as_ref())?;
    mac.update(&data[..end]);
    mac.verify_slice(&data[end..])
        .map_err(|_| anyhow::anyhow!("Wrong profile password or damaged profile"))?;
    let iv = salt_start + 16;
    cbc::Decryptor::<aes::Aes256>::new_from_slices(crypt.as_ref(), &data[iv..iv + 16])?
        .decrypt_padded_vec_mut::<Pkcs7>(&data[iv + 16..end])
        .map_err(|_| anyhow::anyhow!("Invalid encrypted profile padding"))
}
