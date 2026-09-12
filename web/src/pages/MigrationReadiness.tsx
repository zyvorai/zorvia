// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useState, useEffect, useCallback } from 'react'
import { getMigrationReadiness, ReadinessCheck } from '../api/migrations'
import { listVMs, VM } from '../api/vm'
import ErrorBanner from '../components/ErrorBanner'
import { PageHeader } from '../components/ui'
import { formatUserError } from '../utils/apiError'
import { toastFailure } from '../utils/toastError'
import { hintsForError } from '../utils/daemonHints'
import { useToastContext } from '../contexts/ToastContext'

function StatusIcon({ status }: { status: ReadinessCheck['status'] }) {
  if (status === 'Passed') {
    return <span className="flex items-center justify-center w-6 h-6 rounded-full bg-emerald-50 text-emerald-700 text-xs">&#10003;</span>
  }
  if (status === 'Warning') {
    return <span className="flex items-center justify-center w-6 h-6 rounded-full bg-amber-50 text-amber-800 text-xs">&#9888;</span>
  }
  if (status === 'Skipped') {
    return <span className="flex items-center justify-center w-6 h-6 rounded-full bg-[var(--zf-canvas)] text-[var(--zf-muted)] text-xs">&#8212;</span>
  }
  return <span className="flex items-center justify-center w-6 h-6 rounded-full bg-red-50 text-red-700 text-xs">&#10007;</span>
}

function statusBadgeClass(status: ReadinessCheck['status']): string {
  switch (status) {
    case 'Passed': return 'text-emerald-700 bg-emerald-50 border-emerald-200'
    case 'Warning': return 'text-amber-800 bg-amber-50 border-amber-200'
    case 'Skipped': return 'text-[var(--zf-muted)] bg-[var(--zf-canvas)] border-[var(--zf-hairline)]'
    default: return 'text-red-700 bg-red-50 border-red-200'
  }
}

export default function MigrationReadiness() {
  const toast = useToastContext()
  const [vms, setVms] = useState<VM[]>([])
  const [selectedVm, setSelectedVm] = useState('')
  const [targetNode, setTargetNode] = useState('')
  const [checks, setChecks] = useState<ReadinessCheck[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)

  const fetchReadiness = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      const [vmList, checkResults] = await Promise.all([
        listVMs(),
        getMigrationReadiness(selectedVm || undefined, targetNode.trim() || undefined),
      ])
      setVms(vmList)
      setChecks(checkResults)
    } catch (err) {
      const msg = formatUserError(err)
      setLoadError(msg)
      toastFailure(toast, 'Failed to check migration readiness', err)
    } finally {
      setLoading(false)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [toast])

  useEffect(() => { fetchReadiness() }, [fetchReadiness])

  const hasIssues = checks.some((c) => c.status === 'Failed' || c.status === 'Warning')
  const errorCount = checks.filter((c) => c.status === 'Failed').length
  const warningCount = checks.filter((c) => c.status === 'Warning').length

  return (
    <div className="space-y-6">
      <PageHeader
        title="Migration Readiness"
        description="Pre-flight checks against the real cluster before starting a live migration"
        onRefresh={fetchReadiness}
        refreshing={loading}
      />

      <div className="bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-xl p-5">
        <div className="grid grid-cols-1 md:grid-cols-[1fr_1fr_auto] gap-4 items-end">
          <div>
            <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">VM (optional — checks all VMs if empty)</label>
            <select value={selectedVm} onChange={(e) => setSelectedVm(e.target.value)} className="input-field text-sm w-full">
              <option value="">All VMs</option>
              {vms.map(vm => <option key={vm.name} value={vm.name}>{vm.name}</option>)}
            </select>
          </div>
          <div>
            <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Target node (optional)</label>
            <input type="text" value={targetNode} onChange={(e) => setTargetNode(e.target.value)} placeholder="e.g. node-2" className="input-field text-sm w-full" />
          </div>
          <button type="button" onClick={fetchReadiness} disabled={loading} className="zf-btn zf-btn-primary zf-btn-sm">
            {loading ? 'Checking…' : 'Run Checks'}
          </button>
        </div>
      </div>

      {loadError && (
        <ErrorBanner
          title="Could not run readiness checks"
          headline={loadError}
          hints={hintsForError(loadError)}
          onRetry={fetchReadiness}
        />
      )}

      {loading && !loadError ? (
        <div className="bg-[var(--zf-canvas)] rounded-xl p-10 border border-[var(--zf-hairline)] flex flex-col items-center justify-center text-[var(--zf-muted)] gap-3">
          <div className="w-6 h-6 border-2 border-[var(--zf-hairline)] border-t-[var(--zf-link)] rounded-full animate-spin" />
          <span className="text-sm">Checking migration readiness…</span>
        </div>
      ) : !loadError ? (
        <div className="flex flex-col gap-4">
          {checks.length === 0 ? (
            <div className="bg-[var(--zf-canvas)] rounded-xl p-10 border border-[var(--zf-hairline)] text-center text-[var(--zf-muted)] text-sm">
              No VMs to check.
            </div>
          ) : (
            <>
              <div className={`rounded-xl p-4 border flex items-center justify-between ${hasIssues ? 'bg-amber-50 border-amber-200' : 'bg-emerald-50 border-emerald-200'}`}>
                <div className="flex items-center gap-3">
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center ${hasIssues ? 'bg-amber-100 text-amber-800' : 'bg-emerald-100 text-emerald-700'}`}>
                    {hasIssues ? '⚠' : '✓'}
                  </div>
                  <div>
                    <div className="text-sm font-semibold text-[var(--zf-ink)]">{hasIssues ? 'Issues Found' : 'Ready for Migration'}</div>
                    <div className="text-xs text-[var(--zf-muted)]">
                      {hasIssues
                        ? `${errorCount} failed, ${warningCount} warning${warningCount !== 1 ? 's' : ''}`
                        : `All ${checks.length} checks passed`}
                    </div>
                  </div>
                </div>
              </div>

              <div className="bg-[var(--zf-canvas)] rounded-xl border border-[var(--zf-hairline)] divide-y divide-[var(--zf-hairline)]">
                {checks.map((check, idx) => (
                  <div key={idx} className="flex items-start gap-3 p-4">
                    <StatusIcon status={check.status} />
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium text-[var(--zf-ink)]">{check.check_name}</div>
                      <div className="text-xs text-[var(--zf-muted)] mt-0.5">{check.message}</div>
                      <div className="text-[10px] text-[var(--zf-muted)] mt-1 uppercase tracking-wider">{check.check_type} · {check.severity}</div>
                    </div>
                    <span className={`px-2 py-0.5 rounded-full text-xs font-medium border ${statusBadgeClass(check.status)}`}>{check.status}</span>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
      ) : null}
    </div>
  )
}
