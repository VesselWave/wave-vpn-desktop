#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Run on the inspected Proxmox host; argument is an authorized public key file.
set -euo pipefail
test "$(id -u)" = 0
test "$#" = 1
test -s "$1"
if qm list | awk 'NR>1 {print $2}' | grep -qx wave-ubuntu-gnome; then
  echo 'wave-ubuntu-gnome already exists; inspect it instead of creating another.' >&2
  exit 1
fi
image=/tank/dir/template/import/wave-ubuntu-24.04-amd64.img
install -d /tank/dir/template/import /tank/dir/snippets
if ! echo "ffe6203da54deeb6db5d2a98a83f9ec8e55f149d3f7ba622e1abe5fa966ee3d6  $image" | sha256sum -c - >/dev/null 2>&1; then
  curl -fL --retry 3 -o "$image" \
    https://cloud-images.ubuntu.com/releases/noble/release-20260705/ubuntu-24.04-server-cloudimg-amd64.img
fi
echo "ffe6203da54deeb6db5d2a98a83f9ec8e55f149d3f7ba622e1abe5fa966ee3d6  $image" | sha256sum -c -
vmid=$(pvesh get /cluster/nextid)
install -d -m 700 /root/wave-lab-backups
python3 - "$1" "$vmid" <<'PY'
import json, os, pathlib, secrets, subprocess, sys
key = pathlib.Path(sys.argv[1]).read_text().strip()
if not key.startswith(('ssh-ed25519 ', 'ssh-rsa ', 'ecdsa-sha2-')) or '\n' in key:
    raise SystemExit('Expected one SSH public key')
vmid = int(sys.argv[2])
password = secrets.token_urlsafe(24)
hashed = subprocess.run(['openssl', 'passwd', '-6', '-stdin'], input=password+'\n', text=True, capture_output=True, check=True).stdout.strip()
secret = pathlib.Path(f'/root/wave-lab-backups/{vmid}-login-password')
with open(secret, 'x', opener=lambda p, f: os.open(p, f, 0o600)) as f:
    f.write(password+'\n')
setup = '''set -eu
test ! -e /var/lib/wave-lab-provisioned || exit 0
# Apply the desktop's NetworkManager renderer before fetching dotfiles.
netplan apply
timeout 60 bash -c 'until getent ahostsv4 github.com >/dev/null; do sleep 2; done'
install -d -m 755 /etc/gdm3
printf '[daemon]\\nAutomaticLoginEnable=true\\nAutomaticLogin=wave-lab\\n' > /etc/gdm3/custom.conf
runuser -u wave-lab -- bash -c 'set -eu; mkdir -p "$HOME/repos"; if [ ! -d "$HOME/repos/dotfiles/.git" ]; then git clone https://github.com/VesselWave/dotfiles.git "$HOME/repos/dotfiles"; fi; cd "$HOME/repos/dotfiles"; git checkout c4ba9e941e40347bc3d640e65bb29bdd415a2dbc; ./scripts/link fish tmux vim kitty'
systemctl enable qemu-guest-agent gdm3
systemctl set-default graphical.target
systemctl start graphical.target
touch /var/lib/wave-lab-provisioned
'''
cfg = dict(hostname='wave-ubuntu-gnome', manage_etc_hosts=True,
    users=[dict(name='wave-lab', groups=['adm','sudo'], shell='/bin/bash',
        sudo=['ALL=(ALL) NOPASSWD:ALL'], lock_passwd=False, passwd=hashed,
        ssh_authorized_keys=[key])], ssh_pwauth=False, package_update=True,
    packages=['ubuntu-desktop-minimal','qemu-guest-agent','spice-vdagent',
        'git','fish','tmux','vim','kitty','build-essential','pkg-config',
        'libssl-dev','libfontconfig1-dev','libxkbcommon-dev','libwayland-dev',
        'mesa-vulkan-drivers','libegl1-mesa-dev','libgl1-mesa-dri',
        'wireguard-tools','nftables','libsecret-tools','gnome-keyring'],
    runcmd=[['bash','-c',setup]])
snippet = pathlib.Path(f'/tank/dir/snippets/wave-ubuntu-{vmid}.yaml')
with open(snippet, 'x', opener=lambda p, f: os.open(p, f, 0o600)) as f:
    f.write('#cloud-config\n'+json.dumps(cfg, indent=2)+'\n')
PY
qm create "$vmid" --name wave-ubuntu-gnome --memory 3072 --cores 3 --cpu host \
  --machine q35 --bios ovmf --ostype l26 --agent enabled=1 \
  --scsihw virtio-scsi-single --net0 virtio,bridge=vmbr0 \
  --vga virtio-gl,memory=128 --tablet 1
qm set "$vmid" --efidisk0 tank:0,efitype=4m,pre-enrolled-keys=0
qm set "$vmid" --scsi0 "tank:0,import-from=$image,discard=on,ssd=1"
qm resize "$vmid" scsi0 40G
qm set "$vmid" --ide2 tank-dir:cloudinit \
  --cicustom "user=tank-dir:snippets/wave-ubuntu-$vmid.yaml" \
  --ipconfig0 ip=dhcp --boot order=scsi0
echo "Created VM $vmid; login password is private in /root/wave-lab-backups/$vmid-login-password"
qm start "$vmid"
