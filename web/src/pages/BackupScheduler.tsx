// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useState, useEffect, useCallback } from 'react'
import { Plus, Trash2, Power, PowerOff, Clock, Loader2 } from 'lucide-react'
import {
  listBackupPolicies,
  createBackupPolicy,
  deleteBackupPolicy,
  enableBackupPolicy,
  disableBackupPolicy,
  BackupPolicy,
} from '../api/backup'
import { listVMs, VM } from '../api/vm'
import { useToastContext } from '../contexts/ToastContext'
import { useConfirm } from '../hooks/useConfirm'
import ConfirmDialog from '../components/ConfirmDialog'
import ErrorBanner from '../components/ErrorBanner'
import { PageHeader, EmptyState } from '../components/ui'
import { formatUserError } from '../utils/apiError'
import { toastFailure } from '../utils/toastError'
import { hintsForError } from '../utils/daemonHints'

const WEEKDAYS = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday']

export default function BackupScheduler() {
  const toast = useToastContext()
  const { confirmState, confirm, cancel } = useConfirm()
  const [policies, setPolicies] = useState<BackupPolicy[]>([])
  const [vms, setVms] = useState<VM[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const [busyName, setBusyName] = useState<string | null>(null)

  const [name, setName] = useState('')
  const [scheduleType, setScheduleType] = useState<'hourly' | 'daily' | 'weekly' | 'monthly'>('daily')
  const [hour, setHour] = useState('2')
  const [minute, setMinute] = useState('0')
  const [weekday, setWeekday] = useState('sunday')
  const [dayOfMonth, setDayOfMonth] = useState('1')
  const [retentionDays, setRetentionDays] = useState('30')
  const [selectedVms, setSelectedVms] = useState<Set<string>>(new Set())
  const [creating, setCreating] = useState(false)
  const [createError, setCreateError] = useState('')

  const fetchAll = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      const [policyList, vmList] = await Promise.all([listBackupPolicies(), listVMs()])
      setPolicies(policyList)
      setVms(vmList)
    } catch (err) {
      setLoadError(formatUserError(err))
      toastFailure(toast, 'Failed to load backup schedules', err)
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { fetchAll() }, [fetchAll])

  const toggleVm = (name: string) => setSelectedVms(prev => {
    const next = new Set(prev)
    if (next.has(name)) next.delete(name); else next.add(name)
    return next
  })

  const handleCreate = async () => {
    if (!name.trim()) { setCreateError('Name is required'); return }
    if (scheduleType === 'weekly' && !weekday) { setCreateError('Pick a weekday'); return }
    setCreating(true)
    setCreateError('')
    try {
      await createBackupPolicy({
        name: name.trim(),
        schedule_type: scheduleType,
        hour: parseInt(hour, 10) || 0,
        minute: parseInt(minute, 10) || 0,
        weekday: scheduleType === 'weekly' ? weekday : undefined,
        day_of_month: scheduleType === 'monthly' ? parseInt(dayOfMonth, 10) || 1 : undefined,
        vm_names: selectedVms.size > 0 ? Array.from(selectedVms) : undefined,
        retention_days: parseInt(retentionDays, 10) || undefined,
      })
      toast.success(`Schedule "${name}" created`)
      setName(''); setSelectedVms(new Set()); setShowCreate(false)
      fetchAll()
    } catch (err) {
      setCreateError(formatUserError(err))
      toastFailure(toast, 'Failed to create schedule', err)
    } finally {
      setCreating(false)
    }
  }

  const handleToggle = async (policy: BackupPolicy) => {
    setBusyName(policy.name)
    try {
      if (policy.enabled) await disableBackupPolicy(policy.name)
      else await enableBackupPolicy(policy.name)
      fetchAll()
    } catch (err) {
      toastFailure(toast, 'Failed to update schedule', err)
    } finally {
      setBusyName(null)
    }
  }

  const handleDelete = async (policy: BackupPolicy) => {
    if (!await confirm('Delete Schedule', `Delete backup schedule "${policy.name}"?`, { variant: 'danger', confirmLabel: 'Delete' })) return
    setBusyName(policy.name)
    try {
      await deleteBackupPolicy(policy.name)
      setPolicies(prev => prev.filter(p => p.name !== policy.name))
      toast.success('Schedule deleted')
    } catch (err) {
      toastFailure(toast, 'Failed to delete schedule', err)
    } finally {
      setBusyName(null)
    }
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Backup Scheduler"
        description="Recurring backups, checked and run by the server every minute"
        onRefresh={fetchAll}
        refreshing={loading}
        primaryAction={
          <button type="button" onClick={() => setShowCreate(!showCreate)} className="zf-btn zf-btn-primary zf-btn-sm">
            <Plus className="w-4 h-4" /> New Schedule
          </button>
        }
      />

      {loadError && (
        <ErrorBanner title="Could not load schedules" headline={loadError} hints={hintsForError(loadError)} onRetry={fetchAll} />
      )}

      {loading && !loadError ? (
        <div className="flex items-center justify-center h-64 text-[var(--zf-muted)]">
          <div className="animate-spin w-6 h-6 border-2 border-[var(--zf-ink)] border-t-transparent rounded-full mr-3" />
          Loading schedules…
        </div>
      ) : !loadError ? (
        <>
          {showCreate && (
            <div className="bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-xl p-5 space-y-4">
              <h3 className="text-sm font-semibold text-[var(--zf-ink)]">New Schedule</h3>
              <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                <div>
                  <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Name</label>
                  <input type="text" value={name} onChange={(e) => setName(e.target.value)} placeholder="nightly-backups" className="input-field text-sm w-full" />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Frequency</label>
                  <select value={scheduleType} onChange={(e) => setScheduleType(e.target.value as typeof scheduleType)} className="input-field text-sm w-full">
                    <option value="hourly">Hourly</option>
                    <option value="daily">Daily</option>
                    <option value="weekly">Weekly</option>
                    <option value="monthly">Monthly</option>
                  </select>
                </div>
                {scheduleType !== 'hourly' && (
                  <div>
                    <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Time (UTC)</label>
                    <div className="flex gap-2">
                      <input type="number" min={0} max={23} value={hour} onChange={(e) => setHour(e.target.value)} className="input-field text-sm w-full" placeholder="HH" />
                      <input type="number" min={0} max={59} value={minute} onChange={(e) => setMinute(e.target.value)} className="input-field text-sm w-full" placeholder="MM" />
                    </div>
                  </div>
                )}
                {scheduleType === 'hourly' && (
                  <div>
                    <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Minute</label>
                    <input type="number" min={0} max={59} value={minute} onChange={(e) => setMinute(e.target.value)} className="input-field text-sm w-full" />
                  </div>
                )}
                {scheduleType === 'weekly' && (
                  <div>
                    <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Weekday</label>
                    <select value={weekday} onChange={(e) => setWeekday(e.target.value)} className="input-field text-sm w-full capitalize">
                      {WEEKDAYS.map(d => <option key={d} value={d}>{d}</option>)}
                    </select>
                  </div>
                )}
                {scheduleType === 'monthly' && (
                  <div>
                    <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Day of month</label>
                    <input type="number" min={1} max={31} value={dayOfMonth} onChange={(e) => setDayOfMonth(e.target.value)} className="input-field text-sm w-full" />
                  </div>
                )}
                <div>
                  <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Retention (days)</label>
                  <input type="number" min={1} value={retentionDays} onChange={(e) => setRetentionDays(e.target.value)} className="input-field text-sm w-full" />
                </div>
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">VMs (none selected = every VM in the namespace)</label>
                <div className="flex flex-wrap gap-2">
                  {vms.map(vm => (
                    <button key={vm.name} type="button" onClick={() => toggleVm(vm.name)}
                      className={`px-2.5 py-1 text-xs font-medium rounded-lg border transition-colors ${selectedVms.has(vm.name) ? 'bg-[var(--zf-link)] text-white border-[var(--zf-link)]' : 'text-[var(--zf-muted)] bg-white border-[var(--zf-hairline)] hover:border-[var(--zf-ink)]'}`}>
                      {vm.name}
                    </button>
                  ))}
                </div>
              </div>
              {createError && <p className="text-sm text-red-600">{createError}</p>}
              <div className="flex gap-2">
                <button onClick={handleCreate} disabled={creating} className="zf-btn zf-btn-primary zf-btn-sm">
                  {creating ? <Loader2 className="w-4 h-4 animate-spin" /> : <Plus className="w-4 h-4" />}
                  {creating ? 'Creating...' : 'Create Schedule'}
                </button>
                <button onClick={() => { setShowCreate(false); setCreateError('') }} className="zf-btn zf-btn-ghost zf-btn-sm">Cancel</button>
              </div>
            </div>
          )}

          {policies.length === 0 ? (
            <EmptyState icon={<Clock className="w-8 h-8" />} title="No backup schedules" description="Create a schedule to back up VMs automatically." />
          ) : (
            <div className="bg-[var(--zf-surface)] rounded-xl border border-[var(--zf-hairline)] overflow-hidden">
              <table className="w-full text-sm">
                <thead><tr className="border-b border-[var(--zf-hairline)]">
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Name</th>
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Frequency</th>
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">VMs</th>
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Last Run</th>
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Next Run</th>
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Status</th>
                  <th className="text-right px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Actions</th>
                </tr></thead>
                <tbody className="divide-y divide-[var(--zf-hairline)]/30">
                  {policies.map(p => (
                    <tr key={p.name} className="hover:bg-black/[0.04] transition-colors">
                      <td className="px-5 py-3 font-medium text-[var(--zf-ink)]">{p.name}</td>
                      <td className="px-5 py-3 text-[var(--zf-muted)] capitalize">{p.schedule_type}</td>
                      <td className="px-5 py-3 text-[var(--zf-muted)] text-xs">{p.vm_names.length > 0 ? p.vm_names.join(', ') : 'All VMs'}</td>
                      <td className="px-5 py-3 text-xs text-[var(--zf-muted)]">{p.last_run ? new Date(p.last_run).toLocaleString() : 'Never'}</td>
                      <td className="px-5 py-3 text-xs text-[var(--zf-muted)]">{p.next_run ? new Date(p.next_run).toLocaleString() : '—'}</td>
                      <td className="px-5 py-3">
                        <span className={`px-2.5 py-0.5 rounded-full text-xs font-medium border ${p.enabled ? 'text-emerald-700 bg-emerald-50 border-emerald-200' : 'text-[var(--zf-muted)] bg-[var(--zf-canvas)] border-[var(--zf-hairline)]'}`}>
                          {p.enabled ? 'Enabled' : 'Disabled'}
                        </span>
                      </td>
                      <td className="px-5 py-3 text-right">
                        <div className="flex items-center justify-end gap-1">
                          <button onClick={() => handleToggle(p)} disabled={busyName === p.name} className="p-1.5 text-[var(--zf-muted)] hover:text-[var(--zf-ink)] hover:bg-black/[0.04] rounded-lg transition-colors disabled:opacity-50" title={p.enabled ? 'Disable' : 'Enable'}>
                            {p.enabled ? <PowerOff className="w-4 h-4" /> : <Power className="w-4 h-4" />}
                          </button>
                          <button onClick={() => handleDelete(p)} disabled={busyName === p.name} className="p-1.5 text-[var(--zf-muted)] hover:text-red-600 hover:bg-red-500/10 rounded-lg transition-colors disabled:opacity-50" title="Delete">
                            <Trash2 className="w-4 h-4" />
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      ) : null}

      {confirmState && (
        <ConfirmDialog
          title={confirmState.title}
          message={confirmState.message}
          confirmLabel={confirmState.confirmLabel ?? 'Delete'}
          variant={confirmState.variant ?? 'danger'}
          onConfirm={confirmState.onConfirm}
          onCancel={cancel}
        />
      )}
    </div>
  )
}
