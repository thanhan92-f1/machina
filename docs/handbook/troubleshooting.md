# Machina — Troubleshooting

*Part of the [Machina Handbook](README.md) · see also
[Product Guide](product-guide.md) · [Admin & Configuration](admin-configuration.md) ·
[FAQ](faq.md)*

Start with the two built-in checks — they diagnose most issues:

```bash
./machinactl doctor    # host readiness: KVM, systemd, virsh, qemu-img, deps
./machinactl health    # service, API, libvirt, disk, backup timer (exit 0/1/2)
./machinactl status    # systemctl status machina-daemon
journalctl -u machina-daemon -f --no-hostname   # live daemon logs
```

---

## Install & build

### Symptom: build fails on `pam-sys` / bindgen / "libclang not found"
The PAM binding needs `libclang` at compile time.
```bash
# Install clang/llvm, then point bindgen at it:
export LIBCLANG_PATH="$(llvm-config --libdir)"
./machinactl build
```
`machinactl` tries to autodetect `LIBCLANG_PATH`; set it manually if that fails.

### Symptom: `cargo build` fails on macOS with missing libvirt headers
Expected — the workspace builds on **Linux only**. Build on a Linux host or use
`./scripts/deploy-remote.sh USER@HOST --remote-build`. The web UI alone builds on
macOS (`cd web && npm run build`).

### Symptom: `machinactl deps` can't find packages
It detects the distro from `/etc/os-release` and supports dnf (Fedora/RHEL/
Rocky/Alma) and apt (Debian/Ubuntu/Mint/Pop). On other distros, install libvirt,
qemu-kvm, virt-install, build tools, clang/llvm, Rust and Node manually, then
`./machinactl build`.

### Symptom: `install` fails with "target/release/machina-daemon not found"
`make install` requires a release build first. Run `./machinactl build` (or
`make release`) before `install`, or just use `./machinactl deploy`.

---

## Service won't start

### Symptom: `machina-daemon` fails immediately after start
```bash
systemctl status machina-daemon
journalctl -u machina-daemon -n 100 --no-pager
```
Common causes:
- **libvirt not running** — the unit `Requires=libvirtd`.
  `sudo systemctl enable --now libvirtd`.
- **Bad config** — a malformed `/etc/machina/config.toml` aborts startup
  (`toml::from_str` error in the log). Validate/restore from `contrib/machina.toml`.
- **TLS cert/key missing or unreadable** — if `[tls] enabled=true` but
  `cert_path`/`key_path` don't exist. Regenerate with `./machinactl tls`.

### Symptom: "Connected to libvirt" never appears / libvirt connect error
```bash
virsh -c qemu:///system uri        # confirm libvirt reachable
virsh -c qemu:///system list --all
systemctl status libvirtd
```
Ensure the daemon's user (root under systemd) can reach `[libvirt] uri`
(default `qemu:///system`).

### Symptom: port 5092 already in use (EADDRINUSE)
```bash
sudo ss -ltnp | grep 5092
```
Another instance or a stale process holds the port. Stop it
(`./machinactl stop`) or change `[daemon] port`.

---

## Can't reach the API / UI

### Symptom: `curl https://host:5092/api/v1/health` fails with a TLS error
The default cert is self-signed. Use `curl -sk` (as `machinactl` does), or
install a CA-signed cert into `/etc/machina/ssl/` (keep the paths). Verify
health:
```bash
curl -sk https://127.0.0.1:5092/api/v1/health | jq .
```

### Symptom: connection refused from another machine
The daemon binds `0.0.0.0:5092` but a host firewall may block it.
```bash
sudo ss -ltnp | grep 5092                    # is it listening?
# open the port (or deploy with --open-firewall)
sudo firewall-cmd --add-port=5092/tcp --permanent && sudo firewall-cmd --reload
```

### Symptom: web dev server (`:3000`) shows API/proxy errors
In dev, Vite proxies `/api` and `/ws` to `https://localhost:5092`. The daemon
must be running and reachable. Start it, or point the UI elsewhere with
`VITE_MACHINA_CONTROLLER_URL`.

---

## Authentication

### Symptom: login fails with correct Linux credentials
- Login uses the PAM service `[auth].pam_service` (default `sshd`). Confirm the
  account can authenticate: `ssh you@localhost`.
- Check `/etc/pam.d/<service>` exists and permits the account.
- Daemon log shows the active service at boot: `PAM service for web login:
  /etc/pam.d/sshd`.

### Symptom: root can't log in over the web
The `login` PAM service typically blocks root; use `sshd` (the default) or
another permissive service. See [FAQ Q11](faq.md#authentication--access).

### Symptom: everyone has admin rights unexpectedly
`/var/lib/machina/roles.json` is empty or missing — in that state **all users
default to Admin**. Populate it with explicit `username → role` mappings.

### Symptom: LDAP/OIDC login not working
- LDAP: verify `[auth.ldap]` (`url`, `base_dn`, `user_filter`, bind creds).
  Test via the UI (`POST /system/auth/ldap-test`) or Settings.
- OIDC: check `issuer_url`, `client_id`, `redirect_url` are all set — the daemon
  logs `OIDC marked enabled but missing issuer_url/client_id/redirect_url` and
  disables SSO if incomplete. Redirect must be
  `https://<host>/api/v1/auth/oidc/callback`.

### Symptom: WebSocket console closes immediately / 401
WS auth needs a fresh single-use token. Ensure the client calls
`POST /api/v1/ws-token` and passes `?token=` on `/ws/v1/...`.

---

## VMs

### Symptom: VM create fails
```bash
journalctl -u machina-daemon -f          # watch the create error
virsh -c qemu:///system pool-list --all  # is a storage pool active?
virsh -c qemu:///system net-list --all   # is the target network active?
ls -l /dev/kvm                            # KVM present?
```
- With `create_backend = virt_install`, `virt-install` must be installed
  (`machinactl doctor` checks this).
- Golden-image builds (`POST /jobs/virt-image-build`) need free disk — see
  `virt_image_build_min_free_*` in `[libvirt]`; adjust or free space.

### Symptom: no `/dev/kvm` / nested-virt errors
```bash
./machinactl doctor          # reports KVM availability
lsmod | grep kvm
egrep -c '(vmx|svm)' /proc/cpuinfo   # >0 means CPU virt is present
```
Enable virtualization in BIOS/firmware, or load `kvm_intel`/`kvm_amd`.

### Symptom: autostart networks failed at boot
The daemon logs `libvirt network '<name>' autostart failed at daemon boot`.
```bash
virsh -c qemu:///system net-list --all
virsh -c qemu:///system net-start <name>
virsh -c qemu:///system net-autostart <name>
```

### Symptom: live migration fails or stalls
Check connectivity to the destination and tune limits:
```bash
curl -sk https://host:5092/api/v1/vms/<name>/migrate/max-bandwidth
# raise downtime tolerance for busy VMs:
curl -sk -X POST https://host:5092/api/v1/vms/<name>/migrate/max-downtime -d '{"ms":500}'
```

---

## Storage & backups

### Symptom: `health` warns/criticals on backup disk usage
`machinactl health` warns at >80% and criticals at >95% for the backup dir.
Free space, expand the volume, or point `[backup].backup_dir` /
`[backup].nfs_target` off-box.

### Symptom: scheduled backups aren't running
```bash
./machinactl backup status
systemctl status machina-backup.timer
./machinactl backup enable        # (re)enable the daily timer
journalctl -u machina-backup -f   # backup logs
```

### Symptom: storage pool shows no volumes
```bash
virsh -c qemu:///system pool-refresh <pool>
# or via API:
curl -sk -X POST https://127.0.0.1:5092/api/v1/storage/pools/<pool>/refresh
```

---

## Observability & audit

### Symptom: Prometheus scrape empty / metrics history missing
- Scrape endpoint is `GET /api/v1/prometheus`.
- History requires `[metrics_history] enabled=true`; persisted to
  `/var/lib/machina/metrics-history.jsonl` when `persist=true`.

### Symptom: OTLP export not reaching the collector
Set `[observability.otlp] enabled=true` and a reachable `endpoint`
(e.g. `http://127.0.0.1:4318`); add `authorization` if required. See
[../guides/observability.md](../guides/observability.md).

### Symptom: audit log verification fails
```bash
./machinactl audit verify           # via API if daemon up, else local file
# audit log: /var/lib/machina/audit.log
```
Signed verification requires `[audit] sign_lines=true`.

---

## Platform (controller / agent)

### Symptom: platform pages (`/platform/*`) return errors
The daemon reverse-proxies `/api/v1/platform/controller` to the controller at
:5093. Ensure the controller is installed and running:
```bash
systemctl status machina-controller
./scripts/platformctl health
curl -s http://127.0.0.1:5093/... # controller REST
```
The controller must have been installed with `INSTALL_PLATFORM=1` /
`scripts/install-platform.sh`.

### Symptom: controller can't reach a host
```bash
systemctl status machina-agent       # per-host gRPC agent (:50051)
# controller → agent address is MACHINA_AGENT_ADDR (default http://127.0.0.1:50051)
```

---

## Native eBPF

Start with the status: it reports whether programs were compiled in, the
attached interfaces, the enforcement mode and what the kernel supports.

```bash
systemctl status machina-bpfd
journalctl -u machina-bpfd -n 100 --no-hostname
curl -sk -b /tmp/machina.jar https://127.0.0.1:5092/api/v1/bpf/status | jq '{programs_compiled, mode, features}'
```

### Symptom: `programs_compiled: false`
The build had no eBPF toolchain, so `build.rs` staged an empty object. Run
`make bpf-deps` (nightly + rust-src + prebuilt `bpf-linker`), rebuild and
reinstall `machina-bpfd`.

### Symptom: a feature is unavailable / `features` shows `false`
Map the flag to the kernel requirement:

| `features` field | Needs |
|------------------|-------|
| `btf` | `/sys/kernel/btf/vmlinux` (`CONFIG_DEBUG_INFO_BTF`) |
| `tcx` | kernel 6.6+ |
| `cgroup2` | unified cgroup v2 hierarchy |
| `fentry` | BTF + 5.5+ |
| `lsm_bpf` | `bpf` in `/sys/kernel/security/lsm` |
| `sched_ext` | 6.12+ with `CONFIG_SCHED_CLASS_EXT` |
| `xsk` | `CONFIG_XDP_SOCKETS` |

### Symptom: VMM guard reports `lsm_inactive` and refuses enforce
BPF-LSM is compiled in but not active. Append `bpf` to the `lsm=` boot
parameter (keep the existing list, e.g. `lsm=lockdown,capability,landlock,yama,apparmor,bpf`),
update the bootloader and reboot. Audit mode works without it.

### Symptom: `xsk` is `true` but bpfd can't open AF_XDP sockets
Expected. `machina-bpfd.service` sets
`RestrictAddressFamilies=AF_UNIX AF_NETLINK AF_INET AF_INET6`; bpfd never opens
XSK sockets itself, so the probe falls back to `xsk_map_ops` in
`/proc/kallsyms`. AF_XDP consumers run outside that unit, open their own
sockets and hand them to bpfd with `register_xsk`.

### Symptom: enforcement stopped on its own
The lease expired or bpfd restarted. Enforcement is never persisted and fails
open by design. Re-arm with a new lease (Native eBPF → Overview, or
`PUT /api/v1/bpf/mode`). Node isolation, the VMM guard and the scheduler carry
their own leases.

### Symptom: `enforcement_rejected` (HTTP 400) creating a policy
The policy match is invalid for its kind (bad CIDR, port, or `rate_limit`
spec). Nothing was stored; fix the match and retry.

### Symptom: node isolation refuses to enable
Allowlist TCP 22 or add an exempt CIDR. This guard prevents locking yourself
out of the host.

### Symptom: `machina-cni` stopped with status 78 ("another CNI is already configured")
machina-cni is opt-in and never replaces an existing CNI. The journal
(`journalctl -u machina-cni`) lists the foreign configs it found. Either keep
the existing CNI (disable the unit), reinstall the cluster without its bundled
CNI (k3s bootstrap with `cni: "machina"`), or set `MACHINA_CNI_TAKEOVER=1` in
`/etc/default/machina-cni` and restart to replace it deliberately.

### Symptom: CNI pods have no network after an upgrade
`machina-bpfd` rejects a `CniSync` whose ABI version differs. Upgrade
`machina-bpfd` and `machina-cni` together, then `systemctl restart machina-cni`.

More: [../ebpf/README.md](../ebpf/README.md).

---

## Quick reference: diagnostic commands

| Goal | Command |
|------|---------|
| Host readiness | `./machinactl doctor` |
| Deep health check | `./machinactl health` |
| Service state | `systemctl status machina-daemon` |
| Live logs | `journalctl -u machina-daemon -f --no-hostname` |
| API health | `curl -sk https://127.0.0.1:5092/api/v1/health \| jq .` |
| Post-install smoke test | `./machinactl verify` |
| Integrations status | `./machinactl integrations` |
| libvirt reachable? | `virsh -c qemu:///system list --all` |
| Port listening? | `sudo ss -ltnp \| grep 5092` |
| Regenerate TLS cert | `./machinactl tls` |
| Backup timer state | `./machinactl backup status` |
| eBPF datapath state | `curl -sk -b /tmp/machina.jar https://127.0.0.1:5092/api/v1/bpf/status \| jq .` |
| eBPF service logs | `journalctl -u machina-bpfd -f --no-hostname` |

---

*Back to the [Handbook index](README.md).*
