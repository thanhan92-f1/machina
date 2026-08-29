# Hosts

## Purpose

Hosts — Machina Platform page at `/platform/hosts`.

## When to use it

- Operate **Hosts** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/hosts`
- Nav: **Platform → Hosts** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Filters / tabs: **?filter=offline; search**.
3. Add/Enroll host.
4. Sync all.
5. Open host.
6. Open in Machine Finder.
7. **Empty:** No hosts enrolled → Add host.
8. **Success:** Host online + heartbeat.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Host Enrollment](platform-enroll.md)
- [Machine Finder](platform-vms.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
