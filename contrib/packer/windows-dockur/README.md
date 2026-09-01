# Windows golden images (dockur / Podman)

Machina builds Windows 10/11 golden `qcow2` disks via [dockur/windows](https://github.com/dockur/windows) inside **Podman** on the hypervisor.

## Prerequisites

- Linux host with **KVM** (`/dev/kvm`)
- **Podman** (Machina `install.sh` installs it on supported distros)
- `[libvirt] dockur_windows_allowed = true` in `/etc/machina/config.toml`
- Free disk: default **64G** qcow2 per build (override with `dockur_disk_size` in config.toml)

## API / UI

- `POST /api/v1/jobs/packer-golden-build` with `{"guest":"win11"}` or `{"guest":"win10"}`
- Create VM → **Golden Forge** → pick Windows profile → **Build golden qcow2**
- Artifact: `/var/lib/machina/packer-builds/{job-id}/work/output-win11/win11.qcow2`

## Script (manual)

```bash
cd /var/lib/machina/packer-builds/manual
sudo /usr/local/share/machina/packer/build-windows-dockur.sh win11 work
```

## Environment overrides

| Variable | Default |
|----------|---------|
| `MACHINA_DOCKUR_IMAGE` | `docker.io/dockurr/windows` |
| `MACHINA_DOCKUR_DISK_SIZE` | `64G` |
| `MACHINA_DOCKUR_RAM_SIZE` | `4G` |
| `MACHINA_DOCKUR_CPU_CORES` | `2` |
| `MACHINA_DOCKUR_USERNAME` | `Docker` |
| `MACHINA_DOCKUR_PASSWORD` | `admin` |
| `MACHINA_DOCKUR_MAX_WAIT_SECS` | `7200` |

## Security

Default credentials are **known** (`Docker` / `admin`). Rotate before exposing VMs to untrusted networks. For manual unattended Packer + VirtIO, see `../windows-qemu/HOWTO.txt`.

## libvirt clone hints

- Firmware: **UEFI**
- Disk bus: **virtio** (dockur installs VirtIO drivers on clean install)
- Graphics: SPICE or VNC per your console policy
