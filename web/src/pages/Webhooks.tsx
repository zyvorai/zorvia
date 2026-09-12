// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useState, useEffect, useCallback } from 'react'
import { Webhook as WebhookIcon, Plus, Trash2, Send, Loader2 } from 'lucide-react'
import { listWebhooks, createWebhook, deleteWebhook, testWebhook, Webhook } from '../api/webhooks'
import { useToastContext } from '../contexts/ToastContext'
import { useConfirm } from '../hooks/useConfirm'
import ConfirmDialog from '../components/ConfirmDialog'
import ErrorBanner from '../components/ErrorBanner'
import { PageHeader, EmptyState } from '../components/ui'
import { formatUserError } from '../utils/apiError'
import { toastFailure } from '../utils/toastError'
import { hintsForError } from '../utils/daemonHints'

const EVENTS = [
  'vm.created', 'vm.deleted', 'vm.started', 'vm.stopped', 'vm.restarted', 'vm.failed',
  'snapshot.created', 'snapshot.restored', 'backup.completed', 'backup.failed',
  'migration.started', 'migration.completed',
]

export default function Webhooks() {
  const toast = useToastContext()
  const { confirmState, confirm, cancel } = useConfirm()
  const [webhooks, setWebhooks] = useState<Webhook[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const [name, setName] = useState('')
  const [url, setUrl] = useState('')
  const [secret, setSecret] = useState('')
  const [selectedEvents, setSelectedEvents] = useState<Set<string>>(new Set())
  const [creating, setCreating] = useState(false)
  const [createError, setCreateError] = useState('')
  const [busyId, setBusyId] = useState<string | null>(null)

  const fetchAll = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      setWebhooks(await listWebhooks())
    } catch (err) {
      setLoadError(formatUserError(err))
      toastFailure(toast, 'Failed to load webhooks', err)
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { fetchAll() }, [fetchAll])

  const toggleEvent = (event: string) => setSelectedEvents(prev => {
    const next = new Set(prev)
    if (next.has(event)) next.delete(event); else next.add(event)
    return next
  })

  const handleCreate = async () => {
    if (!name.trim()) { setCreateError('Name is required'); return }
    if (!url.trim().startsWith('https://')) { setCreateError('URL must use HTTPS'); return }
    if (selectedEvents.size === 0) { setCreateError('Select at least one event'); return }
    setCreating(true)
    setCreateError('')
    try {
      await createWebhook({ name: name.trim(), url: url.trim(), events: Array.from(selectedEvents), secret: secret.trim() || undefined })
      toast.success(`Webhook "${name}" created`)
      setName(''); setUrl(''); setSecret(''); setSelectedEvents(new Set()); setShowCreate(false)
      fetchAll()
    } catch (err) {
      setCreateError(formatUserError(err))
      toastFailure(toast, 'Failed to create webhook', err)
    } finally {
      setCreating(false)
    }
  }

  const handleTest = async (webhook: Webhook) => {
    setBusyId(webhook.id)
    try {
      const result = await testWebhook(webhook.id)
      if (result.success) toast.success('Test delivery succeeded')
      else toast.error('Test delivery failed — check the endpoint')
      fetchAll()
    } catch (err) {
      toastFailure(toast, 'Failed to test webhook', err)
    } finally {
      setBusyId(null)
    }
  }

  const handleDelete = async (webhook: Webhook) => {
    if (!await confirm('Delete Webhook', `Delete webhook "${webhook.name}"?`, { variant: 'danger', confirmLabel: 'Delete' })) return
    setBusyId(webhook.id)
    try {
      await deleteWebhook(webhook.id)
      setWebhooks(prev => prev.filter(w => w.id !== webhook.id))
      toast.success('Webhook deleted')
    } catch (err) {
      toastFailure(toast, 'Failed to delete webhook', err)
    } finally {
      setBusyId(null)
    }
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Webhooks"
        description="Real HTTPS delivery on VM lifecycle events, with retry/backoff — HTTPS-only, private/internal URLs are rejected"
        onRefresh={fetchAll}
        refreshing={loading}
        primaryAction={
          <button type="button" onClick={() => setShowCreate(!showCreate)} className="zf-btn zf-btn-primary zf-btn-sm">
            <Plus className="w-4 h-4" /> New Webhook
          </button>
        }
      />

      {loadError && (
        <ErrorBanner title="Could not load webhooks" headline={loadError} hints={hintsForError(loadError)} onRetry={fetchAll} />
      )}

      {loading && !loadError ? (
        <div className="flex items-center justify-center h-64 text-[var(--zf-muted)]">
          <div className="animate-spin w-6 h-6 border-2 border-[var(--zf-ink)] border-t-transparent rounded-full mr-3" />
          Loading webhooks…
        </div>
      ) : !loadError ? (
        <>
          {showCreate && (
            <div className="bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-xl p-5 space-y-4">
              <h3 className="text-sm font-semibold text-[var(--zf-ink)]">New Webhook</h3>
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div>
                  <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Name</label>
                  <input type="text" value={name} onChange={(e) => setName(e.target.value)} placeholder="slack-notifier" className="input-field text-sm w-full" />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">URL (HTTPS only)</label>
                  <input type="text" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://example.com/hook" className="input-field text-sm w-full" />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Secret (optional)</label>
                  <input type="password" value={secret} onChange={(e) => setSecret(e.target.value)} placeholder="sent as X-Zorvia-Webhook-Secret" className="input-field text-sm w-full" />
                </div>
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--zf-muted)] mb-1.5">Events</label>
                <div className="flex flex-wrap gap-2">
                  {EVENTS.map(event => (
                    <button key={event} type="button" onClick={() => toggleEvent(event)}
                      className={`px-2.5 py-1 text-xs font-medium rounded-lg border transition-colors font-mono ${selectedEvents.has(event) ? 'bg-[var(--zf-link)] text-white border-[var(--zf-link)]' : 'text-[var(--zf-muted)] bg-white border-[var(--zf-hairline)] hover:border-[var(--zf-ink)]'}`}>
                      {event}
                    </button>
                  ))}
                </div>
              </div>
              {createError && <p className="text-sm text-red-600">{createError}</p>}
              <div className="flex gap-2">
                <button onClick={handleCreate} disabled={creating} className="zf-btn zf-btn-primary zf-btn-sm">
                  {creating ? <Loader2 className="w-4 h-4 animate-spin" /> : <Plus className="w-4 h-4" />}
                  {creating ? 'Creating...' : 'Create'}
                </button>
                <button onClick={() => { setShowCreate(false); setCreateError('') }} className="zf-btn zf-btn-ghost zf-btn-sm">Cancel</button>
              </div>
            </div>
          )}

          {webhooks.length === 0 ? (
            <EmptyState icon={<WebhookIcon className="w-8 h-8" />} title="No webhooks configured" description="Add one to get notified on VM lifecycle events." />
          ) : (
            <div className="bg-[var(--zf-surface)] rounded-xl border border-[var(--zf-hairline)] overflow-hidden">
              <table className="w-full text-sm">
                <thead><tr className="border-b border-[var(--zf-hairline)]">
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Name</th>
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">URL</th>
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Events</th>
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Success Rate</th>
                  <th className="text-left px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Last Triggered</th>
                  <th className="text-right px-5 py-3 text-xs font-medium text-[var(--zf-muted)] uppercase tracking-wider">Actions</th>
                </tr></thead>
                <tbody className="divide-y divide-[var(--zf-hairline)]/30">
                  {webhooks.map(w => (
                    <tr key={w.id} className="hover:bg-black/[0.04] transition-colors">
                      <td className="px-5 py-3 font-medium text-[var(--zf-ink)]">{w.name}</td>
                      <td className="px-5 py-3 text-xs text-[var(--zf-muted)] font-mono truncate max-w-[220px]" title={w.url}>{w.url}</td>
                      <td className="px-5 py-3 text-xs text-[var(--zf-muted)]">{w.events.length} event{w.events.length !== 1 ? 's' : ''}</td>
                      <td className="px-5 py-3 text-xs text-[var(--zf-muted)]">{w.delivery_count > 0 ? `${w.success_rate.toFixed(0)}%` : '—'}</td>
                      <td className="px-5 py-3 text-xs text-[var(--zf-muted)]">{w.last_triggered ? new Date(w.last_triggered).toLocaleString() : 'Never'}</td>
                      <td className="px-5 py-3 text-right">
                        <div className="flex items-center justify-end gap-1">
                          <button onClick={() => handleTest(w)} disabled={busyId === w.id} className="p-1.5 text-[var(--zf-muted)] hover:text-[var(--zf-ink)] hover:bg-black/[0.04] rounded-lg transition-colors disabled:opacity-50" title="Send test delivery">
                            {busyId === w.id ? <Loader2 className="w-4 h-4 animate-spin" /> : <Send className="w-4 h-4" />}
                          </button>
                          <button onClick={() => handleDelete(w)} disabled={busyId === w.id} className="p-1.5 text-[var(--zf-muted)] hover:text-red-600 hover:bg-red-500/10 rounded-lg transition-colors disabled:opacity-50" title="Delete">
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
