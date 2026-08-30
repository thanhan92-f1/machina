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
| **Browse** | Finder, VM list, Fleet Cloud lists | Medium | TahoeHero flat + white/graphite panels |
| **Work** | VM detail, tables, wizards, settings | High | Same hero; denser rows |
| **Immersive** | Console Hub, VNC/SPICE, TTY | Full | Cinema chrome hide — carbon island |

---

## How all pages get the UX

| Layer | Mechanism |
|---|---|
| Shell | `PlatformLayout` Mac desktop for **all** authenticated routes |
| Hero | `TahoeHero` / `PlatformTahoeHero` (flat + ledger) |
| Tokens | Zeus Mist / Magichromatic via `zeus-parity.css` |
| Login | Apple Account `zyvor-premium-login.css` |

---

## Author checklist

1. Prefer `PlatformPageChrome` / `PlatformTahoeHero` / `PageLayout` → Tahoe flat hero  
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
- [x] TahoeHero flat for platform heroes
- [ ] Remaining sky/cyan hardcode sweep on legacy pages
- [ ] Full TahoeSheet / TahoeListKit port
