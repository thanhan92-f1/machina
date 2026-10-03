# machina Documentation

Enterprise Linux hypervisor management platform: single-host daemon, multi-host controller, per-host agent, and a React web UI.

## Start here

| Goal | Document |
|------|----------|
| **Engineering onboarding** (day-by-day plan to a verified first deploy) | [ENGINEERING_ONBOARDING.md](ENGINEERING_ONBOARDING.md) |
| **Customer site readiness** (pilot gate, checklist, acceptance tests) | [CUSTOMER_SITE_READINESS.md](CUSTOMER_SITE_READINESS.md) |
| **Production readiness** | [PRODUCTION_READINESS.md](PRODUCTION_READINESS.md) |
| **Handbook** (product, admin, FAQ, troubleshooting) | [handbook/README.md](handbook/README.md) |
| **Customer docs** (page-by-page UI manual, PDFs) | [customer/README.md](customer/README.md) |
| **User journeys and acceptance criteria** | [USER_STORIES.md](USER_STORIES.md) |

## Operating Machina

| Topic | Document |
|-------|----------|
| Day-to-day runbook | [runbook.md](runbook.md) |
| Platform runbooks and certification matrix | [platform-runbooks.md](platform-runbooks.md), [platform-cert-matrix.md](platform-cert-matrix.md) |
| Platform architecture | [platform.md](platform.md) |
| Deploy plan | [deploy-plan.md](deploy-plan.md) |
| Fleet and high availability | [fleet.md](fleet.md), [fleet-ha.md](fleet-ha.md) |
| Compliance hardening | [compliance-hardening.md](compliance-hardening.md) |
| Packaging and remote builds | [PACKAGE_BINARY_REMOTE.md](PACKAGE_BINARY_REMOTE.md), [CLIENT_BUNDLE_POLICY.md](CLIENT_BUNDLE_POLICY.md), [macos-build.md](macos-build.md) |

## Authentication and integrations

| Topic | Document |
|-------|----------|
| LDAP sign-in | [ldap-auth.md](ldap-auth.md) |
| OIDC and run-as-user | [oidc-run-as-user.md](oidc-run-as-user.md), [oidc-effective-linux-user.md](oidc-effective-linux-user.md) |
| Atlas storage (Ceph / NFS / ZFS volumes) | [atlas-storage.md](atlas-storage.md) |
| KubeVirt migration | [kubevirt-migration.md](kubevirt-migration.md) |
| SOC integrations | [soc-integrations.md](soc-integrations.md) |
| Built-in RDP, console hub, cinema mode | [builtin-rdp.md](builtin-rdp.md), [consolehub-architecture.md](consolehub-architecture.md), [machina-cinema-mode.md](machina-cinema-mode.md) |
| Guides (AD, observability, VM access, integrations) | [guides/](guides/) |

## Web UI and design

Machina's look follows the Netra design: two themes only (light and dark), apple.com blue `#0071e3`, flat 18px cards, neutral hairlines.

| Topic | Document |
|-------|----------|
| **UX contract** (surface tiers, laws, theme map, Netra alignment) | [design/APPLE-UX-CONTRACT.md](design/APPLE-UX-CONTRACT.md) |
| **Light and dark tokens** | [design/DAYLIGHT-CONTRACT.md](design/DAYLIGHT-CONTRACT.md) |
| UX author guide (shell, login, tiers, sidebar and icon rail) | [ux.md](ux.md) |
| UX end-to-end coverage and API-to-UI coverage | [ux-e2e-coverage.md](ux-e2e-coverage.md), [api-ux-coverage.md](api-ux-coverage.md) |
| Customer feature guide | [machina-customer-feature-guide.md](machina-customer-feature-guide.md) |

The design contracts are enforced with `node web/scripts/ux-audit.mjs` (contrast, clipping, accessible names, headings, tap targets); see the contract for how to run it against a baseline.

## API reference

OpenAPI specs: [openapi-daemon.json](openapi-daemon.json), [openapi-controller.json](openapi-controller.json). Route manifests used by the coverage checks: [api-ux-route-manifest.json](api-ux-route-manifest.json), [ux-wiring-live-manifest.json](ux-wiring-live-manifest.json).

Live e2e runs write `docs/e2e-last-run.json` and `docs/ux-wiring-live-report.json`; those files are generated and not committed.

## Licensing and assets

| Topic | Document |
|-------|----------|
| Subscription model (Zyvor Production License) | [SUBSCRIPTION-MODEL.md](SUBSCRIPTION-MODEL.md), [legal/](legal/) |
| Social and launch cards (LinkedIn, X, GitHub preview) | [social/README.md](social/README.md) |
| Client presentations | [client-presentations/](client-presentations/) |

## User stories

Persona-based journeys with acceptance criteria: **[USER_STORIES.md](USER_STORIES.md)**

| Persona | Focus |
|---------|-------|
| Alex (Hypervisor Admin) | Manage VMs, networks, storage on bare metal |
| Morgan (Infra Engineer) | API automation and scheduled actions |
| Jordan (NOC Operator) | Live consoles and fleet metrics |

## Ecosystem

Part of the [Zyvor / HyperSDK platform stack](https://zyvor.dev):

| Product | Role |
|---------|------|
| **hypercluster** | Kubernetes bootstrap |
| **machina** | Bare-metal hypervisor OS |
| **zeus-os (v9s)** | Cloud / KubeVirt control plane |
| **forge** | AI infrastructure on K8s |
| **hypersdk / hyper2kvm** | VM migration |
| **guestkit** | Offline VM assurance |
| **Aether** | Runtime portability |
| **hermes** | Application layer for K8s |

See also: [../README.md](../README.md)
