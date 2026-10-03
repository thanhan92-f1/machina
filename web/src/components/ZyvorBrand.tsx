// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

/**
 * Zyvor suite branding — text only.
 * Brand line: zyvor.dev · © 2026 (all orange, no footer bar).
 */
import React from 'react';

export const ZYVOR_URL = 'https://zyvor.dev';
export const ZYVOR_BRAND = 'Zyvor';
export const ZYVOR_COPY = '© 2026';
export const ZYVOR_LINE = `zyvor.dev · ${ZYVOR_COPY}`;

const ORANGE = '#f97316';

const linkStyle: React.CSSProperties = {
  color: ORANGE,
  textDecoration: 'none',
  fontWeight: 600,
};

const linkHover = (e: React.MouseEvent<HTMLAnchorElement>) => {
  e.currentTarget.style.color = '#fb923c';
};

const linkLeave = (e: React.MouseEvent<HTMLAnchorElement>) => {
  e.currentTarget.style.color = ORANGE;
};

const orangeSep = (
  <span aria-hidden style={{ color: ORANGE }}>
    {' '}
    ·{' '}
  </span>
);

function ZyvorDevLink({ className = '' }: { className?: string }) {
  return (
    <a
      href={ZYVOR_URL}
      target="_blank"
      rel="noopener noreferrer"
      className={className}
      style={linkStyle}
      onMouseEnter={linkHover}
      onMouseLeave={linkLeave}
    >
      zyvor.dev
    </a>
  );
}

type BrandProps = {
  /** @deprecated Ignored in footer — product name is not shown. */
  product?: string;
  className?: string;
  style?: React.CSSProperties;
  includeCopyright?: boolean;
};

type FooterProps = {
  className?: string;
  /** Host OS pretty name (e.g. Rocky Linux 9.4) — shown when provided. */
  hostOs?: string;
  /** @deprecated Ignored — footer is zyvor.dev · © 2026 only. */
  product?: string;
};

/** Page footer — transparent; shows the daemon host OS line only, when present. */
export function ZyvorFooter({ className = '', hostOs }: FooterProps) {
  if (!hostOs) return null;
  return (
    <footer
      className={`zyvor-footer shrink-0 py-3 text-center bg-transparent border-0 ${className}`.trim()}
      style={{ marginTop: 'auto' }}
      role="contentinfo"
    >
      <div
        className="text-[11px] text-[var(--text-muted)]"
        title="Daemon host operating system"
      >
        {hostOs}
      </div>
    </footer>
  );
}

export default ZyvorFooter;
