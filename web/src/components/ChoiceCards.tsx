// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import type { ReactNode } from 'react'
import { Link } from 'react-router'

/** Accent used for selected state (ring + border + tint). */
export type ChoiceTone = 'blue' | 'amber' | 'sky' | 'cyan' | 'purple' | 'emerald' | 'slate' | 'violet'

const selectedClass: Record<ChoiceTone, string> = {
  blue: 'border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_10%,transparent)] ring-1 ring-[color-mix(in_srgb,var(--accent)_45%,transparent)]',
  amber: 'border-amber-500/70 bg-amber-950/20 ring-1 ring-amber-500/40',
  sky: 'border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_10%,transparent)] ring-1 ring-[color-mix(in_srgb,var(--accent)_45%,transparent)]',
  cyan: 'border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_10%,transparent)] ring-1 ring-[color-mix(in_srgb,var(--accent)_40%,transparent)]',
  purple: 'border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_10%,transparent)] ring-1 ring-[color-mix(in_srgb,var(--accent)_40%,transparent)]',
  emerald: 'border-emerald-500/60 bg-[var(--apple-surface)] ring-1 ring-emerald-500/40',
  slate: 'border-[var(--apple-hairline)] bg-[var(--apple-surface)] ring-1 ring-white/10',
  violet: 'border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_10%,transparent)] ring-1 ring-[color-mix(in_srgb,var(--accent)_40%,transparent)]',
}

const iconSelectedClass: Record<ChoiceTone, string> = {
  blue: 'bg-[color-mix(in_srgb,var(--accent)_22%,transparent)] text-[var(--accent)]',
  amber: 'bg-amber-600/25 text-amber-800',
  sky: 'bg-[color-mix(in_srgb,var(--accent)_22%,transparent)] text-[var(--accent)]',
  cyan: 'bg-[color-mix(in_srgb,var(--accent)_22%,transparent)] text-[var(--accent)]',
  purple: 'bg-[color-mix(in_srgb,var(--accent)_22%,transparent)] text-[var(--accent)]',
  emerald: 'bg-emerald-600/25 text-emerald-700',
  slate: 'bg-white/10 text-[var(--text-primary)]',
  violet: 'bg-[color-mix(in_srgb,var(--accent)_22%,transparent)] text-[var(--accent)]',
}

const iconIdleClass: Record<ChoiceTone, string> = {
  blue: 'bg-[var(--surface-hover)]/80 text-[var(--text-secondary)]',
  amber: 'bg-[var(--apple-fill-secondary)] text-amber-800',
  sky: 'bg-[var(--surface-hover)]/80 text-[var(--text-secondary)]',
  cyan: 'bg-[var(--apple-fill-secondary)] text-[var(--text-primary)]',
  purple: 'bg-[var(--surface-hover)]/80 text-[var(--text-secondary)]',
  emerald: 'bg-[var(--surface-hover)]/80 text-[var(--text-secondary)]',
  slate: 'bg-[var(--surface-hover)]/80 text-[var(--text-secondary)]',
  violet: 'bg-[var(--surface-hover)]/80 text-[var(--text-secondary)]',
}

const baseUnselected = 'border-[var(--apple-hairline)] bg-[var(--apple-surface)] hover:border-[var(--apple-hairline)] hover:bg-[var(--apple-surface)]'

const baseButton =
  'rounded-xl border text-left transition flex flex-col gap-2 focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-950 disabled:opacity-45 disabled:pointer-events-none'

export function ChoiceCardGrid({ children, className = '' }: { children: ReactNode; className?: string }) {
  return <div className={`grid grid-cols-1 sm:grid-cols-2 gap-3 ${className}`.trim()}>{children}</div>
}

/** Denser grid for many options (e.g. settings / VM detail tabs). */
export function ChoiceCardDenseGrid({ children, className = '' }: { children: ReactNode; className?: string }) {
  return (
    <div className={`grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-2 ${className}`.trim()}>{children}</div>
  )
}

/** Same shell as an idle choice card, for navigation (e.g. NodeInfo quick links). */
export function ChoiceLinkCard({
  to,
  icon,
  title,
  description,
  className = '',
}: {
  to: string
  icon: ReactNode
  title: ReactNode
  description?: ReactNode
  className?: string
}) {
  return (
    <Link
      to={to}
      className={`${baseButton} p-3 gap-2 ${baseUnselected} hover:border-[var(--accent)]/45 hover:bg-[var(--apple-fill-tertiary)]/55 ${className}`.trim()}
    >
      <span className="flex items-center gap-2 text-sm font-medium text-[var(--text-primary)]">
        <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-[var(--surface-hover)]/80 text-[var(--accent)]">{icon}</span>
        {title}
      </span>
      {description ? <span className="text-xs text-[var(--text-muted)] leading-snug">{description}</span> : null}
    </Link>
  )
}

export function ChoiceCard({
  selected,
  onClick,
  icon,
  title,
  description,
  tone,
  disabled,
  compact,
  largeIcon,
  className = '',
}: {
  selected: boolean
  onClick: () => void
  icon: ReactNode
  title: ReactNode
  description?: ReactNode
  tone: ChoiceTone
  disabled?: boolean
  compact?: boolean
  /** Taller icon tile (e.g. primary flow pickers on Create VM). */
  largeIcon?: boolean
  className?: string
}) {
  const pad = compact ? 'p-2.5 gap-1.5' : 'p-4 gap-2'
  const minh = compact ? '' : 'min-h-[108px]'
  const iconBox = largeIcon ? 'h-10 w-10 shrink-0' : compact ? 'h-8 w-8 shrink-0' : 'h-9 w-9 shrink-0'
  const titleCls = compact ? 'text-sm font-medium text-[var(--text-primary)]' : largeIcon ? 'text-[var(--text-primary)] font-semibold' : 'text-[var(--text-primary)] font-medium'
  const descCls = compact
    ? 'text-[11px] text-[var(--text-muted)] leading-snug line-clamp-2'
    : largeIcon
      ? 'text-sm text-[var(--text-muted)] leading-snug'
      : 'text-xs text-[var(--text-muted)] leading-relaxed'

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`${baseButton} ${pad} ${minh} ${selected ? selectedClass[tone] : baseUnselected} ${className}`.trim()}
    >
      <span className={`flex items-center gap-2 ${compact ? '' : ''}`}>
        <span
          className={`flex ${iconBox} items-center justify-center rounded-lg ${
            selected ? iconSelectedClass[tone] : iconIdleClass[tone]
          }`}
        >
          {icon}
        </span>
        <span className={titleCls}>{title}</span>
      </span>
      {description ? <span className={descCls}>{description}</span> : null}
    </button>
  )
}
