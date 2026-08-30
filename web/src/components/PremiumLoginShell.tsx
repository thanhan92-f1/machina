// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

/**
 * Machina marketing login — oversized product name, then sign-in.
 * Black + Zyvor orange; light/dark atmospheres follow shell theme.
 */
import type { ReactNode } from 'react';
import { AlertCircle } from 'lucide-react';
import { ZyvorMark } from './ZyvorMark';

export type PremiumLoginFeature = {
  icon: ReactNode;
  title: string;
  description: string;
  gradient?: string;
  glow?: string;
  highlight?: boolean;
};

export type PremiumLoginPill = {
  icon?: ReactNode;
  label: string;
  glow?: boolean;
};

export type LoginAccent =
  | 'blue'
  | 'amber'
  | 'orange'
  | 'violet'
  | 'rose'
  | 'cyan'
  | 'copper'
  | 'steel'
  | 'zeus';

export type PremiumLoginShellProps = {
  accent?: LoginAccent;
  pageThemeClass?: string;
  /** @deprecated unused */
  heroWidth?: '55' | '58';
  themeSwitcher?: ReactNode;
  /** Optional override; default is ZyvorMark */
  logo?: ReactNode;
  productName: string;
  productSubtitle?: string;
  /** Marketing line under the product name */
  heroHeadline?: ReactNode;
  /** Quiet supporting sentence */
  heroSubheadline?: string;
  /** @deprecated unused */
  pills?: PremiumLoginPill[];
  /** @deprecated unused */
  features?: PremiumLoginFeature[];
  /** @deprecated unused */
  heroFooter?: ReactNode;
  mobileSubtitle?: string;
  /** Label above the form card */
  panelTitle?: string;
  /** @deprecated unused for marketing layout */
  panelSubtitle?: string;
  panelHint?: ReactNode;
  footer?: ReactNode;
  formClassName?: string;
  children: ReactNode;
};

export function PremiumLoginShell({
  accent = 'orange',
  pageThemeClass = '',
  themeSwitcher,
  logo,
  productName,
  productSubtitle,
  heroHeadline,
  heroSubheadline,
  mobileSubtitle,
  panelTitle = 'Sign in',
  panelHint,
  footer,
  formClassName = '',
  children,
}: PremiumLoginShellProps) {
  const accentClass = accent === 'blue' ? '' : `login-accent-${accent}`;
  const tagline = heroSubheadline ?? productSubtitle ?? mobileSubtitle;
  const marketingLine =
    heroHeadline ??
    'Private cloud, built to feel inevitable.';
  const mark = logo ?? <ZyvorMark to={null} size="xl" className="login-mark zyvor-mark" />;

  return (
    <div className={`login-page min-h-screen flex flex-col ${accentClass} ${pageThemeClass}`.trim()}>
      <div className="login-atmosphere" aria-hidden="true" />
      {themeSwitcher}

      <main className="login-panel" aria-label="Sign in">
        <div className="login-stage relative z-10">
          <div className="login-brand">
            <div className="login-logo">{mark}</div>
            <span className="login-eyebrow">Zyvor</span>
            <p className="login-product" aria-label={productName}>
              {productName}
            </p>
            <h1 className="login-headline">{marketingLine}</h1>
            {tagline ? <p className="login-tagline">{tagline}</p> : null}
          </div>

          <div className="login-card">
            <span className="login-card-label">{panelTitle}</span>
            <div className={`login-form-block ${formClassName}`.trim()}>{children}</div>
            {panelHint ? <p className="login-hint">{panelHint}</p> : null}
          </div>
        </div>
      </main>

      {footer}
    </div>
  );
}

export type LoginErrorVariant = 'credentials' | 'network' | 'generic';

export function LoginError({
  message,
  variant = 'generic',
}: {
  message: string;
  variant?: LoginErrorVariant;
}) {
  const title =
    variant === 'network'
      ? 'Connection problem'
      : variant === 'credentials'
        ? 'Sign-in failed'
        : 'Unable to sign in';

  return (
    <div
      className="flex items-start gap-2.5 bg-destructive/10 border border-destructive/30 rounded-xl p-3 mb-6 login-shake"
      role="alert"
      aria-live="assertive"
    >
      <AlertCircle className="h-4 w-4 text-destructive shrink-0 mt-0.5" aria-hidden />
      <div>
        <p className="text-sm font-medium text-destructive">{title}</p>
        <p className="text-sm text-destructive/90 mt-0.5">{message}</p>
      </div>
    </div>
  );
}

export function LoginField({
  label,
  id,
  children,
}: {
  label: string;
  id: string;
  children: ReactNode;
}) {
  return (
    <div className="mb-4">
      <label htmlFor={id} className="block text-[13px] font-medium mb-1.5 tracking-[-0.01em]" style={{ color: 'var(--login-ink-secondary, #6e6e73)' }}>
        {label}
      </label>
      <div className="relative group">{children}</div>
    </div>
  );
}

export function LoginSubmit({
  loading,
  disabled,
  children,
  className = '',
}: {
  loading?: boolean;
  disabled?: boolean;
  children: ReactNode;
  className?: string;
}) {
  return (
    <button
      type="submit"
      disabled={disabled || loading}
      className={`login-btn-primary ${className}`.trim()}
    >
      {children}
    </button>
  );
}

export function LoginRemember({
  checked,
  onChange,
  label = 'Remember me on this device',
  hint,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  hint?: string;
}) {
  return (
    <div className="mt-5">
      <label className="flex items-center gap-2.5 cursor-pointer select-none">
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
          className="w-4 h-4 rounded border-border bg-card accent-primary"
        />
        <span className="text-sm" style={{ color: 'var(--login-ink-secondary, #6e6e73)' }}>{label}</span>
      </label>
      {hint ? <p className="text-xs mt-1.5 ml-[1.625rem]" style={{ color: 'var(--login-ink-muted, #86868b)' }}>{hint}</p> : null}
    </div>
  );
}

export function LoginSsoButton({
  href,
  label,
}: {
  href: string;
  label: string;
  description?: string;
}) {
  return (
    <a href={href} className="login-btn-secondary mb-3">
      {label}
    </a>
  );
}

export function LoginMethodToggle({
  options,
  value,
  onChange,
}: {
  options: { value: string; label: string }[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div
      role="tablist"
      aria-label="Sign-in method"
      className="login-method-toggle"
    >
      {options.map((opt) => {
        const selected = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            role="tab"
            aria-selected={selected}
            onClick={() => onChange(opt.value)}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}

export function LoginDivider({ label = 'or' }: { label?: string }) {
  return (
    <div
      className="relative py-3 mt-2 text-center text-[11px] uppercase tracking-[0.16em]"
      style={{ color: 'var(--login-ink-muted, #86868b)' }}
    >
      <span
        className="relative z-[1] px-3"
        style={{ background: 'var(--login-card, #fff)' }}
      >
        {label}
      </span>
      <div
        className="absolute inset-x-0 top-1/2 -translate-y-1/2 border-t"
        style={{ borderColor: 'var(--login-line, rgba(0,0,0,0.08))' }}
      />
    </div>
  );
}

/** @deprecated typo guard — use PremiumLoginShell */
export const PremumLoginShell = PremiumLoginShell;
