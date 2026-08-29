# Integrations

## Purpose

Integrations — Machina Platform page at `/platform/integrations`.

## When to use it

- Operate **Integrations** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/integrations`
- Nav: **Platform → Integrations** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Filters / tabs: **Launchpad grid**.
3. Enroll host.
4. Connect tiles.
5. **Empty:** No hypervisors enrolled.
6. **Success:** Integration connected.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Host Enrollment](platform-enroll.md)
- [Fleet Cloud](../fleet-cloud/fleet-cloud.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
