# Minimal OpenStack for Machina (Keystone-only POC)

For UI and wiring tests you only need **Keystone** answering on port **5000**. Full Packstack (Nova, Neutron, Horizon, Swift, …) is not required to validate Machina’s OpenStack integration phases.

## What Machina checks

| Check | Needs |
|-------|--------|
| `platform-info` → OpenStack configured | `[openstack] enabled`, `clouds.yaml` or inline auth in `config.toml` |
| `GET /openstack/status` → `reachable: true` | Keystone identity auth works |
| `compute_reachable` / `glance_reachable` | Nova `list_servers` / Glance `list_images` succeed |
| Instances / create / Glance upload | Nova, Neutron, Glance (full cloud) |

A **Keystone-only** host shows **live** in the UI (Hero, Dashboard) with Nova/Glance marked off in the status bar. Instance and Glance upload flows require `compute_reachable` / `glance_reachable`.

## Minimal install (CentOS Stream 9 + Caracal)

On the hypervisor (as root):

```bash
dnf config-manager --set-enabled crb
dnf install -y centos-release-openstack-caracal
dnf install -y mariadb-server httpd openstack-keystone python3-openstackclient

systemctl enable --now mariadb
mysql -e "CREATE DATABASE keystone;"
mysql -e "GRANT ALL ON keystone.* TO 'keystone'@'localhost' IDENTIFIED BY 'KEYSTONE_DB_PW';"
mysql -e "GRANT ALL ON keystone.* TO 'keystone'@'%' IDENTIFIED BY 'KEYSTONE_DB_PW';"

# Configure keystone (see RDO install guide for full keystone.conf), then:
keystone-manage db_sync
keystone-manage bootstrap \
  --bootstrap-password ADMIN_PW \
  --username admin \
  --email admin@localhost

# httpd WSGI on 5000 (Packstack leaves /etc/httpd/conf.d/10-keystone_wsgi.conf)
systemctl enable --now httpd
```

Or stop a full Packstack run once **Keystone listens on 5000** and wire Machina (see below).

## One-shot bootstrap (recommended)

After Machina deploy and a Packstack-partial host with `/root/keystonerc_admin`:

```bash
sudo ./scripts/openstack-bootstrap-machina.sh /root/keystonerc_admin <CONTROLLER_IP>
```

On a host that already runs Machina libvirt VMs (same as `185.165.240.5`):

```bash
sudo ./scripts/openstack-bootstrap-machina.sh /root/keystonerc_admin 185.165.240.5 --fake-compute
```

The script (idempotent where possible):

1. Fixes httpd port-80 conflicts so Keystone answers on `:5000`
2. Runs `openstack-wire-cloud.sh` and `openstack-minimal-services.sh`
3. Registers Glance / Neutron / Placement Keystone endpoints if missing
4. Installs and configures OVN (`ovn-controller`, `neutron-ovn-agent`) when ML2 uses `ovn`
5. Maps Nova cell hosts, installs `openstack-nova-compute`, syncs service user passwords
6. Seeds `m1.tiny`, `cirros-test`, `private` network (optional: `--skip-seed`)
7. Restarts `machina-daemon` and runs a CLI smoke test

Installed to `/usr/local/share/machina/scripts/` by `install.sh` on the next deploy.

## Wire Machina (manual)

```bash
sudo ./scripts/openstack-wire-cloud.sh /root/keystonerc_admin packstack
sudo systemctl restart machina-daemon
```

Verify Keystone:

```bash
source /root/keystonerc_admin
openstack token issue
curl -s http://127.0.0.1:5000/v3/ | head
```

Verify Machina (after UI login):

```bash
curl -sk -b /tmp/machina-cookies.txt -c /tmp/machina-cookies.txt \
  -H 'Content-Type: application/json' \
  -d '{"username":"root","password":"..."}' \
  https://127.0.0.1:5092/api/v1/auth/login

curl -sk -b /tmp/machina-cookies.txt https://127.0.0.1:5092/api/v1/openstack/status
```

Expect `reachable: false` until Nova/Glance are installed; `connected` may still be true if Keystone auth works but compute/image lists fail.

## Avoid port 80 conflicts

HyperSDK / hyper2kvm often uses **80/443**. Packstack Horizon also wants 80. For minimal tests:

- Set `CONFIG_HORIZON_INSTALL=n` in Packstack answers, **or**
- Stop `h2kweb` only for the Packstack run, then restart it after httpd is configured to share ports.

Keystone itself uses **httpd on 5000** only.

## Minimal Nova + Glance bootstrap

When Packstack stopped after Keystone but left `httpd` WSGI configs:

```bash
sudo ./scripts/openstack-bootstrap-machina.sh /root/keystonerc_admin <CONTROLLER_IP>
```

Or step-by-step:

```bash
sudo ./scripts/openstack-wire-cloud.sh /root/keystonerc_admin packstack
sudo ./scripts/openstack-minimal-services.sh /root/keystonerc_admin <CONTROLLER_IP>
# … OVN / cell_v2 sections below if not using the bootstrap script …
sudo systemctl restart machina-daemon
```

The script:

- Creates `nova` and `glance` users in the `services` project (passwords from existing configs)
- Registers compute endpoints on `http://HOST:8774/v2.1` if missing
- Masks `openstack-nova-api.service` (conflicts with httpd on port 8774)
- Restarts `httpd`, `openstack-glance-api`, and optionally conductor/scheduler

## httpd vs port 80 on Packstack partial hosts

If `website-server` or another process already listens on **80**, `httpd` fails to start and Keystone (`:5000`) never comes up.

1. Disable httpd vhosts that bind `*:80` (Horizon, default-80, aodh, gnocchi):

   ```bash
   for f in 15-default-80.conf 15-horizon_vhost.conf 10-aodh_wsgi.conf 10-gnocchi_wsgi.conf; do
     sudo mv /etc/httpd/conf.d/$f /etc/httpd/conf.d/${f}.disabled 2>/dev/null || true
   done
   ```

2. Comment out `Listen 80` in `/etc/httpd/conf/ports.conf` (Keystone/Nova WSGI use `5000`, `8774`, etc.).

3. `sudo systemctl restart httpd` and verify: `curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:5000/v3/` → `200`.

## OVN networking (ML2 `mechanism_drivers=ovn`)

Packstack often leaves **OVN northbound/southbound** (`:6641` / `:6642`) and `neutron-server` running, but **no compute chassis** in the southbound DB. Symptom: instance create fails with **port binding failed**; `ovn-northd.log` shows:

```text
Unknown chassis '<hostname>' set in options:requested-chassis on LSP '...'
```

`openstack network agent list` is empty until fixed.

On the hypervisor (as root):

```bash
HOST=$(hostname -s)          # must match Nova hypervisor hostname
IP=<CONTROLLER_IP>          # e.g. 185.165.240.5

sudo dnf install -y ovn24.03-host openstack-neutron-ovn-agent

sudo ovs-vsctl --may-exist add-br br-int
sudo ovs-vsctl set open . external-ids:system-id="$HOST"
sudo ovs-vsctl set open . external-ids:ovn-remote="tcp:${IP}:6642"
sudo ovs-vsctl set open . external-ids:ovn-remote-probe-interval=60000
sudo ovs-vsctl set open . external-ids:ovn-encap-type=geneve
sudo ovs-vsctl set open . external-ids:ovn-encap-ip="$IP"
sudo ovs-vsctl set open . external-ids:ovn-bridge-mappings=physnet1:br-ex
sudo ovs-vsctl --may-exist add-br br-ex

sudo systemctl enable --now ovn-controller ovn-northd neutron-ovn-agent openvswitch
```

Verify:

```bash
sudo ovn-sbctl list chassis          # should show name/hostname = $HOST
openstack network agent list         # OVN Controller agent UP
```

Restart Neutron after first chassis registration if agents were empty:

```bash
sudo systemctl restart neutron-ovn-agent neutron-server
```

## Nova cell mapping and compute

After a partial Packstack install, map the compute host into the default cell:

```bash
sudo nova-manage cell_v2 discover_hosts --verbose
```

Install compute if missing:

```bash
sudo dnf install -y openstack-nova-compute
```

Enable the libvirt driver in `/etc/nova/nova.conf` (uncomment or set):

```ini
compute_driver=libvirt.LibvirtDriver
```

**Libvirt conflict:** If Machina already runs libvirt VMs on the host, a new `nova-compute` service may refuse to start:

```text
My hypervisor has existing instances, but I appear to be a new service in this database
```

Options: use a dedicated compute node, align Nova’s inventory with existing domains, or temporarily use `compute_driver=fake.FakeDriver` for API/UI tests (OVN networking still works; VMs are not real hypervisor guests).

Sync Keystone service user passwords from config if conductor logs **HTTP 401**:

```bash
source /root/keystonerc_admin
NOVA_PW=$(grep -E '^password=' /etc/nova/nova.conf | head -1 | cut -d= -f2- | tr -d ' "')
openstack user set --password "$NOVA_PW" nova
```

Register Glance/Neutron/Placement endpoints in Keystone if `openstack image list` reports *endpoint not found* (see host runbook below).

## Seed a minimal catalog

```bash
source /root/keystonerc_admin
openstack flavor create --id 1 --ram 512 --disk 1 --vcpus 1 m1.tiny
curl -fsSL -o /tmp/cirros.img https://download.cirros-cloud.net/0.6.2/cirros-0.6.2-x86_64-disk.img
openstack image create cirros-test --disk-format qcow2 --container-format bare --public --file /tmp/cirros.img
openstack network create private
openstack subnet create --network private --subnet-range 10.0.0.0/24 private-subnet
```

Machina defaults in `openstack-wire-cloud.sh` expect `m1.tiny`, `private`, and an image name you pass at create time.

## Machina E2E test script

After deploy and bootstrap, run the unified E2E script from your laptop:

```bash
./scripts/deploy-remote.sh sus 185.165.240.5 --bind 0.0.0.0 --open-firewall

# On the host (as root):
sudo /usr/local/share/machina/scripts/openstack-bootstrap-machina.sh \
  /root/keystonerc_admin 185.165.240.5 --fake-compute

# From your workstation:
VSPASS=max ./scripts/e2e-test.sh https://185.165.240.5:5092 sus
```

[`scripts/e2e-test.sh`](../scripts/e2e-test.sh) runs:

- **Preflight** — `GET /health`, `GET /system/platform-info`
- **Libvirt** — create / start / VNC / stop / delete VM via API
- **OpenStack** — status, test-connection, flavors/networks/images, instance create/list/delete

Flags: `--skip-libvirt`, `--skip-openstack`, `--require-openstack-ssh`, `--ssh-host HOST`, `--skip-dhcp-check`.

Remote wrapper: [`scripts/e2e-test-remote.sh`](../scripts/e2e-test-remote.sh) runs the same suite from your laptop (`USER HOST` + e2e flags).

Libvirt-only (legacy [`api-test.sh`](../scripts/api-test.sh)): same as `e2e-test.sh --libvirt-only`.

Success: exit 0 and summary `All tests passed`. On hosts with `fake.FakeDriver`, guest SSH is warned, not failed, unless `--require-openstack-ssh`.

**If OpenStack create-instance E2E fails** (`nova-compute` inactive, `compute_id`, Placement 409, or *Host is not mapped to any cell*), on the hypervisor as root:

```bash
sudo ./scripts/openstack-repair-e2e-compute.sh /root/keystonerc_admin
```

Then re-run E2E. [`openstack-repair-e2e-compute.sh`](../scripts/openstack-repair-e2e-compute.sh) resets fake compute identity, Placement resource provider, and cell v2 host mapping (idempotent).

## Remote host `185.165.240.5` (May 2026)

Reference hypervisor for Machina + minimal OpenStack E2E (`sus@185.165.240.5`).

| Item | Value |
|------|--------|
| Machina UI | `https://185.165.240.5:5092/` |
| Deploy | `./scripts/deploy-remote.sh sus 185.165.240.5 --bind 0.0.0.0 --open-firewall` |
| OpenStack RC | `/root/keystonerc_admin` |
| ML2 | `mechanism_drivers=ovn`, tenant networks `geneve` |
| Port 80 | `website-server` holds 80; httpd uses `Listen 5000` / `8774` only (`Listen 80` commented out) |

**Verified (May 2026):**

- `openstack-bootstrap-machina.sh` (or wire + minimal-services) with controller IP `185.165.240.5`
- `ovn24.03-host` + `ovn-controller` chassis `NLDW3-4-31-12`
- `nova-manage cell_v2 discover_hosts`
- Catalog: `m1.tiny`, `cirros-test`, `private` / `private-subnet`
- Machina API: create → list (ACTIVE) → delete
- Nova compute on **fake driver** due to existing Machina libvirt guest on same host

**Still optional:** switch to `libvirt.LibvirtDriver` on this host or add a second compute node for real hypervisor-backed OpenStack instances.

## Production compute (real Nova guests)

Machina libvirt VMs on the same host as `nova-compute` block the libvirt driver unless you handle the conflict.

| Approach | When to use |
|----------|-------------|
| **Option A — same host** | Lab hypervisor; you can stop or migrate existing `virsh` domains |
| **Option B — second compute node** | Production Machina VM must stay on the controller; add another machine with only `nova-compute` + OVN chassis |

**Check before enabling libvirt compute:**

```bash
sudo ./scripts/openstack-bootstrap-machina.sh /root/keystonerc_admin 185.165.240.5 --check-libvirt-conflict
# or:
sudo ./scripts/openstack-enable-libvirt-compute.sh --check-only
```

**Enable libvirt compute (Option A, clean libvirt):**

```bash
sudo ./scripts/openstack-enable-libvirt-compute.sh
```

**Bootstrap flags:**

| Flag | Effect |
|------|--------|
| `--fake-compute` | Force `fake.FakeDriver` (API/E2E without real guests) |
| `--libvirt-compute` | Force libvirt driver |
| `--no-fake-fallback` | Fail instead of silently falling back to fake |
| `--allow-libvirt-conflict` | Try libvirt even when `virsh list` shows domains |
| `--check-libvirt-conflict` | Print conflict and exit (no changes) |

**Verify real guests:**

```bash
VSPASS=max ./scripts/e2e-test.sh https://185.165.240.5:5092 sus --require-openstack-ssh --ssh-host 185.165.240.5
# from laptop after deploy:
VSPASS=max ./scripts/e2e-test-remote.sh sus 185.165.240.5 --require-openstack-ssh
VSPASS=max ./scripts/deploy-remote.sh sus 185.165.240.5 --quick --e2e
```

## Remote host `212.8.252.194` (May 2026, abandoned)

- Packstack was **stopped** after Keystone came up on `:5000`.
- `openstack token issue` works with `/root/keystonerc_admin`.
- Nova/RabbitMQ/conductor issues prevented reliable VM create; use **`185.165.240.5`** instead.
