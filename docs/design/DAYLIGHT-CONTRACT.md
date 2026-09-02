# Daylight Contract — Machina light theme (Zeus OS 1:1)

Shipped **default** light shell is apple.com white / Magichromatic Tahoe Light
(`html.apple-light`, `html[data-theme='tahoe-light']`) — port from
[`../zeus-os/ui/src/index.css`](../../zeus-os/ui/src/index.css).
Classic Blue and other dark shells are opt-in via Appearance.

Live SoT in Machina: `web/src/styles/zeus-parity.css` (re-port from Zeus; do not re-derive).
Legacy aliases: `web/src/styles/machina-daylight.css` (superseded when they conflict).
Story / Browse layout primitives: `web/src/styles/machina-apple-ux.css`.

Dark product theme is **Classic Blue** (`html[data-ui-shell='default']`, `data-theme=tahoe`)
— Mist Blue CTAs on graphite, not System Blue / Zyvor Carbon.

Login follows Zeus Apple Account shell ([`zyvor-premium-login.css`](../../web/src/styles/zyvor-premium-login.css)):
`#f5f5f7` paper, SF Pro / system display stack, hero wordmark **machina**, Zyvor tile icon-only.

Shell / nav / tiers: [APPLE-UX-CONTRACT.md](APPLE-UX-CONTRACT.md).

## Palette (iPhone 17 / 17 Pro)

| Finish | Role |
|---|---|
| **apple.com Blue** (`#0071e3`) | Intent / primary CTAs / links / focus / sidebar active (`--primary` / `--accent` / `--link`) |
| **Sage** | Confirmed-good only |
| **Lavender** | AI accent sparingly |
| **White** | Cards / elevated work surfaces (`.tahoe-glass-card`) |
| **Black** | Graphite text / Classic Blue canvas |
| **Cosmic Orange** (Pro) | Warn / deviation |
| **Silver** (Pro) | Neutral tracks / hairlines |

Hover / pressed CTA blues (AirPods CSS): `#0077ed` / `#006edb`. Dark-mode links: `#2997ff`.

### Box type (Apple shop `.form-selector`)

Card / panel font colors match [Apple TV buy flow](https://www.apple.com/in/shop/buy-tv/apple-tv-4k/64gb) 1:1.
Cascade winners: `zeus-parity.css` `:root` + `html[data-theme='tahoe-light']` (mirrored in `machina-daylight.css` / `.light-glass`).
`.tahoe-glass-card` / `.glass-card` set `color: var(--text-primary)`.

| Token | Light | Dark |
|---|---|---|
| `--text-primary` | `#1d1d1f` | `#f5f5f7` |
| `--text-secondary` | `#6e6e73` | `#a1a1a6` |
| `--text-muted` | `#86868b` | `#86868b` |

Status / deviation icons may use `--amber` / `--verdant` / `--accent` — never slate hex for body type.

### Story type ([AirPods](https://www.apple.com/airpods/))

`.apple-display` / `.apple-lede` use SF Pro Display metrics (clamp ~40–64px hero, ~17–21px lede) with the box-type ink colors above.

## Law 0 — Elevation runs up, not down

| Level | Classic Blue | Tahoe Light |
|---|---|---|
| page | `#000` graphite | flat white `#ffffff` (apple.com Support paper — see below) |
| panel | `#1d1d1f` | soft mist panels |
| card | lighter still | `#FFFFFF` + hairline |
| popover | lightest | white + soft shadow |

**Reference: support.apple.com guide pages** (e.g. the iPod touch User Guide) —
captured live: body `#ffffff` flat, top nav translucent near-white
`rgba(250,250,252,0.8)`, hero directly under the nav a single subtle
`linear-gradient(#ffffff 0%, #f2f2f2 100%)`, everything else flat white. The
earlier "Magichromatic washes" (multiple full-opacity saturated radial-
gradient color blooms layered on `.tahoe-mesh` / `.mac-desktop-root` on every
page) were an intentional earlier direction but read as visibly diverging
from apple.com's own product pages — superseded by this flatter, single-
gradient treatment. `--page-bg` / `--surface-0` are `#ffffff`; `.tahoe-mesh`
is disabled (`opacity: 0`) in Tahoe Light.

**A grey panel on a white page is always a bug.**

## Law 1 — Color is deviation

Nominal values are graphite. Mist Blue = intent only.

## Invariants

- Terminals / Console Hub / Host SSH cinema stay carbon islands (chrome / backdrop) — **type** still uses Apple `--text-*` (no `#64748b` / slate hex).
- Theme attribute: `html[data-theme=tahoe-light]` when Tahoe Light is selected.
- Classic Blue: `html[data-ui-shell=default]`.
- Story pages use `apple-story-stack` / `apple-metric-band` — not dense bordered tile grids.
- Primary CTAs use apple.com blue `#0071e3` (`.btn-primary` / `.tahoe-btn-primary`). Do not invent other blues (`blue-600`, Mist as CTA fill).
