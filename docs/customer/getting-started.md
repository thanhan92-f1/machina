# Getting Started with Machina

## What you need

| Requirement | Notes |
|-------------|--------|
| Linux host with KVM/libvirt | Machina daemon builds/runs on Linux |
| URL | **`https://<host>:5092`** (TLS on by default) |
| Login | PAM (host Linux account) or LDAP/OIDC when configured |
| Browser | Modern Chromium, Firefox, or Safari |

## 1. Open the dashboard

Open `https://<host>:5092`. Accept the self-signed certificate in labs, or install your org cert in production.

Health check:

```bash
curl -sk https://localhost:5092/api/v1/health
```

## 2. Sign in

| Mode | What you do |
|------|-------------|
| PAM | Use your Linux username/password on the host |
| LDAP | Directory credentials when LDAP is enabled |
| OIDC | SSO via `/auth/oidc/login` when configured |

RBAC roles: Admin / Operator / ReadOnly via `roles.json`, OIDC groups, or API tokens. Empty `roles.json` means everyone is Admin — fix that before production.

## 3. Orient yourself

1. **Core** — Dashboard, VMs, Create, Import, Fleet on this host.
2. **Platform** — Mission Control for multi-host fleet (`/platform`).
3. **Infrastructure / OpenStack / Monitoring** — storage, networks, OpenStack, host metrics.
4. Spotlight / Finder for quick jump.

## 4. First workflows

### A. List and open a VM console

`/vms` → select a VM → Console (VNC/SPICE/SSH/serial/RDP as available).

### B. Create a VM

`/create` or Platform → VM Builder / Advanced Create.

### C. Check host health

`/node` or Platform → Hosts.

### D. Wire OpenStack (optional)

Settings → OpenStack (`/settings?openstack=1`) then use `/openstack` pages.

## Next steps

- [Using the Dashboard](using-the-dashboard.md)
- [Admin basics](admin-basics.md)
- [Page guides](pages/README.md)
