# Zyra agent, Boot Doctor and guest-agent setup

What shipped, how it stays safe, and how to turn it on.

## Boot Doctor (VM detail → Overview)

Shown when a machine is not running, has a last error, or you arrive with `?action=bootdoctor`
(also in the command palette: "Boot Doctor").

1. **Check disk** — shuts the guest down cleanly (never force-stops; gives up after 2.5 min and changes nothing),
   then runs `guestkit doctor --explain -o json` on its disk.
2. **Preview the fix** — `guestkit repair --fix boot --dry-run`; nothing is written.
3. **Repair now** — gated behind a confirm box. Backs the disk up first (`--backup`), repairs, starts the VM and runs a
   health check. The backup path is shown so the repair can be undone by restoring it.

Needs GuestKit on the hypervisor that owns the VM. Without it the card says so instead of failing.

Daemon API (same host as the VM; VM must be powered off):

| Route | Purpose |
|-------|---------|
| `GET  /api/v1/guest-repair/capabilities` | `{cli_found, cli_path, agent_binary, agent_binary_found}` |
| `POST /api/v1/vms/{name}/guest-repair/diagnose` | offline doctor report |
| `POST /api/v1/vms/{name}/guest-repair/apply` | `{dry_run, backup}` |
| `POST /api/v1/vms/{name}/guest-agent/inject` | install the guest agent into the disk |

## Guest-agent setup

VM detail → "Set up guest agent" (or the fleet fix-it list). Routes, easiest first:

- **Install automatically** (Linux guests): clean shutdown → `guestkit agent-inject` → start → verify the agent answers.
  Needs `[libvirt] guestkit_agent_binary` in `/etc/machina/config.toml` pointing at the static musl agent
  (`/usr/local/lib/machina/guestkit-agent`). The host CLI must be built with `--features agent`.
- Manual routes (cloud-init, package, ISO) are listed in the same dialog.

The automatic route checks the daemon supports it *before* shutting anything down, and starts the VM again if the
install fails after a shutdown it caused.

## Verify and Undo (Zyra approvals → Recent actions)

Every executed AI action records the VM's state before it ran (`ai_actions`, migration `031`). Each row has
**Check it worked** (re-reads live state) and **Undo** where a safe inverse exists:

| Action | Undo |
|--------|------|
| `enable_ha` | `disable_ha` (only if HA was off before) |
| `start_vm` | `shutdown_vm` (only if the VM was off before) |

Controller routes: `GET /api/v1/ai/actions/history`, `POST /api/v1/ai/actions/{id}/verify|undo`.
Older controllers lack them; the UI hides the section rather than erroring.

## Ask Zyra (agent loop)

Zyra approvals → **Ask Zyra to take care of something**. `POST /api/v1/ai/agent/run {prompt}` runs a bounded
tool-calling loop (max 6 steps) over your configured provider (Anthropic, or OpenAI-compatible incl. Ollama).

- **Read tools:** `list_vms`, `list_hosts`, `recent_events`.
- **Write tool:** `propose_action` (`start_vm`, `enable_ha`, `create_backup`, `install_guest_tools`). It only
  **queues** an item in the approval queue with risk "Review required" (so autopilot never auto-runs it) and source
  `zyra-agent`. Nothing changes until a person approves it, and execution is audited like any other action.
- Tool output is wrapped as untrusted data, so text inside a VM name or event cannot give the model instructions.

Turn it on: Settings → AI Providers, add a provider and key. Until then the endpoint answers
"Zyra AI is turned off". Set `MACHINA_API_KEY_MASTER_KEY` to encrypt stored keys.

## Tests

- Web: `web/e2e/platform-zyra-agent.spec.ts`, `web/e2e/platform-boot-doctor.spec.ts` (mocked API),
  vitest for `summariseDoctorOutput` and the agent response adapters.
- Rust: unit tests in `controller/src/engine/ai/agent_loop.rs` and `action_audit.rs`
  (run `cargo test -p machina-controller` on a Linux host).
- Live: test only on a throwaway VM, never a production machine.
