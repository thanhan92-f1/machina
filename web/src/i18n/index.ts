// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import en from './locales/en.json'
import es from './locales/es.json'

const stored = typeof localStorage !== 'undefined' ? localStorage.getItem('machina_lang') : null

void i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    es: { translation: es },
  },
  lng: stored === 'es' ? 'es' : 'en',
  fallbackLng: 'en',
  interpolation: { escapeValue: false },
})

export function setAppLanguage(lang: 'en' | 'es') {
  void i18n.changeLanguage(lang)
  localStorage.setItem('machina_lang', lang)
}

export default i18n
