# Daylight Contract — Machina light theme (Zeus OS 1:1)

Shipped light shell is Magichromatic Tahoe Light
(`html[data-theme='tahoe-light']`) — **exact port** from
[`../zeus-os/ui/src/index.css`](../../zeus-os/ui/src/index.css).

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
| **Mist Blue** | Intent / primary / links (`--primary` / `--plasma`) |
| **Sage** | Confirmed-good only |
| **Lavender** | AI accent sparingly |
| **White** | Cards / elevated work surfaces (`.tahoe-glass-card`) |
| **Black** | Graphite text / Classic Blue canvas |
| **Cosmic Orange** (Pro) | Warn / deviation |
| **Deep Blue** (Pro) | Migrating / deep info |
| **Silver** (Pro) | Neutral tracks / hairlines |

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

## Law 0 — Elevation runs up, not down

| Level | Classic Blue | Tahoe Light |
|---|---|---|
| page | `#000` graphite | mist paper `210 42% 95%` + Magichromatic washes |
| panel | `#1d1d1f` | soft mist panels |
| card | lighter still | `#FFFFFF` + hairline |
| popover | lightest | white + soft shadow |

**A grey panel on a white page is always a bug.**

## Law 1 — Color is deviation

Nominal values are graphite. Mist Blue = intent only.

## Invariants

- Terminals / Console Hub / Host SSH cinema stay carbon islands (chrome / backdrop) — **type** still uses Apple `--text-*` (no `#64748b` / slate hex).
- Theme attribute: `html[data-theme=tahoe-light]` when Tahoe Light is selected.
- Classic Blue: `html[data-ui-shell=default]`.
- Story pages use `apple-story-stack` / `apple-metric-band` — not dense bordered tile grids.
- Primary CTAs use apple.com blue `#0071e3` (`.btn-primary` / `.tahoe-btn-primary`). Do not invent other blues (`blue-600`, Mist as CTA fill).
