---
sidebar_position: 6
title: Integrations
description: Storage, network security, containers, Kubernetes and automation integrations.
---

# Integrations

Each integration is off by default and enabled with environment variables on the controller.

## Atlas storage

Puts VM disks on Ceph RBD, NFS or ZFS volumes managed by the Atlas storage control plane, with snapshot, backup and
restore.

```bash
ATLAS_ENABLED=1
ATLAS_BASE_URL=http://127.0.0.1:5110
ATLAS_TOKEN=<service-account JWT>
```

Create a VM on Atlas storage by passing `atlas_root_disk: true` to `POST /api/v1/vms`.

## Netra network enforcement

[Netra](https://zyvorai.github.io/netra/) applies kernel-level IP and CIDR deny rules with eBPF. Machina requests a
lease when it applies a policy, and Netra reverts to observe mode (fail-open) when the lease expires, regardless of
controller state.

```bash
NETRA_ENABLED=1
NETRA_BASE_URL=http://127.0.0.1:30870
NETRA_API_KEY=<token>
NETRA_ENFORCE_LEASE=15m
```

## PacketWolf

Network intelligence, SIEM export and TC egress allowlists.

```bash
PACKETWOLF_ENABLED=1
PACKETWOLF_BASE_URL=http://127.0.0.1:9191
PACKETWOLF_API_KEY=<key>
```

## Containers and Kubernetes

- **Containers.** Local Podman or Docker containers and Podman pods run next to your VMs (configure the socket in
  `[vessel]`).
- **KubeVirt.** Inventory of KubeVirt VMs and a documented migration path.

## Migration into KVM

HyperSDK and hyper2kvm convert guests from other platforms into KVM; GuestKit checks them offline before cutover.

## Automation

- [Terraform provider](https://github.com/zyvorai/machina/blob/main/terraform/machina/README.md)
- [TypeScript SDK](https://github.com/zyvorai/machina/blob/main/sdk/typescript/README.md)
- OpenAPI spec and the `machinactl` CLI
