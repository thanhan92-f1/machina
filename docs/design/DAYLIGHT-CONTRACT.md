# Daylight Contract — Machina light theme (Zeus OS 1:1)

Shipped light shell is Magichromatic Tahoe Light
(`html[data-theme='tahoe-light']`) — **exact port** from
[`../zeus-os/ui/src/index.css`](../../zeus-os/ui/src/index.css).

Live SoT in Machina: `web/src/styles/zeus-parity.css` (re-port from Zeus; do not re-derive).
Legacy aliases: `web/src/styles/machina-daylight.css` (superseded when they conflict).

Dark product theme is **Classic Blue** (`html[data-ui-shell='default']`, `data-theme=tahoe`)
— Mist Blue CTAs on graphite, not System Blue / Zyvor Carbon.

Login follows Zeus Apple Account shell (`zyvor-premium-login.css` + theme paper).

## Palette (iPhone 17 / 17 Pro)

| Finish | Role |
|---|---|
| **Mist Blue** | Intent / primary / links (`--primary` / `--plasma`) |
| **Sage** | Confirmed-good only |
| **Lavender** | AI accent sparingly |
| **White** | Cards / elevated work surfaces |
| **Black** | Graphite text / Classic Blue canvas |
| **Cosmic Orange** (Pro) | Warn / deviation |
| **Deep Blue** (Pro) | Migrating / deep info |
| **Silver** (Pro) | Neutral tracks / hairlines |

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

- Terminals / Console Hub cinema stay carbon islands.
- Theme attribute: `html[data-theme=tahoe-light]` when Tahoe Light is selected.
- Classic Blue: `html[data-ui-shell=default]`.
