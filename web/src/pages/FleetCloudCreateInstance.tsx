// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useCallback, useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router'
import { listFlavors, type NativeFlavor } from '../api/flavors'
import { listTemplates, type NativeTemplate } from '../api/nativeTemplates'
import { listNetworks, type NativeNetwork } from '../api/nativeNetworks'
import { createFromTemplate } from '../api/nativeVms'
import { useToastContext } from '../contexts/ToastContext'
import { ChoiceCard, ChoiceCardGrid } from '../components/ChoiceCards'
import { ArrowLeft, Cloud, Disc, Loader2, Network } from 'lucide-react'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import { formatUserError } from '../utils/apiError'

// Native instance creation — there's no more <OpenStackGate> component to gate
// it behind: the daemon's external-OpenStack-client integration has since been
// fully removed. Boots from the native image catalog (api/nativeTemplates.ts)
// with a flavor and network resolved server-side
// (controller::api::vms::create_from_template, extended with flavor_id/network)
// instead of the daemon's Nova instance-create call.
export default function FleetCloudCreateInstancePage() {
  const navigate = useNavigate()
  const toast = useToastContext()
  const [loading, setLoading] = useState(true)
  const [submitting, setSubmitting] = useState(false)

  const [flavors, setFlavors] = useState<NativeFlavor[]>([])
  const [images, setImages] = useState<NativeTemplate[]>([])
  const [networks, setNetworks] = useState<NativeNetwork[]>([])

  const [name, setName] = useState('')
  const [imageId, setImageId] = useState('')
  const [flavorId, setFlavorId] = useState('')
  const [networkId, setNetworkId] = useState('')
  const [cloudInitUser, setCloudInitUser] = useState('')
  const [cloudInitPassword, setCloudInitPassword] = useState('')
  const [cloudInitSshPubkey, setCloudInitSshPubkey] = useState('')

  const loadCatalogs = useCallback(async () => {
    setLoading(true)
    try {
      const [f, i, n] = await Promise.all([
        listFlavors().catch(() => []),
        listTemplates().catch(() => []),
        listNetworks().catch(() => []),
      ])
      setFlavors(f)
      setImages(i)
      setNetworks(n)
      if (!flavorId && f.length > 0) setFlavorId(f[0].id)
      if (!networkId && n.length > 0) setNetworkId(n[0].name)
    } finally {
      setLoading(false)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => { void loadCatalogs() }, [loadCatalogs])

  const selectedImage = images.find((i) => i.id === imageId)

  const canCreate = name.trim().length > 0 && imageId.length > 0 && flavorId.length > 0

  const handleCreate = async () => {
    if (!canCreate || !selectedImage) {
      toast.warning('Name, image, and flavor are required')
      return
    }
    setSubmitting(true)
    try {
      await createFromTemplate({
        name: name.trim(),
        template_ref: selectedImage.name,
        flavor_id: flavorId,
        network: networkId || undefined,
        cloud_init_user: cloudInitUser.trim() || undefined,
        cloud_init_password: cloudInitPassword || undefined,
        cloud_init_ssh_pubkey: cloudInitSshPubkey.trim() || undefined,
      })
      toast.success(`Instance '${name.trim()}' creation queued`)
      navigate('/fleet-cloud/instances')
    } catch (e: unknown) {
      toast.error(`Create failed: ${formatUserError(e)}`)
    } finally {
      setSubmitting(false)
    }
  }

  if (loading) {
    return (
      <PageLayout hideHeader className="max-w-3xl" prepend={<><FleetCloudSubNav /></>}>
        <div className="text-slate-500 py-12 text-center flex flex-col items-center gap-3">
          <Loader2 className="w-8 h-8 animate-spin text-sky-400" />
          Loading catalogs…
        </div>
      </PageLayout>
    )
  }

  return (
    <PageLayout
      className="max-w-3xl"
      prepend={<><FleetCloudSubNav /></>}
      title="Create Fleet Cloud instance"
      icon={<Cloud className="w-7 h-7 text-sky-400" />}
      actions={
        <div className="flex items-center gap-3">
          <button
            type="button"
            disabled={!canCreate || submitting}
            onClick={() => void handleCreate()}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-sky-600 hover:bg-sky-500 text-white disabled:opacity-50 text-sm"
          >
            {submitting && <Loader2 className="w-4 h-4 animate-spin" />}
            Create instance
          </button>
          <Link to="/fleet-cloud/instances" className="inline-flex items-center gap-2 text-slate-400 hover:text-slate-200 text-sm">
            <ArrowLeft className="w-4 h-4" />
            Instances
          </Link>
        </div>
      }
    >
      <div>
        <label className="block text-sm text-slate-400 mb-1">Instance name</label>
        <input
          aria-label="Instance name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-slate-100"
          placeholder="my-vm"
        />
      </div>

      <div>
        <label className="block text-sm text-slate-400 mb-2">Image</label>
        {images.length === 0 ? (
          <p className="text-sm text-slate-500">
            No images in the catalog. <Link to="/fleet-cloud/images" className="text-sky-400 hover:underline">Register one</Link> first.
          </p>
        ) : (
          <ChoiceCardGrid>
            {images.map((img) => (
              <ChoiceCard
                key={img.id}
                tone="sky"
                icon={<Disc className="w-4 h-4" />}
                selected={imageId === img.id}
                onClick={() => setImageId(img.id)}
                title={img.name}
                description={img.os_family || img.category}
              />
            ))}
          </ChoiceCardGrid>
        )}
      </div>

      <div>
        <label className="block text-sm text-slate-400 mb-2">Flavor</label>
        {flavors.length === 0 ? (
          <p className="text-sm text-slate-500">
            No flavors. <Link to="/fleet-cloud/flavors" className="text-sky-400 hover:underline">Create one</Link> first.
          </p>
        ) : (
          <div className="overflow-x-auto rounded-xl border border-slate-700">
            <table className="w-full text-sm" aria-label="Flavors">
              <thead className="bg-slate-900 text-slate-400 text-left">
                <tr>
                  <th scope="col" className="px-3 py-2" />
                  <th scope="col" className="px-3 py-2">Name</th>
                  <th scope="col" className="px-3 py-2">vCPU</th>
                  <th scope="col" className="px-3 py-2">RAM</th>
                  <th scope="col" className="px-3 py-2">Disk</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800">
                {flavors.map((f) => (
                  <tr
                    key={f.id}
                    className={`cursor-pointer hover:bg-slate-800/50 ${flavorId === f.id ? 'bg-sky-500/10' : ''}`}
                    onClick={() => setFlavorId(f.id)}
                  >
                    <td className="px-3 py-2">
                      <input type="radio" checked={flavorId === f.id} readOnly />
                    </td>
                    <td className="px-3 py-2 text-slate-200">{f.name}</td>
                    <td className="px-3 py-2">{f.vcpus}</td>
                    <td className="px-3 py-2">{f.memory_mib} MiB</td>
                    <td className="px-3 py-2">{f.disk_gib} GiB</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <div>
        <label className="block text-sm text-slate-400 mb-2">Network</label>
        {networks.length === 0 ? (
          <p className="text-sm text-slate-500">No networks available.</p>
        ) : (
          <ChoiceCardGrid>
            {networks.map((net) => (
              <ChoiceCard
                key={net.id}
                tone="cyan"
                icon={<Network className="w-4 h-4" />}
                selected={networkId === net.name}
                onClick={() => setNetworkId(net.name)}
                title={net.name}
                description={net.backend}
              />
            ))}
          </ChoiceCardGrid>
        )}
      </div>

      <div className="space-y-3 rounded-xl border border-slate-700 p-4">
        <h2 className="text-sm font-medium text-slate-300">Cloud-init (optional)</h2>
        <div className="grid sm:grid-cols-2 gap-3">
          <input aria-label="Cloud-init user" value={cloudInitUser} onChange={(e) => setCloudInitUser(e.target.value)}
            placeholder="Login user (default: ubuntu)"
            className="px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
          <input aria-label="Cloud-init password" type="password" autoComplete="new-password" value={cloudInitPassword} onChange={(e) => setCloudInitPassword(e.target.value)}
            placeholder="Password (optional)"
            className="px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
        </div>
        <textarea aria-label="SSH public key" value={cloudInitSshPubkey} onChange={(e) => setCloudInitSshPubkey(e.target.value)} rows={2}
          placeholder="SSH public key (optional) — or pick a saved keypair on the Keys page and paste its key here"
          className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-xs font-mono" />
      </div>

      <FleetCloudFooter />
    </PageLayout>
  )
}
