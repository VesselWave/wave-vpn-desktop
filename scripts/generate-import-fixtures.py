#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
from pathlib import Path
import hashlib,hmac,subprocess
out=Path(__file__).resolve().parents[1] / 'protocol-data/fixtures/eddie-import'
out.mkdir(parents=True, exist_ok=True)
xml=b'<eddie><options><option name="login" value="synthetic-user"/><option name="password" value="synthetic-pass"/><option name="wireguard.interface.mtu" value="1380"/><option name="unmapped" value="ignored"/></options></eddie>'
for version in (1,2):
 for variant in ('n','p'):
  for sha in ('sha1','sha256'):
   password=b'profile-test-password' if variant=='p' else b'e6552ddf3ac5c8755a82870d91273a63eab0da1e'
   salt1=bytes(range(16,24));salt2=bytes(range(32,40));iv=bytes(range(16))
   crypt=hashlib.pbkdf2_hmac(sha,password,salt1,10000,32)
   auth=hashlib.pbkdf2_hmac(sha,password,salt2,10000,32)
   enc=subprocess.run(['openssl','enc','-aes-256-cbc','-K',crypt.hex(),'-iv',iv.hex()],input=xml,capture_output=True,check=True).stdout
   payload=b'4af85e84255b077ad890dba297e811b7d016add1'+(b'\1' if sha=='sha256' else b'')+salt1+salt2+iv+enc
   data=f'v{version}{variant}'.encode()+(b'0'*64 if version==2 else b'')+payload+hmac.digest(auth,payload,'sha256')
   (out/f'v{version}{variant}-{sha}.bin').write_bytes(data)
