// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useState, useEffect, useCallback } from 'react'
import { ShieldAlert, Loader2 } from 'lucide-react'
import { listVMs, getHaPolicy, setHaPolicy, VM, EvictionStrategy } from '../api/vm'
import ErrorBanner from '../components/ErrorBanner'
import { PageHeader } from '../components/ui'
import { formatUserError } from '../utils/apiError'
import { toastFailure } from '../utils/toastError'
import { hintsForError } from '../utils/daemonHints'
import { useToastContext } from '../contexts/ToastContext'

const STRATEGIES: { value: EvictionStrategy | null; label: string; description: string }[] = [
  { value: null, label: 'Cluster default', description: 'No per-VM override; whatever the cluster\'s KubeVirt configuration defaults to.' },
  { value: 'LiveMigrate', label: 'Live Migrate', description: 'When the node drains, KubeVirt live-migrates the VM elsewhere with no downtime.' },
  { value: 'LiveMigrateIfPossible', label: 'Live Migrate if Possible', description: 'Attempts live migration; falls back to a normal eviction if the VM can\'t be migrated.' },
  { value: 'External', label: 'External', description: 'An external controller handles eviction (advanced -- only if something else manages this).' },
  { value: 'None', label: 'None', description: 'The VM is evicted normally (stopped) when its node drains -- expect downtime.' },
]

interface RowState {
  vm: VM
  strategy: EvictionStrategy | null | undefined // undefined = not loaded yet
  saving: boolean
}

export default function HaPolicy() {
  const toast = useToastContext()
  const [rows, setRows] = useState<RowState[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)

  const fetchAll = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      const vms = await listVMs()
      const initial = vms.map(vm => ({ vm, strategy: undefined, saving: false }))
      setRows(initial)
      const policies = await Promise.all(vms.map(vm => getHaPolicy(vm.name).catch(() => null)))
      setRows(vms.map((vm, i) => ({ vm, strategy: policies[i]?.eviction_strategy ?? null, saving: false })))
    } catch (err) {
      setLoadError(formatUserError(err))
      toastFailure(toast, 'Failed to load HA policies', err)
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { fetchAll() }, [fetchAll])

  const handleChange = async (vmName: string, strategy: EvictionStrategy | null) => {
    setRows(prev => prev.map(r => r.vm.name === vmName ? { ...r, saving: true } : r))
    try {
      const result = await setHaPolicy(vmName, strategy)
      setRows(prev => prev.map(r => r.vm.name === vmName ? { ...r, strategy: result.eviction_strategy, saving: false } : r))
      toast.success(`Updated HA policy for "${vmName}"`)
    } catch (err) {
      toastFailure(toast, 'Failed to update HA policy', err)
      setRows(prev => prev.map(r => r.vm.name === vmName ? { ...r, saving: false } : r))
    }
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="HA Policy"
        description="Per-VM eviction strategy -- what happens when a VM's node drains or is put into maintenance. KubeVirt has no vSphere-style lockstep fault tolerance; this is the real equivalent."
        onRefresh={fetchAll}
        refreshing={loading}
      />

      {loadError && (
        <ErrorBanner title="Could not load HA policies" headline={loadError} hints={hintsForError(loadError, 'vm')} onRetry={fetchAll} />
      )}

      {loading && !loadError ? (
        <div className="flex items-center justify-center h-64 text-[var(--zf-muted)]">
          <div className="animate-spin w-6 h-6 border-2 border-[var(--zf-ink)] border-t-transparent rounded-full mr-3" />
          Loading…
        </div>
      ) : !loadError ? (
        rows.length === 0 ? (
          <div className="bg-[var(--zf-surface)] rounded-xl p-10 border border-[var(--zf-hairline)] text-center text-[var(--zf-muted)]">
            <ShieldAlert className="w-10 h-10 mx-auto mb-3 opacity-50" />
            <p className="text-sm">No VMs to configure</p>
          </div>
        ) : (
          <div className="bg-[var(--zf-surface)] rounded-xl border border-[var(--zf-hairline)] overflow-hidden">
            <table className="w-full text-sm">
              <thead><tr className="border-b border-[var(--zf-hairline)]">
                <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">VM</th>
                <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Eviction Strategy</th>
                <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">What it means</th>
              </tr></thead>
              <tbody className="divide-y divide-[var(--zf-hairline)]/30">
                {rows.map(row => (
                  <tr key={row.vm.name} className="hover:bg-black/[0.04] transition-colors">
                    <td className="px-5 py-3 font-medium text-[var(--zf-ink)]">{row.vm.name}</td>
                    <td className="px-5 py-3">
                      <div className="flex items-center gap-2">
                        <select
                          value={row.strategy ?? ''}
                          onChange={(e) => handleChange(row.vm.name, (e.target.value || null) as EvictionStrategy | null)}
                          disabled={row.saving || row.strategy === undefined}
                          className="input-field text-sm"
                        >
                          {STRATEGIES.map(s => <option key={s.label} value={s.value ?? ''}>{s.label}</option>)}
                        </select>
                        {row.saving && <Loader2 className="w-4 h-4 animate-spin text-[var(--zf-muted)]" />}
                      </div>
                    </td>
                    <td className="px-5 py-3 text-xs text-[var(--zf-muted)] max-w-md">
                      {STRATEGIES.find(s => (s.value ?? null) === (row.strategy ?? null))?.description}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )
      ) : null}
    </div>
  )
}
