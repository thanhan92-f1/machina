# Tasks

## Purpose

Tasks — Machina Platform page at `/platform/tasks`.

## When to use it

- Operate **Tasks** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/tasks`
- Nav: **Platform → Tasks** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Filters / tabs: **Pending / Running / Completed / Failed**.
3. Refresh.
4. Open task detail.
5. **Empty:** No tasks.
6. **Success:** Task Completed.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Platform events](platform-events.md)
- [Activity Monitor](platform-activity.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
