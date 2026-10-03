---
sidebar_position: 3
title: Kubernetes CNI
description: machina-cni replaces Cilium, flannel and kube-proxy on clusters Machina creates.
---

# Kubernetes CNI

When Machina bootstraps a Kubernetes cluster it installs k3s without flannel,
kube-proxy, network policy or servicelb, and enables **`machina-cni`** instead.

## What you get

- **Pod networking**: one veth per pod, a `/32` (and `/128` in dual-stack) per
  pod, direct routes between nodes, outbound masquerade.
- **NetworkPolicy**: label selectors, `ipBlock` CIDRs and ports, enforced in
  eBPF.
- **Services**: kernel load balancing with Maglev (see
  [Load balancing](load-balancing.md)).
- **Cilium policy migration** (opt-in, `MACHINA_CNI_CILIUM_POLICIES=1`):
  existing `CiliumNetworkPolicy` and `CiliumClusterwideNetworkPolicy` objects are
  enforced at L3/L4. Rules that cannot be honoured exactly (L7, FQDN, deny
  rules) are downgraded with a warning, so you can migrate first and rewrite
  later.

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `MACHINA_CNI_CLUSTER_CIDR` | `10.42.0.0/16` | IPv4 pod range |
| `MACHINA_CNI_CLUSTER_CIDR6` | unset | Set to enable dual-stack |
| `MACHINA_CNI_LB_MODE` | `snat` | `dsr` for direct server return |
| `MACHINA_CNI_XDP` | off | NodePort at XDP |
| `MACHINA_CNI_CILIUM_POLICIES` | off | Enforce Cilium policy CRDs |
| `MACHINA_CNI_MTU` | auto | Pod MTU |

Inspect the state with `GET /api/v1/bpf/cni` or the **Service LB** tab.
