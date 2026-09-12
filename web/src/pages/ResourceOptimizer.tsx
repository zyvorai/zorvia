// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useState, useEffect, useCallback } from 'react'
import { Lightbulb } from 'lucide-react'
import { getOptimizationRecommendations, OptimizationRecommendation } from '../api/optimization'
import ErrorBanner from '../components/ErrorBanner'
import { PageHeader, EmptyState } from '../components/ui'
import { formatUserError } from '../utils/apiError'
import { toastFailure } from '../utils/toastError'
import { hintsForError } from '../utils/daemonHints'
import { useToastContext } from '../contexts/ToastContext'

function priorityBadge(p: string): string {
  switch (p) {
    case 'Critical': return 'text-red-700 bg-red-50 border-red-200'
    case 'High': return 'text-amber-800 bg-amber-50 border-amber-200'
    case 'Medium': return 'text-[var(--zf-link)] bg-blue-50 border-blue-100'
    default: return 'text-[var(--zf-muted)] bg-[var(--zf-canvas)] border-[var(--zf-hairline)]'
  }
}

export default function ResourceOptimizer() {
  const toast = useToastContext()
  const [recommendations, setRecommendations] = useState<OptimizationRecommendation[]>([])
  const [note, setNote] = useState('')
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)

  const fetchRecs = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      const result = await getOptimizationRecommendations()
      setRecommendations(result.recommendations)
      setNote(result.cost_basis.note)
    } catch (err) {
      setLoadError(formatUserError(err))
      toastFailure(toast, 'Failed to load recommendations', err)
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { fetchRecs() }, [fetchRecs])

  return (
    <div className="space-y-6">
      <PageHeader
        title="Resource Optimizer"
        description="Right-sizing and idle-VM recommendations from real VM specs and real usage — potential savings are estimated, not real billing data"
        onRefresh={fetchRecs}
        refreshing={loading}
      />

      {loadError && (
        <ErrorBanner title="Could not load recommendations" headline={loadError} hints={hintsForError(loadError)} onRetry={fetchRecs} />
      )}

      {loading && !loadError ? (
        <div className="flex items-center justify-center h-64 text-[var(--zf-muted)]">
          <div className="animate-spin w-6 h-6 border-2 border-[var(--zf-ink)] border-t-transparent rounded-full mr-3" />
          Analyzing…
        </div>
      ) : !loadError && recommendations.length === 0 ? (
        <EmptyState icon={<Lightbulb className="w-8 h-8" />} title="No recommendations" description="Every VM looks appropriately sized right now." />
      ) : !loadError ? (
        <>
          <p className="text-xs text-[var(--zf-muted)]">{note}</p>
          <div className="bg-[var(--zf-surface)] rounded-xl border border-[var(--zf-hairline)] divide-y divide-[var(--zf-hairline)]/30">
            {recommendations.map(rec => (
              <div key={rec.id} className="px-5 py-4 flex items-start gap-3">
                <span className={`px-2 py-0.5 rounded-full text-xs font-medium border shrink-0 mt-0.5 ${priorityBadge(rec.priority)}`}>{rec.priority}</span>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium text-[var(--zf-ink)]">{rec.vm_name} — {rec.recommendation_type}</div>
                  <div className="text-xs text-[var(--zf-muted)] mt-0.5">{rec.description}</div>
                  <ul className="text-xs text-[var(--zf-muted)] mt-1.5 list-disc list-inside space-y-0.5">
                    {rec.action_items.map((a, i) => <li key={i}>{a}</li>)}
                  </ul>
                </div>
                <div className="text-right shrink-0">
                  <div className="text-sm font-semibold text-emerald-600">${rec.potential_savings.toFixed(0)}/mo</div>
                  <div className="text-[10px] text-[var(--zf-muted)]">est. {rec.savings_percent.toFixed(0)}% savings</div>
                </div>
              </div>
            ))}
          </div>
        </>
      ) : null}
    </div>
  )
}
