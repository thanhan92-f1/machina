# Networks

## Purpose

Networks — Machina Platform page at `/platform/networks`.

## When to use it

- Operate **Networks** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/networks`
- Nav: **Platform → Networks** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Filters / tabs: **Overlay segments; IPAM**.
3. New network.
4. New segment.
5. Import/Sync from hosts.
6. **Empty:** No networks yet → create/enroll.
7. **Success:** Segment + IPAM pool present.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Networks](../infrastructure/networks.md)
- [Network canvas](platform-network-canvas.md)
- [Security Center](../platform-security/platform-zeus-security.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
