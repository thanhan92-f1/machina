# Register Server

## Purpose

Bare Metal — Machina Platform page at `/platform/baremetal`.

## When to use it

- Operate **Register Server** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/baremetal`
- Nav: **Platform → Bare Metal** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Register Server (BMC).
3. **Empty:** No bare metal servers.
4. **Success:** Server registered.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Machina Zyra OS](../platform-security/platform-zyra.md)
- [Security Center](../platform-security/platform-zeus-security.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
