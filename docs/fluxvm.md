# FluxVM backend

[FluxVM](https://github.com/zyvorai/zyvor-fluxvm) is a second VM backend next to libvirt. A host runs
`fluxvm-api` (default `http://127.0.0.1:7788`), which starts VMs on QEMU, Cloud Hypervisor, Firecracker or
flux-vm. Machina shows these VMs next to libvirt VMs and manages them from the same UI, REST API and controller.

- **Daemon** — one host: create, power, consoles, snapshots, backups, hot-add, NICs, live migration.
- **Controller** — the fleet: inventory, power and delete through each host's agent, migration between hosts (and
  DRS), HA re-create on another host.

## Set up

### Daemon (`/etc/machina/config.toml`)

```toml
[fluxvm]
enabled = true
base_url = "http://127.0.0.1:7788"
# token = ""             # bearer token from fluxvm [[auth.tokens]]
# token_file = ""        # or read it from a file
# insecure_tls = false   # accept a self-signed https:// fluxvm-api (also wss:// consoles)
# default_backend = "auto"   # auto | qemu | cloud-hypervisor | firecracker | flux-vm
```

Check it with `GET /api/v1/fluxvm/status` (`enabled`, `reachable`).

### Agent (`/etc/default/machina-platform`)

For the controller to see and manage a host's FluxVM VMs, `machina-agent` on that host needs the fluxvm-api:

| Variable | Default |
| --- | --- |
| `MACHINA_FLUXVM_URL` | the host's `[fluxvm] base_url` when `enabled` |
| `MACHINA_FLUXVM_TOKEN` | `[fluxvm] token` / `token_file` |
| `MACHINA_FLUXVM_INSECURE_TLS` | `[fluxvm] insecure_tls` |

Restart `machina-agent` after changing them. With no fluxvm-api the agent simply reports no FluxVM VMs.

## Create

`POST /api/v1/vms` with `backend: "fluxvm"` (UI: Create VM → FluxVM):

| Field | Meaning |
| --- | --- |
| `fluxvm_backend` | `qemu`, `cloud-hypervisor`, `firecracker`, `flux-vm` or `auto` |
| `fluxvm_image` | Disk image on the host |
| `fluxvm_kernel`, `fluxvm_initrd`, `fluxvm_kernel_args` | Direct kernel boot (Firecracker and flux-vm need a kernel; QEMU takes a bzImage) |
| `fluxvm_agent: false` | Turn off the in-guest agent (on by default; needed for the agent console) |
| `fluxvm_shared_disk: true` | Use the image in place as a shared disk (never cloned or deleted, guarded by `<disk>.fluxvm-lock`). Needed for migration and HA re-create. Not for flux-vm |
| `fluxvm_bridge` | Tap on an existing host bridge |
| `fluxvm_direct_uplink` (+ `fluxvm_direct_mode`, `fluxvm_direct_guest_ips`) | Bridge-less eBPF redirect between a host NIC and the tap |
| `fluxvm_network: "user"` / `"none"` | QEMU user-mode NAT (forces QEMU) / no network |

With none of the network fields, the VM gets a tap in its own network namespace with DHCP and NAT. Every mode except
`user` and `none` gets FluxVM's eBPF VM edge.

## What each engine supports

| Feature | QEMU | Cloud Hypervisor | Firecracker | flux-vm |
| --- | --- | --- | --- | --- |
| Power, metrics, snapshots | yes | yes | yes | yes |
| Serial console | interactive | read-only log | read-only log | read-only log |
| Agent console (guest shell) | yes | yes | yes | yes |
| Hot-add vCPU / memory | yes | yes | no | no |
| Extra NICs | yes | no | no | no |
| Backups (default or shared storage) | running or stopped | stopped | stopped | stopped |
| Live migration, HA re-create | shared disk | no | no | no |

The VM detail page shows only what the VM's engine supports (**Manage** and **Agent console** tabs).

## Operate (daemon)

All VM routes take `?backend=fluxvm` (a name that isn't a libvirt domain falls back to FluxVM).

- **Snapshots** — `GET|POST /api/v1/vms/{name}/snapshots`, `DELETE …/snapshots/{snap}`, `POST …/snapshots/{snap}/revert`.
  A stopped VM relaunches from the snapshot; a running QEMU VM must be stopped first.
- **Backups** — `POST /api/v1/backups {vm_name, backend: "fluxvm", compress}`, list with
  `GET /api/v1/backups?backend=fluxvm&vm=<name>`, `POST /api/v1/backups/restore {backup_id, backend: "fluxvm", vm_name}`
  (VM stopped), `DELETE /api/v1/backups/{id}?backend=fluxvm`. Works on every engine for VMs on FluxVM's default
  storage or a shared disk. A running VM can be backed up only on QEMU with default storage (through a short internal
  snapshot); stop it first on the other engines. Backups are qcow2; a restore converts back to the disk's own format
  (raw on every engine but QEMU with default storage) and replaces the disk, so internal snapshots taken after the
  backup are gone.
- **Hot-add** — `POST /api/v1/vms/{name}/vcpus/{n}` and `…/memory/{mb}`. Add-only: a smaller value is refused. The
  next start boots at the created size. A hot-added VM must be restarted before it can migrate.
- **Extra NICs** — `POST …/nic/attach {network: "<bridge>"}` returns the NIC's `mac`; `POST …/nic/detach/{mac}`
  removes it. Each NIC is a tap on that host bridge. When the primary NIC is in the VM's own network namespace,
  FluxVM passes the bridge taps to QEMU as open file descriptors, at hot-add and on every restart. A VM with extra
  NICs can't migrate; after removing the last one, restart it first (it still has the PCIe layout it booted with).
- **Live migration** — `POST …/migrate {dest_uri, live: true}`. `dest_uri` is `local` (a fresh QEMU process on the
  same host) or another host's fluxvm-api URL with `dest_token`; optional `listen_host`, `advertise_host`,
  `bandwidth_mbps`, `max_downtime_ms`. The disk is never copied.
- **Consoles** — serial at `/ws/v1/console/{name}?backend=fluxvm&token=…`; guest shell at
  `/ws/v1/fluxvm-console/{name}?token=…&cols=&rows=` (needs `vms:write`; keystrokes as binary frames, resize as
  `{"type":"resize","cols":…,"rows":…}` text). Get `token` from `POST /api/v1/ws-token`.

## Fleet (controller)

The agent adds FluxVM VMs to its inventory, and the controller keeps them as `inventory_source = 'fluxvm'` rows
(migration `068`). Rows are removed only when the agent reached the fluxvm-api, and never during a migration.

- **Power and delete** — `vm.power` / `vm.delete` go to the host's fluxvm-api through its agent.
- **Migration** — `POST /api/v1/vms/{id}/migrate {dest_host_id, live: true}` (and DRS, which uses the same task).
  The controller starts a receiver on the destination agent, starts the migration on the source, waits for it,
  removes the paused source and adopts the receiver. The destination may be the same host. A failure cancels both
  sides. The pre-check (`POST …/migrate/precheck`) asks the source agent for the VM's current state and checks the
  engine, shared disk, hot-add and destination capacity.
- **HA** — when a host fails (and the VM has an HA policy), `ha.recover` re-creates the VM from its last FluxVM
  record on the target host, on the same shared disk, breaking the disk lock (`shared_takeover`). The same
  re-create on demand: `POST /api/v1/vms/{id}/fluxvm/recover {host_id?}` (operator; default host = current host).
- **Reconcile** — discovered FluxVM VMs are unmanaged, so the reconciler doesn't fight changes made directly in
  FluxVM. Rows marked managed are reconciled like libvirt VMs.

Disks must be on storage every host can reach (NFS or Ceph RBD mounted at the same path) for migration or HA
between hosts.

## Test

```bash
make regression-fluxvm   # scripts/regression/ops-fluxvm.js
```

It creates three throwaway VMs and covers the consoles, snapshots, hot-add, NICs (host bridge, and on a namespace VM
across a restart), backup/restore on QEMU and on a stopped Firecracker VM, daemon migration, the controller inventory
row, pre-check, `vm.migrate` and HA re-create. Overrides: `FLUXVM_IMAGE`, `FLUXVM_QEMU_KERNEL`,
`FLUXVM_QEMU_INITRD`, `FLUXVM_FC_KERNEL`, `FLUXVM_BRIDGE` (`virbr0`), `FLUXVM_SHARED_DIR`
(`/var/lib/fluxvm/shared`), `FLUXVM_SKIP_PLATFORM=1`. Results: [regression RESULTS](../scripts/regression/RESULTS.md).

## Limits

- Migration and HA re-create were verified host-to-itself on one host; moving between two hosts uses the same code
  path but has not been run on a two-host lab yet.
- Backups need FluxVM's default storage or a shared disk file (not LVM thin, NBD or Ceph RBD). Only QEMU with
  default storage can back up a running VM.
- Hot-add is per engine (table above); extra NICs are QEMU-only.
- Migration needs QEMU on a shared disk, no extra NICs, and no hot-add or NIC removal since the last start.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| `fluxvm/status` says unreachable | `systemctl status fluxvm`, `base_url`, token |
| No FluxVM VMs in the platform inventory | `MACHINA_FLUXVM_URL` on the agent, then `systemctl restart machina-agent` |
| Migration pre-check fails `mobility` | Engine isn't QEMU, disk isn't shared, or the VM was hot-added (restart it) |
| HA re-create fails on the disk lock | Another instance still runs on the disk; stop it, or check fencing |
| Serial shows nothing on a non-QEMU VM | It's the read-only log; use the Agent console for input |
