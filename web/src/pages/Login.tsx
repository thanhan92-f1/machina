// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useEffect, useRef, useState, FormEvent } from 'react'
import { useAuth } from '../contexts/AuthContext'
import { beginOidcLogin, getAuthProviders, type AuthProviders } from '../api/auth'
import { formatUserError } from '../utils/apiError'
import {
  Loader2,
  Eye,
  EyeOff,
  ArrowRight,
  User,
  Lock,
  Monitor,
  ShieldCheck,
  Camera,
  Boxes,
} from 'lucide-react'
import {
  PremiumLoginShell,
  LoginError,
  LoginField,
  LoginRemember,
  LoginSubmit,
  type PremiumLoginFeature,
} from '../components/PremiumLoginShell'
import { ZyvorInline } from '../components/ZyvorBrand'

const FEATURES: PremiumLoginFeature[] = [
  {
    icon: <Monitor className="w-5 h-5 text-blue-100" />,
    gradient: 'from-blue-500/95 to-indigo-700/95',
    glow: 'shadow-blue-500/25',
    title: 'Live VM consoles',
    description: 'VNC, SPICE, native RDP and SSH — right from the browser',
  },
  {
    icon: <Camera className="w-5 h-5 text-sky-100" />,
    gradient: 'from-sky-500/95 to-indigo-800/95',
    glow: 'shadow-sky-500/25',
    title: 'Snapshots & backups',
    description: 'Scheduled snapshots, disk export, and point-in-time recovery',
  },
  {
    icon: <ShieldCheck className="w-5 h-5 text-violet-100" />,
    gradient: 'from-violet-500/95 to-purple-800/95',
    glow: 'shadow-violet-500/25',
    title: 'Zero-trust security',
    description: 'Namespace isolation, PacketWolf enforcement, audit trails',
  },
  {
    icon: <Boxes className="w-5 h-5 text-cyan-100" />,
    gradient: 'from-cyan-500/95 to-blue-800/95',
    glow: 'shadow-cyan-500/25',
    title: 'KubeVirt + libvirt',
    description: 'One control plane for bare-metal and Kubernetes-native VMs',
  },
]

function MachinaLogo({ className = 'w-7 h-7' }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none" stroke="currentColor" strokeWidth="2">
      <rect x="3" y="4" width="18" height="12" rx="2" />
      <path d="M8 20h8M12 16v4" strokeLinecap="round" />
    </svg>
  )
}

export default function LoginPage() {
  const saved = (() => {
    try {
      const raw = localStorage.getItem('machina-saved-login')
      return raw ? (JSON.parse(raw) as { username?: string }) : null
    } catch {
      return null
    }
  })()

  const [username, setUsername] = useState(saved?.username ?? '')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [showPassword, setShowPassword] = useState(false)
  const [rememberMe, setRememberMe] = useState(!!saved)
  const [providers, setProviders] = useState<AuthProviders>({
    pam: { enabled: true },
    ldap: { enabled: false },
    oidc: { enabled: false, button_label: 'Sign in with SSO' },
    saml: { enabled: false, button_label: 'Sign in with SAML', login_available: false },
  })
  const { login } = useAuth()
  const usernameRef = useRef<HTMLInputElement>(null)
  const oidcEnabled = providers.oidc.enabled
  const ldapEnabled = providers.ldap.enabled

  useEffect(() => {
    void getAuthProviders().then(setProviders).catch(() => {})
    const params = new URLSearchParams(window.location.search)
    const errorParam = params.get('error')
    if (errorParam === 'oidc') {
      setError('SSO login failed')
    } else if (errorParam === 'token') {
      setError('Sign-in link is invalid or expired')
    } else if (errorParam === 'saml') {
      setError('SAML authentication failed')
    } else if (errorParam) {
      setError('Authentication failed — please try again')
    }
  }, [])

  useEffect(() => {
    usernameRef.current?.focus()
  }, [])

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    if (!username.trim() || !password) return
    setSubmitting(true)
    setError('')
    try {
      await login(username.trim(), password)
      if (rememberMe) {
        localStorage.setItem('machina-saved-login', JSON.stringify({ username: username.trim() }))
      } else {
        localStorage.removeItem('machina-saved-login')
      }
    } catch (e: unknown) {
      setError(formatUserError(e) || 'Login failed')
    } finally {
      setSubmitting(false)
    }
  }

  const hostLabel = typeof window !== 'undefined' ? window.location.hostname : ''

  return (
    <PremiumLoginShell
      variant="macos"
      accent="blue"
      heroWidth="55"
      logo={
        <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shadow-lg shadow-blue-600/30 border border-white/20">
          <MachinaLogo className="w-7 h-7 text-white" />
        </div>
      }
      productName="machina"
      productSubtitle="Hypervisor Control Plane"
      heroHeadline={
        <>
          Every VM.
          <br />
          <span className="login-text-gradient">One control plane.</span>
        </>
      }
      heroSubheadline="KubeVirt and libvirt hypervisors, consoles, snapshots, and security — from a single dashboard."
      pills={[
        { label: 'PAM / LDAP / OIDC' },
        { label: 'Native RDP + SSH' },
        { label: 'Zero-trust ready' },
      ]}
      features={FEATURES}
      heroFooter={
        <div className="flex items-center gap-2 text-blue-100/40 text-sm">
          <span>PAM authentication</span>
          <span className="text-blue-200/30">·</span>
          <span>{hostLabel || 'machina'}</span>
        </div>
      }
      mobileSubtitle="Hypervisor Control Plane"
      panelTitle="Welcome back"
      panelSubtitle="Sign in with your system account"
      footer={<ZyvorInline className="block text-center py-3 text-white/40" />}
    >
      <form onSubmit={handleSubmit}>
        {error ? <LoginError message={error} /> : null}

        {ldapEnabled ? (
          <p className="text-xs text-slate-400 mb-4">Sign in with your directory (LDAP/AD) username and password.</p>
        ) : null}
        {providers.saml?.enabled && !providers.saml.login_available ? (
          <p className="text-xs text-slate-400 mb-4">
            SAML metadata is configured for IdP federation — use SSO or your local account below.
          </p>
        ) : null}

        <div className="space-y-5">
          <LoginField label="Username" id="login-username">
            <User className="login-field-icon" />
            <input
              ref={usernameRef}
              id="login-username"
              name="username"
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="System username"
              autoComplete="username"
              required
              className="login-input"
            />
          </LoginField>

          <LoginField label="Password" id="login-password">
            <Lock className="login-field-icon" />
            <input
              id="login-password"
              name="password"
              type={showPassword ? 'text' : 'password'}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Password"
              autoComplete="current-password"
              disabled={submitting}
              required
              className="login-input pr-11"
            />
            <button
              type="button"
              onClick={() => setShowPassword(!showPassword)}
              className="absolute right-3.5 top-1/2 -translate-y-1/2 text-white/45 hover:text-white/75 transition-colors"
              aria-label={showPassword ? 'Hide password' : 'Show password'}
            >
              {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </button>
          </LoginField>
        </div>

        <LoginRemember
          checked={rememberMe}
          onChange={setRememberMe}
          label="Keep me signed in"
          hint="Only your username is stored locally — never your password."
        />

        <LoginSubmit loading={submitting} disabled={!username.trim() || !password}>
          {submitting ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin relative z-10" />
              <span className="relative z-10">Signing in…</span>
            </>
          ) : (
            <>
              <span className="relative z-10">Sign in</span>
              <ArrowRight className="h-4 w-4 relative z-10" />
            </>
          )}
        </LoginSubmit>

        {oidcEnabled ? (
          <button
            type="button"
            onClick={() => beginOidcLogin()}
            className="mt-4 flex items-center justify-center gap-2 w-full rounded-lg border border-white/[0.12] py-2.5 text-sm text-white/80 hover:bg-white/[0.06] transition-colors"
          >
            {providers.oidc.button_label}
          </button>
        ) : null}

        <div className="mt-6 pt-5 border-t border-white/[0.08] text-center text-xs text-white/45">
          Forgotten your password? Contact your administrator.
        </div>
      </form>
    </PremiumLoginShell>
  )
}
