# Time Machine

## Purpose

Backup & Restore — Machina Platform page at `/platform/backups`.

## When to use it

- Operate **Time Machine** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/backups`
- Nav: **Platform → Backup & Restore** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Filters / tabs: **Destinations / schedules / events**.
3. Add destination.
4. Queue backup.
5. Add schedule.
6. Restore.
7. Browse VMs.
8. **Empty:** No schedules/events → queue or schedule.
9. **Success:** Backup event completed; restore OK.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Backups (Snapshot Backups)](../infrastructure/backups.md)
- [Machine Finder](platform-vms.md)
- [Fleet snapshot schedules](platform-fleet-snapshots.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
