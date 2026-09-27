// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Maximize2, Minimize2, ScrollText, Search, SquareTerminal, X } from 'lucide-react'
import { DataTable, EmptyState, PageHeader, StatusBadge, type DataTableColumn } from '../components/ui'
import ErrorBanner from '../components/ErrorBanner'
import { listNamespaces, listPods, type PodSummary } from '../api/pods'

const PodLogs = lazy(() => import('../components/PodLogs'))
const PodExec = lazy(() => import('../components/PodExec'))

const REFRESH_MS = 10_000
type Filter = 'all' | 'running' | 'pending' | 'failed'
type Mode = 'logs' | 'exec'

function badgeKey(pod: PodSummary): string {
  const s = pod.status
  if (s === 'Running') return 'running'
  if (s === 'Succeeded' || s === 'Completed') return 'completed'
  if (s === 'Terminating') return 'paused'
  if (/BackOff|Err|Fail|OOMKilled|Evicted|Unknown/i.test(s)) return 'failed'
  return 'pending'
}

function matchesFilter(pod: PodSummary, f: Filter): boolean {
  if (f === 'all') return true
  const k = badgeKey(pod)
  if (f === 'running') return k === 'running'
  if (f === 'failed') return k === 'failed'
  return k === 'pending'
}

function formatAge(secs: number | null): string {
  if (secs === null) return '—'
  if (secs < 60) return `${secs}s`
  if (secs < 3600) return `${Math.floor(secs / 60)}m`
  if (secs < 86_400) return `${Math.floor(secs / 3600)}h`
  return `${Math.floor(secs / 86_400)}d`
}

function containerNames(pod: PodSummary): string[] {
  const main = pod.containers.filter((c) => !c.init).map((c) => c.name)
  const init = pod.containers.filter((c) => c.init).map((c) => c.name)
  return [...main, ...init]
}

const podKey = (p: PodSummary) => `${p.namespace}/${p.name}`

export default function Pods() {
  const [pods, setPods] = useState<PodSummary[]>([])
  const [namespaces, setNamespaces] = useState<string[]>([])
  const [namespace, setNamespace] = useState('')
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<Filter>('all')
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [active, setActive] = useState<{ key: string; mode: Mode; container: string } | null>(null)
  const [panelHeight, setPanelHeight] = useState(460)
  const [maximized, setMaximized] = useState(false)
  const panelRef = useRef<HTMLDivElement>(null)

  const load = useCallback(
    async (quiet = false) => {
      if (!quiet) setRefreshing(true)
      try {
        setPods(await listPods(namespace || undefined))
        setError(null)
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e))
      } finally {
        setLoading(false)
        setRefreshing(false)
      }
    },
    [namespace],
  )

  useEffect(() => {
    void load()
    const t = window.setInterval(() => void load(true), REFRESH_MS)
    return () => window.clearInterval(t)
  }, [load])

  useEffect(() => {
    listNamespaces().then(setNamespaces).catch(() => setNamespaces([]))
  }, [])

  useEffect(() => {
    if (!maximized) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setMaximized(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [maximized])

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase()
    return pods.filter(
      (p) =>
        matchesFilter(p, filter) &&
        (!q ||
          p.name.toLowerCase().includes(q) ||
          p.namespace.toLowerCase().includes(q) ||
          (p.node ?? '').toLowerCase().includes(q)),
    )
  }, [pods, query, filter])

  const counts = useMemo(() => {
    const c = { running: 0, pending: 0, failed: 0 }
    for (const p of pods) {
      const k = badgeKey(p)
      if (k === 'running') c.running++
      else if (k === 'failed') c.failed++
      else if (k === 'pending') c.pending++
    }
    return c
  }, [pods])

  const activePod = active ? pods.find((p) => podKey(p) === active.key) : undefined

  const open = (pod: PodSummary, mode: Mode) => {
    const names = containerNames(pod)
    const keep = active && active.key === podKey(pod) && names.includes(active.container)
    setActive({ key: podKey(pod), mode, container: keep ? active!.container : names[0] ?? '' })
    requestAnimationFrame(() => panelRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' }))
  }

  const startResize = (e: React.PointerEvent) => {
    e.preventDefault()
    const startY = e.clientY
    const startH = panelHeight
    const move = (ev: PointerEvent) =>
      setPanelHeight(Math.min(Math.max(240, startH + ev.clientY - startY), window.innerHeight - 120))
    const up = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
  }

  const columns: DataTableColumn<PodSummary>[] = [
    {
      key: 'name',
      header: 'Name',
      sortable: true,
      sortValue: (p) => p.name,
      render: (p) => (
        <div className="min-w-0">
          <div className="font-medium text-[var(--zf-ink)] truncate max-w-[28rem]" title={p.name}>
            {p.name}
          </div>
          {p.owner_kind && <div className="text-xs text-[var(--zf-tertiary)]">{p.owner_kind}</div>}
        </div>
      ),
    },
    {
      key: 'namespace',
      header: 'Namespace',
      sortable: true,
      sortValue: (p) => p.namespace,
      render: (p) => <span className="text-[var(--zf-secondary)]">{p.namespace}</span>,
    },
    {
      key: 'status',
      header: 'Status',
      sortable: true,
      sortValue: (p) => p.status,
      render: (p) => <StatusBadge status={badgeKey(p)} label={p.status} />,
    },
    { key: 'ready', header: 'Ready', render: (p) => <span className="tabular-nums">{p.ready}</span> },
    {
      key: 'restarts',
      header: 'Restarts',
      sortable: true,
      sortValue: (p) => p.restarts,
      render: (p) => (
        <span className={`tabular-nums ${p.restarts > 0 ? 'text-[var(--zf-warning-text)]' : ''}`}>{p.restarts}</span>
      ),
    },
    {
      key: 'age',
      header: 'Age',
      sortable: true,
      sortValue: (p) => p.age_seconds ?? 0,
      render: (p) => <span className="tabular-nums text-[var(--zf-secondary)]">{formatAge(p.age_seconds)}</span>,
    },
    {
      key: 'node',
      header: 'Node',
      render: (p) => <span className="text-[var(--zf-secondary)]">{p.node ?? '—'}</span>,
    },
    {
      key: 'actions',
      header: '',
      className: 'text-right whitespace-nowrap',
      render: (p) => (
        <div className="inline-flex gap-2" onClick={(e) => e.stopPropagation()}>
          <button type="button" className="zf-btn zf-btn-secondary zf-btn-xs" onClick={() => open(p, 'logs')}>
            <ScrollText className="w-3.5 h-3.5" />
            Logs
          </button>
          <button
            type="button"
            className="zf-btn zf-btn-secondary zf-btn-xs"
            onClick={() => open(p, 'exec')}
            disabled={p.phase !== 'Running'}
            title={p.phase !== 'Running' ? 'Pod is not running' : 'Open a shell'}
          >
            <SquareTerminal className="w-3.5 h-3.5" />
            Terminal
          </button>
        </div>
      ),
    },
  ]

  const names = activePod ? containerNames(activePod) : []

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Ops"
        title="Pods"
        description="Every pod in the cluster. Stream colorized logs or open a shell, Terminal.app style."
        onRefresh={() => void load()}
        refreshing={refreshing}
      />

      {error && (
        <ErrorBanner
          title="Could not load pods"
          headline={error}
          hints={['Pods requires the cluster.admin permission and pods list access for the Zorvia service account.']}
          onRetry={() => void load()}
        />
      )}

      <div className="toolbar-pill">
        <select
          aria-label="Namespace"
          className="input-field"
          style={{ flex: '0 0 220px', borderRadius: 980, minHeight: 36, padding: '6px 14px' }}
          value={namespace}
          onChange={(e) => setNamespace(e.target.value)}
        >
          <option value="">All namespaces</option>
          {namespaces.map((n) => (
            <option key={n} value={n}>
              {n}
            </option>
          ))}
        </select>
        <div className="relative flex-1 min-w-[220px]">
          <Search className="w-4 h-4 absolute left-3.5 top-1/2 -translate-y-1/2 text-[var(--zf-tertiary)] pointer-events-none" />
          <input
            aria-label="Search pods"
            className="input-field"
            style={{ borderRadius: 980, minHeight: 36, padding: '6px 14px 6px 36px' }}
            placeholder="Search pods, namespaces, nodes"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <div className="toolbar-pill__trailing">
          {(
            [
              ['all', `All ${pods.length}`],
              ['running', `Running ${counts.running}`],
              ['pending', `Pending ${counts.pending}`],
              ['failed', `Failed ${counts.failed}`],
            ] as [Filter, string][]
          ).map(([f, label]) => (
            <button
              key={f}
              type="button"
              className={`zf-chip ${filter === f ? 'zf-chip-active' : ''}`}
              aria-pressed={filter === f}
              onClick={() => setFilter(f)}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      <DataTable
        columns={columns}
        rows={visible}
        getRowKey={podKey}
        loading={loading}
        onRowClick={(p) => open(p, 'logs')}
        rowClassName={(p) => (active?.key === podKey(p) ? 'selected' : '')}
        emptyState={
          <EmptyState
            title={pods.length ? 'No pods match' : 'No pods'}
            description={pods.length ? 'Try another namespace, search or status filter.' : 'Nothing is scheduled here yet.'}
          />
        }
      />

      {active && activePod && (
        <div
          ref={panelRef}
          className={maximized ? 'fixed inset-x-3 bottom-3 z-[60] flex flex-col' : 'flex flex-col'}
          style={maximized ? { top: 'calc(var(--console-topbar-height) + 12px)' } : { height: panelHeight }}
          data-testid="pod-panel"
        >
          <div className="flex items-center gap-2 mb-2">
            <button
              type="button"
              className={`zf-chip ${active.mode === 'logs' ? 'zf-chip-active' : ''}`}
              aria-pressed={active.mode === 'logs'}
              onClick={() => setActive({ ...active, mode: 'logs' })}
            >
              <ScrollText className="w-3.5 h-3.5" /> Logs
            </button>
            <button
              type="button"
              className={`zf-chip ${active.mode === 'exec' ? 'zf-chip-active' : ''}`}
              aria-pressed={active.mode === 'exec'}
              onClick={() => setActive({ ...active, mode: 'exec' })}
              disabled={activePod.phase !== 'Running'}
            >
              <SquareTerminal className="w-3.5 h-3.5" /> Terminal
            </button>
            <span className="ml-2 text-sm text-[var(--zf-secondary)] truncate">
              {activePod.namespace}/{activePod.name}
            </span>
            <div className="ml-auto flex items-center gap-1">
              <button
                type="button"
                className="console-icon-btn"
                onClick={() => setMaximized((v) => !v)}
                aria-label={maximized ? 'Restore' : 'Maximize'}
              >
                {maximized ? <Minimize2 className="w-4 h-4" /> : <Maximize2 className="w-4 h-4" />}
              </button>
              <button
                type="button"
                className="console-icon-btn"
                onClick={() => {
                  setActive(null)
                  setMaximized(false)
                }}
                aria-label="Close panel"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
          </div>
          <div className="flex-1 min-h-0">
            <Suspense fallback={<div className="zf-terminal zf-terminal-pro rounded-xl h-full" />}>
              {active.mode === 'logs' ? (
                <PodLogs
                  namespace={activePod.namespace}
                  pod={activePod.name}
                  containers={names}
                  container={active.container}
                  onContainerChange={(c) => setActive({ ...active, container: c })}
                />
              ) : (
                <PodExec
                  namespace={activePod.namespace}
                  pod={activePod.name}
                  containers={names}
                  container={active.container}
                  onContainerChange={(c) => setActive({ ...active, container: c })}
                />
              )}
            </Suspense>
          </div>
          {!maximized && (
            <div
              role="separator"
              aria-orientation="horizontal"
              aria-label="Resize terminal"
              className="h-3 mt-1 cursor-row-resize flex items-center justify-center"
              onPointerDown={startResize}
            >
              <span className="w-10 h-1 rounded-full bg-[var(--zf-hairline-strong)]" />
            </div>
          )}
        </div>
      )}
    </div>
  )
}
