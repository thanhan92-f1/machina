// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import type { LaunchpadDiagnosis } from '../../api/launchpad'
import { statusToneClass } from '../../utils/semanticColors'

type Props = {
  diagnosis: LaunchpadDiagnosis
}

function nodeTone(status?: string): 'ok' | 'warn' | 'error' | 'neutral' {
  switch (status) {
    case 'healthy':
      return 'ok'
    case 'degraded':
      return 'warn'
    case 'broken':
      return 'error'
    default:
      return status ? 'warn' : 'neutral'
  }
}

export default function LaunchpadRouteLens({ diagnosis }: Props) {
  return (
    <div className="space-y-1" data-testid="launchpad-route-lens">
      <p className="text-xs text-slate-400 mb-3">Zyra Lens — route chain from user to backend</p>
      <ol className="relative border-l border-white/10 ml-2 space-y-3">
        {diagnosis.chain.map((node, idx) => {
          const tone = nodeTone(node.status)
          const isLast = idx === diagnosis.chain.length - 1
          return (
            <li key={node.id} className="pl-4 relative">
              <span
                className={`absolute -left-[5px] top-1.5 w-2.5 h-2.5 rounded-full border-2 border-slate-950 ${
                  tone === 'ok'
                    ? 'bg-emerald-400'
                    : tone === 'error'
                      ? 'bg-amber-400'
                      : tone === 'warn'
                        ? 'bg-orange-400'
                        : 'bg-slate-500'
                }`}
                aria-hidden
              />
              <div className="font-mono text-xs text-slate-200">{node.label}</div>
              {node.status ? (
                <span
                  className={`inline-block mt-1 text-[10px] font-semibold uppercase tracking-wide px-2 py-0.5 rounded-full border border-white/10 ${statusToneClass(tone)}`}
                >
                  {node.status}
                </span>
              ) : null}
              {!isLast ? <div className="h-2" aria-hidden /> : null}
            </li>
          )
        })}
      </ol>
      <div className="mt-4 rounded-xl border border-white/10 bg-black/20 p-3 font-mono text-xs text-slate-300">
        <div className="text-slate-500 mb-1">Stable URL</div>
        <div className="break-all">{diagnosis.publicUrl || diagnosis.routePath}</div>
      </div>
    </div>
  )
}
