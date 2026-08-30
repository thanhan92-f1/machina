// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useEffect, useState, FormEvent } from 'react'
import { useAuth } from '../contexts/AuthContext'
import { useTheme } from '../contexts/ThemeContext'
import { beginOidcLogin, getAuthProviders, type AuthProviders } from '../api/auth'
import { useTranslation } from 'react-i18next'
import LanguageSwitcher from '../components/LanguageSwitcher'
import { formatUserError } from '../utils/apiError'
import { Lock, User, ArrowRight, Loader2, Eye, EyeOff } from 'lucide-react'
import {
  PremiumLoginShell,
  LoginDivider,
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

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    if (!username.trim() || !password) {
      setError('Username and password are required')
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

  return (
    <PremiumLoginShell
      accent="orange"
      pageThemeClass="machina-login"
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
      logo={
        <ZyvorMark
          to={null}
          size="xl"
          tone={isLight ? 'onLight' : 'onDark'}
          className="login-mark zyvor-mark"
        />
      }
      productName="Machina"
      heroHeadline="The private cloud that feels like home."
      heroSubheadline={
        hostLabel
          ? `Libvirt, Fleet Cloud, and Zyra — on ${hostLabel}.`
          : 'Libvirt, Fleet Cloud, and Zyra — one quiet control plane.'
      }
      panelTitle="Sign in to continue"
      footer={<ZyvorFooter />}
    >
      <form
        onSubmit={(e) => {
          e.preventDefault()
          if (passwordLogin) void handleSubmit(e)
        }}
        autoComplete={passwordLogin ? 'on' : 'off'}
        aria-label={t('login.title')}
      >
        {error ? <LoginError message={error} /> : null}

        {providers.saml?.enabled && !providers.saml.login_available ? (
          <p className="login-hint" role="status">
            SAML metadata is configured for IdP federation — browser SAML login coming soon.
          </p>
        ) : null}

        {oidcEnabled ? (
          <button type="button" onClick={() => beginOidcLogin()} className="login-btn-primary group w-full mb-4">
            <span className="relative z-10">{providers.oidc.button_label}</span>
            <ArrowRight className="h-4 w-4 relative z-10 group-hover:translate-x-0.5 transition-transform" />
          </button>
        ) : null}

        {oidcEnabled && passwordLogin ? (
          <LoginDivider label="or use password" />
        ) : null}

        {ldapEnabled ? (
          <p className="login-hint mb-4" role="status">
            {t('login.ldapHint')}
          </p>
        ) : null}

        {passwordLogin ? (
          <>
            <LoginField label={t('login.username')} id="login-username">
              <User className="login-field-icon" />
              <input
                id="login-username"
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                className="login-input"
                placeholder={t('login.username')}
                autoComplete="username"
                autoFocus
                required
                disabled={submitting}
              />
            </LoginField>

            <LoginField label={t('login.password')} id="login-password">
              <Lock className="login-field-icon" />
              <input
                id="login-password"
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="login-input pr-11"
                placeholder={t('login.password')}
                autoComplete="current-password"
                required
                disabled={submitting}
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-3.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                aria-label={showPassword ? 'Hide password' : 'Show password'}
              >
                {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </LoginField>

            <LoginRemember
              checked={rememberMe}
              onChange={setRememberMe}
              label="Remember username on this device"
              hint="Only your username is stored locally — never your password."
            />

            <LoginSubmit loading={submitting} disabled={!username.trim() || !password}>
              {submitting ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" aria-hidden />
                  <span>Signing in…</span>
                </>
              ) : (
                <>
                  <span>Continue</span>
                  <ArrowRight className="h-4 w-4" />
                </>
              )}
            </LoginSubmit>
          </>
        ) : null}
      </form>
    </PremiumLoginShell>
  )
}
