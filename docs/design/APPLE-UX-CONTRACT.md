# Zeus OS UX Contract — Machina (1:1 parity)

Goal: **Every authenticated page** uses the Zeus Mac desktop shell and apple.com-style
Tahoe chrome — not a separate admin Navbar.

SoT: [`../zeus-os`](../../zeus-os) (`ui/src/index.css`, Apple Account login, Mac desktop).
Machina port: `web/src/styles/zeus-parity.css`, `PremiumLoginShell`, `PlatformLayout`,
`machina-apple-ux.css`.

Companion: [DAYLIGHT-CONTRACT.md](DAYLIGHT-CONTRACT.md) (Tahoe Light / Classic Blue tokens).
Author guide: [ux.md](../ux.md).

---

## Surface tiers

| Tier | Examples | Density | Layout |
|---|---|---|---|
| **Story** | `/platform` Mission Control, `/` Dashboard, `/fleet-cloud` overview | Very low | `apple-story-stack` — display type, lede, one CTA; metric bands not card grids |
| **Browse** | Finder, VM list, Fleet Cloud lists, hosts/storage/networks | Medium | Flat `PageLayout` header + `TahoeToolbar` / hairline tables |
| **Work** | VM detail, settings, wizards | High | Same tokens; `.tahoe-glass-card` panels; keep density |
| **Immersive** | Console Hub, VNC/SPICE, TTY, host SSH | Full | Cinema chrome hide — carbon island |

---

## How all pages get the UX

| Layer | Mechanism |
|---|---|
| Shell | `PlatformLayout` Mac desktop for **all** authenticated routes (menubar + dock + optional desktop tabs) |
| Menubar brand | `ZyvorTileMark` in Apple-logo slot ([`PlatformMacAppMenus`](../../web/src/components/platform/mac/PlatformMacAppMenus.tsx)) → `/platform` |
| Hero | **Canonical:** `PlatformPageChrome` → `PageLayout` + `machina-apple-ux.css` (`.apple-page-header`). Optional Zeus flat: `PlatformTahoeHero` / `TahoeHero` for icon+stats ledger |
| Browse chrome | Thin Zeus port [`TahoeListKit`](../../web/src/components/platform/tahoe/TahoeListKit.tsx) — `TahoeToolbar`, `TahoeTableWrap`, `TahoeListEmpty` |
| Fleet Cloud nav | Pill row + **More** overflow ([`FleetCloudSubNav`](../../web/src/components/FleetCloudSubNav.tsx)) — not a 15-tab strip |
| Tokens | apple.com blue `#0071e3` + shop box type via `zeus-parity.css` (see [DAYLIGHT-CONTRACT.md](DAYLIGHT-CONTRACT.md)) |
| Dock | Tahoe Liquid Glass tray + neighbor magnification ([`PlatformMacDock`](../../web/src/components/platform/PlatformMacDock.tsx)) |
| Login | Apple Account [`zyvor-premium-login.css`](../../web/src/styles/zyvor-premium-login.css); hero wordmark **machina** (SF Pro / system display); Zyvor mark icon-only |

---

## Navigation (dock-first)

| Layer | Role | Default visibility |
|---|---|---|
| **Menubar** | App menus (Go = Favorites + Hubs + tier-filtered locations), Zyvor tile, Dynamic Island, Control Center | Always (authenticated) |
| **Desktop tabs** | Open window strip ([`PlatformMacDesktopTabs`](../../web/src/components/platform/mac/PlatformMacDesktopTabs.tsx)) | When >1 platform tab |
| **Dock** | Primary app launcher | Always |
| **Sidebar** | Finder **locations** only (Host / Fleet / Platform — Favorites stripped; dock owns app pins) | **Advanced** on by default; **Normal/Power** off (`defaultSidebarVisibleForTier`) — View → Show Sidebar still works |
| **Context bar** | Hub cross-links | Tier-gated (see [ux.md](../ux.md)) |
| **Fleet Cloud pills** | Section switch within `/fleet-cloud/*` | Primary five + More |

**Submenus:** [`PlatformFloatingMenu`](../../web/src/components/platform/mac/PlatformFloatingMenu.tsx) + [`PlatformMenuItem`](../../web/src/components/platform/mac/PlatformMenuItem.tsx) — portaled `role="menu"` panels for menubar, context-bar More, and Fleet Cloud More.

Helpers: [`shouldShowContextBar`](../../web/src/utils/platformNavRegistry.ts), [`defaultSidebarVisibleForTier`](../../web/src/utils/platformDesktopTier.ts), [`sidebarLocationsOnly`](../../web/src/utils/platformNavFilter.ts).

---

## Author checklist

1. Prefer `PlatformPageChrome` / `PageLayout` → apple flat header (`PlatformTahoeHero` only when icon+stats ledger is required)
2. Story first viewport: one composition — eyebrow, `apple-display`, lede, one CTA row (`apple-section` / `apple-story-stack`)
3. Primary CTA = `.btn-primary` / `.tahoe-btn-primary` (apple.com `#0071e3`, not Mist / System-Blue leftovers)
4. Content cards = `.tahoe-glass-card` (flat hairline) — inherit `color: var(--text-primary)`; no content blur on Browse/Work panels
5. Browse lists: wrap search with `TahoeToolbar`; tables in `TahoeTableWrap`
6. Inputs: `input-field`
7. Console / TTY / Host SSH stay immersive (carbon chrome) — type still uses Apple `--text-*` (no slate hex)
8. No hard-coded `slate-*` / `sky-*` / `blue-600` / `#64748b` — use semantic tokens (`--text-*`, `--accent`, `--apple-*`, `--amber`, `--verdant`)
9. Box fonts 1:1 with Apple shop: light `#1d1d1f` / `#6e6e73` / `#86868b`; dark `#f5f5f7` / `#a1a1a6` / `#86868b`

### Conversion recipes

| Anti-pattern | Replace with |
|---|---|
| Hub `NavCard` 3-up grids | [`AppleDestinationList`](../../web/src/components/platform/apple/AppleStoryKit.tsx) linear rows |
| List \| inspector `xl:flex-row` / Finder columns | Chips + full-width stack (detail below or route push) |
| Browse card walls / `MacStatWidget` first viewport | `TahoeToolbar` + `TahoeTableWrap`; counts in subtitle |
| Login split hero + feature cards | Account-style single composition (`PremiumLoginShell`) |

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
- [x] Zeus Apple Account login + apple.com **machina** wordmark
- [x] Menubar Zyvor tile (Apple-logo slot)
- [x] Dock-first sidebar (Normal/Power hidden; Favorites not duplicated in rail)
- [x] Mac desktop tabs mounted
- [x] Flat page heroes via `PlatformPageChrome` / `PageLayout`
- [x] Story rhythm on Dashboard + Fleet Cloud overview
- [x] Thin TahoeListKit + Fleet Cloud pill nav
- [x] Work panels on high-traffic pages → `.tahoe-glass-card`
- [x] High-traffic slate/blue leftover cleanup (HostSSH, ApiDocs, SimpleCreateVmWizard, skip-link)
- [x] Apple shop box font tokens 1:1 (`--text-primary/secondary/muted` in `zeus-parity.css`)
- [x] Console Hub / VNC type remapped off slate hex onto `--text-*`; ChoiceCard amber/emerald icons → `--amber` / `--verdant`
- [x] System-wide apple.com blue (`#0071e3`) for primary / link / accent / dock active
- [x] Tahoe Liquid Glass dock + neighbor magnification
- [x] AirPods Story type (`.apple-display` / `.apple-lede`)
- [x] Story hubs + destination lists (`AppleStoryKit`); kill page-level list|inspector splits
- [x] Browse lists → `TahoeToolbar` + `TahoeTableWrap` (platform / classic / Fleet Cloud packs)
- [x] Apple Account login (single composition — no split hero / feature cards)
- [ ] Full TahoeSheet port (drawer/sheet primitive — deferred)
- [ ] Exhaustive sky/cyan remaps removal from `main.css` bandage blocks
