# Migration Assistant

## Purpose

Migration Radar: bring VMs into Machina from VMware vCenter, ESXi, OVF/OVA, VMDK or cloud images. HyperSDK discovers and converts source VMs; GuestKit checks disks offline before and after conversion.

## When to use it

- Plan a move off VMware: scan a vCenter or ESXi host and review what will convert cleanly
- Import a single OVF/OVA, VMDK or cloud image
- Follow GuestKit offline inspection and doctor jobs

## How to get there

- Route: `/platform/migration`
- Nav: **Platform → Migration Assistant** (or spotlight / Finder search)

## What you can do

1. Pick a source: **VMware vCenter**, **ESXi Host**, **OVF / OVA File**, **VMDK File** or **Cloud Image**.
2. **Scan & migrate**: run a scan; **Scan results** list discovered VMs with **Migration Advisor Warnings** (drivers, firmware, disk layout).
3. For one VM, **Open import wizard** (single-VM import).
4. **GuestKit jobs** (`?tab=jobs`) is the queue of offline inspect / doctor / migrate-plan jobs; enter a disk path such as `/var/lib/libvirt/images/vm.qcow2` to run one.
5. **HyperSDK proxy explorer** lets admins call provider-specific HyperSDK API paths directly.

## If something is wrong

- **HyperSDK or GuestKit tabs disabled:** the integration is not enabled on the controller (`GUESTKIT_ENABLED`, HyperSDK settings).
- **No scan yet:** pick a source and provide its credentials first.

## Related pages

- [Import guest VM](../core/import.md)
- [Placement & HA](platform-placement.md)
- [Machine Finder](platform-vms.md)
- [Getting Started](../../getting-started.md)
- [Dashboard](../core/home.md)
- [Mission Control](platform.md)
- [Page index](../../PAGE_INDEX.md)
