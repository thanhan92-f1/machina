// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { listOpenStackKeypairs, type OpenStackKeyPair } from '../api/openstack'
import { createOpenStackKeypair, deleteOpenStackKeypair } from '../api/openstackExtras'
import OpenStackGate from '../components/OpenStackGate'
import OpenStackSubNav from '../components/OpenStackSubNav'
import OpenStackFooter from '../components/OpenStackFooter'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import PageLayout from '../components/PageLayout'
import { statusActionLinkClasses, statusToneClass } from '../utils/semanticColors'
import { Key, Loader2, RefreshCw } from 'lucide-react'

export default function OpenStackKeypairsPage() {
  return (
    <OpenStackGate title="SSH keypairs">
      <OpenStackKeypairsContent />
    </OpenStackGate>
  )
}

function OpenStackKeypairsContent() {
  const toast = useToastContext()
  const [keys, setKeys] = useState<OpenStackKeyPair[]>([])
  const [loading, setLoading] = useState(true)
  const [name, setName] = useState('')
  const [publicKey, setPublicKey] = useState('')
  const [creating, setCreating] = useState(false)
  const [generatedKey, setGeneratedKey] = useState<{ name: string; privateKey: string } | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const { keypairs } = await listOpenStackKeypairs()
      setKeys(keypairs)
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
      prepend={<OpenStackSubNav />}
      title="SSH keypairs"
      icon={<Key className="w-7 h-7 text-sky-400" />}
      contentLoading={loading && keys.length === 0}
    >
      <div className="rounded-xl border border-slate-700 p-4 space-y-3">
        <div className="flex flex-wrap gap-2">
          <input aria-label="Keypair name" value={name} onChange={(e) => setName(e.target.value)} placeholder="name"
            className="px-2 py-1.5 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
          <button type="button" disabled={creating || !name.trim()}
            className="px-3 py-1.5 rounded-lg bg-sky-600 text-sm text-white disabled:opacity-40 disabled:cursor-not-allowed"
            onClick={async () => {
              if (!name.trim() || creating) return
              setCreating(true)
              try {
                const { keypair } = await createOpenStackKeypair({
                  name: name.trim(),
                  public_key: publicKey.trim() || undefined,
                })
                if (keypair.private_key) {
                  // Nova generated the keypair and returned the private key exactly
                  // once — surface it so the user can save it before it's gone.
                  setGeneratedKey({ name: keypair.name, privateKey: keypair.private_key })
                } else {
                  toast.success(`Keypair '${keypair.name}' imported`)
                }
                setName('')
                setPublicKey('')
                void load()
              } catch (e: unknown) {
                toast.error(formatUserError(e))
              } finally {
                setCreating(false)
              }
            }}>
            {creating ? 'Creating…' : 'Create / import'}
          </button>
        </div>
        <textarea aria-label="SSH public key" value={publicKey} onChange={(e) => setPublicKey(e.target.value)} rows={3}
          placeholder="Optional: paste public key (ssh-rsa AAAA...). Leave empty to let Nova generate."
          className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-xs font-mono" />
      </div>
      <button type="button" onClick={() => void load()}
        className="inline-flex items-center gap-1 px-3 py-1.5 rounded-lg border border-slate-600 text-sm">
        <RefreshCw className="w-4 h-4" /> Refresh
      </button>
      {!loading && (
        <ul className="rounded-xl border border-slate-700 divide-y divide-slate-800">
          {keys.map((k) => (
            <li key={k.name} className="px-4 py-3 flex justify-between items-center text-sm">
              <span className="font-mono text-slate-200">{k.name}</span>
              <span className="text-slate-500 text-xs">{k.fingerprint || '—'}</span>
              <button type="button" className={statusActionLinkClasses('error', 'text-xs')}
                onClick={async () => {
                  if (!confirm(`Delete keypair ${k.name}?`)) return
                  try {
                    await deleteOpenStackKeypair(k.name)
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
      {generatedKey && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4" role="presentation">
          <div role="dialog" aria-modal aria-label="Generated private key" className="w-full max-w-2xl rounded-2xl border border-slate-700 bg-slate-800 shadow-2xl">
            <div className="flex items-center gap-2 p-5 border-b border-slate-700">
              <Key className="w-5 h-5 text-sky-400" />
              <h2 className="text-lg font-semibold">Private key for &lsquo;{generatedKey.name}&rsquo;</h2>
            </div>
            <div className="p-5 space-y-3">
              <p className={`text-sm ${statusToneClass('warn')}`}>
                This is the only time the private key is shown. Save it now — it cannot be retrieved again.
              </p>
              <textarea readOnly aria-label="Private key" value={generatedKey.privateKey} rows={10}
                className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-xs font-mono text-slate-200" />
              <div className="flex flex-wrap gap-2">
                <button type="button" className="px-3 py-1.5 rounded-lg bg-sky-600 text-sm text-white"
                  onClick={() => {
                    const blob = new Blob([generatedKey.privateKey], { type: 'application/x-pem-file' })
                    const url = URL.createObjectURL(blob)
                    const a = document.createElement('a')
                    a.href = url
                    a.download = `${generatedKey.name}.pem`
                    document.body.appendChild(a)
                    a.click()
                    a.remove()
                    URL.revokeObjectURL(url)
                  }}>
                  Download .pem
                </button>
                <button type="button" className="px-3 py-1.5 rounded-lg border border-slate-600 text-sm"
                  onClick={async () => {
                    try {
                      await navigator.clipboard.writeText(generatedKey.privateKey)
                      toast.success('Private key copied')
                    } catch {
                      toast.error('Copy failed — use Download instead')
                    }
                  }}>
                  Copy
                </button>
                <button type="button" className="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-sm ml-auto"
                  onClick={() => setGeneratedKey(null)}>
                  I&rsquo;ve saved it
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
      <OpenStackFooter />
    </PageLayout>
  )
}
