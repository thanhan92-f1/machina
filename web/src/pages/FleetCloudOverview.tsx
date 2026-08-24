// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { Link } from 'react-router'
import { Cloud, Server, HardDrive, Plus, GitBranch, ArrowRight, Globe, Camera } from 'lucide-react'
import Hero from '../components/Hero'
import PageLayout from '../components/PageLayout'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import { usePlatformInfo } from '../contexts/PlatformInfoContext'

const QUICK_LINKS = [
  {
    to: '/fleet-cloud/instances',
    icon: Server,
    title: 'Instances',
    description: 'List, start, stop, reboot, console, volumes, floating IPs, security groups.',
  },
  {
    to: '/fleet-cloud/images',
    icon: HardDrive,
    title: 'Images',
    description: 'Pull images to the hypervisor, delete, boot new instances from golden images.',
  },
  {
    to: '/fleet-cloud/create',
    icon: Plus,
    title: 'Create instance',
    description: 'Wizard: image, flavor, network, keypair, security groups, cloud-init.',
  },
  {
    to: '/fleet-cloud/volumes',
    icon: HardDrive,
    title: 'Volumes',
    description: 'Create, clone, transfer, attach, snapshots, bootable volumes.',
  },
  {
    to: '/fleet-cloud/flavors',
    icon: Cloud,
    title: 'Flavors',
    description: 'Flavor catalog — create and delete with admin role.',
  },
  {
    to: '/fleet-cloud/floating-ips',
    icon: Globe,
    title: 'Floating IPs',
    description: 'Allocate, associate, and release floating IPs.',
  },
  {
    to: '/fleet-cloud/volume-snapshots',
    icon: Camera,
    title: 'Volume snapshots',
    description: 'Snapshot list and restore workflows.',
  },
  {
    to: '/fleet-cloud/migrations',
    icon: GitBranch,
    title: 'Bulk migrations',
    description: 'HyperSDK export pipelines when hypersdk is enabled on the daemon.',
  },
] as const

export default function FleetCloudOverviewPage() {
  const { info } = usePlatformInfo()
  const hypersdkEnabled = Boolean(info?.hypersdk?.enabled)
  const quickLinks = hypersdkEnabled
    ? QUICK_LINKS
    : QUICK_LINKS.filter((l) => l.to !== '/fleet-cloud/migrations')

  return (
    <PageLayout hideHeader>
      <Hero
        title="Fleet Cloud"
        subtitle="Instance and image management on this hypervisor — native, no external cloud required."
        icon={<Cloud className="w-6 h-6" />}
        actions={
          <Link
            to="/fleet-cloud/create"
            className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-sky-600 hover:bg-sky-500 text-white text-sm font-medium"
          >
            <Plus className="w-4 h-4" />
            Create instance
          </Link>
        }
      />
      <FleetCloudSubNav />

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {quickLinks.map(({ to, icon: Icon, title, description }) => (
          <Link
            key={to}
            to={to}
            className="group rounded-xl border border-slate-700/60 bg-slate-800/40 p-4 hover:border-sky-500/40 hover:bg-sky-950/20 transition"
          >
            <Icon className="w-6 h-6 text-sky-400 mb-2" />
            <h3 className="font-semibold text-slate-100 flex items-center gap-2">
              {title}
              <ArrowRight className="w-4 h-4 opacity-0 -translate-x-1 group-hover:opacity-100 group-hover:translate-x-0 transition" />
            </h3>
            <p className="text-xs text-slate-400 mt-1 leading-relaxed">{description}</p>
          </Link>
        ))}
      </div>

      <FleetCloudFooter />
    </PageLayout>
  )
}
