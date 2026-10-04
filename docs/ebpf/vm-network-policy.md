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
| Daemon | `/api/v1/vm-network-policies*` (including `/fqdn-cache` and `/auth`), `/api/v1/flows`, `/api/v1/flows/stream`, `/api/v1/vms/{name}/labels` |
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
| `toPorts[].rules` | `http` (`method`, `path` as anchored regexes, `host`, `headers`, `headerMatches` with `mismatch: LOG`), `kafka` (`role` or `apiKey`, `apiVersion`, `clientID`, `topic`), `dns` (`matchName` / `matchPattern`). Also on `toFQDNs` rules. See [L7 rules](#l7-rules). |
| `toPorts[].serverNames` | TLS SNI names, matched on the ClientHello. |
| `toGroups` / `fromGroups` | Select `CiliumCIDRGroup` objects (any provider key; there is no cloud API on a hypervisor). See [Groups](#cidr-groups-and-togroups). |
| `cidrGroupRef` | In `toCIDRSet` / `fromCIDRSet`: the prefixes of a named `CiliumCIDRGroup`. |
| `authentication.mode` | `required`, or `test-always-fail`. See [Authentication](#authentication). |
| `enableDefaultDeny` | `ingress: false` / `egress: false` keeps a direction open even when the spec has rules for it (additive policies). |
| `nodeSelector` (CCNP) | Accepted; host policies are not applied to VMs (warning). |

Named ports come from VM labels: `machina.io/port.<name>=<number>` (for
example `machina.io/port.http=8080`).

### Accepted with a warning

- `toServices` matches nothing (there are no Kubernetes services).
- `originatingTLS` / `terminatingTLS`: TLS is not intercepted, so encrypted
  traffic is matched by `serverNames` only.
- `listener` (Envoy) is ignored.
- `headerMatches[].mismatch` other than `LOG` is rejected, because rewriting
  headers needs a terminating proxy.

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

## L7 rules

```yaml
ingress:
  - fromEndpoints: [{matchLabels: {app: web}}]
    toPorts:
      - ports: [{port: "80", protocol: TCP}]
        rules:
          http:
            - method: GET
              path: "/v1/.*"
egress:
  - toEndpoints: [{matchLabels: {app: kafka}}]
    toPorts:
      - ports: [{port: "9092", protocol: TCP}]
        rules: {kafka: [{role: produce, topic: orders}]}
  - toFQDNs: [{matchPattern: "*.example.com"}]
    toPorts: [{ports: [{port: "443", protocol: TCP}], serverNames: [api.example.com]}]
  - toEntities: [world]
    toPorts:
      - ports: [{port: "53", protocol: UDP}]
        rules: {dns: [{matchPattern: "*.example.com"}]}
```

Enforced natively, without Envoy or a proxy in the data path:

1. The L3/L4 match marks the rule entry L7. A connection is allowed as usual,
   but the VM edge program copies the first payload segment of each request
   (up to 4 KiB) to bpfd over the `VM_L7_EVENTS` ring buffer. UDP DNS on an
   L7 port is copied the same way.
2. With the enforcement lease, the edge drops that segment and marks the flow
   pending. bpfd parses it (HTTP/1.x, including pipelined requests; Kafka
   request headers and Produce/Fetch topics; the TLS ClientHello SNI; DNS
   queries) and checks it against every L7 rule that applies to the pair.
3. **Allowed:** bpfd opens a window in the flow's `VM_L7_FLOW` entry, and the
   client's retransmit of the segment passes, along with the rest of that
   request. The next request on a keep-alive connection is checked again.
4. **Denied:** bpfd answers in place of the server: `HTTP/1.1 403 Access
   denied` for HTTP, a TCP reset for Kafka and TLS, `REFUSED` for DNS. It
   resets the server side too, and the flow stays closed.
5. DNS queries that are allowed are forwarded by bpfd from the host, and the
   answer is injected back into the tap, where `toFQDNs` learning sees it.

Each decision is a flow record with the L7 type and request (for example
`GET /v1/users`), shown with ◆ in `machinactl flow observe` and the UI
terminal; `machinactl flow top --by l7` groups them. Without the lease, a
denied request is an **AUDIT** flow and the traffic passes.

Rules add up the way Cilium's do: a request is allowed if any L7 rule for the
pair matches it. If a pair also has a plain L3/L4 allow for the same port,
no L7 check applies.

Limits:

- Holding the first segment costs one TCP retransmission timeout (at least
  200 ms) per checked request under enforcement.
- Only the first 4 KiB of a request is seen. HTTP headers past that, and
  chunked request bodies, are not parsed; the rest of such a request passes.
- TLS is matched on SNI only. After the ClientHello the connection is open.
- `rules.dns` covers DNS over UDP only.
- For a denied egress connection, the reset to the server goes out through the
  tap's bridge. A tap without a bridge leaves the server side to time out.

## CIDR groups and toGroups

```yaml
apiVersion: cilium.io/v2alpha1
kind: CiliumCIDRGroup
metadata:
  name: partner-nets
  labels: {tier: partner, machina.io/region: eu}
  annotations: {machina.io/group-id: sg-123}
spec:
  externalCIDRs: [198.51.100.0/24, "2001:db8::/64"]
```

Apply it like a policy. Its prefixes are then used by:

- `toCIDRSet: [{cidrGroupRef: partner-nets}]` (and `fromCIDRSet`);
- `toGroups` / `fromGroups`. Every provider key (`aws`, `machina`, …) is
  read the same way: `names` and `securityGroupsNames` match the group name,
  `securityGroupsIds` matches the `machina.io/group-id` annotation, `labels`
  match group labels and `region` matches the `machina.io/region` label.

A reference to a group that doesn't exist matches nothing and the dry run
warns. Editing the group recompiles every policy that uses it.

## Authentication

```yaml
ingress:
  - fromEndpoints: [{matchLabels: {app: web}}]
    authentication: {mode: required}
```

Cilium uses SPIFFE identities with mutual TLS. Machina authenticates against
its own inventory instead:

- The first packet of a new connection on a rule with `authentication` needs
  an entry for the pair in the `VM_AUTH` map. Without one it is dropped with
  reason `auth-required` (with the lease) or audited, and bpfd is asked to
  authenticate the pair.
- bpfd accepts a pair when the peer is a VM identity it knows on this host or
  elsewhere in the fleet. The TCP retransmit after authentication passes.
  Entries last an hour; after that, the next new connection triggers
  authentication again.
- `test-always-fail` never authenticates (reason `auth-test-always-fail`), as
  in Cilium.
- **Source guard.** While any authentication rule exists, every tap checks
  that a packet from its VM carries a source address that bpfd maps to that
  VM, or to no identity at all. A VM sending as another VM's address is
  dropped as `spoofed-source`, so identities can't be borrowed.

`machinactl netpol auth` and `GET /vm-network-policies/auth` list the
authenticated pairs, and `machinactl netpol test` shows ⚿ on rules that need
authentication.

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
   is a default-deny drop. An allow entry can also require authentication
   (checked against `VM_AUTH` for new connections) or L7 (the request's first
   segment goes to bpfd, see [L7 rules](#l7-rules)).

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
| `cilium policy trace` with L7 | `machinactl netpol test … --http-method GET --path /v1/x` (also `--host`, `--header`, `--sni`, `--dns-name`, `--kafka-api-key`, `--kafka-topic`, `--kafka-client-id`, `--kafka-api-version`) |
| `cilium-dbg bpf auth list` | `machinactl netpol auth` |
| `hubble observe` | `machinactl flow observe [-f] [--vm X] [--verdict DROPPED] [--port 443] …` |
| — | `machinactl flow top --by pair\|src\|dst\|port\|policy\|l7`, `machinactl flow stats` |

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
  (`policy-deny`, `default-deny`, `l7-deny`, `auth-required`,
  `auth-test-always-fail` or `spoofed-source`);
- the policy rule that decided it;
- for L7 decisions, the request type and summary.

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
- L7: HTTP allow, 403 and keep-alive requests; TLS SNI with `openssl`; Kafka
  Produce to an allowed and a denied topic; DNS forwarded and REFUSED; AUDIT
  in observe mode.
- Authentication: `test-always-fail`, `required` against a fleet peer, the
  auth table, and the source guard with and without the lease.

Compiler and tracer unit tests: `cargo test -p machina-bpf netpol`.
