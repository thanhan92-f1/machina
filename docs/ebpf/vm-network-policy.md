# VM network policy

Decide which VM may talk to which, in both directions, with the
**CiliumNetworkPolicy schema**. Policies select VMs by label and compile into
the VM edge programs `machina-bpfd` attaches to each VM tap. Cilium does not
need to be installed: libvirt VMs are enforced by Machina's own eBPF. Where
Cilium is present (for example on a Kubernetes node that is also a
hypervisor), it keeps enforcing its own endpoints and Machina enforces the VMs.

| Where | What |
|---|---|
| Web UI | Platform → Security → **VM Network Policies** (`/platform/zyra/security/network-policies`): policies, YAML editor with dry-run preview and templates, policy tester, endpoints / selectors, live **Flows** terminal. Scope switch: *This host* (daemon) or *Fleet* (controller). VM detail → **Network** tab: labels, enforcement state, live flows for that VM. |
| CLI | `machinactl netpol …`, `machinactl flow …`, `machinactl vm label …` (add `--fleet` to go through the controller); `scripts/platformctl netpol|flow|label` talks to the controller directly. |
| Daemon | `/api/v1/vm-network-policies*`, `/api/v1/flows`, `/api/v1/flows/stream`, `/api/v1/vms/{name}/labels` |
| Controller | Same paths; labels are `/api/v1/vms/{id}/labels`. Adds `PUT /vm-network-policies/{name}/enabled` and `POST /vm-network-policies/sync`. |

## Policy documents

Accepted kinds: `CiliumNetworkPolicy` and `CiliumClusterwideNetworkPolicy`
(`apiVersion: cilium.io/v2`) and Machina's own `VmNetworkPolicy`. A request
can carry several YAML documents separated by `---`, a JSON array, or a
`kind: List`. `spec` and `specs` both work. `metadata.namespace` is ignored:
VMs have no namespaces.

```yaml
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: db-from-web
spec:
  description: Only web VMs reach the database, on 5432/TCP
  endpointSelector:
    matchLabels:
      app: db
  ingress:
    - fromEndpoints:
        - matchLabels:
            app: web
      toPorts:
        - ports:
            - port: "5432"
              protocol: TCP
```

Validation is strict, and errors carry Cilium-style paths such as
`document[0].spec.ingress[0].toPorts[0].ports[0].endPort`.

### Supported fields (enforced)

| Field | Notes |
|---|---|
| `endpointSelector` | `matchLabels` and `matchExpressions` (`In`, `NotIn`, `Exists`, `DoesNotExist`). `{}` selects every VM. Key prefixes `k8s:`, `any:`, `machina:` are stripped; `reserved:host` / `reserved:world` and similar become entities. |
| `ingress`, `egress` | Allow rules. |
| `ingressDeny`, `egressDeny` | Deny rules. **Deny wins over allow**, and it applies even to VMs that are not otherwise isolated. |
| `fromEndpoints` / `toEndpoints` | VM selectors, on any host in scope. |
| `fromRequires` / `toRequires` | Narrows every allow peer in the same spec: the peer must also match. |
| `fromCIDR` / `toCIDR`, `fromCIDRSet` / `toCIDRSet` | IPv4/IPv6 prefixes; `except` carves out narrower prefixes. |
| `fromEntities` / `toEntities` | `all`, `world`, `world-ipv4`, `world-ipv6`, `unmanaged`, `host`, `remote-node`, `cluster` (alias `fleet`: every VM). `health`, `init`, `kube-apiserver` and `ingress` match nothing on a hypervisor (a warning says so). |
| `toPorts[].ports[]` | `port` (number or **named port**), `endPort` (ranges up to 256 ports), `protocol` `TCP` / `UDP` / `SCTP` / `ANY`. |
| `icmps[].fields[]` | `type` as a number or Cilium name (`EchoRequest`, `DestinationUnreachable`, …), `family` `IPv4` / `IPv6`. |
| `toFQDNs` | `matchName` (exact) and `matchPattern` (Cilium wildcards: `*` stays within one label, a lone `*` matches every name, a leading `**.` matches one or more labels). Allow rules only, as in Cilium; `toFQDNs` in `egressDeny` is rejected. See [DNS names](#dns-names-tofqdns). |
| `enableDefaultDeny` | `ingress: false` / `egress: false` keeps a direction open even when the spec has rules for it (additive policies). |
| `nodeSelector` (CCNP) | Accepted; host policies are not applied to VMs (warning). |

Named ports come from VM labels: `machina.io/port.<name>=<number>` (for
example `machina.io/port.http=8080`).

### Accepted, not enforced yet

`toPorts[].rules` (HTTP / Kafka / DNS L7), `toPorts[].serverNames`,
`originatingTLS` / `terminatingTLS`, `toServices`, `toGroups`,
`authentication`. Policies containing them validate and apply with a warning
that names the field; the L3/L4 parts of the rule are still enforced. These
are the next phase and will be native too (an in-process L7 proxy), not
delegated to Cilium or Envoy.

## DNS names (toFQDNs)

```yaml
egress:
  - toEntities: [world]
    toPorts: [{ports: [{port: "53", protocol: UDP}]}]
  - toFQDNs:
      - matchName: github.com
      - matchPattern: "*.githubusercontent.com"
    toPorts: [{ports: [{port: "443", protocol: TCP}]}]
```

Enforced natively by snooping DNS, without a DNS proxy:

1. The VM edge program on the tap of every VM with a `toFQDNs` rule copies
   UDP DNS replies (source port 53) to bpfd. The copy goes to the
   `DNS_EVENTS` ring buffer, the same one the host DNS log uses.
2. bpfd parses each reply. If the query name or any name in its CNAME chain
   matches a `toFQDNs` pattern, bpfd keeps the A/AAAA addresses. Each one is
   held for the record TTL, but at least 10 minutes and at most a day,
   because clients cache answers longer than the TTL says. The cache holds
   up to 8192 addresses.
3. A learned address that isn't a VM, the host or a policy CIDR gets its own
   identity, as if it were a /128 CIDR peer. That identity first inherits
   every rule of the identity the address had before: its longest policy
   CIDR, or `world`. So `egressDeny` towards `world` or a CIDR still wins.
   It then gets allow entries for each `toFQDNs` rule whose pattern matches
   one of its names. Bindings are global, as in Cilium: any VM whose rule
   selects the name may use the address, whichever VM looked it up.
4. When the names expire or the rule goes away, the identity and its entries
   are removed.

Things to know:

- Policy has to allow the lookup itself (UDP 53 to the resolver), as in the
  example. Otherwise the reply never arrives and nothing is learned.
- Learning is asynchronous. The first SYN sent right after the reply can
  race it and be dropped; the TCP retransmit a second later goes through.
- DNS over TCP, DoT and DoH aren't snooped.
- `machinactl netpol fqdn` (like `cilium fqdn cache list`), the UI
  **Endpoints → DNS names** table and `GET /vm-network-policies/fqdn-cache`
  list the learned names. `machinactl netpol test --to api.example.com`
  evaluates a name against `toFQDNs` rules.

## Semantics

These follow Cilium:

- **No policy, no change.** A VM that no policy selects keeps talking to
  everything.
- **Default deny per direction.** Once any spec that selects a VM has an
  `ingress` or `ingressDeny` section, that VM is default-deny for ingress, and
  the same for egress. Traffic the rules don't allow is dropped.
- **Allow rules add up** across every policy that selects the VM.
- **Deny wins.** A matching deny beats any allow.
- A rule with only `toPorts` allows those ports from or to **any** peer. An
  empty rule `{}` allows **nothing**: it only turns default deny on.
- A connection must pass **egress at the source VM and ingress at the
  destination VM**, which may be on different hosts.
- Return traffic of an allowed connection is always allowed (conntrack, with
  ICMP echo tracked by identifier). DHCP and IPv6 link-local / neighbour
  discovery always pass.

## Labels

VM labels use Kubernetes syntax (`[prefix/]name`, value up to 63 characters).

- Daemon: stored in `/var/lib/machina/vm-labels.json`.
- Controller: the `vms.labels` column (migration 029). The migration copies
  any `key=value` tags into labels.
- VMs without labels fall back to their `key=value` tags.
- Reserved labels are added automatically: `machina.io/vm-name`,
  `machina.io/host` and `machina.io/project`.

```bash
machinactl vm label web-1 app=web tier=frontend   # add / overwrite
machinactl vm label web-1 tier-                   # remove
machinactl vm label web-1                         # show
```

## How it is enforced

1. **Compile.** The compiler in `machina-bpf` (`bpf/machina-bpf/src/netpol/`)
   turns the policies and the VM inventory into a `VmEdgeState`:
   - every VM gets a stable 32-bit identity (an FNV hash of its name);
   - CIDR peers, the host and other hypervisors get identities too;
   - rules are keyed (subject identity, peer identity, direction, protocol,
     port), with a deny flag and a source string such as
     `db-from-web spec.ingress[0]` for flow attribution.
2. **Push.**
   - Single host: the daemon compiles and pushes on every change, on VM
     start/stop, and every 60 s, because guest addresses change with DHCP.
   - Fleet: the controller compiles one state per host and pushes it through
     the agent. That happens every 30 s on the leader, only when the state
     changed, plus forced pushes every 10 ticks and `POST …/sync`.
   - Whoever writes the state records itself as the edge **owner**. The
     daemon leaves a controller-owned edge alone, so local policies show as
     inactive.
3. **Datapath.** On each tap, the TCX program looks up the peer address in
   `VM_IPS` (exact match), then `VM_CIDR_IDS` (longest prefix), and otherwise
   treats it as `world`. It then checks `VM_POLICY` with wildcards: any peer,
   any port, any protocol. A deny match wins; a miss on an isolated direction
   is a default-deny drop.

The usual [safety model](README.md#safety-model) applies. Without the
enforcement lease, a drop is recorded as an **AUDIT** flow and the packet is
forwarded. With the lease, it is dropped and recorded as **DROPPED**.
Policies are persisted; the enforce mode and its lease never are.

## Tools (Cilium / Hubble equivalents)

| Cilium | Machina |
|---|---|
| `kubectl apply -f cnp.yaml` | `machinactl netpol apply -f cnp.yaml` (`--dry-run` previews selected VMs and compiled rule count) |
| `cilium policy get` | `machinactl netpol get [-o yaml\|json\|wide]` |
| `cilium policy trace` | `machinactl netpol test --from web-1 --to db-1 --port 5432` — evaluates egress at the source and ingress at the destination and names the matching rule |
| `cilium policy selectors` | `machinactl netpol selectors` |
| `cilium endpoint list` | `machinactl netpol endpoints` |
| `cilium status` | `machinactl netpol status` |
| `cilium fqdn cache list` | `machinactl netpol fqdn` |
| `hubble observe` | `machinactl flow observe [-f] [--vm X] [--verdict DROPPED] [--port 443] …` |
| — | `machinactl flow top --by pair\|src\|dst\|port\|policy`, `machinactl flow stats` |

Auth: `MACHINA_API_TOKEN`, or `MACHINA_USER` + `MACHINA_PASS`. With a user
and password, the CLI keeps its session in `~/.machina/cli-session` (mode
0600; override with `MACHINA_COOKIE_JAR`) and logs in again only when that
session expires, because `/auth/login` is rate limited.

## Flows

Every new connection on a policy-managed tap emits a flow event; drops and
audits are rate-limited to one per flow per second. Each flow carries:

- source and destination VM name, address, port, identity and labels;
- protocol, TCP flags or ICMP type, and size;
- verdict `FORWARDED`, `DROPPED` or `AUDIT`, with the drop reason
  (`policy-deny` or `default-deny`);
- the policy rule that decided it.

bpfd keeps the last 5000 flows. The controller merges flows from every host
and tags each one with its host.

Filters, the same in the API (query string), the CLI (flags) and the UI
terminal prompt: `vm`, `from_vm`, `to_vm`, `label` (`k=v`), `ip`, `cidr`,
`port`, `protocol`, `verdict` (comma list), `drop_reason`, `policy`
(substring), `direction`, `host` (fleet).

`machinactl flow observe` colours its output on a TTY; set `NO_COLOR` or
pass `--color never` to turn that off. `-o json` prints one record per line.
The UI **Flows** tab is a black macOS-style terminal that takes the same flags
at its prompt; Ctrl-C pauses it and Ctrl-L clears it.

## Test

`scripts/bpf/vm-edge-smoke.sh` has a *VM network policy* section. It runs on
a veth pair in a scratch netns and covers:

- identity peers;
- port ranges and ICMP type rules;
- deny over allow;
- CIDR longest-prefix match;
- `ingressDeny` without isolation;
- AUDIT, FORWARDED and DROPPED flow events with rule attribution;
- `toFQDNs`: a stub resolver on the host answers one name. The checks cover
  blocking before the lookup, learning from the reply, allowing after it,
  ignoring NXDOMAIN and unmatched names, `world` deny still winning, and
  removing the rule.

Compiler and tracer unit tests: `cargo test -p machina-bpf netpol`.
