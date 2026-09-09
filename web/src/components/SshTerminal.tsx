// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from 'react'
import { Terminal as XTerm } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'
import { getToken } from '../api/client'
import { RotateCw } from 'lucide-react'
import { AppleTerminalFrame } from './AppleTerminalFrame'

interface SshTerminalProps {
  vmName: string
  user?: string
}

type Status = 'connecting' | 'connected' | 'disconnected'

export default function SshTerminal({ vmName, user = 'zorvia' }: SshTerminalProps) {
  const terminalRef = useRef<HTMLDivElement>(null)
  const [status, setStatus] = useState<Status>('connecting')
  const [connectAttempt, setConnectAttempt] = useState(0)
  const [sshUser, setSshUser] = useState(user)

  useEffect(() => {
    if (!terminalRef.current) return
    setStatus('connecting')
    terminalRef.current.replaceChildren()

    const term = new XTerm({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
      theme: {
        background: '#1c1c1e',
        foreground: '#f5f5f7',
        cursor: '#f5f5f7',
        selectionBackground: '#0a84ff66',
      },
    })
    term.open(terminalRef.current)

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const token = getToken()
    const params = new URLSearchParams()
    if (token) params.set('token', token)
    params.set('user', sshUser)
    const wsUrl = `${protocol}//${window.location.host}/ws/ssh/${vmName}?${params.toString()}`
    const ws = new WebSocket(wsUrl)
    ws.binaryType = 'arraybuffer'

    ws.onopen = () => {
      setStatus('connected')
      term.write(`SSH session for ${sshUser}@${vmName}\r\n`)
    }
    ws.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        term.write(new Uint8Array(event.data))
      } else {
        term.write(event.data)
      }
    }
    ws.onerror = () => term.write('\r\nWebSocket error\r\n')
    ws.onclose = () => {
      setStatus('disconnected')
      term.write('\r\nSSH connection closed\r\n')
    }
    term.onData((data) => {
      if (ws.readyState === WebSocket.OPEN) ws.send(data)
    })

    return () => {
      ws.close()
      term.dispose()
    }
  }, [vmName, sshUser, connectAttempt])

  const statusMeta: Record<Status, { label: string; color: string }> = {
    connecting: { label: 'Connecting…', color: '#ffd60a' },
    connected: { label: 'Connected', color: '#28c840' },
    disconnected: { label: 'Disconnected', color: '#8e8e93' },
  }
  const meta = statusMeta[status]

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <label className="text-xs text-[var(--zf-muted)]">User</label>
        <input
          value={sshUser}
          onChange={(e) => setSshUser(e.target.value.replace(/[^A-Za-z0-9._-]/g, ''))}
          className="zf-input zf-input-sm w-40 font-mono text-xs"
          maxLength={32}
        />
      </div>
      <AppleTerminalFrame
        title={`${sshUser}@${vmName} — ssh`}
        live={status === 'connected'}
        trailing={
          <div className="flex items-center gap-2 shrink-0">
            <span className="text-[11px]" style={{ color: meta.color }}>{meta.label}</span>
            {status === 'disconnected' && (
              <button
                type="button"
                onClick={() => setConnectAttempt((n) => n + 1)}
                className="flex items-center gap-1.5 px-2 py-1 rounded-md text-[11px] font-medium text-white/70 hover:text-white hover:bg-white/10 transition-colors"
              >
                <RotateCw className="w-3.5 h-3.5" />
                Reconnect
              </button>
            )}
          </div>
        }
        bodyClassName="p-2"
      >
        <div ref={terminalRef} className="w-full" style={{ minHeight: '500px' }} />
      </AppleTerminalFrame>
    </div>
  )
}
