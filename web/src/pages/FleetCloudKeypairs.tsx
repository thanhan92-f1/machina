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

// Native SSH keypair catalog — no old external-cloud gate component in the way
// any more (the daemon's external-cloud-client integration has since been
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
      className="w-full max-w-none"
      prepend={<FleetCloudSubNav />}
      title="SSH keypairs"
      icon={<Key className="w-7 h-7 text-[var(--accent)]" />}
      contentLoading={loading && keys.length === 0}
    >
      <div className="rounded-2xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] p-4 space-y-3">
        <div className="flex flex-wrap gap-2">
          <input aria-label="Keypair name" value={name} onChange={(e) => setName(e.target.value)} placeholder="name"
            className="input-field text-sm" />
          <button type="button" disabled={creating || !name.trim() || !publicKey.trim()}
            className="btn-primary text-sm disabled:opacity-40 disabled:cursor-not-allowed"
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
          className="w-full input-field text-xs font-mono" />
      </div>
      <button type="button" onClick={() => void load()}
        className="btn-secondary text-sm inline-flex items-center gap-1">
        <RefreshCw className="w-4 h-4" /> Refresh
      </button>
      {!loading && (
        <ul className="rounded-2xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] divide-y divide-[var(--apple-hairline)]">
          {keys.map((k) => (
            <li key={k.id} className="px-4 py-3 flex justify-between items-center text-sm">
              <span className="font-mono text-[var(--text-primary)]">{k.name}</span>
              <span className="text-[var(--text-muted)] text-xs">{k.fingerprint}</span>
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
