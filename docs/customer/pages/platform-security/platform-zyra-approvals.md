# Zyra Approvals

## Purpose

Approvals — Machina Platform / Security page at `/platform/zyra/approvals`.

## When to use it

- Operate **Zyra Approvals** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/zyra/approvals`
- Nav: **Platform / Security → Approvals** (or spotlight / Finder search)

## Notes

- Formerly documented under `/platform/zeus*` (except Zeus Security Center, which remains `/platform/zeus/security`).

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Approve / Deny.
3. **Empty:** No pending approvals.
4. **Success:** Queue empty after approve.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Incident Commander](platform-zyra-incidents.md)
- [Tasks](../platform/platform-tasks.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](../platform/platform.md)
- [Page index](../../PAGE_INDEX.md)
