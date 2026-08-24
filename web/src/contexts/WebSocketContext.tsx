// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { createContext, useContext, useEffect, useState, useRef, useCallback, ReactNode } from 'react'
import { getWsToken } from '../api/client'

interface WSMessage {
  type: string
  data?: unknown
}

export interface VMEvent {
  event: 'state_change' | 'vm_added' | 'vm_removed'
  name: string
  old_state?: string
  new_state?: string
  state?: string
  timestamp: number
}

/** `live` = VM watch socket open; `connecting` = fetching token or opening socket / backoff; `offline` = repeated token failure (e.g. session gone). */
export type WsConnection = 'live' | 'connecting' | 'offline'

interface WebSocketContextType {
  /** True when real-time `/ws/v1/watch` is connected. */
  isConnected: boolean
  connection: WsConnection
  subscribe: (callback: (msg: WSMessage) => void) => () => void
  events: VMEvent[]
}

const WebSocketContext = createContext<WebSocketContextType>({
  isConnected: false,
  connection: 'connecting',
  subscribe: () => () => {},
  events: [],
})

// Exponential backoff bounds for reconnecting the watch socket.
const WS_RETRY_INITIAL_MS = 1000
const WS_RETRY_MAX_MS = 30000
// After this many consecutive ws-token failures, surface `offline` instead of endlessly
// retrying silently — e.g. the session/cookie is gone and every retry will fail the same way.
const WS_TOKEN_FAIL_OFFLINE_THRESHOLD = 6
// Cap the in-memory event log so a long-lived tab doesn't grow this array unbounded.
const WS_EVENTS_BUFFER_SIZE = 50

export function WebSocketProvider({ children }: { children: ReactNode }) {
  const [isConnected, setIsConnected] = useState(false)
  const [connection, setConnection] = useState<WsConnection>('connecting')
  const [events, setEvents] = useState<VMEvent[]>([])
  const subscribersRef = useRef<Set<(msg: WSMessage) => void>>(new Set())
  const wsRef = useRef<WebSocket | null>(null)
  const tokenFailRef = useRef(0)

  useEffect(() => {
    let cancelled = false
    let retryDelay = WS_RETRY_INITIAL_MS
    let retryTimer: ReturnType<typeof setTimeout> | null = null

    async function connect() {
      if (cancelled) return
      if (retryTimer) {
        clearTimeout(retryTimer)
        retryTimer = null
      }
      setConnection('connecting')
      setIsConnected(false)
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
      let token: string
      try {
        token = await getWsToken()
        tokenFailRef.current = 0
      } catch {
        tokenFailRef.current += 1
        if (tokenFailRef.current >= WS_TOKEN_FAIL_OFFLINE_THRESHOLD) {
          setConnection('offline')
        }
        if (import.meta.env.DEV) {
          console.warn('machina: ws-token failed; real-time updates unavailable until it succeeds')
        }
        retryTimer = setTimeout(connect, retryDelay)
        retryDelay = Math.min(retryDelay * 2, WS_RETRY_MAX_MS)
        return
      }
      const ws = new WebSocket(`${protocol}//${window.location.host}/ws/v1/watch?token=${encodeURIComponent(token)}`)
      wsRef.current = ws

      ws.onopen = () => {
        if (cancelled) return
        setIsConnected(true)
        setConnection('live')
        retryDelay = 1000
      }
      ws.onclose = (ev) => {
        if (cancelled) return
        setIsConnected(false)
        setConnection('connecting')
        if (import.meta.env.DEV) {
          console.warn('machina: /ws/v1/watch closed', ev.code, ev.reason || '(no reason)')
        }
        retryTimer = setTimeout(connect, retryDelay)
        retryDelay = Math.min(retryDelay * 2, WS_RETRY_MAX_MS)
      }
      ws.onerror = () => ws.close()
      ws.onmessage = (e) => {
        try {
          const data = JSON.parse(e.data)
          if (data.changes && Array.isArray(data.changes)) {
            const newEvents: VMEvent[] = data.changes.map((c: VMEvent) => ({ ...c, timestamp: Date.now() }))
            setEvents(prev => [...newEvents, ...prev].slice(0, WS_EVENTS_BUFFER_SIZE))
          }
          const msg = data as WSMessage
          // Daemon sends JSON every 2s (`heartbeat` or `changes`). Only notify subscribers on `changes`
          // so list pages do not refetch /api/v1/vms every heartbeat (wasteful; floods errors if API is down).
          if ((data as { event?: string }).event !== 'changes') {
            return
          }
          subscribersRef.current.forEach((cb) => {
            try {
              cb(msg)
            } catch (err) {
              console.error('WebSocket subscriber error:', err)
            }
          })
        } catch {
          if (import.meta.env.DEV) {
            console.warn('Non-JSON WebSocket message:', e.data)
          }
        }
      }
    }

    connect()
    return () => {
      cancelled = true
      if (retryTimer) clearTimeout(retryTimer)
      wsRef.current?.close()
    }
  }, [])

  const subscribe = useCallback((callback: (msg: WSMessage) => void) => {
    subscribersRef.current.add(callback)
    return () => { subscribersRef.current.delete(callback) }
  }, [])

  return (
    <WebSocketContext.Provider value={{ isConnected, connection, subscribe, events }}>
      {children}
    </WebSocketContext.Provider>
  )
}

export function useWebSocketContext() {
  return useContext(WebSocketContext)
}
