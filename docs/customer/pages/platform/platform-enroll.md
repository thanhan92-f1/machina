# Add Host

## Purpose

Add Host — Machina Platform page at `/platform/enroll`.

## When to use it

- Open this page when the job matches the purpose above
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/enroll`
- Nav: **Platform → Add Host** (or spotlight / Finder search)

## What you can do

1. Open `/platform/enroll` against the Machina daemon (`https://<host>:5092`).
2. Use filters and host/VM selectors when the page provides them.
3. Drill into a VM, host, or OpenStack resource for consoles and detail panels.
4. For mutating actions (create/delete VM, apply firewall, OpenStack change): confirm the target host and role (Admin/Operator).

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires OpenStack, HyperSDK, or Launchpad to be enabled.

## Related pages

- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
