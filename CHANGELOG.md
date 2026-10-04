# Changelog

## 2026-10-04 — VM network policy and packet flows

Which VM may talk to which, in both directions, using the CiliumNetworkPolicy
schema. Enforced natively by `machina-bpfd` on each VM tap, so Cilium is not
needed. See [docs/ebpf/vm-network-policy.md](docs/ebpf/vm-network-policy.md).

- **Policies.**
  - Accepts `CiliumNetworkPolicy`, `CiliumClusterwideNetworkPolicy` and
    `VmNetworkPolicy` YAML.
  - L3/L4 support: endpoint selectors with expressions, ingress and egress,
    `ingressDeny`/`egressDeny` (deny wins), `from/toRequires`, CIDR sets with
    `except`, entities, port ranges, named ports, ICMP types and
    `enableDefaultDeny`.
  - Validation errors carry Cilium-style paths.
  - `toFQDNs` (`matchName` / `matchPattern` with Cilium wildcards) is
    enforced natively. The VM edge snoops DNS replies on the tap. bpfd gives
    each learned address its own identity, which inherits the rules of its
    CIDR or of `world`, so denies still win, and adds the FQDN allows. The
    learned names are listed by `machinactl netpol fqdn`, the UI
    *DNS names* table and `GET …/fqdn-cache`. The policy tester accepts DNS
    names.
  - L7 rules are enforced natively, without Envoy:
    - covers HTTP (method, path, host, headers), Kafka (role or apiKey,
      version, clientID, topic), TLS `serverNames` and DNS `matchName` /
      `matchPattern`, including L7 on `toFQDNs` rules;
    - the VM edge holds client segments past the allowed window; bpfd parses
      the stream (HTTP headers across segments, `Content-Length` and chunked
      bodies, keep-alive, whole Kafka requests, TLS SNI, DNS over UDP and
      TCP), then reinjects the allowed frames at once through a private veth
      (no retransmission wait), or answers with HTTP 403, a TCP reset or DNS
      REFUSED;
    - windows are per tap, so both ends of a VM-to-VM connection on one host
      are checked;
    - `toFQDNs` also learns from DNS answers over TCP;
    - flows carry the L7 request.
  - `toGroups` / `fromGroups` and `cidrGroupRef` resolve `CiliumCIDRGroup`
    objects.
  - `authentication` (`required`, `test-always-fail`): new connections need
    a bpfd-authenticated VM identity pair. A source guard drops VMs that
    send as another VM's address. `machinactl netpol auth` and
    `GET …/vm-network-policies/auth` list the authenticated pairs.
  - `machinactl netpol test` and the UI tester take an L7 request.
  - `toServices` is accepted with a warning and matches nothing.
- **Labels.** Each VM has key/value labels, which policies select on. The
  daemon stores them in `vm-labels.json`. On the controller, migration 029
  adds a `vms.labels` column, seeded from `key=value` tags.
- **Single host or fleet.**
  - The daemon compiles policies for its own host and resyncs on lifecycle
    events and every 60 s.
  - The controller stores policies (migration 029) and pushes a per-host
    compiled state to every host's bpfd.
  - An owner field keeps the two writers apart.
- **Datapath.**
  - Identity-keyed rules with deny precedence and port ranges.
  - ICMP type rules.
  - CIDR identities through an LPM map.
  - ICMP echo conntrack keyed by identifier.
  - A ring buffer of FORWARDED, DROPPED and AUDIT flow events with rule
    attribution.
  - Observe by default; drops only under the enforcement lease.
- **Tools.**
  - `machinactl netpol` subcommands: `apply`, `get`, `delete`, `validate`,
    `test` (policy trace), `selectors`, `endpoints` and `status`.
  - `machinactl flow observe|top|stats`, with colours on a TTY.
  - `machinactl vm label`.
  - The same commands in `scripts/platformctl` against the controller.
- **UI.**
  - A **VM Network Policies** page: policies, YAML editor with dry-run
    preview and templates, policy tester, endpoints and selectors.
  - A black macOS-style **Flows** terminal.
  - A labels and policy panel on the VM's Network tab.

## 2026-10-04 — machina-cni is opt-in

- **Cluster bootstrap keeps the default CNI.** `POST /api/v1/k8s/cluster-bootstrap`
  now installs k3s with its bundled flannel, NetworkPolicy controller and
  kube-proxy and waits for nodes Ready. Pass `"cni": "machina"` (or tick
  **Use machina-cni** on the Kubernetes page) to get the previous behaviour:
  k3s without those three, plus `machina-bpfd` and `machina-cni`.
- **Takeover guard.** `machina-cni agent` exits with status 78 without
  touching the node when another CNI config is present in the CNI conf dirs;
  `contrib/machina-cni.service` has `RestartPreventExitStatus=78`. The
  bootstrap `cni` phase runs the same check. `MACHINA_CNI_TAKEOVER=1` overrides.

## 2026-10-03 — Native eBPF datapath (waves 1 and 2)

`machina-bpfd` replaces the Cilium, Tetragon, Netra and PacketWolf
integrations with Machina's own pure-Rust (Aya) programs. See
[docs/ebpf/](docs/ebpf/README.md).

- **Wave 1** — Policies with an observe default and a datapath-checked enforce
  lease (`MACHINA_BPF_ENFORCE_LEASE_SECS`, never persisted); flows, DNS, L7,
  per-VM accounting, captures, QoS and connection rate limits; `machina-cni`
  (dual-stack routing, NetworkPolicy, opt-in CiliumNetworkPolicy, Maglev
  services, XDP NodePort); VM edge and QEMU cgroup sandbox; XDP DDoS shield;
  TCP health and ICMP error histograms; opt-in TLS / JA4 visibility; node
  isolation with its own short lease. Daemon `/api/v1/bpf/*`, controller fleet
  views under `/api/v1/zeus-security/*` through agent `BpfCall`.
- **Wave 2** — Network-change audit (rtnetlink), sampled L7, VM runtime
  intelligence, VMM guard (BPF-LSM), direct tap redirect, QUIC-LB, AF_XDP and
  the `machina-scx` sched_ext VM scheduler. The Native eBPF page now has 22
  tabs.
- **Kernel features** — `GET /api/v1/bpf/status` reports `features` (`btf`,
  `tcx`, `cgroup2`, `fentry`, `lsm_bpf`, `sched_ext`, `xsk`) detected natively
  (no `bpftool` needed); the Overview tab shows them as pills. The `xsk` probe
  falls back to `/proc/kallsyms` because the unit's `RestrictAddressFamilies`
  blocks AF_XDP.
- **Guest policy relay** — `GET/PUT /api/v1/vms/{name}/guest-policy` and
  `/guest-lsm` relay per-container network and BPF-LSM rules to GuestKit's
  `guestkitd` through QGA guest-exec (allowlisted to four methods). New VM
  detail **Guest policy** tab.

## 2026-10-03 — Security fixes and regression TLS

- **CodeQL** — Verified TLS in the regression API helper, a URL scheme guard
  for controller-derived links (`safeHref`, http/https only), markdown
  escaping.
- **Dependabot** — `jsonwebtoken` 10, `async-nats` 0.50, `pam-client`, npm
  lockfile bumps.
- **Regression TLS** — `scripts/regression` verifies certificates by default.
  Set `MACHINA_CA_FILE` to trust a self-signed host CA, or
  `MACHINA_INSECURE_TLS=1` to skip verification on a lab host.

## 2026-10-03 — Docs refresh

- New `docs/ebpf/` reference, `docs/controller-ha.md`,
  `docs/daemon-peer-fleet.md` and `docs/handbook/runbook.md`; old fleet and
  runbook docs are stubs pointing to them. Snapshots moved to `docs/archive/`.
- One port table and full controller / agent / bpfd environment tables in the
  admin guide; Native eBPF troubleshooting.
- Website: networking category, intro, controller HA, upgrade and backup,
  troubleshooting, reference (ports, environment, API) and local search.
- Customer docs: Native eBPF, Guest policy and ten rewritten page guides; new
  routes (Alert Rules, Scheduled Jobs, Machine Finder, Native eBPF); feature
  guide PDF built by `build-customer-pdfs.mjs`.
- `scripts/check-doc-links.py` and a customer-routes freshness check run in CI.

## Earlier platform batches (moved from docs/platform.md)

- **Batch 12** — VM `lifecycle_phase`, structured API errors with
  remediation, host join validation, reconcile loop, live migrate after snap
  clone.
- **Batch 13** — Task drawer, command palette platform search,
  dashboard → platform link, structured error banners.
- **Batch 14** — Agent storage / network provisioning
  (`storage.pool.provision`, `network.provision`).
- **Batch 15** — Policy rules, project quotas, support bundle, upgrade
  manager, task-failure alerts.
- **Batch 16** — `scripts/platformctl`, Terraform stub under
  `terraform/machina/`, chaos / soak scripts.

## 2026-09-03 — Regression harness + host-sync 404 + LB DNAT -p

- **Stale host UUID** — Many `ops-*.js` / `ui-*.js` scripts defaulted to a
  retired lab host id, causing host-sync FK **500**s and catalog/mission 404s.
  Added `lib/ids.js` (`resolveIds`) and env-based `cfg.hostId` /
  `cfg.platformVmId` with live discovery after login.
- **`POST …/hosts/{id}/sync`** — Returns **404** when the host is missing
  instead of enqueueing a task that fails SQLite FK 787.
- **Native LB member push** — Backend DNAT rules now include `-p <protocol>`
  so nftables iptables accepts `--to-destination ip:port`.
- Soft-skip absent Zeus/Launchpad/KubeVirt routes; deepen `fleetcloud`
  (projects/stacks/templates lists + SG-only stack CRUD).

## 2026-09-03 — Fleet Cloud regression + port-forward delete

- **`npm run fleetcloud` / `make regression-fleetcloud`** — Native Fleet Cloud
  CRUD sweep (flavors, keypairs, security groups + rules, load balancers,
  port-forwards) via the daemon → controller proxy. Documented in
  `scripts/regression/RESULTS.md` (**25/25** on `212.8.248.187`).
- **Port-forward delete** — `core`/`machina-agent`: create used
  `-m comment --comment machina:…`, but delete issued a comment-less
  `iptables -D`, so the API returned `ok:true` while DNAT stayed. Deletes
  now use line numbers and fail if the rule remains. Agent rebuilt on lab.

## 2026-09-03 — Hotplug: PCI slots, honest disk detach, CD-ROM test

Live regression on `212.8.248.187` exposed three gaps after the TUI drop; all
retested green (`feature-test` **26/26**, `npm run lifecycle` **11/11**).

- **q35 NIC hotplug** — New domains get eight spare `<controller type='pci'
  model='pcie-root-port'/>` entries at create time. Existing VMs that hit
  “No more available PCI slots” on `POST …/nic/attach` now hot-add one root
  port and retry. Exhaustion maps to HTTP **409** `pci_slots_exhausted`
  instead of a bare 500.
- **Disk / NIC detach** — Live unplug wait extended to 8s; responses include
  `requires_restart` when `live_removed` is false (config already updated,
  guest hasn’t released the device). Regression lifecycle polls, then
  stop/start to apply config-only detach before asserting live XML.
- **feature-test CD-ROM** — Asserts the auto-picked target was not already
  occupied. Virtio-root Linux guests correctly receive free SATA `sda`; the
  old hardcoded `!= sda` check only fit Windows SATA-root VMs.

## 2026-08-15 — Firecracker: a third sprite backend

Sprites (`POST /v1/sprites`) can now boot on **Firecracker** as well as
libvirt/QEMU and Cloud Hypervisor — select via `backend: "firecracker"`.
Same disposable/TTL-reaped/destroy-only model as the other two backends
(`core/src/firecracker/`, mirrors `core/src/cloud_hypervisor/` closely
enough to diff side-by-side), with two real differences:

- **API-driven, not CLI-flag-driven.** `firecracker` starts serving only
  its control API over a Unix socket; boot config (vcpus/memory, kernel,
  drive, vsock, network interface) is a sequence of `PUT` calls, and the
  machine only actually boots on `PUT /actions {"action_type":"InstanceStart"}`.
  Each call shells `curl --unix-socket`, matching this project's existing
  "shell the CLI, don't link an HTTP client" convention.
- **Raw disk, no partition table.** Firecracker's drive backend is
  raw-only, and — a real bug found and fixed via a live boot, not
  assumed — it auto-appends `root=/dev/vda rw` (unpartitioned) for
  whichever drive has `is_root_device: true`, *after* whatever `boot_args`
  the caller supplies, so a caller-set `root=/dev/vda1` silently loses
  (kernel takes the last `root=` on the line; documented upstream as
  firecracker-microvm/firecracker#2709). Every golden image is a
  GPT-partitioned qcow2, so `materialize_raw_disk` now extracts partition
  1's content into an unpartitioned raw file (parsing `sfdisk -d`, `dd`-ing
  just that byte range) instead of handing Firecracker a whole partitioned
  disk — confirmed live: booting the full converted disk kernel-panicked
  with "Unable to mount root fs on /dev/vda"; booting the extracted
  partition mounts cleanly and boots straight through to a DHCP lease.
  Runs on every boot (no pre-extracted sibling file required), at a real
  cost — ~97s for the ~8.6 GB `debian-egress-test` golden image on this
  lab host, the slowest boot of the three backends.
- Firecracker itself needs no built-in BIOS/bootloader/qcow2 support the
  way the other two backends' quirks did — it boots a host-supplied kernel
  (`vmlinux`) directly. `install.sh`'s `ensure_firecracker` fetches the
  `firecracker`/`jailer` release tarball and a prebuilt `vmlinux` from
  Firecracker's own CI kernel bucket (the same source its getting-started
  guide uses) — optional, warn-and-continue on any fetch failure, same
  posture as `ensure_cloud_hypervisor`.
- `core/src/sprite_net.rs` (new) — promoted the TAP/bridge helpers,
  `SPRITE_RUN_DIR`, and VMM-binary discovery out of `cloud_hypervisor` into
  a shared module both backends now use, ahead of a third backend needing
  the same thing a third time.
- Jailer sandboxing (Firecracker's own chroot/cgroup/seccomp isolation) is
  deliberately out of scope for this pass — `firecracker` runs as a direct,
  unsandboxed daemon child, same posture Cloud Hypervisor already has.
  Documented as a future hardening item, not silently dropped.

Verified live end-to-end on a real host: real `firecracker`/`vmlinux`
install via `ensure_firecracker`, a real sprite boot against the fully
network-hardened `debian-egress-test` golden image (DHCP lease, SSH login,
`networkctl status`, `curl https://github.com` → `HTTP 200`), clean
teardown (process/TAP/run-dir all gone), automatic TTL reaping, and three
concurrent sprites — one per backend — drawing distinct vsock CIDs (3, 4,
5) from the same shared, backend-agnostic allocator with no collision.

## 2026-08-15 — Sprite fleet visibility, and golden-image networking hardening

**New: read-only sprite fleet visibility**, without reversing sprites'
deliberate exclusion from the controller's SQLite `vms` table/reconciler
(see `daemon/src/sprite_registry.rs`'s doc comment — sprites stay
TTL-reaped, disposable, and out of the reconcile-latency path).

- `machina-agent` gained a `ListSprites` gRPC method: it pulls its
  co-located daemon's `GET /api/v1/sprites` over loopback and relays the
  result, authenticated with a short-lived, read-only ("viewer") platform
  JWT it mints itself (`agent/src/jwt.rs`). The daemon now accepts platform
  JWTs from either `machina-controller` or `machina-agent` as issuer
  (`daemon/src/auth.rs`) — both rely on the same `MACHINA_JWT_SECRET`
  operators must already provision consistently for the existing KubeVirt
  inventory sync to work.
- The controller's existing per-host `host.inventory` task now also pulls
  sprite inventory (best-effort — an agent that predates `ListSprites`, or
  whose co-located daemon is down, doesn't fail the whole inventory tick)
  into a new in-memory-only cache (`controller/src/engine/sprite_inventory.rs`)
  — never written to `pool`/`vms`, so a controller restart just starts the
  cache empty again until the next tick repopulates it.
- `GET /api/v1/sprites` on the controller — fleet-wide sprite listing
  (optionally `?host_id=`), backed entirely by that cache, so it never
  blocks on a slow/unreachable host.

**Golden-image networking, baked in instead of patched live:** the
`debian-egress-test` golden image only had its DNS fix and `curl` applied
to a *running* sprite's overlay disk, not the base image — every future
sprite booted from it would still lack both. Rebuilt directly into the
base image this time, and expanded well past `curl`: a netshoot-equivalent
network-debugging toolkit (`ping`, `dig`, `traceroute`, `mtr`, `nc`,
`tcpdump`, `nmap`, `socat`, `telnet`) — because a disposable sandbox VM
with only `curl` isn't much of a debugging environment.

- **`guestkit rescue -o install-packages`** (new, upstreamed to the
  `guestkit` project): bind-mounts `/proc`,`/sys`,`/dev` into the mounted
  guest root (reusing `grub_repair`'s chroot machinery) and runs
  `apt-get`/`dnf`/`apk`/`pacman` inside via chroot — `--network`
  temporarily swaps the guest's `/etc/resolv.conf` for the host's so the
  package manager can resolve real repositories, restoring the original
  file afterward regardless of outcome. `virt-customize`'s network backend
  (`passt`) is broken on this lab host, which is exactly the class of
  problem this avoids — no libguestfs appliance network stack involved.
  Verified live: installed `jq` into the real golden image, confirmed it
  runs on next boot.

## 2026-08-15 — Sprites: network egress, machinactl, and Cloud Hypervisor fixes found live

Follow-on to the Cloud Hypervisor sprite backend below — everything here
came out of actually running the feature end-to-end rather than unit tests
alone.

**Fixed, from real boot failures:**
- Cloud Hypervisor has no built-in BIOS (unlike QEMU) — booting a disk
  without `--firmware`/`--kernel` failed immediately. `install.sh` now
  fetches `CLOUDHV.fd` from `cloud-hypervisor/edk2` releases alongside the
  binaries; `core::cloud_hypervisor` resolves it the same way it resolves
  the VMM binary itself.
- Cloud Hypervisor's qcow2 backend rejects backing-file overlays outright
  (`MaxNestingDepthExceeded`), even one level deep — the `cloudhypervisor`
  backend now materializes a full `cp --reflink=auto --sparse=always` copy
  instead (near-instant on reflink-capable filesystems, a plain copy
  otherwise — both cheaper than the alternative `qemu-img convert -c`,
  which compresses every cluster).
- Two independent vsock CID allocators (the daemon's own counter for Cloud
  Hypervisor, libvirt's kernel-side `<cid auto='yes'/>`) both started at
  CID 3 and collided the first time each backend's first sprite booted
  around the same time. `SpriteRegistry` now tracks CIDs from both
  backends in one shared set.
- stderr was previously discarded (`Stdio::null()`) — now piped to
  `tracing::warn!` continuously, which is what made the two boot-failure
  bugs above slow to diagnose in the first place.

**New:**
- `network_egress` (opt-in, off by default) — attaches a sprite to the
  host's existing libvirt "default" NAT network instead of staying
  vsock-only. Libvirt sprites get a `<interface type='network'>`; Cloud
  Hypervisor sprites get a TAP device created and bridged by the daemon.
  Verified live via a real DHCP lease, not just the TAP/bridge plumbing —
  see below.
- Sprites web UI (`/sprites`) — create modal (golden image picker, backend
  toggle, TTL, network egress checkbox), live list with expiry countdown,
  delete.
- `machinactl sprite <list|get|create|delete|golden-images>` — CLI parity
  with the API/UI, `MACHINA_API_TOKEN` for auth.
- `GET /v1/sprites/golden-images` — lists available golden images, backing
  the picker above.

**Verification gap closed:** the `network_egress` feature's own unit tests
use a blank synthetic disk (proves the TAP/bridge/NAT plumbing works, not
that a guest can actually get an address). Building a real test golden
image surfaced a second, unrelated bug: `virt-builder`'s plain templates
bake in a build-time-specific predictable interface name (e.g. `ens2`)
that doesn't match a sprite's actual device topology, so `ifupdown` never
brings the interface up. Fixed by switching the test image to
`systemd-networkd` with a `Name=en* eth*` wildcard match instead of a
hardcoded name. `scripts/sprite-verify-egress.sh` now automates the whole
check (create via the real dashboard, poll `virsh domifaddr`, fail loudly
with the sprite left running for inspection if no address appears) —
confirmed passing, DHCP lease in 9s. `scripts/sprite-remote-test.sh`
codifies the "fix `target/` ownership, run tests as root so the live
cloud-hypervisor boot tests actually run" sequence that was otherwise
hand-typed over SSH throughout this work.

## 2026-08-14 — Cloud Hypervisor backend for disposable "sprite" VMs

Sprites (`POST /v1/sprites` — instant, TTL-reaped, headless sandbox microVMs,
see `spec/src/sprite.rs`) can now boot on **Cloud Hypervisor** as an
alternative to the original libvirt/QEMU backend, selected per-request via a
new `backend` field (`"libvirt"`, the default, or `"cloudhypervisor"`).

- `core/src/cloud_hypervisor/` (new) — boots `cloud-hypervisor` as a direct
  child process of `machina-daemon` (there's no libvirtd in this path),
  reusing the existing golden-image qcow2-overlay registry
  (`core::libvirt::sprite::resolve_golden_image`,
  `core::libvirt::template_apply::materialize_from_base`). Teardown shells
  `ch-remote shutdown-vmm`, falling back to `SIGKILL` — same destroy-only
  semantics the libvirt backend already uses, no ACPI-graceful shutdown
  attempted.
- `daemon/src/sprite_registry.rs` — the in-memory registry/TTL reaper is now
  backend-agnostic (`SpriteBackendHandle::{Libvirt, CloudHypervisor}`) and
  hands out host-wide-unique vsock guest CIDs across both backends (Cloud
  Hypervisor requires an explicit CID, unlike libvirt's `<cid auto='yes'/>`,
  and CIDs are arbitrated by the kernel regardless of hypervisor).
- `daemon/src/routes/sprites.rs` — `create_sprite` dispatches on
  `req.backend`; `list`/`get`/`delete` are unchanged.

Deliberately out of scope for this pass: network egress
allow-list integration (Cloud Hypervisor sprites stay vsock-only, matching
today's libvirt sprites), a Kubernetes CRD/operator wrapper, a pluggable
disk-backend abstraction, and multi-host scheduling through the controller.

Verified with `cargo test -p machina-spec -p machina-core -p machina-daemon`
(25 passing: 5 new/updated in `core`, 13 in `spec`, 7 in `daemon`) plus a
full `install.sh` deploy on two Linux hosts (one redeploy onto an existing
install, one from-scratch). Not yet verified: a live `cloudhypervisor`-backend
sprite boot/teardown against a real `cloud-hypervisor` install (neither test
host has the binary installed).

## 2026-07-23 – 2026-07-25 — Security & correctness hardening marathon

Over three days, a multi-wave audit swept the entire Machina codebase — the full
Rust workspace (`core`, `daemon`, `controller`, `agent`, `tui`, `spec`,
`translate`, `rvb`, `virt-image-build`), the web frontend, ~90 shell/deploy
scripts, SQL migrations, CI, the Kubernetes/Helm chart, Docker, and the
Windows/Linux golden-image pipelines. **360+ discrete bugs** were found and
fixed across **19 waves**, each independently verified (`cargo check`/
`cargo test --workspace` on a Linux build host, `tsc --noEmit`/`vitest run`
for web changes, `bash -n`/`helm lint` where applicable) before merging.

Four dedicated self-review passes (waves 14/16/17/18/19) re-audited prior
waves' own fixes rather than hunting new ground, and found a real,
verifiable gap in **every single pass** — most notably an HA-recovery
`host_id`-revert compensation path (added in wave 14) that took **five
follow-up waves** to fully close off across seven independent bypass
routes (task retry, task cancel, task-bus republish failure, and two
startup crash-reapers).

### Highlights (most severe findings)

- **Sendmail flag injection** (wave 19) — a notification-channel email
  address, fully attacker/operator-controlled and validated only for `@`
  and no whitespace, reached `sendmail` as a bare positional argument. A
  value like `-C/tmp/evil.cf@x` was a working flag-injection primitive.
- **HA-recovery `host_id` never reverted on failure** (waves 14, 16, 17,
  18, 19) — an HA failover writes the destination `host_id` before its
  recovery task exists, so any way to fail/cancel/retry/crash out of that
  task without reverting it permanently stranded the VM's control-plane
  record. Closed across 7 independent code paths.
- **Zeus firewall `force`-flag authz gap** (wave 12) — bypassing the
  firewall-change approval gate required only `operator`, unlike every
  sibling "skip the safety gate" action in the same file.
- **Fail-open migration/health prechecks** (wave 16, confirmed a prior
  flag) — an unreachable source host or guest agent during a migration
  precheck or VM health check silently produced a passing/healthy result
  instead of blocking or flagging degraded state.
- **Host-cockpit privilege escalation** (wave 19) — an operator-gated
  endpoint forwarded an unrestricted action string to the same agent RPC
  its sibling endpoint correctly gates behind `require_admin`, reaching
  `storage.pool.delete`/`storage.volume.delete`/`network.delete`.
- **Unauthenticated plaintext-credential leaks** (waves 15, 19) — VM
  domain-XML (embeds VNC/SPICE passwords), host process lists (leak
  secrets via `/proc/<pid>/cmdline`), and RBAC role enumeration were all
  reachable with no role check, unlike their guarded siblings.
- **Atlas backup/snapshot/restore false success** (wave 14) — task
  handlers marked DB records `'completed'` the instant Atlas *accepted*
  a job (202), without polling to a terminal state; a failed Atlas-side
  operation silently reported success.
- **Zeus firewall temporary rules never enforced** (waves 17/18) — a
  "break-glass" temporary rule was recorded as `applied: true` and shown
  as live/auto-expiring, but no code path ever pushed it to a host
  firewall. Now honestly reported as audit-only.
- **CSV/YAML export injection** (waves 15/16) — exported VM/team/cost
  names starting with `=`/`+`/`-`/`@` could execute as a spreadsheet
  formula on open (CWE-1236); YAML exports interpolated names unescaped.
- **Helm chart hardcoded JWT secret** (wave 16) — `values.yaml` shipped
  `jwtSecret: "change-me-in-production"` as a literal default, which the
  daemon (unlike its own known dev secret) would accept as genuine,
  letting anyone forge admin tokens on an unmodified deployment.
- **Command-injection in remote deploy/install scripts** (waves 13, 19) —
  unquoted variables spliced into root SSH command strings
  (`install.sh --bind`, `deploy-remote.sh`'s license-key export), and a
  systemic sweep of every `Command::new`/`virsh`/`systemctl`/`nmcli`/
  `sendmail` call site for the "leading `-` as flag injection" pattern.
- **Unverified golden-image supply chain** (wave 15) — the Ubuntu
  desktop golden-image build downloaded the base cloud image over HTTPS
  with no checksum verification before using it as every VM's base.
- **Graceful shutdown gap** (wave 15) — the daemon's TLS (production)
  listener never wired up graceful shutdown; SIGTERM hard-killed
  in-flight VNC/SPICE/SSH console sessions instead of draining them.
- **OIDC hardening** (wave 18) — no timeout on IdP HTTP calls (could
  hang auth handlers indefinitely), and no issuer-pinning check on the
  fetched discovery document (OIDC Discovery 1.0 §4.3).
- **RCE in e2e test infrastructure** (wave 11) — a guest-exec helper's
  mismatched shell-escaping allowed command injection when a test VM's
  guest script contained an apostrophe.
- **Live VNC console regression** (self-caught same day) — a bundled
  vs. system noVNC version mismatch broke console fallback; found via
  live browser verification and fixed within the same session.

### Wave-by-wave summary

| Wave | Date (UTC-ish) | Commit | Fixes | Focus |
|---|---|---|---|---|
| — | 2026-07-23 21:06 | `23f51dbd` | — | Pre-marathon: closed daemon/controller authz gaps, hardened secrets, fixed task-delivery bugs |
| 1 | 2026-07-24 07:04 | `0df82f91` | 65 | Broad sweep: daemon, controller, core, agent, tui, web |
| 2 | 2026-07-24 08:19 | `18caf8a9` | 55 | Second broad wave: controller, daemon, spec, web |
| 3a | 2026-07-24 14:38 | `fbe6fc3d` | 3 | Partial: URL-encoding gaps, CSV injection |
| 3 | 2026-07-24 15:49 | `ffcf7e13` | 19 | Agent crate, web api/utils, deploy scripts |
| 4 | 2026-07-24 18:17 | `f4ae5c36` | 22 | Dedicated re-review of highest-value files |
| 5 | 2026-07-24 18:40 | `32acc7e1` | 4 | Cross-file and cross-layer deep reviews |
| 6 | 2026-07-24 19:08 | `a15089e3` | 6 | Security headers, rate limiting, systemd hardening, deps |
| — | 2026-07-24 20:13 | `e6ae7591` | 1 | Live VNC console regression hotfix |
| 7 | 2026-07-24 20:56 | `36a0ffbe` | 8 | Daemon rate limiting, cookie hardening, second passes |
| 8 | 2026-07-24 22:12 | `91e00ab8` | 17 | CI, K8s chart, Docker, Windows golden image, polkit |
| 9 | 2026-07-24 23:26 | `81a70da7` | 13 | K8s/observability configs, example configs, docs drift |
| 10 | 2026-07-25 00:02 | `2a165be2` | 15 | machinactl, privileged scripts, e2e test quality |
| 11 | 2026-07-25 00:12 | `87068167` | 6 | Remaining e2e/bundle scripts, incl. one real RCE |
| 12 | 2026-07-25 00:34 | `7748e958` | 1 | Zeus firewall force-flag authz gap |
| 13 | 2026-07-25 11:21 | `27b526eb` | 21 | AI engine, agent, TUI, spec/translate, deploy scripts |
| 14 | 2026-07-25 12:29 | `1a7cf65b` | 16 | HA/DRS, task bus, JWT auth, daemon routes, Atlas, bootstrap race |
| 15 | 2026-07-25 13:50 | `a529d49f` | 18 | API authz, agent RPC timeouts, graceful shutdown, supply chain |
| 16 | 2026-07-25 14:12 | `cf641483` | 14 | Fail-open precheck bugs, Helm secret default, CSV/YAML injection, KubeVirt false success |
| 17 | 2026-07-25 14:47 | `f07077e1` | 17 | Core crate first pass, console session lifecycle, HA revert gaps |
| 18 | 2026-07-25 15:36 | `145ddb4d` | 11 | 6th HA-revert bypass, OIDC hardening, firewall honesty, glass/hooks |
| 19 | 2026-07-25 18:27 | `ae8e608e` | 34 | Sendmail flag injection, 7th HA-revert bypass, host-cockpit privilege escalation, systemic argv sweep |

Fix counts are as documented in each wave's commit message; the session's
own running tally (quoted informally as work progressed) landed a little
higher (~385) after accounting for extra fixes self-review passes found
inside a wave that weren't reflected in that wave's headline count.

For full technical detail on any fix, see the corresponding commit message
(`git show <hash>`) — each documents the specific bug, the failure
scenario, and the fix applied.
