# Compliance and hardening checklist

Use this when preparing Machina for security review (not a certification).

## Network

- [ ] TLS enabled (`[tls]` with org-signed certs)
- [ ] Daemon bound only as needed (`host = 0.0.0.0` vs `127.0.0.1` + reverse proxy)
- [ ] Firewall allows only operator subnets to port 5092
- [ ] No legacy `/ws/v1/ssh/{host}` unless required (`[ssh_terminal].legacy_plain_host_websocket`)

## Authentication

- [ ] PAM or LDAP/OIDC — disable unused methods
- [ ] LDAP: `ldaps://` or STARTTLS; avoid `insecure_tls` in production
- [ ] OIDC: confidential client, short-lived sessions
- [ ] API tokens: least-privilege `scopes` (see automation API)
- [ ] `roles.json` reviewed — unknown users default to readonly

## Host

- [ ] Daemon runs as dedicated user where possible (default install: root for libvirt)
- [ ] `NoNewPrivileges` in systemd unit (see `contrib/machina-daemon.service`)
- [ ] Audit log retained: `/var/lib/machina/audit.log` (optional rotation, syslog, HTTP webhook in `[audit]`)
- [ ] OTLP export configured (`[observability.otlp]` → Alloy / OpenTelemetry Collector)
- [ ] Linux auditd visible via `GET /api/v1/host/linux-audit` and optional `health_avc_threshold`
- [ ] Export audit via `GET /api/v1/audit/export` (NDJSON) to SIEM

## Observability

- [ ] Prometheus scrape configured (`GET /api/v1/prometheus`)
- [ ] Optional Grafana dashboard: `contrib/grafana/machina-overview.json`

## Data

- [ ] Backups scheduled (`machina-backup.timer`)
- [ ] Backup checksum verification periodic

## Not in scope today

