# Machina UX contract — apple.com, Netra-flat

Every authenticated page uses one shell and one flat look: white ground, neutral hairlines,
apple.com blue for intent only, quiet cards/toolbars/tables, and no ambient gradients in the light
theme. The look is shared with the Netra dashboard (`../netra`, `docs/design/APPLE-UX-CONTRACT.md`
there); this document is the Machina side of that contract.

Companion: [DAYLIGHT-CONTRACT.md](DAYLIGHT-CONTRACT.md) (tokens and the light/dark palettes).
Author guide: [ux.md](../ux.md).

---

## Where the look lives

CSS is loaded in this order (`web/src/main.tsx` imports `main.css`, then `netra-look.css`):

| File | Role |
|---|---|
| `styles/main.css` | Tailwind v4 (`@import "tailwindcss"`), dark "carbon" `:root`, legacy component rules, then imports the files below |
| `styles/platform-tahoe.css`, `machina-daylight.css`, `machina-apple-ux.css`, `zeus-parity.css` | Earlier layers: tahoe primitives, light tokens, Story/Browse primitives (`.apple-*`), Zeus HSL tokens |
| `styles/nav-global-bar.css` | GlobalBar, flyout, SideNav, ChapterBar |
| `styles/zyvor-premium-login.css` | Login |
| **`styles/netra-look.css`** | **The final look layer. Wins by source order.** New look changes go here. |

Why one last layer instead of editing the four earlier ones: they define the same tokens in
different formats and carry ~260 `!important`s. A single file where every override is visible is
auditable, and superseded rules can be deleted from the older files later.

**Rules for `netra-look.css`**

1. **Namespace new tokens `--nl-*`.** Tailwind v4 owns `--radius-*`, `--ease-*` and `--shadow-*`
   (2xs..2xl); redefining them silently restyles `rounded-lg` everywhere.
2. **No global element resets** (for example a `:where(input, select)` base). Tailwind utilities
   live in a cascade *layer*, so any unlayered rule beats them, even at zero specificity, and would
   clobber `h-8 px-2` on every input. Style classes, not bare elements.
3. **Don't touch immersive chrome**: `.tty-surface`, `[data-tty]`, Mission Control carbon, Console
   Hub, SSH and VNC views stay dark in both themes.
4. **Only light and dark are restyled.** `steel`, `aurora` and `rack` keep their own tokens and
   inherit the dark base. Token fallbacks (`var(--nl-x, #fallback)`) keep them legible.

## Surface tiers

| Tier | Examples | Density | Layout |
|---|---|---|---|
| **Story** | `/platform` Mission Control, `/` Dashboard, `/fleet-cloud` overview | Very low | `apple-story-stack`: display type, lede, one CTA; metric bands, not card grids |
| **Browse** | VM list, Fleet Cloud lists, hosts, storage, networks, events, audit, jobs | Medium | `PageLayout` header + `TahoeToolbar` + hairline `TahoeTableWrap` |
| **Work** | VM detail, settings, wizards | High | `.tahoe-glass-card` panels; VM detail leads with the console hero |
| **Immersive** | Console Hub, VNC/SPICE, TTY, host SSH | Full | Carbon chrome, no shell chrome |

## Shell

One layout, `layouts/PlatformLayout.tsx`, wraps every authenticated route (Console Hub hides all
chrome).

| Part | Component | Notes |
|---|---|---|
| Top bar | `components/nav/GlobalBar.tsx` | 45px translucent bar; product groups open a full-width **flyout** (multi-column, capped to the viewport and scrolls inside). Right side: Live chip, Search (⌘K), Control Center, Ask Zyra, New VM, avatar, sign out. |
| Sidebar | `components/nav/SideNav.tsx` | Section tree with the needs-attention dot; collapse state per section in `localStorage` (`machina-sidenav-<id>`); off-canvas drawer on phones. |
| Chapter bar | `components/nav/ChapterBar.tsx` | In-section pills (max 6 + More). Tier-gated: never at Normal, hub roots at Power, always at Advanced. |
| Menus | `PlatformMacMenuDropdown`, `PlatformFloatingMenu`, `PlatformMenuItem` | Portaled `role="menu"` panels; the floating menu restores focus to its trigger. |

Nav data is unchanged and test-covered: `utils/platformNavRegistry.ts`, `platformMacMenus.ts`,
`platformSidebarNav.ts`. Group ids (`workloads`, `infra`, `ops`, `secure`, `admin`, `more`) are
asserted by vitest; do not rename them for styling.

**Phone (≤ 640px)** the bar icon-ises so it fits 390px: the Live chip shows its dot, **New VM** is a
`+` button (its text stays in the DOM, so the accessible name is unchanged), the decorative avatar
and the duplicate sign-out icon are hidden (Sign Out remains in the Machina menu).

**Removed and orphaned.** The Mac menubar, 68px icon rail, hover flyouts, desktop tabs and dock are
gone (commit `de22b0e9`). `PlatformSidebar`, `PlatformMacAppMenus`, `PlatformProductNavMenus`,
`PlatformMacDesktopTabs`, `PlatformMacDock`, `PlatformDynamicIsland`, `Navbar.tsx` and
`Breadcrumb.tsx` are still in the tree with no importers. **Do not reintroduce a dock.** Some e2e
specs still target the old markup (`.mac-menubar-inner`, `.platform-sidebar`, `.tahoe-context-bar`);
they fail against the live shell independent of styling.

## Laws

1. **Elevation runs up.** Dark: page `#000` → panel `#1d1d1f` → card lighter → popover lightest.
   Light: white page, white card with a neutral hairline, popover with a soft shadow. **A grey panel
   on a white page is a bug.**
2. **Color is deviation.** Nominal values are graphite; blue means intent (CTA, link, focus);
   red/amber/green only when a value really deviates or is confirmed good.
3. **One primary action per view**: `.btn-primary`, apple.com `#0071e3`. Secondary is a tinted
   neutral; `btn-destructive`/`btn-danger` is the only solid non-blue fill.
4. **No ambient gradients in light.** `.tahoe-mesh` is `display: none` in the light theme; the dark
   theme keeps its hero glow.
5. **Cards are flat**: `.tahoe-glass-card`, 18px radius, hairline border, no blur, no shadow, no
   hover lift. A table wrapper inside a card is frameless (no card-in-card).
6. **Type-safe status color.** Pale Tailwind shades (`text-emerald-400`, `text-orange-400`, …) are
   dark-theme colors (~2:1 on white). In the light theme they are remapped centrally to the
   text-safe tokens (`--nl-accent-*-text`, ≥ 4.5:1). Prefer semantic tokens in new code; never
   `slate-*`, `sky-*` or `blue-600`.
7. **Immersive views stay carbon** but still use the `--text-*` tokens.

## Accessibility

- **Headings.** One `h1` per page (`PageLayout` renders it). Card and section titles are `h2`;
  sub-headings inside a card are `h3`. No skipped levels. Visually hidden pages (SSH, Console Hub)
  carry an `sr-only` `h1`. `ErrorBanner` and empty-state titles are `h2`.
- **Focus ring is a box-shadow.** A universal `:focus-visible` rule sets `outline: none` and draws
  `box-shadow: var(--focus-ring)`. Any component that sets `box-shadow: none !important` (all
  `.btn-*`) must restore it; `netra-look.css` does this for the button variants. An outline
  would not work: the universal rule (specificity 0,2,1 in light) removes it.
- **Dialogs, drawers, sheets.** Capture the trigger **during render** with
  `hooks/useCaptureTrigger.ts` and restore focus on close. Reading `document.activeElement` inside
  an effect is too late whenever a child has `autoFocus` (React applies it at commit, before
  effects), and focus falls to `<body>`. `useFocusTrap` and `GlassModal` use the hook;
  `ConfirmDialog` uses `useFocusTrap` (Tab trap, Escape for the topmost surface, restore, a real
  heading title, `aria-describedby`). New overlays must too.
- **Names.** Every input, select, textarea and icon-only button has an accessible name (a `<label>`,
  `aria-label`, or visible text). A placeholder is not a name.
- **Touch targets.** Buttons and links are ≥ 36px on phones (44px is the `.btn-*` default);
  small icon buttons extend their hit area with an invisible `::after`.
- **Phone.** No content clipped at 390px; wide tables scroll inside their wrapper.
- **Errors.** `formatUserError` / `sanitizeErrorText` unwrap `{"error": …}` envelopes; never show
  raw JSON.

## Author checklist

1. Pick the tier first.
2. Story: one composition (eyebrow, `apple-display`, lede, one CTA), metric bands, no tile walls.
3. Browse: `TahoeToolbar` + `TahoeTableWrap`; empty lists use `TahoeListEmpty` /
   `EmptyState` (title + sentence + next action).
4. Two-way switches (view mode, range) use `components/ui/SegmentedControl`; `ChoiceCard` is for
   wizard-style "pick a path" tiles only.
5. Very long lists: `components/ui/ExpandableList` + `utils/topN.ts` (rank, show the top N, expand
   on request). Keep existing pagination where a page has it.
6. Run the audit (below) and leave it no worse than before.

## Pitfalls (each cost time once)

- **`--border` and `--accent` are defined in more than one format** across the CSS layers (a full
  colour in one, a bare HSL triplet for `hsl(var(--x))` in another). Verify the computed value in
  the browser before trusting a `var()`; add new tokens as `--nl-*`.
- **Tailwind v4 emits `oklab()` / `color(srgb …)`** for opacity modifiers (`bg-emerald-600/90`);
  tools that parse `rgb()` only misread them as transparent.
- **jsdom has no layout**, so `offsetParent` is always `null`; tests of focus-trap logic shim it.

## Verifying

```bash
cd web && npm ci && npm run build
npm run preview -- --host 127.0.0.1 --port 5192 &
node scripts/ux-audit.mjs --out /tmp/ux.json                       # save a baseline
node scripts/ux-audit.mjs --baseline /tmp/ux.json                  # compare after a change
```

`scripts/ux-audit.mjs` loads all 130 routes in `scripts/regression/fixtures/pages.json` with the e2e
API mock (no daemon) in light and dark at 1440px and light at 390px, and reports page errors,
contrast (composited backgrounds), right-edge clipping, heading outline, unnamed controls, tap
targets and missing focus indicators. The mock is thin (empty or minimal data), so a clean run
means "no regressions", not "perfect": populated-state bugs need a real host
(`make regression-pages`). Extend `EXTRA_MOCKS` in the script when a route crashes on missing shape.

## Theme map

| Machina `machina-theme` | `data-theme` | Status |
|---|---|---|
| `light` (default) | `tahoe-light` | Restyled (Netra-flat) |
| `dark` | `tahoe` | Restyled |
| `steel`, `aurora`, `rack` | own | Inherit the dark base; not restyled or audited |

## Rollout status

- [x] `netra-look.css` final layer: neutral hairlines, flat cards/toolbars/tables, no light gradients
- [x] Shell: flyout capped to the viewport, 36px links, phone bar fits 390px
- [x] Login: single centered composition
- [x] Focus restore fixed in `GlassModal`, `useFocusTrap`, `ConfirmDialog` (tests reproduce the `autoFocus` case)
- [x] Button focus ring; light-theme text-safe accent remap; `ErrorBanner` tokens
- [x] `SegmentedControl` for view switches; headings and names fixed on audited routes
- [x] `scripts/ux-audit.mjs` sweep with a saved baseline
- [x] `useExpandable`/`ExpandableToggle` applied fleet-wide: `AuditLog`, `PlatformSoc`'s six
      sections, `PlatformNotifications`, then a second batch covering every strong candidate a
      ~175-file survey found — `VMList`, `AdminSessions`, `PlatformActivityMonitor`,
      `PlatformHosts` (all four Finder view modes), `PlatformEvents`, the firewall pages,
      `PlatformThreatHunting`, `PlatformWebhooks`, `PlatformBackups`, `FleetCloudInstances`,
      `FleetCloudVolumes`, `K8sWorkloads` (six independent lists), `PlatformRuntimeEnforcement`.
      `ExpandableList` itself is now built on the same hook. A few borderline items from that
      survey (`K8sOverview`, `PlatformIncidentCommander`, `SocAlertDetailPanel`,
      `HostNetworking`, other FleetCloud resource pages, classic Storage/Networks/Containers) were
      not reviewed — worth a pass if this keeps coming up in practice.
- [ ] Delete the superseded rules from `main.css` / `machina-daylight.css` / `zeus-parity.css`
- [x] Update or remove e2e specs that target the removed shell markup — `cross-shell.spec.ts`,
      `platform-chaos-navigation.spec.ts`, `platform-jarvis-shell.spec.ts`,
      `platform-nav-coverage.spec.ts`, `platform-batch-48.spec.ts`, `platform-full.spec.ts` all
      pass against current markup now (two tests are `test.skip` with a dated finding comment: a
      real, reproducible hang on `/platform/zeus`'s default Fleet tab, unrelated to the shell
      rewrite and not yet root-caused).
- [ ] Real-host confirmation of populated states
