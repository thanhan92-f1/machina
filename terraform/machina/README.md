# Machina Platform Terraform (GA v1 schemas)

Terraform resource schemas are exported from the controller API. Use the HTTP provider example until the native `zyvor/machina` provider is published to the Registry.

## Quick start

```bash
export MACHINA_CONTROLLER_URL=http://127.0.0.1:5093
export MACHINA_CONTROLLER_USER=admin
export MACHINA_CONTROLLER_PASS=...

cd terraform/machina/examples
terraform init
terraform plan
```

## Schema API

```bash
./scripts/platformctl developer
curl -s -u "$MACHINA_CONTROLLER_USER:$MACHINA_CONTROLLER_PASS" \
  "$MACHINA_CONTROLLER_URL/api/v1/developer/terraform/schema" | jq .
```

## Resources (v1)

| Name | Kind | API |
|------|------|-----|
| `machina_vm` | resource | `POST /api/v1/vms` |
| `machina_host` | data | `GET /api/v1/hosts` |
| `machina_storage_pool` | resource | `POST /api/v1/storage/pools` |
| `machina_network` | resource | `POST /api/v1/networks` |

See [`examples/README.md`](examples/README.md).
