// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

// Shared copy for the home page, gallery and screenshot strip. Every claim maps to code in the repo.

export const REPO = 'https://github.com/zyvorai/machina';
export const DEMO_URL = 'https://zyvor.dev/schedule?utm_source=pages&utm_medium=machina&utm_campaign=site';
export const POC_URL = 'https://zyvor.dev/poc?utm_source=pages&utm_medium=machina&utm_campaign=site';
export const INSTALL = 'git clone https://github.com/zyvorai/machina.git && cd machina && ./machinactl deploy';

export type Shot = {src: string; title: string; caption: string; url: string};

export const SHOTS: Shot[] = [
  {src: '/machina-dashboard.png', title: 'Mission Control', caption: 'Fleet health at a glance: VMs, hosts, memory, alerts and a Zyra briefing.', url: 'https://machina:5092/'},
  {src: '/machina-vms.png', title: 'Virtual machines', caption: 'Every KVM guest on the host with one-click console, power and snapshot actions.', url: 'https://machina:5092/vms'},
  {src: '/machina-vm-detail.png', title: 'Live console', caption: 'VM detail leads with a live noVNC console proxied by the daemon.', url: 'https://machina:5092/vms/web-frontend-01'},
  {src: '/machina-fleet.png', title: 'High availability', caption: 'HA policy, host fencing and failover events from the controller.', url: 'https://machina:5092/platform/ha'},
  {src: '/machina-fleet-cloud.png', title: 'Fleet Cloud', caption: 'Self-service instances, images, volumes, flavors and more, all native.', url: 'https://machina:5092/fleet-cloud'},
  {src: '/machina-zyra.png', title: 'Zyra AI', caption: 'Fleet intelligence, security graph, knowledge and services in one place.', url: 'https://machina:5092/platform/zyra'},
];

export type Feature = {
  eyebrow: string;
  title: string;
  body: string;
  bullets: string[];
  shot: Shot;
  doc: string;
};

export const FEATURES: Feature[] = [
  {
    eyebrow: 'Run',
    title: 'Every VM operation, in one place.',
    body: 'Create, clone, snapshot, back up and migrate KVM guests from a UI, a REST API with 900+ routes, a CLI or Terraform.',
    bullets: ['Cloud-init and golden images (Packer)', 'GPU and PCI passthrough', 'Networks, storage pools and nwfilters'],
    shot: SHOTS[1],
    doc: '/docs/getting-started/quickstart',
  },
  {
    eyebrow: 'Reach',
    title: 'Consoles in the browser. No gateway to deploy.',
    body: 'noVNC, SPICE, serial and SSH are proxied by machina-daemon itself, behind the same RBAC and audit log as everything else.',
    bullets: ['PAM, OIDC, SAML and LDAP sign-in', 'Role-based access control', 'Audit trail for every change'],
    shot: SHOTS[2],
    doc: '/docs/core-concepts/consoles',
  },
  {
    eyebrow: 'Scale',
    title: 'A fleet, not a host.',
    body: 'A gRPC agent per hypervisor. The controller keeps desired state, fails VMs over when a host dies, balances load with DRS and live-migrates between hosts.',
    bullets: ['HA failover and host fencing', 'DRS balancing and live migration', 'Embedded SQLite, optional NATS'],
    shot: SHOTS[3],
    doc: '/docs/core-concepts/fleet-ha',
  },
  {
    eyebrow: 'Self-service',
    title: 'Fleet Cloud: a public-cloud experience on your metal.',
    body: 'Flavors, images, instances, volumes, security groups, keypairs, floating IPs, server groups, stacks, projects and load balancers, all native controller APIs.',
    bullets: ['OpenStack-style primitives, no OpenStack', 'Load balancers as host iptables rules', 'Cloud-init on every instance'],
    shot: SHOTS[4],
    doc: '/docs/core-concepts/fleet-cloud',
  },
  {
    eyebrow: 'Operate',
    title: 'Zyra AI: an operator that asks first.',
    body: 'Autonomous diagnostics, incident correlation, rightsizing and natural-language operations, with an approval queue in front of every change.',
    bullets: ['Bring your own LLM provider', 'API keys encrypted at rest (AES-256-GCM)', 'Human approval before actions'],
    shot: SHOTS[5],
    doc: '/docs/core-concepts/zyra-ai',
  },
];

export const WHY = [
  {problem: 'OpenStack is a six-week project and a full-time team.', answer: 'One machinactl deploy: three binaries, embedded SQLite, a browser UI minutes later.'},
  {problem: 'VMware renewal quotes keep climbing.', answer: 'Open KVM/libvirt underneath, with HA failover, DRS and live migration on top.'},
  {problem: 'libvirt ops live in a pile of virsh scripts.', answer: 'One dashboard, a REST API, a CLI and a Terraform provider over the same model.'},
  {problem: 'Every console needs its own gateway.', answer: 'noVNC, SPICE, serial and SSH built into the daemon, with RBAC and audit.'},
  {problem: 'On-call triages the same incidents at 3 a.m.', answer: 'Zyra AI diagnoses and proposes the fix, then waits for a human approval.'},
  {problem: 'Network and storage live in other silos.', answer: 'Atlas puts disks on Ceph, NFS or ZFS; Netra adds eBPF deny rules that fail open.'},
];
