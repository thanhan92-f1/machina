// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import React from 'react'
import ReactDOM from 'react-dom/client'
import './i18n'
import App from './App'
import AppErrorBoundary from './components/AppErrorBoundary'
import './styles/main.css'
import './styles/netra-look.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppErrorBoundary surface="app">
      <App />
    </AppErrorBoundary>
  </React.StrictMode>,
)
