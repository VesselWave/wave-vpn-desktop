// SPDX-License-Identifier: GPL-3.0-or-later
// Wire format reference: AirVPN/Eddie Service.FetchUrl and AssocToUtf8Bytes.
// Eddie copyright (C) 2014-2026 AirVPN.
use crate::Bootstrap;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use anyhow::{anyhow, ensure, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::{rngs::OsRng, RngCore};
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};
use std::collections::BTreeMap;
use zeroize::Zeroize;

pub fn association(values: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut result = String::new();
    for (name, value) in values {
        result.push_str(&STANDARD.encode(name));
        result.push(':');
        result.push_str(&STANDARD.encode(value));
        result.push('\n');
    }
    result.into_bytes()
}

pub struct Envelope {
    key: [u8; 32],
    iv: [u8; 16],
    rsa: RsaPublicKey,
    used: bool,
}
impl Envelope {
    pub fn new(bootstrap: &Bootstrap) -> Result<Self> {
        let mut key = [0u8; 32];
        let mut iv = [0u8; 16];
        OsRng.fill_bytes(&mut key);
        OsRng.fill_bytes(&mut iv);
        let rsa = bootstrap.public_key()?;
        Ok(Self {
            key,
            iv,
            rsa,
            used: false,
        })
    }
    pub fn encode(
        &mut self,
        parameters: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>> {
        ensure!(!self.used, "Envelope already used");
        self.used = true;
        let mut session = association(&BTreeMap::from([
            ("key".into(), self.key.to_vec()),
            ("iv".into(), self.iv.to_vec()),
        ]));
        let s = self.rsa.encrypt(&mut OsRng, Pkcs1v15Encrypt, &session);
        session.zeroize();
        let mut data = association(
            &parameters
                .iter()
                .map(|(k, v)| (k.clone(), v.as_bytes().to_vec()))
                .collect(),
        );
        let d = cbc::Encryptor::<aes::Aes256>::new(&self.key.into(), &self.iv.into())
            .encrypt_padded_vec_mut::<Pkcs7>(&data);
        data.zeroize();
        Ok(BTreeMap::from([
            ("s".into(), STANDARD.encode(s?)),
            ("d".into(), STANDARD.encode(d)),
        ]))
    }
    pub fn decode(&self, response: &[u8]) -> Result<String> {
        ensure!(
            self.used && !response.is_empty() && response.len().is_multiple_of(16),
            "Invalid encrypted response"
        );
        let mut plain = cbc::Decryptor::<aes::Aes256>::new(&self.key.into(), &self.iv.into())
            .decrypt_padded_vec_mut::<Pkcs7>(response)
            .map_err(|_| anyhow!("Invalid encrypted response"))?;
        let result =
            String::from_utf8(plain.clone()).map_err(|_| anyhow!("Invalid response encoding"));
        plain.zeroize();
        result
    }
}
impl Drop for Envelope {
    fn drop(&mut self) {
        self.key.zeroize();
        self.iv.zeroize();
    }
}
