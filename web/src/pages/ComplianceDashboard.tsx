// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useState, useEffect, useCallback } from 'react'
import { Shield, CheckCircle, AlertTriangle, XCircle, RefreshCw } from 'lucide-react'
import { getComplianceSummary, rescan, ComplianceSummary } from '../api/compliance'
import { useToastContext } from '../contexts/ToastContext'
import ErrorBanner from '../components/ErrorBanner'
import { PageHeader } from '../components/ui'
import { formatUserError } from '../utils/apiError'
import { toastFailure } from '../utils/toastError'
import { hintsForError } from '../utils/daemonHints'

function scoreColor(score: number): string {
  if (score >= 90) return 'text-emerald-600'
  if (score >= 70) return 'text-amber-600'
  return 'text-red-600'
}

function scoreBorder(score: number): string {
  if (score >= 90) return 'border-emerald-500'
  if (score >= 70) return 'border-amber-500'
  return 'border-red-500'
}

function statusIcon(status: string) {
  switch (status) {
    case 'pass': return <CheckCircle className="w-5 h-5 text-emerald-600" />
    case 'warning': return <AlertTriangle className="w-5 h-5 text-amber-600" />
    default: return <XCircle className="w-5 h-5 text-red-600" />
  }
}

function statusBadge(status: string): string {
  switch (status) {
    case 'pass': return 'text-emerald-700 bg-emerald-50 border-emerald-200'
    case 'warning': return 'text-amber-800 bg-amber-50 border-amber-200'
    default: return 'text-red-700 bg-red-50 border-red-200'
  }
}

export default function ComplianceDashboard() {
  const toast = useToastContext()
  const [data, setData] = useState<ComplianceSummary | null>(null)
  const [loading, setLoading] = useState(true)
  const [scanning, setScanning] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [selectedCategory, setSelectedCategory] = useState('all')

  const fetchCompliance = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      setData(await getComplianceSummary())
    } catch (err) {
      setLoadError(formatUserError(err))
      toastFailure(toast, 'Failed to load compliance data', err)
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { fetchCompliance() }, [fetchCompliance])

  const handleRescan = async () => {
    setScanning(true)
    try {
      setData(await rescan())
      toast.success('Compliance scan complete')
    } catch (err) {
      toastFailure(toast, 'Rescan failed', err)
    } finally {
      setScanning(false)
    }
  }

  const filteredChecks = data?.checks.filter(c => selectedCategory === 'all' || c.category === selectedCategory) ?? []

  return (
    <div className="space-y-6">
      <PageHeader
        title="Compliance"
        description="Live PCI-DSS / HIPAA / SOC 2 checks against real VM specs — recomputed on every load"
        onRefresh={fetchCompliance}
        refreshing={loading}
        primaryAction={
          <button type="button" onClick={handleRescan} disabled={scanning} className="zf-btn zf-btn-primary zf-btn-sm">
            <RefreshCw className={`w-4 h-4 ${scanning ? 'animate-spin' : ''}`} /> {scanning ? 'Scanning…' : 'Rescan'}
          </button>
        }
      />

      {loadError && (
        <ErrorBanner title="Could not load compliance data" headline={loadError} hints={hintsForError(loadError)} onRetry={fetchCompliance} />
      )}

      {loading && !data && !loadError ? (
        <div className="flex items-center justify-center h-64 text-[var(--zf-muted)]">
          <div className="animate-spin w-6 h-6 border-2 border-[var(--zf-ink)] border-t-transparent rounded-full mr-3" />
          Running compliance checks…
        </div>
      ) : !loadError && data ? (
        <>
          <div className="grid grid-cols-1 lg:grid-cols-4 gap-3">
            <div className="bg-[var(--zf-surface)] rounded-xl border border-[var(--zf-hairline)] p-6 flex flex-col items-center justify-center">
              <div className={`w-24 h-24 rounded-full flex items-center justify-center bg-white border-4 ${scoreBorder(data.score)}`}>
                <span className={`text-3xl font-bold ${scoreColor(data.score)}`}>{data.score}</span>
              </div>
              <span className="text-xs text-[var(--zf-muted)] mt-2">Compliance Score</span>
            </div>
            <div className="bg-[var(--zf-surface)] rounded-xl border border-[var(--zf-hairline)] px-4 py-3 flex flex-col justify-center">
              <div className="text-2xl font-bold text-emerald-600">{data.passed}</div>
              <div className="text-xs text-[var(--zf-muted)] mt-1">Passed</div>
            </div>
            <div className="bg-[var(--zf-surface)] rounded-xl border border-[var(--zf-hairline)] px-4 py-3 flex flex-col justify-center">
              <div className="text-2xl font-bold text-amber-600">{data.warnings}</div>
              <div className="text-xs text-[var(--zf-muted)] mt-1">Warnings</div>
            </div>
            <div className="bg-[var(--zf-surface)] rounded-xl border border-[var(--zf-hairline)] px-4 py-3 flex flex-col justify-center">
              <div className="text-2xl font-bold text-red-600">{data.failed}</div>
              <div className="text-xs text-[var(--zf-muted)] mt-1">Failed</div>
            </div>
          </div>

          <div className="flex flex-wrap gap-2">
            <button onClick={() => setSelectedCategory('all')} className={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${selectedCategory === 'all' ? 'bg-[var(--zf-link)] text-white border-[var(--zf-link)]' : 'text-[var(--zf-muted)] bg-white border-[var(--zf-hairline)] hover:border-[var(--zf-ink)]'}`}>All ({data.checks.length})</button>
            {data.categories.map(cat => (
              <button key={cat} onClick={() => setSelectedCategory(cat)} className={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${selectedCategory === cat ? 'bg-[var(--zf-link)] text-white border-[var(--zf-link)]' : 'text-[var(--zf-muted)] bg-white border-[var(--zf-hairline)] hover:border-[var(--zf-ink)]'}`}>{cat}</button>
            ))}
          </div>

          <div className="bg-[var(--zf-surface)] rounded-xl border border-[var(--zf-hairline)] overflow-hidden">
            {filteredChecks.length === 0 ? (
              <div className="p-10 text-center text-[var(--zf-muted)] text-sm">
                <Shield className="w-10 h-10 mx-auto mb-3 opacity-50" />
                No checks in this category
              </div>
            ) : (
              <div className="divide-y divide-[var(--zf-hairline)]/30">
                {filteredChecks.map(check => (
                  <div key={check.id} className="px-5 py-3 flex items-start gap-3">
                    {statusIcon(check.status)}
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium text-[var(--zf-ink)]">{check.name}</div>
                      <div className="text-xs text-[var(--zf-muted)] mt-0.5">{check.description}</div>
                      {check.remediation && <div className="text-xs text-[var(--zf-muted)] mt-1 font-mono">{check.remediation}</div>}
                    </div>
                    <span className={`px-2 py-0.5 rounded-full text-xs font-medium border shrink-0 ${statusBadge(check.status)}`}>{check.category}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </>
      ) : null}
    </div>
  )
}
