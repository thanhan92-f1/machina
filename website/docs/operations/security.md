---
sidebar_position: 3
title: Security
description: Authentication, RBAC, audit, TLS and how to report vulnerabilities.
---

# Security

## Sign-in

- **PAM** for local Linux accounts (default service `sshd`).
- **OIDC** single sign-on, with group claims mapped to roles.
- **SAML** and **LDAP** for enterprise directories.

## Access control and audit

Role-based access applies to every API route and every console session. Actions are written to an audit log, and
audit lines can be signed and shipped off-host.

## Transport

- The daemon serves HTTPS on `:5092`; the installer enables TLS with a certificate under `/etc/machina/ssl/`.
- The controller talks to each `machina-agent` over gRPC with TLS.
- The agent binds to `127.0.0.1` by default.

## Secrets

- The controller generates a random JWT secret per process when `MACHINA_JWT_SECRET` is unset, and refuses to boot on
  the well-known development secret unless explicitly allowed.
- LLM provider keys are encrypted at rest with AES-256-GCM when `MACHINA_API_KEY_MASTER_KEY` is set.

## Network enforcement

With the [Netra integration](../core-concepts/integrations.md#netra-network-enforcement), deny rules are enforced in
the kernel with eBPF and every enforcement window is a lease that fails open when it expires.

## Reporting a vulnerability

Report privately as described in
[SECURITY.md](https://github.com/zyvorai/machina/blob/main/SECURITY.md). Please do not open public issues for
security problems.
