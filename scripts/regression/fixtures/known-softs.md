# Known intermittent SOFT pages (short/empty body under hydrate race)

Observed during continuous page sweeps (Chrome CDP). These are **not** hard failures
(crashes / route-not-found). Re-run usually passes.

| Path | Notes |
|------|--------|
| `/platform/hosts/finder` | Machine Finder hydrate race |
| `/platform/zeus/security/hunt` | Zeus hunt panel slow hydrate |
| `/platform/settings` | Settings shell slow hydrate |
| `/capabilities` | Classic capabilities empty briefly |
| `/services` | Classic services empty briefly |
| `/k8s` | K8s shell slow hydrate |
| `/openstack/instances` | OpenStack list hydrate |
| `/openstack/load-balancers` | OpenStack LB hydrate |
| `/openstack/keypairs` | OpenStack keypairs hydrate |
| `/openstack/floating-ips` | OpenStack FIP hydrate |
| `/vms/chrome-e2e-vm/consolehub` | ConsoleHub shell race |

Hard fail criteria for page-sweep: body matches `Something went wrong` or `Route not found`.
