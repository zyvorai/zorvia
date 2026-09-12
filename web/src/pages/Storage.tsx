// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useState, useEffect, useCallback } from 'react'
import { HardDrive } from 'lucide-react'
import { listStorageVolumes, StorageVolume } from '../api/storage'
import ErrorBanner from '../components/ErrorBanner'
import { PageHeader, EmptyState } from '../components/ui'
import { formatUserError } from '../utils/apiError'
import { toastFailure } from '../utils/toastError'
import { hintsForError } from '../utils/daemonHints'
import { useToastContext } from '../contexts/ToastContext'

function statusBadge(status: string): string {
  switch (status.toLowerCase()) {
    case 'bound': return 'text-emerald-700 bg-emerald-50 border-emerald-200'
    case 'pending': return 'text-amber-800 bg-amber-50 border-amber-200'
    case 'lost': return 'text-red-700 bg-red-50 border-red-200'
    default: return 'text-[var(--zf-muted)] bg-[var(--zf-canvas)] border-[var(--zf-hairline)]'
  }
}

export default function Storage() {
  const toast = useToastContext()
  const [volumes, setVolumes] = useState<StorageVolume[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)

  const fetchVolumes = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      setVolumes(await listStorageVolumes())
    } catch (err) {
      setLoadError(formatUserError(err))
      toastFailure(toast, 'Failed to load storage volumes', err)
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { fetchVolumes() }, [fetchVolumes])

  return (
    <div className="space-y-6">
      <PageHeader
        title="Storage"
        description="Every PersistentVolumeClaim in the namespace, and which VM (if any) each one is attached to"
        onRefresh={fetchVolumes}
        refreshing={loading}
      />

      {loadError && (
        <ErrorBanner title="Could not load storage volumes" headline={loadError} hints={hintsForError(loadError, 'storage')} onRetry={fetchVolumes} />
      )}

      {loading && !loadError ? (
        <div className="flex items-center justify-center h-64 text-[var(--zf-muted)]">
          <div className="animate-spin w-6 h-6 border-2 border-[var(--zf-ink)] border-t-transparent rounded-full mr-3" />
          Loading volumes…
        </div>
      ) : !loadError && volumes.length === 0 ? (
        <EmptyState icon={<HardDrive className="w-8 h-8" />} title="No volumes found" description="PersistentVolumeClaims created by VM disks will appear here." />
      ) : !loadError ? (
        <div className="bg-[var(--zf-surface)] rounded-xl border border-[var(--zf-hairline)] overflow-hidden">
          <table className="w-full text-sm">
            <thead><tr className="border-b border-[var(--zf-hairline)]">
              <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Name</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Status</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Capacity</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Storage Class</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Attached VMs</th>
            </tr></thead>
            <tbody className="divide-y divide-[var(--zf-hairline)]/30">
              {volumes.map(v => (
                <tr key={v.name} className="hover:bg-black/[0.04] transition-colors">
                  <td className="px-5 py-3 font-mono text-xs text-[var(--zf-ink)]">{v.name}</td>
                  <td className="px-5 py-3"><span className={`px-2.5 py-0.5 rounded-full text-xs font-medium border ${statusBadge(v.status)}`}>{v.status}</span></td>
                  <td className="px-5 py-3 text-[var(--zf-muted)]">{v.capacity || '—'}</td>
                  <td className="px-5 py-3 text-[var(--zf-muted)]">{v.storage_class || '—'}</td>
                  <td className="px-5 py-3 text-[var(--zf-muted)]">{v.attached_vms.length > 0 ? v.attached_vms.join(', ') : '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </div>
  )
}
