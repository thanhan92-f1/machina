// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useEffect, useState } from 'react'
import { useConsoleViewportOptional } from './ConsoleViewportContext'

type Props = {
  visible?: boolean
}

export default function FloatingConsoleHud({ visible = true }: Props) {
  const vp = useConsoleViewportOptional()
  const [show, setShow] = useState(true)
  const [idle, setIdle] = useState(false)

  useEffect(() => {
    if (!visible) return
    let timer: ReturnType<typeof setTimeout>
    const reset = () => {
      setShow(true)
      setIdle(false)
      clearTimeout(timer)
      timer = setTimeout(() => setIdle(true), 3000)
    }
    reset()
    window.addEventListener('mousemove', reset)
    return () => {
      window.removeEventListener('mousemove', reset)
      clearTimeout(timer)
    }
  }, [visible])

  if (!visible || !vp) return null

  const tone = vp.connected ? 'bg-emerald-500' : 'bg-amber-500'
  const label = vp.connected ? 'Connected' : 'Disconnected'

  return (
    <div
      className={`absolute top-3 right-3 z-30 transition-opacity duration-300 ${show && !idle ? 'opacity-100' : 'opacity-0 pointer-events-none'}`}
    >
      <div className="flex items-center gap-2 px-3 py-1.5 rounded-full border border-white/10 bg-black/60 backdrop-blur-md text-xs text-[#e2e8f0] shadow-lg">
        <span className={`w-2 h-2 rounded-full ${tone}`} />
        <span>{label}</span>
        <span className="text-[#64748b]">·</span>
        <span className="uppercase text-[#94a3b8]">{vp.protocol}</span>
        <span className="text-[#64748b]">·</span>
        <span className="font-mono text-[#cbd5e1]">{vp.resolution}</span>
        <span className="text-[#64748b]">·</span>
        <span className="text-[#94a3b8] capitalize">{vp.mode}</span>
        {vp.mode === 'zoom' ? (
          <>
            <span className="text-[#64748b]">·</span>
            <span>{vp.zoom}%</span>
          </>
        ) : null}
      </div>
    </div>
  )
}
