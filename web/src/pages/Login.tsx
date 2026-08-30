// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useEffect, useRef, useState, FormEvent } from 'react'
import { useAuth } from '../contexts/AuthContext'
import { useTheme } from '../contexts/ThemeContext'
import { beginOidcLogin, getAuthProviders, type AuthProviders } from '../api/auth'
import { useTranslation } from 'react-i18next'
import LanguageSwitcher from '../components/LanguageSwitcher'
import { formatUserError } from '../utils/apiError'
import { Loader2, Eye, EyeOff, ChevronLeft } from 'lucide-react'
import {
  PremiumLoginShell,
  LoginError,
  LoginField,
  LoginRemember,
  LoginSubmit,
} from '../components/PremiumLoginShell'
import { ZyvorMark } from '../components/ZyvorMark'
import { ZyvorFooter } from '../components/ZyvorBrand'

export default function LoginPage() {
  const saved = (() => {
    try {
      const raw = localStorage.getItem('machina-saved-login')
      return raw ? (JSON.parse(raw) as { username?: string; password?: string }) : null
    } catch {
      return null
    }
  })()

  const [step, setStep] = useState<'identify' | 'password'>('identify')
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
  const { t } = useTranslation()
  const { login } = useAuth()
  const { theme, setTheme } = useTheme()
  const isLight = theme === 'light'
  const hostLabel = typeof window !== 'undefined' ? window.location.hostname : ''
  const oidcEnabled = providers.oidc.enabled
  const pamEnabled = providers.pam.enabled
  const ldapEnabled = providers.ldap.enabled
  const passwordLogin = pamEnabled || ldapEnabled

  const usernameRef = useRef<HTMLInputElement>(null)
  const passwordRef = useRef<HTMLInputElement>(null)

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
    if (step === 'identify') {
      usernameRef.current?.focus()
    } else {
      passwordRef.current?.focus()
    }
  }, [step])

  const handleIdentify = (e: FormEvent) => {
    e.preventDefault()
    if (!username.trim()) {
      setError('Enter your username')
      return
    }
    setError('')
    setStep('password')
  }

  const handleBack = () => {
    setStep('identify')
    setPassword('')
    setError('')
    setShowPassword(false)
  }

  const handlePasswordSubmit = async (e: FormEvent) => {
    e.preventDefault()
    if (!password) {
      setError('Password is required')
      return
    }
    setSubmitting(true)
    setError('')
    try {
      await login(username.trim(), password)
      if (rememberMe) {
        localStorage.setItem(
          'machina-saved-login',
          JSON.stringify({ username: username.trim() }),
        )
      } else {
        localStorage.removeItem('machina-saved-login')
      }
    } catch (e: unknown) {
      setError(formatUserError(e) || 'Login failed')
    } finally {
      setSubmitting(false)
    }
  }

  const panelSubtitle =
    step === 'password' ? (
      <>
        Enter the password for <span className="login-apple-host">{username}</span>
      </>
    ) : hostLabel ? (
      <>
        Continue on <span className="login-apple-host">{hostLabel}</span>
      </>
    ) : (
      'Enter your account details to continue.'
    )

  return (
    <PremiumLoginShell
      themeSwitcher={
        <div className="absolute top-3 right-3 sm:top-4 sm:right-4 z-30 flex items-center gap-2">
          <div className="login-theme-toggle" role="group" aria-label="Appearance">
            <button
              type="button"
              aria-pressed={isLight}
              onClick={() => setTheme('light')}
            >
              Light
            </button>
            <button
              type="button"
              aria-pressed={!isLight}
              onClick={() => setTheme('dark')}
            >
              Dark
            </button>
          </div>
          <LanguageSwitcher />
        </div>
      }
      logo={<ZyvorMark to={null} size="xl" tone="onLight" className="login-mark zyvor-mark" />}
      panelTitle={
        <>
          Sign <em>in</em>
        </>
      }
      panelSubtitle={panelSubtitle}
      footer={<ZyvorFooter className="login-apple-footer" />}
    >
      {step === 'identify' ? (
        <form onSubmit={handleIdentify} autoComplete="on" className="text-left login-apple-step" key="identify">
          {error ? <LoginError message={error} /> : null}

          {providers.saml?.enabled && !providers.saml.login_available ? (
            <p className="login-apple-note">
              SAML metadata is configured for IdP federation — use SSO or your local account to sign in.
            </p>
          ) : null}

          {ldapEnabled ? <p className="login-apple-note">{t('login.ldapHint')}</p> : null}

          {passwordLogin ? (
            <>
              <div className="login-apple-fields">
                <LoginField label={t('login.username')} id="login-username">
                  <input
                    ref={usernameRef}
                    id="login-username"
                    name="username"
                    type="text"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    required
                    autoComplete="username"
                    placeholder={t('login.username')}
                    className="login-input"
                  />
                </LoginField>
              </div>

              <LoginSubmit disabled={!username.trim()}>
                <span>Continue</span>
              </LoginSubmit>
            </>
          ) : null}

          {oidcEnabled ? (
            <button type="button" onClick={() => beginOidcLogin()} className="login-sso">
              {providers.oidc.button_label}
            </button>
          ) : null}

          <details className="login-help">
            <summary>Need help?</summary>
            <div>
              <p>Forgotten your password, or can&rsquo;t sign in? Contact your administrator.</p>
            </div>
          </details>
        </form>
      ) : (
        <form onSubmit={handlePasswordSubmit} autoComplete="on" className="text-left login-apple-step" key="password">
          <button
            type="button"
            onClick={handleBack}
            className="login-apple-identity"
            aria-label={`Back, change username (currently ${username})`}
          >
            <ChevronLeft className="h-4 w-4" aria-hidden />
            <span>{username}</span>
          </button>
          {/* Hidden username field helps browser password managers correlate the two forms. */}
          <input type="text" name="username" value={username} autoComplete="username" readOnly hidden />

          {error ? <LoginError message={error} /> : null}

          <div className="login-apple-fields">
            <LoginField label={t('login.password')} id="login-password">
              <input
                ref={passwordRef}
                id="login-password"
                name="password"
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                disabled={submitting}
                autoComplete="current-password"
                placeholder={t('login.password')}
                className="login-input login-input--password"
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-3.5 top-1/2 -translate-y-1/2 transition-colors"
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

          <LoginSubmit loading={submitting} disabled={!password}>
            {submitting ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" aria-hidden />
                <span>Signing in…</span>
              </>
            ) : (
              <span>Sign In</span>
            )}
          </LoginSubmit>

          <details className="login-help">
            <summary>Need help?</summary>
            <div>
              <p>Forgotten your password? Contact your administrator.</p>
            </div>
          </details>
        </form>
      )}
    </PremiumLoginShell>
  )
}
