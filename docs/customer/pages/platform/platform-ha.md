# HA Events

## Purpose

High Availability — Machina Platform page at `/platform/ha`.

## When to use it

- Operate **HA Events** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/ha`
- Nav: **Platform → High Availability** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Refresh.
3. **Empty:** No HA events / No hosts.
4. **Success:** HA status healthy.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Placement & HA](platform-placement.md)
- [Hosts](platform-hosts.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
