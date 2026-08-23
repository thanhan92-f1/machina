// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

//! OpenStack-COMPATIBLE inbound API surface: lets a real `openstack` CLI, an OpenStack
//! SDK, or Terraform's `openstack` provider point at Machina itself (via `OS_AUTH_URL`)
//! and manage Machina's own libvirt VMs using OpenStack's wire protocol — no external
//! Packstack/RDO OpenStack deployment required.
//!
//! This is the reverse direction of `core/src/openstack/*` + `daemon/src/routes/openstack*.rs`,
//! which are an OUTBOUND client connecting Machina to an already-existing external
//! OpenStack. That code is unrelated and untouched by this module.
//!
//! Handlers here translate OpenStack JSON shapes to/from Machina's existing internal
//! types and delegate to the SAME handlers the native Machina API uses (`api::vms`,
//! `api::networks`, `api::storage`, ...) wherever one already exists, rather than
//! duplicating VM/network/storage lifecycle logic.

pub mod cinder;
pub mod glance;
pub mod keystone;
pub mod neutron;
pub mod nova;
