# Machina Platform Roadmap (Batches 12–30+)

This document tracks the vCenter-class platform plan on libvirt/KVM. See also [`platform.md`](platform.md) for architecture.

## Batch status

| Batch | Theme | Status |
|-------|--------|--------|
| 12 | libvirt ops hardening — lifecycle phases, structured errors, host validation, live migrate after snap clone, reconcile | **Shipped** |
| 13 | Unified UX — proxy default, task drawer, global search, dashboard fusion, error banners | **Shipped** |
| 14 | Storage/network engine v1 — agent provision via virsh | **Shipped (v1)** |
| 15 | Enterprise ops — policy-lite, quotas, alerts via webhooks, support bundle, upgrade manager | **Shipped (v1)** |
| 16 | Ecosystem — `platformctl`, Terraform stub, chaos/soak scripts, cert matrix | **Shipped (v1)** |
| 17 | VM discover/adopt — unmanaged inventory from host sync, adopt API + UI | **Shipped (v1)** |
| 18 | Per-VM metrics — agent ListVms stats, `vm_metrics` table, Prometheus gauges, VM detail | **Shipped (v1)** |
| 19 | Content library v1 — ISO/image catalog API + UI | **Shipped (v1)** |
| 20 | Disk attach — agent `AttachDisk`, `vm.disk.attach` task, VM detail form | **Shipped (v1)** |
| 21 | Task/event SSE — `/api/v1/events/stream`, task drawer live refresh | **Shipped (v1)** |
| 22 | E2E + platformctl extensions for batches 17–21 | **Shipped (v1)** |
| 23 | macOS-like UX shell — dashboard, Finder VMs, tabs, Control Center, Settings hub, Migration/Activity/Recommendations shells | **Shipped (v1)** |
| 24 | Drag-and-drop migrate + network fix-it buttons | **Shipped (v1)** |
| 25 | Guest Tools agent RPC + health sync + UI strip | **Shipped (v1)** |
| 26 | Windows VM wizard + Migration Assistant HyperSDK scan | **Shipped (v1)** |
| 27 | Topology map + application groups API | **Shipped (v1)** |
| 28 | Health check engine + Fix It buttons | **Shipped (v1)** |
| 29 | Actionable notification center | **Shipped (v1)** |
| 30 | Template marketplace + ISO approval workflow | **Shipped (v1)** |
| 31 | Blueprints / automation shortcuts | **Shipped (v1)** |
| 32 | Workspaces (project-scoped views) | **Shipped (v1)** |
| 33 | FinOps capacity + cost dashboard | **Shipped (v1)** |
| 34 | Safe delete / approval workflows | **Shipped (v1)** |
| 35 | Task-centric rollback UX (cancel/retry) | **Shipped (v1)** |
| 36 | Natural language command bar (palette) | **Shipped (v1)** |
| 37 | Mobile-responsive platform nav | **Shipped (v1)** |
| 38 | Support Assistant + bundle export | **Shipped (v1)** |
| 39 | ISO checksum field on submit | **Shipped (v1)** |
| 40 | Full Migration Assistant + HyperSDK submit | **Shipped (v1)** |
| 41 | Mac UI wave 1 — VMs, Hosts, Tasks, Alerts + shared empty/filter components | **Shipped (v1)** |
| 42 | Storage pool discover + Welcome wizard + Storage Mac UI | **Shipped (v1)** |
| 43 | Template readiness API + deploy traffic-light UX | **Shipped (v1)** |
| 44 | Spotlight ops — platform command router + review/confirm in ⌘K | **Shipped (v1)** |
| 45 | Mac UI wave 2 — Content, Backups, Migration, Activity, Reports | **Shipped (v1)** |
| 46 | Mac UI wave 3 (admin pages) + Control Center 2.0 | **Shipped (v1)** |
| 47 | Help dialog Platform tab + docs sync | **Shipped (v1)** |
| 48 | Host stale detection, E2E extensions, remote deploy verify | **Shipped (v1)** |
| 49 | Mac UI wave 4 — VM/Host detail, Settings hub, Blueprints, Topology glass panels | **Shipped (v1)** |
| 50 | Control Center 3.0 — module grid + Zeus firewall strip | **Shipped (v1)** |
| 51 | Settings hub macOS sidebar (`MacSettingsPane`) — Security + Network firewall panes | **Shipped (v1)** |
| 52 | VM Security tab — guest firewall ports | **Shipped (v1)** |
| 53 | Spotlight intents — firewall settings, block incoming, open ports | **Shipped (v1)** |
| 54 | Zeus Firewall macOS Security pane — toggles, stealth, profiles (AI-372–378) | **Shipped (v1)** |
| 55 | Fleet firewall views — Overview, Activity, Ports, K8s/Cloud/Connectivity (AI-379–384) | **Shipped (v1)** |
| 56 | E2E + docs AI-372–391 / UX-49–56 | **Shipped (v1)** |
| 57 | Cross-shell consistency sweep — semantic colors v2, bridge/Help parity, e2e matrix | **Shipped (v1)** — see [`next-big-sweep.md`](next-big-sweep.md) |
| 69 | Zeus Infrastructure AI Program (AI-138–147) — graph brain, RCA, twin, rightsizing, NL ops | **Shipped (v2)** — see [`zeus-infra-ai-program.md`](zeus-infra-ai-program.md) |
| 70 | Security Operations Center (SOC) — detection, ASM, Splunk/Elastic/Sentinel/QRadar, playbooks | **Shipped (v2)** — alert detail + playbook editor; see [`soc-integrations.md`](soc-integrations.md) |

## Batches 71–75 (Zeus Libvirt/KVM Platform — Phases A–E)

| Batch | Phase | Theme | Status |
|-------|-------|--------|--------|
| 71 | A | Disk-safe clone, template git/approval, cloud-init validate, VM retire/export/IaC APIs | **Shipped (v1)** |
| 72 | A | Platform UI — Cloud-Init Studio, snapshot quiesce, template sync/approve, VM lifecycle actions | **Shipped (v1)** |
| 73 | B | Fleet snapshot schedules, storage backend discover, incremental backup worker | **Shipped (v1)** |
| 74 | C | Network canvas stub, host GPU inventory RPC, datacenter by `inventory_source` | **Shipped (v1)** |
| 75 | E | KubeVirt inventory sync API; VMware/Proxmox remain import-only | **Shipped (v1)** |
| 77 | A/B | Migration bandwidth/postcopy, VM Time Machine timeline, template deploy vars, IaC bundle download | **Shipped (v1)** |
| 78 | A | Controller compile fixes, platform ISO create wizard, template git webhook sync | **Shipped (v1)** |
| 79 | B/C | NFS/LVM pool provision hardening, network canvas + PacketWolf flows API | **Shipped (v1)** |
| 80 | A/D/E | Platform ISO create API, AI VM builder, IaC ZIP export, Proxmox scope, API module split | **Shipped (v1)** |

### Batch 71–72 deliverables (Phase A)

- **Clone:** `clone_mode` linked/full on agent, controller, classic + platform VM detail
- **Templates:** `040_templates_unify.sql`, git sync (`MACHINA_TEMPLATES_GIT_DIR`), approval PATCH, publish-from-VM
- **Cloud-init:** `POST /api/v1/cloud-init/validate`, [`CloudInitStudio`](/platform/cloud-init)
- **Lifecycle:** `POST /vms/{id}/retire`, `POST /vms/{id}/disk/export`, `GET /vms/{id}/export` (Terraform/Ansible/XML)
- **Web API modules:** `platformCloudInit.ts`, `platformVmLifecycle.ts`, `platformTemplatesExtra.ts`, …

### Batch 73 deliverables (Phase B)

- **Snapshots:** Rich `CreateSnapshotBody` + guest `virDomainFSFreeze` / thaw
- **Fleet schedules:** `041_fleet_snapshot_schedules.sql`, scheduler, [`/platform/fleet-snapshots`](/platform/fleet-snapshots)
- **Storage:** Pool `backend` from libvirt XML; NFS/LVM/Ceph labels on discover
- **Backup:** Incremental chain in worker; S3 `--endpoint-url` on backup targets

### Batch 74–75 deliverables (Phases C & E)

- **Network canvas:** [`/platform/network-canvas`](/platform/network-canvas) — topology + PacketWolf Network Brain (service map graph, workloads, timeline, threats, top talkers, K8s nodes)
- **GPU:** `ListHostGpus` agent RPC, `GET /hosts/{id}/gpus`
- **Datacenter:** [`/platform/datacenter`](/platform/datacenter) — VMs grouped by `inventory_source`
- **KubeVirt:** `POST /api/v1/kubevirt/sync` inventory reconcile + **K8s Workloads CRUD** (start/stop/restart/delete, YAML create via `/k8s/apply`); platform VM detail lifecycle for `inventory_source=kubevirt`

**Deferred (honest):** OVS SDN, Ceph pool wizards, AI VM builder GA, browser RDP, federated marketplace, Proxmox adapter, full `platform.ts` folder split.

### Batch 76 deliverables (Phase A/B UI wiring)

- **Storage:** NFS/LVM backend selector on add-pool sheet; discover shows libvirt `backend` label
- **Backups:** Incremental backup type + link to fleet snapshot schedules
- **GPU:** PCI/IOMMU panel via `GET /hosts/{id}/gpus` on online hosts
- **Datacenter:** KubeVirt inventory sync button
- **Content:** Approved ISO → `/create?iso_path=…` deep link (classic virt-install wizard)

### Batch 77 deliverables (migration, Time Machine, templates)

- **Migration:** `bandwidth_mib` + `postcopy` on agent RPC, controller task, platform VM settings (live/cold, MiB/s cap)
- **Time Machine:** `GET /api/v1/vms/{id}/timeline` — unified backup + snapshot list on VM snapshots tab
- **Templates:** `template_vars` on `POST /vms/from-template` with `{{ hostname }}` substitution; deploy sheet hostname field
- **IaC:** `downloadVmIacBundle()` — single JSON export alongside per-file downloads
- **Web API modules:** `platformVmMigrate.ts`, `platformVmTimeline.ts`

### Batch 78 deliverables (ISO wizard, webhook, CI fixes)

- **Controller:** Fix GPU list JSON serialization, IaC `get_domain_xml` return type, fleet snapshot `enqueue_task` error mapping
- **ISO wizard:** [`/platform/create-iso`](/platform/create-iso) — approved ISO picker → classic `/create` with `iso_path`, `name`, `vcpus`, `memory_mb`, `disk_gb` prefilled
- **Templates:** `POST /api/v1/templates/sync-git/webhook` + optional `X-Machina-Template-Sync-Token` / `MACHINA_TEMPLATES_SYNC_TOKEN`

### Batch 79 deliverables (storage + network canvas)

- **Storage:** Agent NFS `host:/export` parsing (`--source-host`, `--source-dir`, local mount under `/var/lib/machina/nfs/`); LVM skips `mkdir` on `/dev/*`; directory backend rejects NFS syntax
- **Network canvas:** `GET /api/v1/network-canvas` — topology + PacketWolf flows/stats/anomalies; enriched [`/platform/network-canvas`](/platform/network-canvas) UI
- **Web API:** `platformNetworkCanvas.ts`

### Batch 83 deliverables (platform UX ease-of-use sweep)

- **Shared wizard shell:** `PlatformStepWizard` — fixed header/footer, scrollable body; used across Create VM, ISO, Windows, storage, network, and host enroll flows
- **Create VM:** API-backed templates + `GET …/readiness` gate; Linux deploy via `createVmFromTemplate`; live networks; task toast with link
- **ISO / Windows:** `PlatformIsoCreate` 4-step wizard; `WindowsCreateWizard` reuses main wizard + Windows options
- **Storage / Networks:** `StoragePoolWizard`, `NetworkCreateWizard` (discover CTA when empty); empty-state CTAs open wizards
- **Onboarding:** `PlatformWelcome` “Create your first VM” opens unified wizard; Hosts empty state opens `HostEnrollWizard`
- **E2E:** `platform-wizard-ux.spec.ts` — readiness block, storage/network wizard smoke
- **Golden images:** Catalog `download_url` + `template_image_fetch` SSH pull on first `vm.apply`; readiness `auto_fetch` allows Create VM; `GET /templates/missing-images` + `POST /templates/prefetch-missing` bulk download; Fleet insights + Templates prefetch CTA; task toast **View task** link; marketplace `auto_fetch` badge + manual-upload copy
- **SSH for auto-fetch:** Controller uses stored host credentials to run `curl -fL` or `wget` on the hypervisor into `source_disk` (typically `/var/lib/libvirt/images/`). Host must be **online**, SSH reachable from the controller, and have outbound HTTPS to the catalog URL (redirects followed).

### Batch 82 deliverables (PacketWolf network canvas depth)

- **Discovery:** `packetwolf_discover.rs` — auto-find PacketWolf API via localhost probes + `kubectl get svc` (LoadBalancer/NodePort); `PACKETWOLF_AUTO_DISCOVER` (default on)
- **Network Brain API:** `packetwolf/app/network_brain.py` — `/api/v1/network/overview`, `service-map`, `workloads`, `timeline`, `threats`, `top-talkers`, `/api/v1/nodes`
- **Network canvas API:** `GET /api/v1/network-canvas` merges PacketWolf Network Brain — overview, service-map, workloads, timeline, threats, top-talkers, K8s nodes
- **UI:** [`/platform/network-canvas`](/platform/network-canvas) — SVG service map graph, workloads/timeline panels, threat pulse, K8s node chips, discovery banner

### Batch 84 deliverables (Cockpit parity + globe + KubeVirt CRUD)

- **Compute:** Overview CPU/memory topology modals; Settings tab links to overview (no duplicate resize)
- **Storage:** Pool volume list/create/delete; libvirt pool destroy on host
- **Snapshots:** VFIO precheck (single VM + Machine Finder batch); revert/clone action warnings
- **Networking:** NIC IP on VM network tab; Finder guest-IP batch fallback; libvirt network destroy on host
- **Overview:** Domain XML inline edit; hypervisor filesystems/processes panel
- **WebGL globe:** Lazy `three.js` in `InfrastructureEarthGlobe` with canvas fallback
- **KubeVirt:** Daemon lifecycle routes + K8s Workloads CRUD + platform VM detail actions
- **Host Cockpit modules:** `GET /api/v1/hosts/{id}/cockpit` — storaged (RAID/LUKS/LVM/Stratis/VDO/multipath), NetworkManager + firewalld editor, kdump/SELinux/Tuned/realmd/systemd/journal on host Storage/Network/System tabs; classic NodeInfo parity
- **VM polish:** Machine Finder pool/network resource strip; live CPU/memory bars on VM overview; virt-install import + osinfo-detect + RHSM URL resolve
- **ConsoleHub:** KubeVirt serial/VNC in ConsoleHub; session history in Command Center; platform host **Terminal** tab (browser SSH via daemon)

### Batch 81 deliverables (storage + VMware + reports)

- **Storage agent:** Ceph RBD, iSCSI (IQN), and ZFS pool provision in `agent/src/provision_ops.rs`
- **Platform storage UI:** Pool wizard backends `ceph`, `iscsi`, `zfs` with path hints
- **Web API:** `platformStorage.ts` (pools, tiers, SLA) re-exported from `platform.ts`
- **VMware:** `POST /api/v1/vmware/sync` honest import + migration-advisor scope; datacenter **VMware scope** button
- **Reports:** Fleet capacity planner panel on `/platform/reports` (storage totals, forecast, recommendations)

### Batch 80 deliverables (huge sweep)

- **ISO install:** `POST /api/v1/vms/from-iso` + `install_iso` label → CDROM in domain XML; [`/platform/create-iso`](/platform/create-iso) native create
- **AI VM builder:** `POST /api/v1/ai/vm-builder` + [`/platform/vm-builder`](/platform/vm-builder)
- **IaC:** `GET /api/v1/vms/{id}/export.zip` server-side ZIP; VM detail download button
- **Proxmox:** `POST /api/v1/proxmox/sync` honest import-only scope; datacenter UI button
- **Capacity:** `GET /api/v1/reports/capacity` merges AI planner forecasts + recommendations
- **Web API split:** `platformContent.ts`, `platformVmCreate.ts`, `platformAiVmBuilder.ts`, `platformProxmoxSync.ts` (re-exported from `platform.ts`)

## Batch 70 deliverables (SOC Program)

- **SOC-1–SOC-5:** Migration `038_soc.sql` — `soc_events`, `soc_alerts`, `soc_detection_rules`, `soc_integrations`, playbooks
- **SOC-6–SOC-10:** Ingest (firewall, audit, PacketWolf, platform) + detection engine + background worker
- **SOC-11–SOC-14:** `/api/v1/soc/*` APIs with RBAC; Splunk HEC forwarder
- **SOC-15–SOC-18:** Elastic bulk, Sentinel DCR, QRadar REST adapters
- **SOC-19–SOC-22:** [`PlatformSoc`](/platform/soc) hub — Overview, Alerts, Detections, ASM, Integrations
- **SOC-23–SOC-24:** SOAR playbooks (`soc_playbooks`) + webhook/notify steps
- **SOC-25–SOC-27:** Alert detail API (linked events, MITRE tags, playbook runs); playbook CRUD + global webhook settings (`039_soc_v2.sql`)
- **E2E:** [`soc.spec.ts`](../web/e2e/soc.spec.ts)

## Batch 312–331 deliverables (Bare metal + Zeus Firewall — Phase 23)

- **312–318:** Metal profiles in `core/src/firewall/profiles.rs`; synthetic inventory in `core/src/firewall/metal.rs`
- **314–317:** `bare_metal` targets in Zeus overview/detail/plan/apply; `024_baremetal_firewall.sql`
- **315/319/323:** Exposure scan + temporary PXE/BMC rules APIs
- **326–328:** GitOps policy on register; timeline events; SIEM `metal` tag
- **320/331:** Compliance kind `metal`; docs `zeus-os-ai-312-331.md`; E2E Phase 23 smoke

## Batch 41–48 deliverables (Awesome Sweep)

- **41:** `PlatformEmptyState`, `PlatformFilterPills`; Mac polish on VMs, Hosts, Tasks, Alerts
- **42:** Agent `ListStoragePools`, `storage_sync`, `POST /api/v1/storage/pools/discover`, `PlatformStorage` cards, `PlatformWelcome`
- **43:** `GET /api/v1/templates/{name}/{version}/readiness`, deploy sheet readiness banner
- **44:** `web/src/utils/platformCommands.ts`, CommandPalette review → confirm flow
- **45–46:** Mac UI on remaining platform pages; Control Center memory/offline/alerts sparkline row
- **47:** Help → Platform tab; `platform-ux-vision.md` / `platform-roadmap.md` updated
- **48:** Host `last_heartbeat_at` > 2m → `offline` in API; E2E storage discover + template readiness — see [`platform-batch-48.spec.ts`](../web/e2e/platform-batch-48.spec.ts)

## Batch 57 deliverables (cross-shell consistency — shipped)

Cross-shell consistency pass documented in [`next-big-sweep.md`](next-big-sweep.md):

- **A:** Semantic color system v2 — [`semanticColors.ts`](../web/src/utils/semanticColors.ts) across Classic, OpenStack, K8s, Platform; NodeInfo/Storage/Backups gauges; CommandPalette badges
- **B:** Platform Help → Platform tab; tier-aware shell bridge links
- **C:** JsonInspector / operator surface tail (VMDetails, OpenStack detail, K8sOverview, PlatformReports)
- **D:** E2e matrix — [`cross-shell.spec.ts`](../web/e2e/cross-shell.spec.ts), batch-48 + nav-coverage extensions (69 tests green)
- **E:** Small backend hooks for runbooks / discover (only if UI blocked)

## Batch 58 deliverables (cross-shell color tail — shipped)

Presentation-only sweep documented in [`next-big-sweep.md`](next-big-sweep.md):

- **Zeus Firewall:** security pages under [`web/src/pages/platform/security/`](../web/src/pages/platform/security/) use `riskTone`, `statusBadgeClasses`, `statusSurfaceClasses`
- **Platform widgets:** dashboard posture chips, template readiness, topology warnings, Zeus OS diagnosis/runbook lines, storage/network compliance grades
- **Classic tail:** VMDetails warn/error surfaces, Fleet hub links, form focus rings on Logs/NodeInfo/Backups
- **Shared primitives:** GlassButton danger, PlatformFilterPills active chip, BuildStepTimeline complete step, KubeVirtExposeServiceModal warn callouts, AI panel severity/result lines
- **Deferred:** `ChoiceCards` selection accent palette (documented in [`ux.md`](ux.md))

## Batch 59 deliverables (platform color tail — shipped)

- **Platform pages:** Events severity badges, Security Center graph/critical lists, VM detail guest health, Observability SLO chips, Activity/Host detail utilization bars, Maintenance host badges, Migration HyperSDK readiness, Content approval queue, Notifications unread styling, API key token banner, Threat Hunting severity, Reports cost highlights
- **Classic:** Settings OpenStack unreachable hints, HostNetworking route/sysctl warn callouts + focus ring token
- **Helper:** `utilizationBarClass()` in [`semanticColors.ts`](../web/src/utils/semanticColors.ts)

## Batch 60 deliverables (classic / OpenStack / K8s color tail — shipped)

- **Classic:** Dashboard integration banners + HyperSDK link; K8sOverview health probes, upgrade insights, node cordon/uncordon/drain chips; K8sWorkloads operator action chips; NodeInfo libvirt boot / consistency / package-update warn surfaces
- **OpenStack:** Instances/InstanceDetail/CreateInstance/Migrations warn banners; locked badges; hypervisor maintenance, port admin-down, floating-IP dissociate, volume-transfer auth key, network delete confirm; semantic hover on row action icons
- **Deferred:** primary CTAs (`bg-emerald-600` start/create), `ChoiceCards` accents, Identity admin buttons (`bg-amber-700`)

## Batch 61 deliverables (classic operator color tail — shipped)

- **Classic pages:** Networks libvirt-boot + active-network edit warn; DiskImages OpenStack/mkosi/virt-builder hints; Jobs empty-state link; KataContainers k3s/RKE2 callout; SystemCheck OpenStack hint; Services stop hover; HostNetworking topology legend; Console virt-viewer download chip; VMList row action hovers
- **VMDetails:** tune-disk hover + filesystem mount tag tone
- **Shared:** Hero capability badges; LibvirtOpenStackPushModal running hint; KubeVirtQcow2Modal exec hint; KubeVirtExposeServiceModal command tone; BuildStepTimeline failed step

## Batch 62 deliverables (cross-shell sweep closure — shipped)

- **Help parity:** Classic [`Navbar`](../web/src/components/Navbar.tsx) Help menu → Platform guide tab (desktop + mobile); e2e on `/vms`
- **Operator UX:** [`OpenStackInstanceDetail`](../web/src/pages/OpenStackInstanceDetail.tsx) persistent action `ErrorBanner` with `openStackErrorHints`; [`VMDetails`](../web/src/pages/VMDetails.tsx) KubeVirt exec result semantic exit surface
- **Shell bridge:** [`Fleet`](../web/src/pages/Fleet.tsx) disabled-state links to Platform desktop + Integrations via `hubLinkClasses()`
- **Dashboard:** GuestKit / Integrations secondary link chips (`statusBadgeClasses`); orange brand panels unchanged
- **E2e:** mobile jump → Resources (power tier); **71/71** local bundle green

## Batch 63 deliverables (Infrastructure OS Phase 53 + Phase 7 ops — shipped)

- **Machine Finder:** [`PlatformMachineFinder`](../web/src/pages/platform/PlatformMachineFinder.tsx) + [`MachineFinderGeography`](../web/src/components/platform/MachineFinderGeography.tsx) — site → rack → host → VM columns at `/platform/hosts/finder`; Mission Control → Open Machine Finder
- **Ops UX:** [`PlatformReports`](../web/src/pages/platform/PlatformReports.tsx) runbook execute `ErrorBanner`, showback empty/retry, compliance export links on showback tab; [`PlatformOperationsHub`](../web/src/pages/platform/PlatformOperationsHub.tsx) Reports & Runbooks tile
- **E2e:** [`platform-machine-finder.spec.ts`](../web/e2e/platform-machine-finder.spec.ts), [`platform-ops-reports.spec.ts`](../web/e2e/platform-ops-reports.spec.ts); **76/76** local bundle green

## Batch 64 deliverables (GPU Command Center — shipped)

- **Backend:** `GET /api/v1/fleet/gpu` — [`fleet_gpu.rs`](../controller/src/engine/fleet_gpu.rs) host tag inventory (MIG/vGPU/CUDA/passthrough) + GPU VM rollup
- **UI:** [`PlatformGpuCommandCenter`](../web/src/pages/platform/PlatformGpuCommandCenter.tsx) at `/platform/gpu`; Resources hub tile; CUDA placement advisor via existing `getGpuPlacement`
- **E2e:** [`platform-gpu-command-center.spec.ts`](../web/e2e/platform-gpu-command-center.spec.ts); **78/78** local bundle green

## Batch 65 deliverables (Infrastructure OS Big Sweep — shipped)

- **Maintenance Mission:** `GET /api/v1/fleet/maintenance-mission` — [`fleet_maintenance_mission.rs`](../controller/src/engine/fleet_maintenance_mission.rs); [`PlatformMaintenance`](../web/src/pages/platform/PlatformMaintenance.tsx) **Mission** tab with [`BuildStepTimeline`](../web/src/components/BuildStepTimeline.tsx), operator actions (schedule / enter / exit / upgrade), Spotlight `maintenance mission` → `/platform/maintenance?tab=mission`
- **Infrastructure DNA:** `GET /api/v1/fleet/dna` — [`fleet_dna.rs`](../controller/src/engine/fleet_dna.rs); [`InfrastructureDnaStrip`](../web/src/components/platform/InfrastructureDnaStrip.tsx) on dashboard (power+) and Mission Control header
- **E2e:** [`platform-maintenance-mission.spec.ts`](../web/e2e/platform-maintenance-mission.spec.ts), [`platform-infrastructure-dna.spec.ts`](../web/e2e/platform-infrastructure-dna.spec.ts); **81/81** local bundle green

## Batch 66 deliverables (Full Jarvis shell — shipped)

- **Backend:** `GET /api/v1/ai/jarvis/landing` — curated navigate intents via [`intent_router.rs`](../controller/src/engine/ai/intent_router.rs) `jarvis_landing_intents`
- **UI:** [`PlatformJarvisBriefing`](../web/src/components/platform/PlatformJarvisBriefing.tsx) intent-first landing (all tiers); [`platformJarvisShell.ts`](../web/src/utils/platformJarvisShell.ts) hides sidebar on Normal tier; Control Center toggle; Spotlight open event from Jarvis search
- **E2e:** [`platform-jarvis-shell.spec.ts`](../web/e2e/platform-jarvis-shell.spec.ts); **84/84** local bundle green

## Batch 69 deliverables (Linux Base OS sweep — Phase 59)

- **Agent:** `ApplyLinuxPackageUpgrade`, `HostLinuxReboot`, `GetLinuxFilesystems`, `GetLinuxTopProcesses`
- **Controller:** maintenance-gated package apply + reboot tasks; audit/network/updates API normalizers; `PackageUpdateCheck.packages[]`
- **UI:** Host System Settings Linux depth; Maintenance mission apply; fleet Linux health on Zeus OS, Activity Monitor, hosts list; reboot-required in Control Center / Dynamic Island
- **E2e:** [`platform-linux-os.spec.ts`](../web/e2e/platform-linux-os.spec.ts); doc [`zeus-os-ai-692-701.md`](zeus-os-ai-692-701.md)

| Phase | AI | Focus |
|-------|-----|--------|
| **59** | 692–701 | **Linux Base OS** — remote patch apply, reboot, host+fleet depth |

## Batch 68 deliverables (Infrastructure Earth v2 — shipped)

- **Globe site legend:** clickable site chips on [`InfrastructureEarthGlobe`](../web/src/components/platform/InfrastructureEarthGlobe.tsx) deep-link to Machine Finder (`?site=…`)
- **Machine Finder:** globe + legend above geography columns on [`PlatformMachineFinder`](../web/src/pages/platform/PlatformMachineFinder.tsx)
- **Mission Control:** resilient parallel load (`.catch` on inventory fetches); incident analyze + mission-stack mocks in e2e
- **E2e:** [`platform-infrastructure-earth-globe.spec.ts`](../web/e2e/platform-infrastructure-earth-globe.spec.ts) (Finder legend); **87/87** local bundle green

## Batch 67 deliverables (Infrastructure Earth globe + Enterprise UX — shipped)

- **Phase 58 v1:** [`InfrastructureEarthGlobe`](../web/src/components/platform/InfrastructureEarthGlobe.tsx) canvas wireframe globe with site health markers in [`InfrastructureEarthView`](../web/src/components/platform/InfrastructureEarthView.tsx) / Mission Control
- **Enterprise slice:** [`EnterpriseSecurityStrip`](../web/src/components/platform/EnterpriseSecurityStrip.tsx) on advanced dashboard; [`PlatformEnterprise`](../web/src/pages/platform/PlatformEnterprise.tsx) vault sync `ErrorBanner`; Jarvis landing Enterprise Keychain intent
- **E2e:** [`platform-enterprise.spec.ts`](../web/e2e/platform-enterprise.spec.ts), [`platform-infrastructure-earth-globe.spec.ts`](../web/e2e/platform-infrastructure-earth-globe.spec.ts) (advanced strip); **87/87** local bundle green

## Batch 49–56 deliverables (Mac UX wave 4 + Zeus Firewall macOS UX)

- **49–52:** `MacGlassPanel` on VM detail, Host detail, Settings, Blueprints, Topology; `MacSettingsPane` Settings hub; Control Center 3.0 module grid
- **AI-372–374:** `MacToggle`, `MacSettingsPane`, `MacListRow`, `MacSegmentedControl` in `PlatformMacUi.tsx`
- **AI-375–378:** Firewall target detail — working on/off, stealth, profile apply with preview sheet
- **AI-379–384:** Fleet Overview (Launchpad grid), Activity timeline, Ports list rows; K8s/Cloud/Connectivity without page-body JSON
- **AI-385–387:** VM Security tab, Settings Network/Firewall pane, Control Center Zeus strip
- **AI-388–391:** Spotlight intents, E2E smoke extensions, `docs/zeus-os-ai-372-391.md`
- **Backend:** `enable: false` firewall apply in `core/src/firewall/apply.rs`; `firewall_approval_sla_hours` in cluster settings API


Batches are grouped into **phases** (~10 batches each). Phases 1–2 (batches 1–16) established core platform + enterprise v1. Phases 3–10 below extend toward full vCenter parity.

| Phase | Batches | Focus |
|-------|---------|--------|
| 3 | 17–26 | Inventory depth — discover, metrics, content, disks, import/export |
| 4 | 27–36 | Multi-site — federation, DR replicas, stretch clusters |
| 5 | 37–46 | Networking — NSX-class overlays, micro-segmentation, IPAM |
| 6 | 47–56 | Storage — vSAN-class tiers, snapshots at scale, backup SLAs |
| 7 | 57–66 | Operations — runbooks automation, compliance, cost showback |
| 8 | 67–76 | Developer ecosystem — Terraform provider GA, SDK, marketplace |
| 9 | 77–86 | Observability — tracing, SLO dashboards, predictive placement |
| 10 | 87–100 | Enterprise hardening — Vault/MFA, air-gap, cert FIPS, multi-tenant isolation |

## Batch 17 deliverables

- Migration `012_platform_batch17.sql`: `vm_metrics`, `content_images`, `network_reservations`
- Host inventory inserts **unmanaged** VMs when libvirt domain not in DB; emits `vm.discovered`
- `POST /api/v1/vms/{id}/adopt`, `GET /api/v1/vms?managed=false`
- Web: discovered-only toggle + Adopt on VMs list and detail

## Batch 18 deliverables

- Agent `ListVms` returns per-VM memory and disk IOPS from libvirt metrics
- Controller upserts `vm_metrics` on `host.inventory`
- `GET /api/v1/vms/{id}/metrics`, Prometheus `machina_vm_*` gauges
- VM detail metrics strip

## Batch 19 deliverables

- `GET/POST /api/v1/content/images`
- Web `/platform/content` content library page

## Batch 20 deliverables

- Agent `AttachDisk` gRPC + virsh attach-disk
- `POST /api/v1/vms/{id}/disks/attach` → `vm.disk.attach` task
- VM detail attach-disk form

## Batch 21 deliverables

- `GET /api/v1/events/stream` SSE on controller event bus
- Platform task drawer subscribes via EventSource + polling fallback

## Batch 22 deliverables

- E2E smoke: content images, VM metrics, managed filter, SSE probe
- `platformctl` commands: `content`, `vm metrics`, `vm adopt`, `events-stream`

## Batch 29 deliverables

- Actionable notification center with Retry backup, Open VM, View tasks

## Batch 30 deliverables

- Migration `014_platform_batch30.sql`: content approval fields, template marketplace metadata
- ISO workflow: new images start `pending`; `POST /api/v1/content/images/{id}/approve|reject`
- Template marketplace: `GET /api/v1/templates/marketplace`, featured/category metadata on templates
- Web: approval queue on `/platform/content`, App Store-style `/platform/templates` marketplace
- E2E smoke: marketplace GET, ISO submit → approve; `platformctl marketplace`, `content-approve`

## Explicit deferrals

See [`enterprise-backlog.md`](enterprise-backlog.md): Vault/MFA, multi-site DR, plugin marketplace, in-browser RDP.

## Zeus OS batches (AI-49+)

See [`zeus-os-vision.md`](zeus-os-vision.md) for the full vision.

| Batch | Deliverable |
|-------|-------------|
| AI-49 | AI foundation: settings, context assembler, Copilot shell, Spotlight ⌘Space |
| AI-50 | Machina Doctor 0–100 + unified VM detail tab |
| AI-51 | Migration Radar + HyperSDK integration |
| AI-52 | Cost Guardian + Capacity Planner |
| AI-53 | Security Sentinel + Network Lens |
| AI-54 | Runbook generator + notification actions |
| AI-55 | Blueprint Studio + Explain button rollout |
| AI-56 | Mission Control + Dock + Control Center AI mode |
| AI-57 | Network Lens UI + Copilot network queries |
| AI-58 | Time Machine backup timeline |
| AI-59 | AI Terminal Companion |
| AI-60 | Policy Generator YAML export |
| AI-61 | Navbar Copilot + VM context + polish |
| AI-62 | Autopilot preview propose/execute |
| AI-63 | Compliance report + export |
| AI-64 | Time Machine restore UX |
| AI-65 | Terminal Companion v2 |
| AI-66 | AI settings sync + E2E |
| AI-67 | Autopilot run (guarded batch) |
| AI-68 | Compliance HTML/PDF export |
| AI-69 | Terminal command suggestions |
| AI-70 | Autopilot dashboard |
| AI-71 | v4 E2E + docs |
| AI-72 | Copilot SSE streaming |
| AI-73 | Compliance PDF export |
| AI-74 | Scheduled Autopilot cron |
| AI-75 | v5 UI polish |
| AI-76 | v5 E2E + docs |
| AI-77 | Autopilot history API |
| AI-78 | Cost Guardian CFO CSV |
| AI-79 | Dashboard schedule status |
| AI-80 | Reports history + CSV UI |
| AI-81 | v6 E2E + docs |
| AI-82 | NL create VM Spotlight + wizard prefill |
| AI-83 | Capacity CSV export |
| AI-84 | Mission Control AI strip |
| AI-85 | Copilot quick chips |
| AI-86 | Reports capacity CSV |
| AI-87 | v7 E2E + docs |
| AI-88 | Machina Zeus OS vision + product positioning |
| AI-89 | Digital Twin graph + impact simulation |
| AI-90 | Intent-based environment planner (NL) |
| AI-91 | AI SRE exhaustion forecasts |
| AI-92 | AI Root Cause + infrastructure timeline |
| AI-93 | Mission Control Zeus OS + Digital Twin UI |
| AI-94 | FinOps next-month cost prediction |
| AI-95 | Phase 9 E2E + docs |
| AI-96 | Fleet heat map (hot/cold/power waste) |
| AI-97 | Fleet rebalance proposals (DRS-class) |
| AI-98 | Security graph (users/hosts/VMs/networks/keys) |
| AI-99 | Attack path analysis |
| AI-100 | Infrastructure knowledge search |
| AI-101 | Service graph (VM → service → DB) |
| AI-102 | Infrastructure memory (incident recall) |
| AI-103 | AI Mission Stack (GPU/K8s/inference preview) |
| AI-104 | Bare metal inventory + capacity plan |
| AI-105 | Phase 10 E2E + Machina Zeus OS hub UI |
| AI-106 | Digital Twin migrate + network blast simulation |
| AI-107 | FinOps per-team/project cost attribution |
| AI-108 | Fleet rebalance execute (preview + admin enqueue) |
| AI-109 | Compliance frameworks (CIS/PCI/SOC2/HIPAA) |
| AI-110 | Bare metal BMC power lifecycle (preview) |
| AI-111 | Phase 11 UI — attribution, twin sim, Zeus OS polish |
| AI-112 | Spotlight intents for phase 11 features |
| AI-113 | Phase 11 E2E + docs |
| AI-114 | Mission stack execute (GPU VM preview + admin enqueue) |
| AI-115 | FinOps chargeback CSV export |
| AI-116 | Fleet GPU / NUMA placement advisor |
| AI-117 | Knowledge NL diagnose ("why is X slow") |
| AI-118 | Service blast-radius impact simulation |
| AI-119 | Infrastructure memory similar-incident recall |
| AI-120 | Phase 12 UI — Mission Control execute, Reports CSV, Zeus OS |
| AI-121 | Phase 12 E2E + docs |
| AI-122 | Environment intent execute (preview + admin VM enqueue) |
| AI-123 | SRE remediation proposals from forecasts |
| AI-124 | Compliance framework remediation bridge |
| AI-125 | Zeus OS unified summary API |
| AI-126 | Fleet power / carbon waste optimizer |
| AI-127 | Bare metal PXE provision preview |
| AI-128 | Phase 13 UI — environment planner, Zeus summary strip |
| AI-129 | Phase 13 E2E + docs |
| AI-130 | Unified remediation hub (SRE + compliance + fleet power) |
| AI-131 | Knowledge → runbook bridge (diagnose + operator steps) |
| AI-132 | FinOps budget guard (spend vs budget alerts) |
| AI-133 | Mission stack status tracker (GPU + environment VMs) |
| AI-134 | Digital twin storage pool drain simulation |
| AI-135 | Phase 14 UI — hub, runbook, budget, stack status, Zeus strip |
| AI-136 | Phase 14 Spotlight intents |
| AI-137 | Phase 14 E2E + docs |
| AI-138 | **Infrastructure AI WS1** — Unified graph brain (`GET /ai/graph`, path, query) |
| AI-139 | **Infrastructure AI WS2** — Incident RCA + evidence (`/ai/incidents/analyze`) |
| AI-140 | **Infrastructure AI WS3** — Infrastructure memory + pre-outage delta |
| AI-141 | **Infrastructure AI WS4** — Digital twin what-if simulation |
| AI-142 | **Infrastructure AI WS5** — VM troubleshoot / doctor panel |
| AI-143 | **Infrastructure AI WS6** — Explain object + rightsizing FinOps |
| AI-144 | **Infrastructure AI WS7** — Unified failure predictions + auto-alerts |
| AI-145 | **Infrastructure AI WS8** — Incident commander + war room |
| AI-146 | **Infrastructure AI WS9** — Time Machine graph-at-timestamp + timeline replay |
| AI-147 | **Infrastructure AI WS10** — NL ops (`/ai/nl-ops`, dry-run default) |

> **AI-138–147 realignment:** IDs now map to the [Zeus Infrastructure AI Program](zeus-infra-ai-program.md) (10 workstreams). Former GuestKit + early Zeus Firewall batch labels for this numeric range live under Phase 16+ in [`zeus-os-ai-172-371.md`](zeus-os-ai-172-371.md).

| AI-148 | Firewall timeline + audit integration |
| AI-149 | VM `firewall_profile` template wiring |
| AI-150 | AI explain + secure-machine plan |
| AI-151 | Simulation + drift detection |
| AI-152 | Stealth mode + Emergency lockdown |
| AI-153 | K8s NetworkPolicy + Cilium read adapters |
| AI-154 | PacketWolf bridge + activity UI |
| AI-155 | Traffic-rule correlation recommendations |
| AI-156 | Migration advisor firewall dependencies |
| AI-157 | Risky change approval workflow |
| AI-158 | MachineFirewallPolicy GitOps export/sync |
| AI-159 | Compliance reports (production, SSH, DB, drift) |
| AI-160 | SIEM export hooks on firewall timeline |
| AI-161 | Template firewall profiles + remediate hub firewall items |

### Phase 15 — Zeus Firewall hardening + PacketWolf live (AI-162–171)

| Batch | Deliverable |
|-------|-------------|
| AI-162 | PacketWolf live REST bridge (`/api/v1/flows`, stats) + `PACKETWOLF_API_KEY` |
| AI-163 | Firewall drift auto-sync on `host.inventory` + timeline events |
| AI-164 | `firewall_approvals` table + list/approve/reject APIs |
| AI-165 | MachineFirewallPolicy GitOps export + sync (`/policies/gitops/*`) |
| AI-166 | Zeus summary `firewall_drift_hosts` + compliance approvals UI |
| AI-167 | E2E retry for flaky fleet heatmap + Phase 15 smoke |
| AI-168 | Spotlight intents — firewall approvals + GitOps |
| AI-169 | Guest QEMU-agent port enrichment via `GetGuestFirewallPorts` |
| AI-170 | Cloud security group read adapters (AWS/Azure/GCP CLI) |
| AI-171 | Phase 15 E2E + docs |

### Phases 16–25 — next 200 batches (AI-172–371)

Full batch table: [`zeus-os-ai-172-371.md`](zeus-os-ai-172-371.md)

| Phase | AI range | Theme | Status |
|-------|----------|--------|--------|
| 16 | 172–191 | K8s NetworkPolicy/Cilium apply + GitOps operator | **Shipped (v1)** |
| 17 | 192–211 | PacketWolf deep — anomalies, correlation | **Shipped (v1)** |
| 18 | PW-1–PW-9 | **PacketWolf Zeus Security Fabric** — Tetragon ingest, Security Center, machine tabs, AI copilot | **Shipped (v1)** |
| 19 | PW-10–PW-12 | **PacketWolf Phase 3** — threat correlation, fleet timeline, alert sync, threat hunting, OpenSearch hook | **Shipped (v1)** |
| 20 | PW-13–PW-15 | **PacketWolf Phase 4** — K8s container hierarchy, Tetragon Helm enrollment task | **Shipped (v1)** |
| 21 | PW-16–PW-18 | **PacketWolf Phase 5** — LLM-backed explain, attack reconstruct, NL search, hunt summary | **Shipped (v1)** |
| 22 | PW-19–PW-21 | **PacketWolf Phase 6** — runtime eBPF enforcement (deny process/DNS/port/IP) | **Shipped (v1)** |
| 23 | PW-22–PW-24 | **PacketWolf Phase 7** — ClickHouse persistence, agent TracingPolicy bundle pull | **Shipped (v1)** |
| 24 | PW-25–PW-27 | **PacketWolf Phase 8** — agent-side TracingPolicy apply, fabric status, inventory sync | **Shipped (v1)** |
| 25 | PW-28–PW-30 | **PacketWolf Phase 9** — production Tetragon install (systemd + export), K8s Helm apply | **Shipped (v1)** |
| 26 | PW-31–PW-33 | **PacketWolf Phase 10** — K8s Tetragon → PacketWolf export forwarder | **Shipped (v1)** |
| 27 | PW-34–PW-36 | **PacketWolf Phase 11** — OpenSearch hunt playbooks, merged search, fabric health | **Shipped (v1)** |
| 27b | PW-37–PW-42 | **PacketWolf Phase 12** — enforcement lifecycle, new policy kinds, fleet Tetragon, threat→action bridges | **Shipped (v1)** |
| 28 | 212–231 | Cloud edge — AWS/Azure/GCP SG read | **Shipped (v1)** |
| 19 | 232–251 | Guest in-guest QEMU-agent port scan | **Shipped (v1)** |
| 20 | 252–271 | Connectivity matrix simulation GA | **Shipped (v1)** |
| 21 | 272–291 | Enterprise — PDF export, approval SLA | **Shipped (v1)** |
| 22 | 292–311 | FinOps × Security exposure cost | **Shipped (v1)** |
| 23 | 312–331 | Bare metal + firewall profiles | **Shipped (v1)** |
| 24 | 332–351 | Multi-site federated policy | **Shipped (v1)** |
| 25 | 352–371 | AI operator autonomous secure-machine | **Shipped (v1)** |
| 26 | 392–411 | NSX-class overlays + micro-segmentation | **Shipped (v1)** |
| 27 | 412–431 | Storage tiers + backup SLA stubs | **Shipped (v1)** |
| 28 | 432–451 | Vault/MFA inventory + air-gap bundles | **Shipped (v1)** |
| 29 | 452–471 | Operations runbooks + compliance showback | **Shipped (v1)** |
| 30 | 472–491 | Developer ecosystem — SDK + Terraform GA schemas | **Shipped (v1)** |
| 31 | 492–511 | Observability — tracing, SLO dashboards, runbook scheduler | **Shipped (v1)** |
| 32 | 512–531 | Enterprise hardening — Vault sync, MFA compliance, FIPS, tenants | **Shipped (v1)** |
| 33 | 532–541 | Host Linux OS lift — agent RPC, host MacSettingsPane, guest VM panes | **Shipped (v1)** |
| 34 | 542–551 | Host/VM AI diagnose + Fix It — Copilot context, Spotlight intents | **Shipped (v1)** |
| 35 | 552–561 | Fleet desktop shell — menu bar, dock, `GET /fleet/desktop` | **Shipped (v1)** |
| 36 | 562–571 | Fleet linux-health rollup + fleet AI diagnose | **Shipped (v1)** |
| 37 | 572–581 | Fleet Activity Monitor — VMs + host Linux PSI tabs | **Shipped (v1)** |
| 38 | 582–591 | Time Machine fleet — backup rollup + day-grouped timeline | **Shipped (v1)** |
| 39 | 592–601 | Finder — smart folders + tag/project sidebar | **Shipped (v1)** |
| 40 | 602–611 | Network — System Settings pane + Network Lens tab | **Shipped (v1)** |
| 41 | 612–621 | Disk Utility — storage pool health rings + SMART rollup | **Shipped (v1)** |
| 42 | 622–631 | Console — unified fleet log tail | **Shipped (v1)** |
| 43 | 632–641 | Software Update — host patch catalog | **Shipped (v1)** |
| 44 | 642–651 | Keychain — secrets inventory Mac pane | **Shipped (v1)** |
| 45 | 652–661 | Users & Groups — tenant switcher menu bar | **Shipped (v1)** |
| 46 | 662–671 | Shortcuts — blueprint Launchpad grid | **Shipped (v1)** |
| 47 | 672–681 | Stage Manager — workspace spaces strip | **Shipped (v1)** |

### Horizon — macOS OS Manager (Phases 48–237)

Full **200-phase** macOS metaphor map: [`machina-macos-os-manager-roadmap.md`](machina-macos-os-manager-roadmap.md).

| Phase | AI range | macOS app | Theme |
|-------|----------|-----------|--------|
| 48–237 | 682–2581 | All macOS layers | 20 layers × ~10 phases each |

See [`enterprise-backlog.md`](enterprise-backlog.md) for explicit deferrals.
