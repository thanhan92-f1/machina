# Fleet Cloud: features OpenStack doesn't have

Fleet Cloud covers the usual self-service primitives (flavors, images,
instances, volumes, security groups, stacks, load balancers). This page
describes what goes past them. Each feature runs on the controller and the
host agents that are already deployed; none needs an extra service.

## Scale to zero

An idle VM can sleep: its memory is saved to disk (libvirt managed save),
the host gets its RAM and vCPUs back, and the first packet sent to one of
its addresses restores it. A client's TCP connection usually just takes a
few seconds longer, because the kernel retransmits the SYN while the VM is
restored. OpenStack's closest equivalent, shelve, has to be undone by hand
and takes minutes.

### How it works

1. **Sleep.** `POST /api/v1/vms/{id}/sleep` (or auto-sleep, below) sends a
   `managedsave` power action to the agent. Once the save succeeds, the
   controller sets `desired_state = 'sleeping'` and `lifecycle_phase =
   'sleeping'`. Reconcile never restarts a sleeping VM.
2. **Wake set.** The controller pushes every sleeping VM on a host, with
   its guest addresses, to that host's `machina-bpfd` (`vm_wake_set`).
   bpfd installs two nftables tables called `machina_wake`, which count
   packets without changing them:
   - `inet machina_wake` uses the forward and output hooks. It sees routed
     traffic, port-forwarded traffic (floating IPs), overlay traffic after
     DNAT, and connections the host opens itself.
   - `bridge machina_wake` sees ARP requests for the VM's IPv4 address
     from other VMs on the same bridge.

   bpfd also pins the host's neighbour entry for each sleeping address
   (`nud permanent`, with the VM's MAC). Without the pin, the host's ARP
   for a VM that can't answer fails after about 3 s and the sender gets
   "no route to host" before the restore finishes. The pin goes back to
   `stale` when the VM leaves the wake set.
3. **Wake.** While anything is asleep, bpfd reads the counters every
   250 ms. When a counter moves, it publishes a `vm_wake` event. It repeats
   the event every 3 s for up to a minute, so a subscriber that fell behind
   still gets it. The agent subscribes to `vm_wake` and restores the domain
   (`virDomainCreate` on the managed save). It only acts when the domain is
   off *and* has a managed save, so an out-of-date wake set can never boot
   a VM that someone shut down on purpose.
4. **Back to running.** When inventory reports the VM running while its
   desired state is still `sleeping`, the controller sets it back to
   `running`, records a `wake (traffic)` event and removes the VM from the
   wake set.

A sleeping VM has no tap, so the tap eBPF programs can't see traffic for
it. That is why the hook is in nftables and not on the tap.

### Auto-sleep

A VM is idle when guest CPU stays below 3 % and its NICs move less than
2 KiB/s. ARP, NTP and DHCP chatter stays below that. Every inventory sample
that shows activity stamps `last_active_at`. Once a VM has been idle for
its policy's number of minutes, the controller sends it to sleep. The idle
check runs every 30 s.

Policy, from most to least specific:

| Setting | Meaning |
|---|---|
| VM `sleep_after_minutes = N` (5–10080) | Sleep after N idle minutes |
| VM `sleep_after_minutes = 0` | Never auto-sleep, whatever the project says |
| VM `sleep_after_minutes = null` | Use the project default |
| Project default | `PUT /api/v1/sleep/policies/{project}` |

The controller only auto-sleeps a VM whose guest address it knows (from the
guest agent, the host's ARP and NDP tables, or tap traffic). Without one,
traffic couldn't wake it.

### API

| Route | |
|---|---|
| `POST /api/v1/vms/{id}/sleep` | Sleep now (the VM must be running) |
| `POST /api/v1/vms/{id}/wake` | Restore now |
| `GET/PUT /api/v1/vms/{id}/sleep-policy` | `{ "sleep_after_minutes": 30 \| 0 \| null }`; the response also has idle minutes, whether traffic can wake the VM, and its last 20 sleep/wake events |
| `GET /api/v1/sleep/policies`, `PUT/DELETE /api/v1/sleep/policies/{project}` | Project defaults |
| `GET /api/v1/sleep/summary` | Sleeping VMs, RAM and vCPUs handed back, sleeps and wakes in the last 24 h |

bpfd (local socket): `vm_wake_set {config: {entries: [{vm, addresses}]}}`,
`vm_wake_status`, and the `vm_wake` stream topic (`{vm, address, at}`).

### CLI and UI

```bash
machinactl vm sleep web-1
machinactl vm wake web-1
machinactl vm sleep-policy web-1 30          # or: inherit | never
machinactl vm sleep-policy --project dev 60  # project default (never | clear)
machinactl vm sleeping                       # what's asleep, RAM handed back
```

The Fleet Cloud instance list has a **Sleeping** filter and Sleep/Wake
buttons. The instance detail page has a **Scale to zero** card for the
policy, idle time and history.

### Limits

- On the same bridge, IPv6 neighbour solicitations don't wake the VM yet.
  Routed IPv6 traffic does.
- Wake-on-traffic needs the wake hook on the VM's own host. The overlay
  covers this: traffic from other hosts is DNATed on the VM's host and
  goes through the forward hook there.
- Anything sent to the address wakes the VM, including a monitoring probe
  or the retransmissions of a TCP connection that was open when the VM went
  to sleep. Leave auto-sleep off (`0`) for VMs that are probed constantly.
- The save image is as large as the guest's RAM and is written to
  libvirt's managed-save directory (`/var/lib/libvirt/qemu/save`).

### Tests

- `scripts/bpf/vm-edge-smoke.sh`: the wake tables on veth/netns. A packet
  sent by the host, a packet routed from another namespace and an ARP
  request from a bridge peer each publish `vm_wake`. An address claimed by
  two VMs is rejected, and an empty set removes the tables.
- `scripts/bpf/vm-netpol-realvm.sh`, "fleet: scale to zero": sleeps a real
  VM, checks the managed save and the wake set, wakes it with an HTTP
  request from the host and again from the other VM, and checks that the
  controller returns it to `running`. It also checks that the host's
  neighbour entry is pinned while the VM sleeps and released on wake.

## Time travel

Every instance has a timeline of restore points. You can fork a running
VM into a second VM in seconds without stopping it, fork from any earlier
point, or rewind the VM to a point. A fork can also copy the source's RAM,
so the copy carries on from the same running processes on an isolated
network: a safe place to debug a live incident. OpenStack snapshots are
full image uploads to Glance, and a server can't be rewound in place.

### How it works

- **Restore point.** The agent takes an external, disk-only snapshot of
  every disk at once, with the guest's filesystems frozen through the guest
  agent when it is installed. The current disk file becomes a read-only
  layer and the VM keeps running on a new overlay
  (`{vm}-{disk}.rp-<time>-<id>.qcow2` next to the original). Nothing is
  copied, so it takes about a second whatever the disk size.
- **Fork.** The fork gets its own thin overlays on the source's frozen
  layers and a new domain: a new name, UUID and MACs. By default it also
  gets a new cloud-init seed, so it boots with its own instance-id,
  hostname and machine-id, and requests DHCP with its MAC as the client
  identifier so it never claims the source's lease. Forking "now" takes a
  restore point of the source first.
- **Memory fork.** The agent takes an external snapshot that includes
  memory, rewrites the domain XML inside the save image (new name and UUID,
  disks on the fork's overlays) and restores it as the fork. The fork keeps
  the source's MACs, because its running kernel still uses them, so it is
  attached to `machina-fork`, a libvirt network with no uplink and no
  addresses. Reach it through the console or the guest agent.
- **Rewind.** The VM is stopped, its overlays newer than the point are
  deleted, and it restarts on a fresh overlay on the point's layers. Later
  restore points go with them. A rewind is refused while a fork depends on
  a later point; detach the fork first.
- **Detach.** Copies the source's layers into the fork (a live block pull,
  or `qemu-img rebase` when the fork is off), so the fork no longer depends
  on the source.
- **Scheduled points.** With a schedule, the controller takes a point every
  N minutes and keeps the newest K. Older points are merged away with a
  live block commit, so the backing chain stays the same length. Merging is
  paused for a VM while it has forks, because they sit on its layers.

### API

| Route | |
|---|---|
| `GET /api/v1/vms/{id}/restore-points` | Points (oldest first), schedule, forks of this VM and what it was forked from |
| `POST /api/v1/vms/{id}/restore-points` | `{ "note": "before upgrade" }`; returns a task |
| `PUT /api/v1/vms/{id}/restore-points/policy` | `{ "every_minutes": 60, "keep": 24 }`; `0` turns the schedule off. Range 5–10080 minutes, keep 1–168 |
| `POST /api/v1/vms/{id}/restore-points/{point}/rewind` | 409 `fork_pins_later_point` if a fork depends on a later point |
| `POST /api/v1/vms/{id}/fork` | `{ "name", "restore_point_id"?, "memory", "isolate", "reseed", "start" }`; memory forks need a running VM and no point |
| `POST /api/v1/vms/{id}/fork/detach` | Called on the fork |

All of them except GET run as tasks (`vm.restore_point`, `vm.rewind`,
`vm.fork`, `vm.fork.detach`).

### CLI and UI

```bash
machinactl vm restore-point web-1 --note "before upgrade" --wait
machinactl vm restore-points web-1                    # list; 1 = oldest, -1 = newest
machinactl vm restore-points web-1 --every 60 --keep 24
machinactl vm fork web-1 web-1-debug --wait           # a copy of web-1 as it is now
machinactl vm fork web-1 web-1-old --at 1 --wait      # from the oldest point
machinactl vm fork web-1 web-1-live --memory --wait   # with RAM, isolated network
machinactl vm rewind web-1 -1 --wait
machinactl vm fork-detach web-1-debug --wait
```

The instance detail page has a **Time travel** card: a slider over the
restore points, Rewind here, Fork now or Fork here, the schedule, and the
VM's forks.

### Limits

- Disks must be file-backed qcow2 or raw. Atlas (RBD) disks and LVM
  volumes aren't supported yet.
- Restore points are on the same storage as the VM; they don't replace
  backups.
- Without the guest agent, points are crash-consistent rather than
  quiesced.
- Deleting a VM keeps its disk files (the existing delete policy), so a
  fork that hasn't been detached keeps working after its source is deleted.
- A memory fork can't join the source's network while the source runs,
  because they share MACs and addresses.

### Tests

- `scripts/fleet/vm-timetravel-realvm.sh` runs a real Ubuntu VM: a
  quiesced point; a live fork with its own MAC, address, hostname and
  machine-id, and writes that stay apart; a refused rewind, detach, then a
  rewind; a memory fork that still has a tmpfs file and can't reach the
  libvirt network; and a 5-minute schedule keeping 2 points while the
  chain doesn't grow.
- `web/e2e/fleet-cloud-time-travel.spec.ts` (mocked): the slider, rewind,
  fork from a point, memory fork, schedule and create.

## Stacks you describe

Describe a stack in a sentence ("3 web servers behind the internet on
443, a postgres they can reach on 5432, nightly backups") and Machina
drafts the template, shows what it would do, and deploys it after an
approval. Once it's running, the controller checks the stack against the
template every minute and puts back anything that drifted. Heat needs a
hand-written HOT template, and it doesn't notice when someone deletes a
server behind its back.

### Template

A stack template has instance groups and policies, alongside the older
`vms` / `volumes` / `security_groups` lists:

```json
{
  "instances": [
    { "name": "web", "count": 3, "flavor": "m1.small", "image": "ubuntu-24.04",
      "anti_affinity": true, "sleep_after_minutes": 30 },
    { "name": "db", "count": 1, "flavor": "m1.medium", "ha": true,
      "restore_points": { "every_minutes": 60, "keep": 24 },
      "backup": { "interval_hours": 24, "retain": 7 } }
  ],
  "policies": [
    { "from": "internet", "to": "web", "ports": [443] },
    { "from": "web", "to": "db", "ports": [5432] }
  ]
}
```

- A group makes `count` VMs named `{stack}-{group}-{i}` (just
  `{stack}-{group}` when `count` is 1), labelled `machina.io/stack` and
  `machina.io/stack-group`. `flavor` or `vcpus` / `memory`, `image` (an
  approved template), `network`, `labels`, `anti_affinity` (spread over
  hosts), `ha`, `sleep_after_minutes` ([scale to zero](#scale-to-zero)),
  `restore_points` ([time travel](#time-travel)) and `backup` are all
  applied when the VMs are made.
- Policies are compiled into one VM network policy per target group
  (`stack-{stack}-{group}`, ingress). `from` is a group, `internet`,
  `fleet`, `host`, `any` or a CIDR.
- Up to 20 VMs per group and 40 per stack.

### Draft, plan, approve

- **Draft.** `POST /api/v1/stacks/draft` sends the description to the
  configured LLM provider with the catalogue (flavors and images). The
  reply is validated; if it's wrong the model gets one repair round with
  the errors. With no provider, or if the model still gets it wrong, a
  rule-based parser drafts it from the words it knows (roles such as web,
  api, worker, db, cache and lb; counts; "backups", "ha", "dev"). The response says which one wrote it.
- **Plan.** `POST /api/v1/stacks/plan` is a dry run: project quota for
  every new VM at once, a placement simulation (memory and anti-affinity
  as the VMs pile up), a monthly cost from the cluster's FinOps rates, the
  compiled policies replayed against the flows the fleet actually saw, and
  for an update the VMs it would create, delete or resize. A plan that
  fails quota, can't place a VM, or has template errors is blocked.
- **Approve.** `POST /api/v1/stacks/propose` files a `stack.deploy`
  action with the plan's summary. Approving it creates or updates the
  stack; the action's verify step waits for the result, and undo deletes
  a new stack or puts the previous template back.

### Drift and healing

Every minute (on the leader), each created stack with groups or policies
is compared with its template: missing VMs, VMs that shouldn't
be there, wrong size, missing labels, missing or changed policies and
backup schedules. With auto-heal on (the default) the controller fixes
what it can: it recreates missing VMs and puts labels, policies and
schedules back. A different size is reported, not fixed,
because it needs a restart. The result is stored on the stack.

An update (`PUT /api/v1/stacks/{id}`) applies the new template the same
way. If it fails, the VMs it added are deleted and the previous template
is applied again.

### API

| Route | |
|---|---|
| `POST /api/v1/stacks/draft` | `{ "prompt", "name"?, "project_id"? }` → template, source (`llm` / `rules`), plan |
| `POST /api/v1/stacks/plan` | `{ "name", "template", "project_id"?, "stack_id"? }`; `stack_id` plans an update |
| `POST /api/v1/stacks/propose` | Same body plus `prompt`; 409 `stack_plan_blocked` with the plan when blocked |
| `PUT /api/v1/stacks/{id}` | `{ "template" }`; applies now (operators) |
| `GET /api/v1/stacks/{id}/drift` | Checks now and returns `{ in_sync, open, items }` |
| `POST /api/v1/stacks/{id}/converge` | Heals now |
| `PUT /api/v1/stacks/{id}/auto-heal` | `{ "enabled": false }` to report only |

### UI

Fleet Cloud → Stacks opens with **Compose stack**: describe it, Draft
plan, edit the template if you want, Propose for approval, then approve.
The plan shows cost, quota, placement, the policy graph and the VMs. Stack
detail has a Drift tab (check, converge, auto-heal) and a Template tab
(plan an update, propose it, or apply now).

### Limits

- Resizes are reported but not applied; stop the VM and resize it.
- Stacks made from the older `vms` list aren't reconciled.
- Deleting a stack deletes its VMs, policies and backup schedules; VM
  disks follow the usual delete policy.

### Tests

- `scripts/fleet/stack-realvm.sh` on real VMs: draft from a sentence,
  plan, a blocked proposal, approve and deploy three VMs with labels and a
  policy, delete a VM and the policy behind the stack's back and watch
  drift report them and converge put them back, scale a group up with an
  update, then undo the deploy.
- Controller unit tests: the template compiler and diff, the rules
  drafter, plan quota/placement/cost, drift on a stack whose VMs are
  missing, a blocked and a queued proposal, and `stack.deploy` undo.
- `web/e2e/fleet-cloud-stacks.spec.ts` (mocked): compose, plan, propose
  and approve; drift, converge and auto-heal on stack detail.
