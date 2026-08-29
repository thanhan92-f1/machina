# Security Center

## Purpose

Zeus Security — Machina Platform / Security page at `/platform/zeus/security`.

## When to use it

- Operate **Security Center** when your job matches this page
- Use Mission Control (`/platform`) for fleet-wide work; use Core routes for this host only
- Confirm PAM/OIDC login and roles if actions are missing

## How to get there

- Route: `/platform/zeus/security`
- Nav: **Platform / Security → Zeus Security** (or spotlight / Finder search)

## Operate from the console (UX)

1. Open the route against the Machina daemon (`https://<host>:5092`) and wait for live data.
2. Filters / tabs: **Hub tiles → firewall/ports/services/…**.
3. Enroll Tetragon.
4. Runtime enforcement.
5. Open Ports.
6. Threat hunting.
7. **Empty:** PacketWolf fabric unreachable.
8. **Success:** Posture tiles green/nominal.

If the page stays empty, check daemon health (`/api/v1/health`), libvirt connectivity, and whether the feature requires Fleet Cloud or Mission Control enrollment.

## Related pages

- [Security Operations Center](../platform/platform-soc.md)
- [Machina Zyra OS](platform-zyra.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](../platform/platform.md)
- [Page index](../../PAGE_INDEX.md)
