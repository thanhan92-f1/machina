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
  themeSwitcher?: ReactNode;
  /** Optional override; default is ZyvorMark */
  logo?: ReactNode;
  /** Rendered above the form — a heading on the identify step, a back-row on the password step */
  headerSlot?: ReactNode;
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
  headerSlot,
  panelHint,
  footer,
  formClassName = '',
  children,
}: PremiumLoginShellProps) {
  const accentClass = accent === 'blue' ? '' : `login-accent-${accent}`;
  const mark = logo ?? (
    <div className="absolute top-3 left-3 sm:top-4 sm:left-4 z-30">
      <ZyvorMark to={null} size="md" className="login-mark zyvor-mark" />
    </div>
  );

  return (
    <div className={`login-page min-h-screen flex flex-col ${accentClass} ${pageThemeClass}`.trim()}>
      {mark}
      {themeSwitcher}

      <main className="login-panel" aria-label="Sign in">
        <div className="login-stage relative z-10">
          <div className="login-card">
            {headerSlot}
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
