// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useEffect, useState } from 'react'
import { Link } from 'react-router'
import { Key, Shield } from 'lucide-react'
import { getEnterpriseSecurityOverview, type EnterpriseSecurityOverview } from '../../api/platform'
import { statusChipClasses } from '../../utils/semanticColors'

type Props = {
  className?: string
}

export default function EnterpriseSecurityStrip({ className = '' }: Props) {
  const [overview, setOverview] = useState<EnterpriseSecurityOverview | null>(null)

  useEffect(() => {
    void getEnterpriseSecurityOverview()
      .then(setOverview)
      .catch(() => setOverview(null))
  }, [])

  if (!overview) return null

  const vaultTone = overview.vault_connected < overview.vault_providers ? 'warn' : 'ok'

  return (
    <div
      className={`flex flex-wrap items-center gap-3 rounded-xl border border-white/[0.06] bg-[var(--apple-surface)] px-4 py-3 ${className}`}
      data-testid="enterprise-security-strip"
    >
      <div className="min-w-0">
        <p className="text-xs font-semibold uppercase tracking-wider text-[var(--text-muted)] flex items-center gap-1.5">
          <Key className="w-3.5 h-3.5" /> Enterprise security
        </p>
        <p className="text-sm text-[var(--text-primary)] truncate">{overview.summary}</p>
      </div>
      <div className="flex flex-wrap gap-1.5 flex-1">
        <span className={statusChipClasses(vaultTone)} title="Vault providers">
          Vault {overview.vault_connected}/{overview.vault_providers}
        </span>
        <span className={statusChipClasses('info')} title="MFA enrolled users">
          <Shield className="w-3 h-3 inline mr-0.5" />
          MFA {overview.mfa_enrolled_users}
        </span>
        <span className={statusChipClasses('neutral')}>
          Tenants {overview.tenant_policies}
        </span>
      </div>
      <Link to="/platform/enterprise?tab=keychain" className="text-xs text-orange-300/90 hover:underline shrink-0">
        Open Keychain →
      </Link>
    </div>
  )
}
