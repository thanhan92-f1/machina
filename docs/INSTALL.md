# Installing Machina

Three ways in, all from the same release assets. Pick by how much network the host has.

| You have | Use |
|----------|-----|
| a package repo / the `.deb` or `.rpm` files | **Packages** — `apt`/`dnf` handle dependencies and upgrades |
| an air-gapped or locked-down host | **Offline bundle** — one tarball, one script, no network |
| the source tree | `./machinactl deploy` (builds from source; see the README) |

Every release ships `SHA256SUMS` and a CycloneDX SBOM (`machina-<version>.sbom.cdx.json`). Verify before installing:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

## What gets installed

| Package | Contains | Put it on |
|---------|----------|-----------|
| `machina` | daemon, web UI, `machinactl`, backup timer | the host you browse to (and every host if you want per-host consoles) |
| `machina-controller` | fleet control plane (127.0.0.1:5093 behind the daemon's proxy) | one management host |
| `machina-agent` | agent + `machina-bpfd` (eBPF datapath) | every compute node |

A **single host** needs all three. A **fleet** is one controller plus an agent on each compute node.

## Single host, packages (≈ 5 minutes)

```bash
# Debian / Ubuntu
sudo apt install ./machina_*_amd64.deb ./machina-controller_*_amd64.deb ./machina-agent_*_amd64.deb
# RHEL / Rocky / Alma / Fedora
sudo dnf install ./machina-*.rpm
```

The first install generates `/etc/default/machina-platform` (controller JWT secret, controller↔agent token, bootstrap admin
password; mode 0600) and starts the services. Then:

```bash
sudo cat /etc/machina/INITIAL_ADMIN_PASSWORD     # sign in as admin; change it after first login
```

Open `https://<host>:5092`. The certificate is self-signed until you install your own (see `docs/handbook/admin-configuration.md`, TLS).
Set `MACHINA_NO_START=1` before installing to skip starting the services.

## Air-gapped / offline bundle

Copy `machina-<version>-linux-amd64.tar.gz` to the host (USB, internal mirror), then:

```bash
tar xzf machina-*-linux-amd64.tar.gz && cd machina-*-linux-amd64
sudo ./install.sh --all            # or --daemon / --controller / --agent
```

The script verifies the bundle, checks that libvirt and QEMU are present (it **never** installs OS packages and says exactly
what is missing), installs to `/usr/local`, creates first-run secrets, and starts the services. `--bind 0.0.0.0` makes the
web UI listen on all interfaces. `--uninstall [--purge]` removes it (data is kept unless you pass `--purge`).

## Adding a compute node to a fleet

On the controller create a one-time enrollment token (`POST /api/v1/enrollment/tokens`, admin only), then on the new host:

```bash
sudo apt install ./machina-agent_*_amd64.deb         # or: sudo ./install.sh --agent
# use the same controller↔agent token as the controller (/etc/default/machina-platform → MACHINA_AGENT_TOKEN):
echo 'MACHINA_AGENT_TOKEN=<value from the controller>' | sudo tee -a /etc/default/machina-platform >/dev/null && sudo chmod 600 /etc/default/machina-platform
sudo machina-agent join --controller https://<controller-host>:5093 --token <enrollment-token>
```

Tokens are single-use and can expire; the host appears as *pending validation* until the controller has reached its agent.

## Compatibility

The binaries are built on the oldest distribution the release workflow targets and link the C library and libvirt of that
system, so they run on that release **and newer**. Check `machina-daemon --help` on the target: if it fails to start with a
`GLIBC_` or `libvirt` error, use the assets built for your distribution. Needs x86_64, KVM (`/dev/kvm`), libvirt ≥ 8 and QEMU.

## Upgrading

Packages: `apt install ./new.deb` / `dnf upgrade ./new.rpm`. Configuration (`/etc/machina/config.toml`,
`/etc/default/machina-*`) and data (`/var/lib/machina`) are kept; services restart on upgrade. Offline bundle: run the new
`install.sh` again. **Back up first** (`machina-ha.sh drill` shows the controller-database replica path; see
[controller-ha.md](controller-ha.md)). Upgrade the controller before the agents.

## Uninstall

`apt remove machina machina-controller machina-agent` (or `dnf remove …`) keeps your data; `apt purge` / deleting
`/etc/machina`, `/etc/default/machina-*` and `/var/lib/machina` removes it.
