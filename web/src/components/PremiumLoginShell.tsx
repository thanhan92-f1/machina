// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

/**
 * Apple ID–style centered sign-in shell — flat paper canvas, single form panel.
 */
import type { ReactNode } from 'react';
import { AlertCircle } from 'lucide-react';
import { ZyvorMark } from './ZyvorMark';

export type PremiumLoginShellProps = {
  pageThemeClass?: string;
  themeSwitcher?: ReactNode;
  /** Optional override; default is ZyvorMark */
  logo?: ReactNode;
  panelTitle?: ReactNode;
  panelSubtitle?: ReactNode;
  panelHint?: ReactNode;
  footer?: ReactNode;
  formClassName?: string;
  children: ReactNode;
};

export function PremiumLoginShell({
  pageThemeClass = '',
  themeSwitcher,
  logo,
  panelTitle = 'Sign in',
  panelSubtitle,
  panelHint,
  footer,
  formClassName = '',
  children,
}: PremiumLoginShellProps) {
  const mark = logo ?? <ZyvorMark to={null} size="xl" tone="onLight" className="login-mark zyvor-mark" />;

  return (
    <div className={`login-page login-page-apple ${pageThemeClass}`.trim()}>
      {themeSwitcher}
      <div className="login-apple-stage">
        <div className="login-apple-stack">
          <div className="login-apple-mark">
            <div className="login-logo-ring">{mark}</div>
          </div>
          <h1 className="login-apple-title">{panelTitle}</h1>
          {panelSubtitle ? <p className="login-apple-subtitle">{panelSubtitle}</p> : null}
          <div className={`login-apple-card ${formClassName}`.trim()}>{children}</div>
          {panelHint ? <p className="login-apple-hint">{panelHint}</p> : null}
        </div>
      </div>
      {footer}
    </div>
  );
}

export function LoginError({ message }: { message: string }) {
  return (
    <div className="flex items-center gap-2.5 mb-6 login-shake" role="alert">
      <AlertCircle className="h-4 w-4 shrink-0" aria-hidden />
      <span>{message}</span>
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
    <div className="login-field relative">
      <label htmlFor={id} className="login-field-label">
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
    <div>
      <label className="login-remember cursor-pointer select-none">
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
          className="w-3.5 h-3.5 rounded border"
        />
        <span>{label}</span>
      </label>
      {hint ? <p className="login-apple-note" style={{ marginTop: '0.35rem', marginBottom: 0 }}>{hint}</p> : null}
    </div>
  );
}

/** @deprecated typo guard — use PremiumLoginShell */
export const PremumLoginShell = PremiumLoginShell;
