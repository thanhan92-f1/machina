// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { createKeypair, deleteKeypair, listKeypairs, type NativeKeypair } from '../api/nativeKeypairs'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import PageLayout from '../components/PageLayout'
import { statusActionLinkClasses } from '../utils/semanticColors'
import { Key, RefreshCw } from 'lucide-react'

// Native SSH keypair catalog — no <OpenStackGate> component to gate it behind
// any more (the daemon's external-OpenStack-client integration has since been
// fully removed). Import-only: no server-side keypair generation (Machina
// never hands out private keys over an API) — see api/nativeKeypairs.ts.
export default function FleetCloudKeypairsPage() {
  return <FleetCloudKeypairsContent />
}

function FleetCloudKeypairsContent() {
  const toast = useToastContext()
  const [keys, setKeys] = useState<NativeKeypair[]>([])
  const [loading, setLoading] = useState(true)
  const [name, setName] = useState('')
  const [publicKey, setPublicKey] = useState('')
  const [creating, setCreating] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const list = await listKeypairs()
      setKeys(list)
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => {
    void load()
  }, [load])

  return (
    <PageLayout
      className="max-w-3xl"
      prepend={<FleetCloudSubNav />}
      title="SSH keypairs"
      icon={<Key className="w-7 h-7 text-sky-400" />}
      contentLoading={loading && keys.length === 0}
    >
      <div className="rounded-xl border border-slate-700 p-4 space-y-3">
        <div className="flex flex-wrap gap-2">
          <input aria-label="Keypair name" value={name} onChange={(e) => setName(e.target.value)} placeholder="name"
            className="px-2 py-1.5 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
          <button type="button" disabled={creating || !name.trim() || !publicKey.trim()}
            className="px-3 py-1.5 rounded-lg bg-sky-600 text-sm text-white disabled:opacity-40 disabled:cursor-not-allowed"
            onClick={async () => {
              if (!name.trim() || !publicKey.trim() || creating) return
              setCreating(true)
              try {
                await createKeypair({ name: name.trim(), public_key: publicKey.trim() })
                toast.success(`Keypair '${name.trim()}' imported`)
                setName('')
                setPublicKey('')
                void load()
              } catch (e: unknown) {
                toast.error(formatUserError(e))
              } finally {
                setCreating(false)
              }
            }}>
            {creating ? 'Importing…' : 'Import'}
          </button>
        </div>
        <textarea aria-label="SSH public key" value={publicKey} onChange={(e) => setPublicKey(e.target.value)} rows={3}
          placeholder="Paste public key (ssh-rsa AAAA... or ssh-ed25519 AAAA...)"
          className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-xs font-mono" />
      </div>
      <button type="button" onClick={() => void load()}
        className="inline-flex items-center gap-1 px-3 py-1.5 rounded-lg border border-slate-600 text-sm">
        <RefreshCw className="w-4 h-4" /> Refresh
      </button>
      {!loading && (
        <ul className="rounded-xl border border-slate-700 divide-y divide-slate-800">
          {keys.map((k) => (
            <li key={k.id} className="px-4 py-3 flex justify-between items-center text-sm">
              <span className="font-mono text-slate-200">{k.name}</span>
              <span className="text-slate-500 text-xs">{k.fingerprint}</span>
              <button type="button" className={statusActionLinkClasses('error', 'text-xs')}
                onClick={async () => {
                  if (!confirm(`Delete keypair ${k.name}?`)) return
                  try {
                    await deleteKeypair(k.id)
                    toast.success('Deleted')
                    void load()
                  } catch (e: unknown) {
                    toast.error(formatUserError(e))
                  }
                }}>Delete</button>
            </li>
          ))}
        </ul>
      )}
      <FleetCloudFooter />
    </PageLayout>
  )
}
