// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, useCallback } from 'react'
import { listVMs, VM } from '../api/vm'
import { Activity, Server, Cpu, Power, ArrowUpRight } from 'lucide-react'
import { useWebSocketContext } from '../contexts/WebSocketContext'
import { useToastContext } from '../contexts/ToastContext'
import { SkeletonDashboard } from '../components/Skeleton'
import { StatusBadge } from '../components/ui'
import { Link } from 'react-router'
import ErrorBanner from '../components/ErrorBanner'
import { formatUserError } from '../utils/apiError'
import { toastFailure } from '../utils/toastError'
import { hintsForError } from '../utils/daemonHints'
import { GettingStarted } from '../components/GettingStarted'
import { FabricGraphic } from '../components/FabricGraphic'
import { RadialGauge } from '../components/RadialGauge'
import { useCountUp } from '../hooks/useCountUp'

export default function Dashboard() {
  const [vms, setVMs] = useState<VM[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [refreshError, setRefreshError] = useState<string | null>(null)
  const { subscribe, vmUpdates } = useWebSocketContext()
  const toast = useToastContext()

  const loadVMs = useCallback(async () => {
    try {
      setVMs(await listVMs())
      setLoadError(null)
      setRefreshError(null)
    } catch (err) {
      const msg = formatUserError(err)
      setVMs((prev) => {
        if (prev.length === 0) {
          setLoadError(msg)
          toastFailure(toast, 'Failed to load virtual machines', err)
        } else {
          setRefreshError(msg)
        }
        return prev
      })
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => {
    void loadVMs()
  }, [loadVMs])
  useEffect(() => {
    const unsub = subscribe((msg) => {
      if (['vm_state_changed', 'vm_created', 'vm_deleted'].includes(msg.type)) loadVMs()
    })
    return unsub
  }, [subscribe, loadVMs])
  useEffect(() => {
    if (vmUpdates.size > 0) {
      setVMs((prev) =>
        prev.map((vm) => {
          const u = vmUpdates.get(vm.name)
          return u ? { ...vm, ...u } : vm
        }),
      )
    }
  }, [vmUpdates])

  const stats = {
    total: vms.length,
    running: vms.filter((v) => v.state === 'running').length,
    stopped: vms.filter((v) => v.state === 'stopped').length,
    totalCPU: vms.reduce((a, v) => a + v.cpus, 0),
    totalMem: vms.reduce((a, v) => a + v.memory, 0),
  }
  const fleetHealthPct = stats.total > 0 ? (stats.running / stats.total) * 100 : 0

  const totalCount = useCountUp(stats.total)
  const runningCount = useCountUp(stats.running)
  const stoppedCount = useCountUp(stats.stopped)
  const memCount = useCountUp(Math.round(stats.totalMem >= 1024 ? stats.totalMem / 1024 : stats.totalMem))

  if (loading) return <SkeletonDashboard />

  const greeting = (() => {
    const h = new Date().getHours()
    return h < 5 ? 'Good night' : h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening'
  })()

  return (
    <div className="space-y-6">
      <div className="glass-panel relative overflow-hidden px-6 py-6 sm:px-8 sm:py-7">
        <div className="pointer-events-none absolute -right-6 -top-10 w-72 h-72 opacity-70">
          <FabricGraphic ambient />
        </div>
        <div className="relative flex flex-col sm:flex-row sm:items-center gap-6">
          <div className="flex-1 min-w-0">
            <p className="text-xs font-semibold text-[var(--zf-link)] uppercase tracking-[0.04em] mb-1">{greeting}</p>
            <h1 className="text-[32px] sm:text-[40px] font-semibold text-[var(--zf-ink)] tracking-[-0.022em] leading-none">
              Zorvia
            </h1>
            <p className="text-[17px] text-[var(--zf-secondary)] mt-2 max-w-md tracking-[-0.022em] leading-snug">
              {stats.total === 0
                ? 'Your private cloud control plane is ready.'
                : `Watching ${stats.total} VM${stats.total === 1 ? '' : 's'}. ${stats.running} running right now.`}
            </p>
            <div className="flex items-center gap-3 mt-4">
              <Link to="/app/create" className="zf-btn zf-btn-primary">
                Create VM
              </Link>
              <button type="button" onClick={() => void loadVMs()} className="zf-btn zf-btn-ghost">
                Refresh
              </button>
            </div>
          </div>
          {stats.total > 0 && (
            <div className="shrink-0 flex items-center gap-3 self-center">
              <RadialGauge
                percent={fleetHealthPct}
                color={
                  fleetHealthPct >= 70
                    ? 'var(--zf-success)'
                    : fleetHealthPct >= 30
                      ? 'var(--zf-warning)'
                      : 'var(--zf-danger)'
                }
                label="fleet up"
              />
            </div>
          )}
        </div>
      </div>

      {loadError && (
        <ErrorBanner
          title="Could not load dashboard"
          headline={loadError}
          hints={hintsForError(loadError)}
          onRetry={loadVMs}
        />
      )}
      {!loadError && refreshError && (
        <ErrorBanner
          title="Could not refresh dashboard"
          headline={refreshError}
          onRetry={loadVMs}
          tone="amber"
        />
      )}

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="stat-card-blue rounded-xl border border-[var(--zf-hairline)] p-5 transition-all hover:scale-[1.02]">
          <div className="flex items-center justify-between mb-3">
            <div className="icon-tile icon-tile-md">
              <Server className="h-5 w-5 text-[var(--zf-ink)]" />
            </div>
            <span className="text-[10px] font-medium px-2 py-0.5 rounded-full bg-[var(--zf-canvas)] text-[var(--zf-muted)]">
              total
            </span>
          </div>
          <div className="text-2xl font-bold text-[var(--zf-ink)] tabular-nums">{totalCount}</div>
          <div className="text-[13px] text-[var(--zf-secondary)] mt-1">Total VMs</div>
        </div>

        <div className="stat-card-green rounded-xl border border-[var(--zf-hairline)] p-5 transition-all hover:scale-[1.02]">
          <div className="flex items-center justify-between mb-3">
            <div className="icon-tile icon-tile-md">
              <Activity className="h-5 w-5 text-[var(--zf-ink)]" />
            </div>
            <span className="text-[10px] font-medium px-2 py-0.5 rounded-full bg-emerald-50 text-emerald-700">
              {stats.total > 0 ? `${Math.round((stats.running / stats.total) * 100)}%` : '0%'}
            </span>
          </div>
          <div className="text-2xl font-bold text-[var(--zf-ink)] tabular-nums">{runningCount}</div>
          <div className="text-[13px] text-[var(--zf-secondary)] mt-1">Running</div>
        </div>

        <div className="stat-card-red rounded-xl border border-[var(--zf-hairline)] p-5 transition-all hover:scale-[1.02]">
          <div className="flex items-center justify-between mb-3">
            <div className="icon-tile icon-tile-md">
              <Power className="h-5 w-5 text-[var(--zf-ink)]" />
            </div>
            <span className="text-[10px] font-medium px-2 py-0.5 rounded-full bg-red-50 text-red-700">
              stopped
            </span>
          </div>
          <div className="text-2xl font-bold text-[var(--zf-ink)] tabular-nums">{stoppedCount}</div>
          <div className="text-[13px] text-[var(--zf-secondary)] mt-1">Stopped</div>
        </div>

        <div className="stat-card-purple rounded-xl border border-[var(--zf-hairline)] p-5 transition-all hover:scale-[1.02]">
          <div className="flex items-center justify-between mb-3">
            <div className="icon-tile icon-tile-md">
              <Cpu className="h-5 w-5 text-[var(--zf-ink)]" />
            </div>
            <span className="text-[10px] font-medium px-2 py-0.5 rounded-full bg-[var(--zf-canvas)] text-[var(--zf-muted)]">
              {stats.totalCPU} vCPU
            </span>
          </div>
          <div className="text-2xl font-bold text-[var(--zf-ink)] tabular-nums">
            {memCount}
            <span className="text-sm text-[var(--zf-muted)] font-medium ml-1">
              {stats.totalMem >= 1024 ? 'GB' : 'MB'}
            </span>
          </div>
          <div className="text-[13px] text-[var(--zf-secondary)] mt-1">Total Memory</div>
        </div>
      </div>

      {vms.length === 0 ? (
        <GettingStarted />
      ) : (
        <div className="bg-[var(--zf-canvas)] rounded-xl border border-[var(--zf-hairline)] overflow-hidden">
          <div className="px-5 py-4 border-b border-[var(--zf-hairline)] flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="icon-tile icon-tile-sm">
                <Server className="w-4 h-4 text-[var(--zf-ink)]" />
              </div>
              <h2 className="text-base font-semibold text-[var(--zf-ink)]">Virtual Machines</h2>
            </div>
            <div className="flex items-center gap-3">
              <span className="text-xs font-medium text-[var(--zf-muted)] bg-[var(--zf-canvas)] px-2.5 py-1 rounded-full">
                {vms.length} VMs
              </span>
              <Link
                to="/app/vms"
                className="flex items-center gap-1 text-xs text-[var(--zf-link)] hover:text-[var(--zf-link-hover)] transition-colors"
              >
                View all <ArrowUpRight className="w-3.5 h-3.5" />
              </Link>
            </div>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-[var(--zf-hairline)]">
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">
                    Name
                  </th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">
                    Status
                  </th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">
                    CPU
                  </th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">
                    Memory
                  </th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">
                    IP
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--zf-hairline)]/30">
                {vms.slice(0, 8).map((vm) => (
                  <tr key={vm.name} className="table-row-hover transition-colors">
                    <td className="px-5 py-3">
                      <Link
                        to={`/app/vms/${vm.name}`}
                        className="font-medium text-[var(--zf-ink)] hover:text-[var(--zf-link)] transition-colors"
                      >
                        {vm.name}
                      </Link>
                    </td>
                    <td className="px-4 py-3">
                      <StatusBadge status={vm.state} />
                    </td>
                    <td className="px-4 py-3 text-[var(--zf-muted)] tabular-nums">{vm.cpus} vCPU</td>
                    <td className="px-4 py-3 text-[var(--zf-muted)] tabular-nums">
                      {vm.memory >= 1024 ? `${(vm.memory / 1024).toFixed(1)} GB` : `${vm.memory} MB`}
                    </td>
                    <td className="px-4 py-3 text-[var(--zf-muted)] font-mono text-xs">{vm.ip || '-'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  )
}
