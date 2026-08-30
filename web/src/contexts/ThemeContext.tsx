// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react'

/**
 * Product themes mapped 1:1 onto Zeus OS identities.
 * `light` → tahoe-light; `dark` → Classic Blue (`data-ui-shell=default`).
 */
export type AppTheme = 'dark' | 'steel' | 'aurora' | 'rack' | 'light'

const THEME_CYCLE: AppTheme[] = ['dark', 'steel', 'aurora', 'rack', 'light']

export const THEME_LABELS: Record<AppTheme, string> = {
  light: 'Tahoe Light',
  dark: 'Classic Blue',
  steel: 'Dark Steel',
  aurora: 'Aurora',
  rack: 'Rack',
}

/** html[data-theme] values — match Zeus themeStore HTML_THEME. */
const HTML_THEME: Record<AppTheme, string> = {
  light: 'tahoe-light',
  dark: 'tahoe',
  steel: 'dark-steel',
  aurora: 'aurora',
  rack: 'rack',
}

/** html[data-ui-shell] — Zeus palette selectors (Classic Blue = default). */
const HTML_UI_SHELL: Record<AppTheme, string> = {
  light: 'tahoe-light',
  dark: 'default',
  steel: 'dark-steel',
  aurora: 'aurora',
  rack: 'rack',
}

interface ThemeContextType {
  theme: AppTheme
  setTheme: (t: AppTheme) => void
  cycleTheme: () => void
}

const ThemeContext = createContext<ThemeContextType>({
  theme: 'light',
  setTheme: () => {},
  cycleTheme: () => {},
})

function parseStoredTheme(raw: string | null): AppTheme {
  if (raw === 'light' || raw === 'aurora' || raw === 'steel' || raw === 'rack' || raw === 'dark') return raw
  // Zeus default
  return 'light'
}

function applyHtmlThemeAttrs(theme: AppTheme) {
  const root = document.documentElement
  root.dataset.theme = HTML_THEME[theme]
  root.setAttribute('data-ui-shell', HTML_UI_SHELL[theme])
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<AppTheme>(() =>
    parseStoredTheme(typeof localStorage !== 'undefined' ? localStorage.getItem('machina-theme') : null),
  )

  const setTheme = useCallback((t: AppTheme) => {
    setThemeState(t)
  }, [])

  useEffect(() => {
    localStorage.setItem('machina-theme', theme)
    const root = document.documentElement
    root.classList.remove(
      'steel-theme',
      'aurora-theme',
      'rack-theme',
      'liquid-glass-app',
      'apple-light',
      'light-theme',
    )
    root.classList.add('machina-clean')
    applyHtmlThemeAttrs(theme)
    if (theme === 'steel') {
      root.classList.add('steel-theme')
    } else if (theme === 'aurora') {
      root.classList.add('aurora-theme')
    } else if (theme === 'rack') {
      root.classList.add('rack-theme')
    } else if (theme === 'light') {
      root.classList.add('apple-light', 'light-theme')
    } else {
      // Classic Blue — Zeus graphite + Mist (not System Blue liquid-glass)
      root.classList.add('liquid-glass-app')
    }
  }, [theme])

  const cycleTheme = useCallback(() => {
    setThemeState((t) => {
      const i = THEME_CYCLE.indexOf(t)
      return THEME_CYCLE[(i + 1) % THEME_CYCLE.length]
    })
  }, [])

  return (
    <ThemeContext.Provider value={{ theme, setTheme, cycleTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}

export function useTheme() {
  return useContext(ThemeContext)
}
