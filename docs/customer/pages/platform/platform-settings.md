# Settings

## Purpose

Settings — Machina Platform page at `/platform/settings`.

## When to use it

- Operate **Settings** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/settings`
- Nav: **Platform → Settings** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Filters / tabs: **General / Identity & SSO / Zyra / AI Providers / Security**.
3. Save.
4. Generate YAML.
5. Open Keychain/Network Lens.
6. **Empty:** —.
7. **Success:** Settings persist.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [AI Providers](platform-ai-providers.md)
- [Configure Zyra](../platform-security/platform-zyra-configure.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
