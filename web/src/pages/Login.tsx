// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useEffect, useRef, useState, FormEvent } from 'react'
import { useAuth } from '../contexts/AuthContext'
import { beginOidcLogin, getAuthProviders, type AuthProviders } from '../api/auth'
import { formatUserError } from '../utils/apiError'
import { Loader2, Eye, EyeOff, ArrowRight, User, Lock } from 'lucide-react'
import {
  PremiumLoginShell,
  LoginError,
  LoginField,
  LoginRemember,
  LoginSubmit,
} from '../components/PremiumLoginShell'
import { ZyvorTileMark } from '../components/ZyvorMark'
import { ZYVOR_URL } from '../components/ZyvorBrand'

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

  return (
    <PremiumLoginShell
      variant="macos"
      layout="account"
      accent="blue"
      logo={<ZyvorTileMark size={56} className="block" idPrefix="login-tile" />}
      productName="machina"
      mobileSubtitle="Sign in with your system account"
      footer={
        <a
          href={ZYVOR_URL}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center justify-center opacity-50 hover:opacity-80 transition-opacity"
          aria-label="Zyvor"
        >
          <ZyvorTileMark size={22} idPrefix="login-footer-tile" />
        </a>
      }
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
