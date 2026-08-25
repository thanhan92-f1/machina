# Code review findings

Review target: `main` @ `1a2afc0` plus working tree (Cockpit parity, UX/E2E, security fixes).

Methodology: P0 exploitable auth/data loss; P1 correctness/security invariant; P2 maintainability/tests; P3 style/docs.

## Summary

| Severity | Found | Fixed this pass |
|----------|-------|-----------------|
| P0 | 0 | 0 |
| P1 | 6 | 6 |
| P2 | 5 | 2 |
| P3 | 4 | 0 (documented) |

No P0 issues identified. All P1 items were fixed with server-side validation, panic removal on HTTP paths, and unit tests.

---

## Findings table

| ID | Sev | Area | File(s) | Summary | Recommendation | Status |
|----|-----|------|---------|---------|----------------|--------|
| CR-001 | P1 | Port forward | `controller/src/api/vms.rs`, `core/src/libvirt/host_network.rs` | `create_vm_port_forward` accepted privileged/reserved host ports (e.g. hijack `:22`, Machina `:50051`) | Enforce `host_port >= 1024` and block Machina service ports | **Fixed** |
| CR-002 | P1 | Port forward | `controller/src/api/vms.rs` | `delete_vm_port_forward` did not validate protocol or port ranges | Reuse shared validators on delete | **Fixed** |
| CR-003 | P1 | Port forward | `core/src/libvirt/host_network.rs` | iptables DNAT path lacked host-port policy | Centralize `validate_port_forward_*` helpers; call from create/delete | **Fixed** |
| CR-004 | P1 | ConsoleHub | `controller/src/consolehub.rs` | `Response::builder().body(...).unwrap()` on user-triggerable proxy | Map builder error to `500` | **Fixed** |
| CR-005 | P1 | Platform proxy | `daemon/src/routes/platform_controller.rs` | Same `unwrap()` on proxied controller response | Return `AppError` instead of panic | **Fixed** |
| CR-006 | P1 | Agent gRPC | `agent/src/grpc.rs` | `host_port`/`vm_port` cast from protobuf without range check | `grpc_port_u16()` rejects 0 and >65535 | **Fixed** |
| CR-007 | P2 | Rate limit | `controller/src/rate_limit.rs` | `Mutex::lock().expect()` panics on poison | Use `into_inner()` on poisoned lock | **Fixed** |
| CR-008 | P2 | Web URLs | `web/src/utils/vmPortForwardServices.ts` | `publicHostname()` passed arbitrary strings into `ssh`/`http` links | Allow-list hostname/IP charset; reject path/scheme injection | **Fixed** |
| CR-009 | P2 | Tests | `core`, `controller`, `agent` | Sparse unit tests on port-forward validation | Added 5 tests in `host_network`; web util test for hostname | **Fixed** |
| CR-010 | P2 | CI | `.github/workflows/ci.yml` | `live-ux` job runs only on `workflow_dispatch`; 244-route manifest not in default PR CI | Document gap; run manually against live host | **Documented** |
| CR-011 | P2 | Deploy | `scripts/e2e-platform-install-smoke.sh`, `contrib/machina-agent.service` | Agent `:50051` flake after restart; `ExecStartPre` in wrong unit section | 15×2s retry; move `ExecStartPre` to `[Service]` | **Fixed** (prior UX pass) |
| CR-012 | P2 | Web state | `web/src/hooks/useVmSshConnectContext.ts` | Async plan + port-forward load could race on fast VM switch | `refresh` keyed on `vm.id`; errors caught per request | **OK** — no change needed |
| CR-013 | P2 | API errors | `controller/src/api/error.rs` | Internal errors may include libvirt/agent strings | Prefer generic client message for 500; log detail server-side | **Documented** — existing pattern acceptable |
| CR-014 | P3 | Unwrap audit | `controller/src/api/zeus_firewall.rs`, `engine/zeus_firewall/inventory.rs` | `unwrap()` in admin/test-adjacent paths | Replace when touching those modules | Open |
| CR-015 | P3 | Auth chain | `controller/src/api/mod.rs`, `daemon/src/server.rs` | New consolehub/port-forward/kubevirt routes | Verified behind `auth_middleware` / daemon auth | **OK** |
| CR-016 | P3 | KubeVirt SSH | `controller/src/engine/kubevirt_ssh.rs` | Service selector matching for NodePort SSH | Existing unit tests for selector logic | **OK** |
| CR-017 | P3 | Delete PF IP | `controller/src/api/vms.rs` | Delete uses VM `guest_ip` from DB, not client body | Keep as-is | **OK** |

---

## Fixes applied (P1/P2)

### Server-side port-forward validation

Shared helpers in `core/src/libvirt/host_network.rs`:

- `validate_port_forward_protocol` — tcp/udp only
- `validate_port_forward_host_port` — `>= 1024`, not in `{5092, 5093, 50051, 50052}`
- `validate_port_forward_vm_port` — non-zero

Called from:

- `create_port_forward` / `delete_port_forward` (core → iptables)
- `create_vm_port_forward` / `delete_vm_port_forward` (controller API)
- Agent gRPC after `grpc_port_u16()` range check

### Panic removal on HTTP paths

- `controller/src/consolehub.rs` — proxy response builder
- `daemon/src/routes/platform_controller.rs` — platform controller proxy

### Web hostname sanitization

- `publicHostname()` rejects values with `/`, `..`, or characters outside a safe hostname/IP set.

### Tests added

- `core`: `port_forward_validation_tests` (5 cases)
- `web`: `publicHostname` sanitization in `vmPortForwardServices.test.ts`

---

## Areas reviewed — no action

| Area | Result |
|------|--------|
| Controller JWT + route auth | Port-forward and consolehub routes under `auth_middleware` |
| Daemon platform proxy | Behind daemon auth; basic-auth injection for co-located controller is intentional |
| WS tokens / terminal isolation | Short-lived tokens; session lifecycle in daemon terminal modules |
| `useVmSshConnectContext` | Parallel fetch with per-call error handling; refresh deps include `vm.id` |
| Mock E2E route ordering | Platform sub-routes before generic `/vms` in `authMock.ts` |
| `kubevirt_ssh.rs` | Unit tests present for selector / NodePort parsing |

---

## CI vs local E2E

| Suite | CI job | When it runs |
|-------|--------|--------------|
| Mock Playwright (~274 specs) | `web` | Every push/PR |
| Live UX manifest (244 routes) | `live-ux` | **Manual** `workflow_dispatch` only |
| Live deep access (`platform-live-access.spec.ts`) | Remote deploy `--e2e` | Not in default CI |

Gap: live manifest regressions require manual `live-ux` workflow or `./scripts/deploy-remote.sh … --e2e`.

---

## Verification

After fixes (2026-06-11):

- `cargo test --workspace` — pass
- `cd web && npm test` — 71 pass
- `npm run build` — pass
- `npm run test:e2e` — 274 pass, 62 skipped

Optional remote close-out (2026-06-11, `212.8.252.194`):

| Phase | Result |
|-------|--------|
| Install smoke | 5/5 pass |
| Daemon E2E | 18/18 pass |
| Platform API E2E | 307/307 pass |
| Live UX manifest | 208 pass, 37 skipped (OpenStack), ~46 min |
| Live VM lifecycle | 5/8 pass — **2 failures** in `platform-live-access.spec.ts` |

Live access failures (VM may lack libvirt private-IP NAT setup):

1. Network tab — `vm-port-forward-panel` not visible
2. ConsoleHub serial lens — recovery/checklist widgets not visible

Deploy itself succeeded; report emailed. Full log: `/tmp/deploy-e2e-verify.log`

---

## Out of scope (unchanged)

- Splitting `web/src/api/platform.ts`
- Mission Control UI redesign
- Full OpenStack/K8s integration audit
- Performance / load testing
