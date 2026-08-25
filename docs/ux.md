# Machina web UX conventions

Shared patterns for integration states, errors, and empty lists in `web/src/`.

## Integration phases

Optional backends use the same mental model:

| Phase | Meaning |
|-------|---------|
| `off` | Disabled in daemon config |
| `needsSetup` / `needsWire` | Enabled but not configured (OpenStack only) |
| `unreachable` | Configured but API not answering |
| `live` | Healthy; operational UI enabled |

Hooks:

- [`useOpenStackConnection`](../web/src/hooks/useOpenStackConnection.ts) — `platform-info` + `GET /openstack/status`
- **Help → About** — top-nav **Help** menu (`?` shortcuts, **About** tab with [zyvor.dev](https://zyvor.dev) links and copyright)
- **TUI OpenStack** — sidebar group (+ Create instance wizard, Glance images, instances), colon commands for Nova/Glance lifecycle, Cinder attach/detach, floating IPs, security groups (`:openstack create` or Enter on “+ Create instance”; see `?` help)
- [`useHypersdkConnection`](../web/src/hooks/useHypersdkConnection.ts) — `GET /hypersdk/status`
- K8s — [`K8sConnectionErrorBanner`](../web/src/components/K8sConnectionErrorBanner.tsx) + [`k8sErrors.ts`](../web/src/utils/k8sErrors.ts)

Gate destructive or cloud-side actions on `phase === 'live'`. Nav and command palette may still list destinations with disabled labels (“Wire cloud first”).

## Primitives

| Component | Use when |
|-----------|----------|
| [`EmptyState`](../web/src/components/EmptyState.tsx) | Zero rows in a list; include primary CTA |
| [`PlatformEmptyState`](../web/src/components/platform/PlatformEmptyState.tsx) | Platform Mac pages — glass panel empty state with CTA |
| [`semanticColors.ts`](../web/src/utils/semanticColors.ts) | Status/task/host tone helpers — prefer over raw Tailwind green/amber/red |

### Semantic color helpers (Batch 57–60)

| Helper | Use when |
|--------|----------|
| `statusToneClass(tone)` | Inline text for ok / warn / error / info / neutral |
| `statusBadgeClasses(tone)` | Pill/chip backgrounds (host health, compliance grades) |
| `statusPillClasses(tone)` | Bordered action chips (K8s node ops, KubeVirt live console) |
| `statusSurfaceClasses(tone, extra?)` | Bordered callout panels (readiness, drift, destructive hints) |
| `statusDestructiveButtonClasses(extra?)` | Secondary destructive actions (OpenStack delete/dissociate) |
| `hubLinkClasses()` | Platform hub links and cross-shell navigation accents (info tone) |
| `navActiveChipClasses()` | Active filter pills / segmented nav chips |
| `riskTone(risk)` | Firewall / security risk strings → critical/high → error, warning/medium → warn, low/info → ok |
| `utilizationBarClass(percent, thresholds?)` | Gauge/progress bars for CPU, memory, PSI, thermal |

**Intentionally unchanged:** primary CTAs (`bg-blue-600`), [`ChoiceCards`](../web/src/components/ChoiceCards.tsx) selection accents (wizard tone palette, not operational status), ApiDocs HTTP method colors, orange Zeus branding in Help.

**Help — Platform guide:** Both Platform menubar ([`PlatformMacAppMenus`](../web/src/components/platform/mac/PlatformMacAppMenus.tsx)) and classic Navbar ([`Navbar`](../web/src/components/Navbar.tsx)) open the in-app Help dialog **Platform** tab via `onOpenHelp('platform')`.

**Machine Finder geography:** Mission Control ([`InfrastructureEarthView`](../web/src/components/platform/InfrastructureEarthView.tsx)) links to `/platform/hosts/finder` — four-column site → rack → host → VM browser aligned with `GET /api/v1/fleet/mission`.

**GPU Command Center:** [`PlatformGpuCommandCenter`](../web/src/pages/platform/PlatformGpuCommandCenter.tsx) at `/platform/gpu` — MIG/vGPU/CUDA profile chips from host tags, GPU VM inventory, and CUDA placement advisor (`GET /api/v1/fleet/gpu` + `GET /api/v1/ai/fleet/gpu-placement`).

**Maintenance Mission:** [`PlatformMaintenance`](../web/src/pages/platform/PlatformMaintenance.tsx) **Mission** tab (`?tab=mission`) — 7-step [`BuildStepTimeline`](../web/src/components/BuildStepTimeline.tsx) per host from `GET /api/v1/fleet/maintenance-mission`; operator-confirmed schedule / enter / exit / agent upgrade (no autonomous package apply). Default tab when fleet has pending updates.

**Infrastructure DNA:** [`InfrastructureDnaStrip`](../web/src/components/platform/InfrastructureDnaStrip.tsx) — score ring + grade + pillar chips from `GET /api/v1/fleet/dna` on Platform dashboard (power tier+) and Mission Control header.

**Full Jarvis shell (Phase 57):** [`PlatformJarvisBriefing`](../web/src/components/platform/PlatformJarvisBriefing.tsx) on all tiers — landing intents from `GET /api/v1/ai/jarvis/landing`, inline search opens Spotlight (`⌘Space`), Normal tier hides sidebar when Jarvis shell is on (Control Center toggle).

**Infrastructure Earth globe (Phase 58 v3):** [`InfrastructureEarthGlobe`](../web/src/components/platform/InfrastructureEarthGlobe.tsx) — **WebGL** globe (lazy `three.js`) with **canvas 2D fallback**, per-site health markers, and a site legend (links to Machine Finder) on Mission Control and Machine Finder topology lens.

**Enterprise security strip:** [`EnterpriseSecurityStrip`](../web/src/components/platform/EnterpriseSecurityStrip.tsx) on advanced dashboard; vault sync failures use persistent [`ErrorBanner`](../web/src/components/ErrorBanner.tsx) on [`PlatformEnterprise`](../web/src/pages/platform/PlatformEnterprise.tsx).

| [`ShellBridgeBar`](../web/src/components/ShellBridgeBar.tsx) | Classic / OpenStack / K8s routes — link back to Platform desktop |
| [`JsonInspector`](../web/src/components/platform/JsonInspector.tsx) | Power-user API payloads — human summary first, raw JSON behind toggle |
| [`PlatformIntegrationEmbeds`](../web/src/components/platform/PlatformIntegrationEmbeds.tsx) | Integrations hub — live OpenStack/K8s inventory preview when backends are reachable |
| [`ErrorBanner`](../web/src/components/ErrorBanner.tsx) | Actionable failure with hints + optional copy |
| [`PageHeader`](../web/src/components/PageHeader.tsx) | Title, subtitle, refresh, primary action |
| [`CopyButton`](../web/src/components/CopyButton.tsx) | Wire scripts, kubectl, verify commands |
| [`WizardStepper`](../web/src/components/WizardStepper.tsx) | Multi-step Create VM / Import VM |

## API errors (daemon JSON, HTML, codes)

The daemon returns `{ "error": "…", "error_code": "operation_failed" }` on failure. Proxies or down OpenStack services may return **HTML** instead of JSON.

| Utility | Use when |
|---------|----------|
| [`formatHttpErrorBody`](../web/src/utils/apiError.ts) | Parsing a non-OK `fetch` body (used by [`client.ts`](../web/src/api/client.ts)) |
| [`parseResponseError`](../web/src/api/parseResponseError.ts) | Custom `fetch` calls outside `apiPost` / `readJsonObject` |
| [`formatUserError`](../web/src/utils/apiError.ts) | Any `catch (e: unknown)` shown in toasts or banners |
| [`toastFailure`](../web/src/utils/toastError.ts) | `toastFailure(toast, 'Label', e)` shorthand |

**Do not** display raw `response.text()` or bare `error_code` strings. Toasts run through [`Toast.tsx`](../web/src/components/Toast.tsx), which sanitizes error messages globally.

Page loads: set `loadError` state and show [`ErrorBanner`](../web/src/components/ErrorBanner.tsx) with domain hints ([`openstackHints.ts`](../web/src/utils/openstackHints.ts), [`libvirtHints.ts`](../web/src/utils/libvirtHints.ts), or [`k8sErrors.ts`](../web/src/utils/k8sErrors.ts)). Use `Promise.allSettled` when loading multiple catalogs so one failure does not hide partial data.

Tests: `cd web && npm test` ([`apiError.test.ts`](../web/src/utils/apiError.test.ts)). E2E: `cd web && npm run test:e2e` (Playwright, mocked API).

Rust/TUI: [`core/src/api_error.rs`](../core/src/api_error.rs) mirrors web formatting; TUI HTTP client uses it for status bar messages.

OpenStack-specific: [`OpenStackUnreachablePanel`](../web/src/components/OpenStackUnreachablePanel.tsx), [`openstackHints.ts`](../web/src/utils/openstackHints.ts).

Multi-host VM list: configure `[libvirt] extra_uris` in daemon config; VMs from remote URIs appear with a connection badge (read-only federation; lifecycle on primary/dual connections only).

HyperSDK: [`HypersdkStatusBanner`](../web/src/components/HypersdkStatusBanner.tsx) on migrations and push modals when enabled but unreachable.

## Developer / API Console

- [`PlatformDeveloper.tsx`](../web/src/pages/platform/PlatformDeveloper.tsx) — SDK tab + **API Console** (OpenAPI try-it for all controller routes)
- [`PlatformApiConsole.tsx`](../web/src/components/platform/PlatformApiConsole.tsx) — **Controller | Host** tabs, OpenAPI try-it, agent/ws hints
- Generate specs: `node scripts/generate-openapi.mjs` → [`docs/openapi-controller.json`](openapi-controller.json), [`docs/openapi-daemon.json`](openapi-daemon.json)
- Coverage gate: `cd web && npm run api-ux-coverage:check` (see [`docs/api-ux-coverage.json`](api-ux-coverage.json))

## Live UX → API verification (P12)

Proves buttons and page loads hit working backends on a real host (not mocked Playwright).

```bash
# Regenerate page matrix (~200+ routes, tabs, and dynamic detail sweeps)
node scripts/generate-ux-live-manifest.mjs

# Against remote host (requires PAM credentials)
VSPASS='…' ./scripts/e2e-live-ux-remote.sh sus 212.8.252.194

# Included in deploy when --e2e and VSPASS are set (skip with --skip-live-ux)
VSPASS='…' ./scripts/deploy-remote.sh sus HOST --quick --e2e
```

Report: [`docs/ux-wiring-live-report.json`](ux-wiring-live-report.json) — pass/fail per route with API failure details.

Optional GitHub Actions: workflow_dispatch job `live-ux` (secrets: `LIVE_HOST`, `LIVE_USER`, `LIVE_PASS`).

## Overall UX polish (P14)

Cross-shell presentation pass after backend wiring (P6–P13). See [`backend-ux-wiring-audit.md`](backend-ux-wiring-audit.md) P14 for the full checklist.

| Area | Pattern |
|------|---------|
| Initial fetch | [`PageSkeleton`](web/src/components/PageSkeleton.tsx) — never a blank content area |
| Zero rows | [`PlatformEmptyState`](web/src/components/platform/PlatformEmptyState.tsx) (Platform) or [`EmptyState`](web/src/components/EmptyState.tsx) (Classic/OpenStack/K8s) with at least one CTA |
| API payloads | [`JsonInspector`](web/src/components/platform/JsonInspector.tsx) — summary/table first; raw JSON behind toggle |
| Domain failures | [`formatUserError`](web/src/utils/apiError.ts) + hints ([`libvirtHints`](web/src/utils/libvirtHints.ts), [`openstackHints`](web/src/utils/openstackHints.ts), [`hostErrorPresentation`](web/src/utils/hostErrorPresentation.ts), [`storageErrorPresentation`](web/src/utils/storageErrorPresentation.ts)) |
| Cross-shell nav | [`ShellBridgeBar`](web/src/components/ShellBridgeBar.tsx) on Classic, OpenStack, K8s routes |

**E2E (mocked):**

```bash
cd web && npm run build && npm run test:e2e -- e2e/platform-full.spec.ts e2e/shell-bridge.spec.ts
```

**Manual QA additions (P14):**

| Scenario | Check |
|----------|--------|
| Platform Storage discover with no hosts | Structured banner + link to Hosts |
| Platform Host detail, agent offline | Remediation links to Enroll + classic Node |
| K8s Workloads explorer | Table/summary default; raw JSON toggle |
| OpenStack enabled but unreachable on Migration | OpenStackUnreachablePanel |

## Platform macOS desktop (Wave 3)

Navigation layers are tier-aware to avoid triple nav on tab-heavy routes:

| Layer | Role | When visible |
|-------|------|--------------|
| **Menubar** | App menus, Dynamic Island (fleet + Zeus pending), Control Center | Always on `/platform/*` |
| **Context bar** | Cross-links between hubs | **Normal:** hidden · **Power:** hub roots only (`/platform/operations`, `/platform/resources`, `/platform/zeus/security`, `/platform/integrations`) · **Advanced:** unless route uses in-page [`DetailTabs`](web/src/components/platform/DetailTabs.tsx) |
| **Page header** | [`PlatformPageChrome`](web/src/components/platform/PlatformPageChrome.tsx) title, stats, actions | Every platform page |
| **DetailTabs** | In-app sections with `?tab=` | Tab-heavy pages only |
| **Dock** | Primary app launcher | Always (Normal + Power) |
| **Sidebar** | Full nav tree | Advanced default; Power collapsed icon rail; Normal hidden (Jarvis shell) |

Helpers: [`shouldShowContextBar`](web/src/utils/platformNavRegistry.ts), [`suppressContextBar`](web/src/utils/platformNavRegistry.ts).

**Zeus status:** pending approvals surface in [`PlatformDynamicIsland`](web/src/components/platform/mac/PlatformDynamicIsland.tsx); [`ZyraAmbientBar`](web/src/components/ai/ZyraAmbientBar.tsx) is hidden on `/platform/*`.

**Dashboard tiers:** [`PlatformDashboard`](web/src/pages/platform/PlatformDashboard.tsx) — Normal: Zeus search + greeting + Launchpad; Power/Advanced: header stats + slim Zeus search + Launchpad + collapsed [`PlatformFleetInsights`](web/src/components/platform/PlatformFleetInsights.tsx) (DNA, remediate, enterprise on Advanced).

**Glass tokens:** `--glass-panel` in `main.css` unifies [`MacGlassPanel`](web/src/components/platform/mac/PlatformMacUi.tsx), `.tahoe-glass-card`, and `.platform-mac-panel`.

## Platform lean desktop (Wave 4)

**Naming:** Product shell stays **Machina**; the AI assistant is always **Zyra** ([`aiBrand.ts`](../web/src/config/aiBrand.ts), [`AskZyraButton`](../web/src/components/ai/AskZyraButton.tsx)). Use "Ask Zyra" — not "Ask Machina" or "Copilot" — in user-facing AI entry points.

| Surface | Rule |
|---------|------|
| **Jarvis strip** | Slim search bar + up to 5 intent chips; Normal tier adds greeting; no duplicate action buttons (dock/menubar own Mission Control & Spotlight) |
| **Fleet insights** | Collapsed by default; badge only on header; no approval queue or posture panel on dashboard (Dynamic Island + Security Center) |
| **Classic shell** | Navbar **Zeus** button only — [`ZyraAmbientBar`](web/src/components/ai/ZyraAmbientBar.tsx) hidden when Navbar Zeus is shown |
| **About / tasks** | Marketing copy on Support only; no Recent tasks panel on dashboard |

## Dashboard & shell

- **Help** (top bar) — dropdown: **Keyboard shortcuts** (`?`) and **About** ([`HelpDialog.tsx`](../web/src/components/HelpDialog.tsx), [`ZyvorAbout.tsx`](../web/src/components/ZyvorAbout.tsx)): [zyvor.dev](https://zyvor.dev), product links, copyright © 2026, documentation hub.
- [`Dashboard.tsx`](../web/src/pages/Dashboard.tsx) — integration cards (libvirt, OpenStack, K8s, HyperSDK)
- [`Hero.tsx`](../web/src/components/Hero.tsx) — capability badges reflect phase, not config-only
- Command palette — always list OpenStack routes; sublabel when not live

## Theming

New UI should work in **dark**, **steel**, and **aurora** themes (all dark; aurora uses prismatic accents via `.aurora-theme` in `main.css`). Avoid hard-coded colors that only read on one shell background.

## Login & accessibility

- [`Login.tsx`](../web/src/pages/Login.tsx) — **Machina** branding via [`PremiumLoginShell`](../web/src/components/PremiumLoginShell.tsx) `variant="macos"`; SSO button first when OIDC is enabled; PAM form below; host label from `window.location.hostname`
- Login CSS: [`zyvor-macos-login.css`](../web/src/styles/zyvor-macos-login.css) (default Machina), [`zyvor-secure-login.css`](../web/src/styles/zyvor-secure-login.css) (optional `variant="secure"`), legacy aurora/particles in `zyvor-premium-login.css`
- **URL behavior:** the login page renders outside `BrowserRouter` when unauthenticated. `/` and `/login` both work. After auth, [`AuthContext`](../web/src/contexts/AuthContext.tsx) replaces `/login` with `/`, and authenticated routes register `<Navigate from="/login" to="/" />` so bookmarked `/login` never shows 404
- **Zeus AI shell:** [`AiProvider`](../web/src/contexts/AiContext.tsx) must stay **inside** `BrowserRouter` (uses `useLocation` / `useParams` for ambient route context)
- [`usePrefersReducedMotion`](../web/src/hooks/usePrefersReducedMotion.ts) — skips login orbs/particles; macOS variant omits scanlines/particles by default
- E2E: [`smoke.spec.ts`](../web/e2e/smoke.spec.ts) — `authenticated /login redirects to dashboard`
- [`ConnectionStatus`](../web/src/components/ConnectionStatus.tsx) — `role="status"` + `aria-label` (not color-only)
- [`NotFound.tsx`](../web/src/pages/NotFound.tsx) — dashboard styling + Ctrl+K hint

## Manual QA (Phase 7)

| Scenario | Check |
|----------|--------|
| OpenStack off / needs wire / unreachable / live | Dashboard, Hero, Instances, Settings |
| Zero VMs | VM list EmptyState |
| K8s API down | K8s overview + workloads banner |
| OIDC enabled | Login: SSO primary, password secondary |
| Sign in at `/login` | Lands on dashboard (`/`), not 404 |
| `prefers-reduced-motion` | Login: no orb animation |
| Light / dark / steel | Dashboard, Login, one OpenStack page |

## UX Wave 9 (2026-06)

Compound operating surfaces across admin, security, and observability — briefing stats, `DetailTabs`, `GlassDataTable`, and `PlatformEmptyState` on zero-row panels.

| Route / area | Pattern |
|------------|---------|
| `/platform/users` | `OperatingSurfaceLayout` + user/workspace tabs + `GlassDataTable` |
| `/platform/projects` | Spaces briefing + project table empty state |
| `/platform/api-keys`, `/platform/webhooks` | Command bar + glass table empty states |
| `/platform/observability` | SLO / traces / metrics lens tabs + briefing stats |
| `/platform/events` | Audit log stream + source tabs |
| `/platform/zeus/security/ports`, `/services`, `/activity` | `SecurityLensLayout` + exposure briefing |
| `/platform/applications`, `/enroll`, `/placement`, `/recommendations`, `/fleet-snapshots` | Operations compound surfaces (Wave 3) |
| Hub roots (`/platform/infrastructure`, `/workloads`, `/operations`) | `PageSkeleton` while async stats load |
| Classic (`/vms`, `/networks`, `/events`, `/jobs`) | `PageSkeleton` + `EmptyState` CTAs |
| OpenStack detail routes | `PageSkeleton` replaces full-page `Loader2` |
| K8s overview / workloads | Full-page `PageSkeleton` on initial fetch |

E2E: `cd web && npm run test:e2e -- e2e/platform-admin-ux.spec.ts e2e/platform-observability-ux.spec.ts e2e/platform-security-ux.spec.ts e2e/classic-operator-ux.spec.ts`

Build: `cd web && npm run build`. Deploy: `./scripts/deploy remote user@host --quick` then re-run `openstack-wire-cloud.sh` if install reset config.

**Live E2E (optional):**

```bash
PLAYWRIGHT_LIVE_URL=https://HOST:5092 PLAYWRIGHT_LIVE_USER=sus PLAYWRIGHT_LIVE_PASS=… \
  npm run test:e2e -- e2e/live-host.spec.ts
```

Includes PAM login at `/login` → dashboard when credentials are set.

## Docs

- OpenStack phases: [`openstack.md`](openstack.md)
- Platform VM detail UX: [`guides/platform-vm-detail-ux.md`](guides/platform-vm-detail-ux.md)
- Connect hub & daily access: [`guides/vm-daily-access.md`](guides/vm-daily-access.md)
- Feature QA matrix (F01–F13): [`guides/platform-feature-qa.md`](guides/platform-feature-qa.md)
- Cinema / Studio modes: [`machina-cinema-mode.md`](machina-cinema-mode.md)
- Client presentation decks: [`client-presentations/`](client-presentations/) · `./scripts/generate-client-presentation-pdfs.sh`
