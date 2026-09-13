// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

interface StatusBadgeProps {
  status: string
  variant?: 'dot' | 'pill'
  title?: string
}

const SUCCESS = 'text-[var(--zf-success)] bg-[var(--zf-success)]/10 border-[var(--zf-success)]/25'
const DANGER = 'text-[var(--zf-danger)] bg-[var(--zf-danger)]/10 border-[var(--zf-danger)]/25'
const WARNING = 'text-[var(--zf-warning)] bg-[var(--zf-warning)]/10 border-[var(--zf-warning)]/25'

const statusStyles: Record<string, string> = {
  running: SUCCESS,
  active: SUCCESS,
  enabled: SUCCESS,
  healthy: SUCCESS,
  completed: SUCCESS,
  success: SUCCESS,
  stopped: DANGER,
  failed: DANGER,
  error: DANGER,
  disabled: DANGER,
  paused: WARNING,
  warning: WARNING,
  pending: WARNING,
  in_progress: WARNING,
  unknown: 'text-[var(--zf-muted)] bg-[var(--zf-canvas)] border-[var(--zf-hairline)]',
}

const dotColors: Record<string, string> = {
  running: 'bg-[var(--zf-success)]',
  active: 'bg-[var(--zf-success)]',
  enabled: 'bg-[var(--zf-success)]',
  healthy: 'bg-[var(--zf-success)]',
  completed: 'bg-[var(--zf-success)]',
  success: 'bg-[var(--zf-success)]',
  stopped: 'bg-[var(--zf-danger)]',
  failed: 'bg-[var(--zf-danger)]',
  error: 'bg-[var(--zf-danger)]',
  disabled: 'bg-[var(--zf-danger)]',
  paused: 'bg-[var(--zf-warning)]',
  warning: 'bg-[var(--zf-warning)]',
  pending: 'bg-[var(--zf-warning)]',
  in_progress: 'bg-[var(--zf-warning)]',
  unknown: 'bg-[var(--zf-muted)]',
}

const isRunning = (s: string) =>
  ['running', 'active', 'enabled', 'healthy'].includes(s.toLowerCase())

export function StatusBadge({ status, variant = 'pill', title }: StatusBadgeProps) {
  const key = status.toLowerCase()
  const style = statusStyles[key] || statusStyles.unknown
  const pulse = isRunning(key)

  if (variant === 'dot') {
    const dot = dotColors[key] || dotColors.unknown
    return (
      <span className="flex items-center gap-2" title={title}>
        <span className="relative flex h-2 w-2">
          {pulse && <span className={`absolute inset-0 rounded-full ${dot} opacity-40 animate-ping`} />}
          <span className={`relative w-2 h-2 rounded-full ${dot}`} />
        </span>
        <span className="capitalize text-[var(--zf-ink)]">{status}</span>
      </span>
    )
  }

  return (
    <span
      title={title}
      className={`inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-medium capitalize border ${style}`}
    >
      <span className="relative flex h-1.5 w-1.5">
        {pulse && (
          <span
            className={`absolute inset-0 rounded-full ${dotColors[key] || dotColors.unknown} opacity-40 animate-ping`}
          />
        )}
        <span className={`relative w-1.5 h-1.5 rounded-full ${dotColors[key] || dotColors.unknown}`} />
      </span>
      {status}
    </span>
  )
}
