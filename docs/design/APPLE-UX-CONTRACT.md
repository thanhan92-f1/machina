# Zeus OS UX Contract — Machina (1:1 parity)

Goal: **Every authenticated page** uses the Zeus Mac desktop shell and Tahoe chrome —
not a separate apple.com admin Navbar.

SoT: [`../zeus-os`](../../zeus-os) (`ui/src/index.css`, TahoeHero, Apple Account login).
Machina port: `web/src/styles/zeus-parity.css`, `PremiumLoginShell`, `PlatformLayout`.

---

## Surface tiers

| Tier | Examples | Density | Layout |
|---|---|---|---|
| **Story** | `/platform` home, empty states | Very low | Full-bleed Mac desktop, Mission Control |
| **Browse** | Finder, VM list, Fleet Cloud lists | Medium | Flat apple page header + white/graphite panels |
| **Work** | VM detail, tables, wizards, settings | High | Same hero; denser rows |
| **Immersive** | Console Hub, VNC/SPICE, TTY | Full | Cinema chrome hide — carbon island |

---

## How all pages get the UX

| Layer | Mechanism |
|---|---|
| Shell | `PlatformLayout` Mac desktop for **all** authenticated routes |
| Hero | **Canonical:** `PlatformPageChrome` → `PageLayout` + `machina-apple-ux.css` (`.apple-page-header`). Optional Zeus flat: `PlatformTahoeHero` / `TahoeHero` for call sites that need icon+stats ledger |
| Tokens | Zeus Mist / Magichromatic via `zeus-parity.css` |
| Login | Apple Account `zyvor-premium-login.css` |

---

## Author checklist

1. Prefer `PlatformPageChrome` / `PageLayout` → apple flat header (`PlatformTahoeHero` only when icon+stats ledger is required)  
2. Primary CTA = `.btn-primary` / `.tahoe-btn-primary` (Mist, not System Blue)  
3. Content cards = `.tahoe-glass-card` (flat hairline) — no content blur  
4. Inputs: `input-field`  
5. Console / TTY stay immersive  

---

## Theme map

| Machina | Zeus |
|---|---|
| `light` | `tahoe-light` |
| `dark` | Classic Blue (`default` / `data-theme=tahoe`) |
| `steel` | `dark-steel` |
| `aurora` | `aurora` |
| `rack` | Machina-only |

---

## Rollout status

- [x] Zeus token + CTA port (`zeus-parity.css`)
- [x] Unified Mac desktop shell (Navbar retired as primary chrome)
- [x] Zeus Apple Account login
- [x] Flat page heroes via `PlatformPageChrome` / `PageLayout` (TahoeHero available for ledger call sites)
- [x] Remaining sky/cyan hardcode sweep on high-traffic leftovers (HostSSH, ApiDocs, SimpleCreateVmWizard, skip-link)
- [x] Thin TahoeListKit port (`TahoeToolbar` / `TahoeTableWrap` / `TahoeListEmpty`) + Fleet Cloud pill nav
- [ ] Full TahoeSheet port (drawer/sheet primitive — deferred)
- [ ] Exhaustive sky/cyan remaps removal from `main.css` bandage blocks