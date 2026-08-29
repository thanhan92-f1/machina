# Upgrade Guide

## Purpose

Upgrade Matrix — Machina Platform page at `/platform/upgrade`.

## When to use it

- Operate **Upgrade Guide** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/upgrade`
- Nav: **Platform → Upgrade Matrix** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Apply agent upgrades.
3. **Empty:** No hosts enrolled.
4. **Success:** Hosts on target version.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Maintenance](platform-maintenance.md)
- [Hosts](platform-hosts.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
