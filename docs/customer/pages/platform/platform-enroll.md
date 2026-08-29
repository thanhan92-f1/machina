# Host Enrollment

## Purpose

Add Host — Machina Platform page at `/platform/enroll`.

## When to use it

- Operate **Host Enrollment** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/enroll`
- Nav: **Platform → Add Host** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Filters / tabs: **Token history**.
3. Generate join token.
4. Copy install command.
5. **Empty:** No enrollment tokens yet.
6. **Success:** Host appears on /platform/hosts.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Hosts](platform-hosts.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
