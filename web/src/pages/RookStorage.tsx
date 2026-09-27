// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from 'react'
import { Database, HardDrive, FolderTree, Archive, Plus, Trash2, Rocket, Layers } from 'lucide-react'
import {
  bootstrapRook,
  getClusterStatus,
  createCluster,
  listPools,
  createPool,
  deletePool,
  listFilesystems,
  createFilesystem,
  deleteFilesystem,
  listObjectStores,
  createObjectStore,
  deleteObjectStore,
  createStorageClass,
  CephHealthSummary,
  RookPool,
  RookFilesystem,
  RookObjectStore,
} from '../api/rook'
import {
  getAtlasStatus,
  listAtlasBackends,
  listAtlasClusters,
  listAtlasPools,
  listAtlasVolumes,
  createAtlasVolume,
  expandAtlasVolume,
  deleteAtlasVolume,
  listAtlasJobs,
  cancelAtlasJob,
  pollAtlasJob,
  isAtlasJobTerminal,
  atlasVolumeOwnerForVm,
  AtlasStatus,
  AtlasBackend,
  AtlasCluster,
  AtlasPool,
  AtlasVolume,
  AtlasJob,
} from '../api/atlas'
import { useToastContext } from '../contexts/ToastContext'
import { toastFailure } from '../utils/toastError'
import { formatUserError } from '../utils/apiError'
import { formatBytes } from '../utils/format'
import ErrorBanner from '../components/ErrorBanner'
import { PageHeader } from '../components/ui/PageHeader'
import { useConfirm } from '../hooks/useConfirm'
import ConfirmDialog from '../components/ConfirmDialog'

const DEFAULT_NAMESPACE = 'rook-ceph'

const HEALTH_STYLES: Record<string, string> = {
  NotProvisioned: 'text-[var(--zf-muted)] bg-[var(--zf-canvas)] border-[var(--zf-hairline)]',
  Provisioning: 'text-[var(--zf-warning)] bg-[var(--zf-warning)]/10 border-[var(--zf-warning)]/25',
  Healthy: 'text-[var(--zf-success)] bg-[var(--zf-success)]/10 border-[var(--zf-success)]/25',
  Warning: 'text-[var(--zf-warning)] bg-[var(--zf-warning)]/10 border-[var(--zf-warning)]/25',
  Error: 'text-[var(--zf-danger)] bg-[var(--zf-danger)]/10 border-[var(--zf-danger)]/25',
  Unknown: 'text-[var(--zf-muted)] bg-[var(--zf-canvas)] border-[var(--zf-hairline)]',
}

export default function RookStorage() {
  const toast = useToastContext()
  const { confirmState, confirm, cancel } = useConfirm()
  const [namespace, setNamespace] = useState(DEFAULT_NAMESPACE)
  const [status, setStatus] = useState<CephHealthSummary | null>(null)
  const [pools, setPools] = useState<RookPool[]>([])
  const [filesystems, setFilesystems] = useState<RookFilesystem[]>([])
  const [objectStores, setObjectStores] = useState<RookObjectStore[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)

  const loadAll = async (silent = false) => {
    if (!silent) setLoadError(null)
    try {
      const [s, p, f, o] = await Promise.all([
        getClusterStatus(namespace),
        listPools(namespace).catch(() => []),
        listFilesystems(namespace).catch(() => []),
        listObjectStores(namespace).catch(() => []),
      ])
      setStatus(s)
      setPools(p)
      setFilesystems(f)
      setObjectStores(o)
      setLoadError(null)
    } catch (err) {
      const msg = formatUserError(err)
      setLoadError(msg)
      if (!silent) toastFailure(toast, 'Failed to load Rook-Ceph status', err)
    } finally {
      if (!silent) setLoading(false)
    }
  }

  useEffect(() => {
    setLoading(true)
    void loadAll(false)
    const interval = setInterval(() => void loadAll(true), 10000)
    return () => clearInterval(interval)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [namespace])

  const run = async (key: string, fn: () => Promise<unknown>, success: string) => {
    setBusy(key)
    try {
      await fn()
      toast.success(success)
      await loadAll(true)
    } catch (e) {
      toastFailure(toast, 'Action failed', e)
    } finally {
      setBusy(null)
    }
  }

  const runDestructive = async (key: string, resourceLabel: string, fn: () => Promise<unknown>, success: string) => {
    if (!(await confirm(`Delete ${resourceLabel}`, `Delete ${resourceLabel}? Any data on it is destroyed.`, { variant: 'danger', confirmLabel: 'Delete' }))) {
      return
    }
    await run(key, fn, success)
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-[var(--zf-ink)]"></div>
      </div>
    )
  }

  const provisioned = status?.state !== 'NotProvisioned'

  return (
    <div className="space-y-6">
      {loadError && (
        <ErrorBanner title="Could not load Rook-Ceph status" headline={loadError} onRetry={() => void loadAll()} />
      )}
      <PageHeader
        title="Distributed Storage"
        description="Rook-Ceph cluster, block pools, filesystems, and object stores"
        icon={Database}
        onRefresh={() => void loadAll()}
      />

      <div className="bg-[var(--zf-surface)] rounded-lg border border-[var(--zf-hairline)] p-6 space-y-4">
        <div className="flex items-center justify-between flex-wrap gap-3">
          <div>
            <label className="block text-xs text-[var(--zf-muted)] mb-1">Namespace</label>
            <input
              value={namespace}
              onChange={(e) => setNamespace(e.target.value.trim() || DEFAULT_NAMESPACE)}
              className="w-48 px-3 py-1.5 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-sm font-mono"
            />
          </div>
          <span className={`px-3 py-1 rounded-full text-xs font-medium border ${HEALTH_STYLES[status?.state ?? 'Unknown']}`}>
            {status?.state ?? 'Unknown'}
          </span>
        </div>
        {status?.phase && <p className="text-sm text-[var(--zf-muted)]">Phase: {status.phase}</p>}
        {status?.ceph_health && <p className="text-sm text-[var(--zf-muted)]">Ceph health: {status.ceph_health}</p>}
        {status?.message && (
          <p className="text-sm text-[var(--zf-warning)] bg-[var(--zf-warning)]/10 border border-[var(--zf-warning)]/25 rounded px-3 py-2">{status.message}</p>
        )}

        {!provisioned && (
          <div className="flex flex-wrap gap-2 pt-2 border-t border-[var(--zf-hairline)]">
            <button
              type="button"
              disabled={busy !== null}
              onClick={() =>
                run(
                  'bootstrap',
                  () => bootstrapRook(namespace),
                  'Rook operator bootstrap applied — re-run if some objects were skipped waiting on CRDs',
                )
              }
              className="zf-btn zf-btn-primary zf-btn-sm"
            >
              <Rocket className="w-4 h-4" />
              {busy === 'bootstrap' ? 'Bootstrapping…' : 'Bootstrap Rook operator'}
            </button>
            <button
              type="button"
              disabled={busy !== null}
              onClick={() =>
                run(
                  'create-cluster',
                  () => createCluster({ namespace, use_all_nodes: true, use_all_devices: false }),
                  'CephCluster created — it will take a few minutes to reach Ready',
                )
              }
              className="zf-btn zf-btn-ghost zf-btn-sm"
            >
              <Plus className="w-4 h-4" />
              {busy === 'create-cluster' ? 'Creating…' : 'Create CephCluster'}
            </button>
          </div>
        )}
        <p className="text-xs text-[var(--zf-muted)]">
          Bootstrap installs the Rook operator's CRDs, RBAC, and Deployment from a pinned upstream release; it can be
          safely re-run if some objects were skipped on the first pass (they need CRDs Established first). Creating a
          CephCluster uses every node with unformatted devices by default.
        </p>
      </div>

      <ResourceSection
        title="Block Pools"
        icon={HardDrive}
        busy={busy}
        emptyHint="Block pools back VM disks provisioned via RBD StorageClasses."
        items={pools.map((p) => ({
          name: p.metadata.name,
          detail: p.spec.replicated
            ? `replicated ×${p.spec.replicated.size}`
            : p.spec.erasureCoded
              ? `erasure-coded ${p.spec.erasureCoded.dataChunks}+${p.spec.erasureCoded.codingChunks}`
              : 'unconfigured',
          phase: p.status?.phase,
        }))}
        onDelete={(name) =>
          runDestructive(`del-pool-${name}`, `pool '${name}'`, () => deletePool(name, namespace), `Pool '${name}' deleted`)
        }
        onCreateStorageClass={(name) =>
          run(
            `sc-${name}`,
            () => createStorageClass({ type: 'rbd', name: `rook-ceph-${name}`, namespace, pool: name }),
            `StorageClass 'rook-ceph-${name}' created`,
          )
        }
        createForm={
          <CreatePoolForm
            disabled={busy !== null}
            onCreate={(req) => run('create-pool', () => createPool({ ...req, namespace }), `Pool '${req.name}' created`)}
          />
        }
      />

      <ResourceSection
        title="Filesystems"
        icon={FolderTree}
        busy={busy}
        emptyHint="A shared, POSIX filesystem (CephFS) usable via ReadWriteMany volumes."
        items={filesystems.map((f) => ({ name: f.metadata.name, detail: 'CephFS', phase: f.status?.phase }))}
        onDelete={(name) =>
          runDestructive(`del-fs-${name}`, `filesystem '${name}'`, () => deleteFilesystem(name, namespace), `Filesystem '${name}' deleted`)
        }
        onCreateStorageClass={(name) =>
          run(
            `sc-fs-${name}`,
            () => createStorageClass({ type: 'cephfs', name: `rook-cephfs-${name}`, namespace, filesystem: name }),
            `StorageClass 'rook-cephfs-${name}' created`,
          )
        }
        createForm={
          <CreateNamedForm
            placeholder="cephfs"
            disabled={busy !== null}
            onCreate={(name) =>
              run('create-fs', () => createFilesystem({ name, namespace }), `Filesystem '${name}' created`)
            }
          />
        }
      />

      <ResourceSection
        title="Object Stores"
        icon={Archive}
        busy={busy}
        emptyHint="S3-compatible object storage (RGW) for application data."
        items={objectStores.map((o) => ({ name: o.metadata.name, detail: 'S3 (RGW)', phase: o.status?.phase }))}
        onDelete={(name) =>
          runDestructive(`del-os-${name}`, `object store '${name}'`, () => deleteObjectStore(name, namespace), `Object store '${name}' deleted`)
        }
        createForm={
          <CreateNamedForm
            placeholder="s3-store"
            disabled={busy !== null}
            onCreate={(name) =>
              run('create-os', () => createObjectStore({ name, namespace }), `Object store '${name}' created`)
            }
          />
        }
      />

      <AtlasSection />

      {confirmState && (
        <ConfirmDialog
          title={confirmState.title}
          message={confirmState.message}
          confirmLabel={confirmState.confirmLabel}
          variant={confirmState.variant}
          onConfirm={confirmState.onConfirm}
          onCancel={cancel}
        />
      )}
    </div>
  )
}

function ResourceSection({
  title,
  icon: Icon,
  items,
  emptyHint,
  busy,
  onDelete,
  onCreateStorageClass,
  createForm,
}: {
  title: string
  icon: typeof Database
  items: { name: string; detail: string; phase?: string }[]
  emptyHint: string
  busy: string | null
  onDelete: (name: string) => void
  onCreateStorageClass?: (name: string) => void
  createForm: React.ReactNode
}) {
  return (
    <div className="bg-[var(--zf-surface)] rounded-lg border border-[var(--zf-hairline)] p-6 space-y-4">
      <h2 className="flex items-center gap-2 text-sm font-semibold text-[var(--zf-ink)]">
        <Icon className="w-4 h-4 text-[var(--zf-muted)]" />
        {title}
      </h2>
      {items.length === 0 ? (
        <p className="text-sm text-[var(--zf-muted)]">{emptyHint}</p>
      ) : (
        <div className="divide-y divide-[var(--zf-hairline)]">
          {items.map((item) => (
            <div key={item.name} className="flex items-center justify-between py-2 gap-3">
              <div>
                <div className="font-medium text-sm text-[var(--zf-ink)]">{item.name}</div>
                <div className="text-xs text-[var(--zf-muted)]">
                  {item.detail}
                  {item.phase ? ` · ${item.phase}` : ''}
                </div>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                {onCreateStorageClass && (
                  <button
                    type="button"
                    disabled={busy !== null}
                    onClick={() => onCreateStorageClass(item.name)}
                    className="zf-btn zf-btn-ghost zf-btn-sm"
                  >
                    StorageClass
                  </button>
                )}
                <button
                  type="button"
                  disabled={busy !== null}
                  onClick={() => onDelete(item.name)}
                  className="zf-btn zf-btn-danger zf-btn-sm"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
      {createForm}
    </div>
  )
}

function CreatePoolForm({
  disabled,
  onCreate,
}: {
  disabled: boolean
  onCreate: (req: { name: string; replicated_size?: number; erasure_coded?: [number, number] }) => void
}) {
  const [name, setName] = useState('')
  const [mode, setMode] = useState<'replicated' | 'erasure'>('replicated')
  const [size, setSize] = useState(3)
  const [dataChunks, setDataChunks] = useState(4)
  const [codingChunks, setCodingChunks] = useState(2)

  return (
    <div className="flex items-end gap-2 flex-wrap pt-2 border-t border-[var(--zf-hairline)]">
      <div>
        <label className="block text-xs text-[var(--zf-muted)] mb-1">Name</label>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="fast-ssd"
          className="w-36 px-2.5 py-1.5 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-sm"
        />
      </div>
      <select
        value={mode}
        onChange={(e) => setMode(e.target.value as 'replicated' | 'erasure')}
        className="px-2.5 py-1.5 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-sm"
      >
        <option value="replicated">Replicated</option>
        <option value="erasure">Erasure-coded</option>
      </select>
      {mode === 'replicated' ? (
        <div>
          <label className="block text-xs text-[var(--zf-muted)] mb-1">Replicas</label>
          <input
            type="number"
            min={1}
            max={9}
            value={size}
            onChange={(e) => setSize(parseInt(e.target.value) || 3)}
            className="w-20 px-2.5 py-1.5 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-sm"
          />
        </div>
      ) : (
        <>
          <div>
            <label className="block text-xs text-[var(--zf-muted)] mb-1">Data chunks</label>
            <input
              type="number"
              min={1}
              value={dataChunks}
              onChange={(e) => setDataChunks(parseInt(e.target.value) || 4)}
              className="w-20 px-2.5 py-1.5 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-sm"
            />
          </div>
          <div>
            <label className="block text-xs text-[var(--zf-muted)] mb-1">Coding chunks</label>
            <input
              type="number"
              min={1}
              value={codingChunks}
              onChange={(e) => setCodingChunks(parseInt(e.target.value) || 2)}
              className="w-20 px-2.5 py-1.5 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-sm"
            />
          </div>
        </>
      )}
      <button
        type="button"
        disabled={disabled || !name.trim()}
        onClick={() =>
          onCreate(
            mode === 'replicated'
              ? { name: name.trim(), replicated_size: size }
              : { name: name.trim(), erasure_coded: [dataChunks, codingChunks] },
          )
        }
        className="zf-btn zf-btn-primary zf-btn-sm"
      >
        <Plus className="w-4 h-4" />
        Add pool
      </button>
    </div>
  )
}

function CreateNamedForm({
  placeholder,
  disabled,
  onCreate,
}: {
  placeholder: string
  disabled: boolean
  onCreate: (name: string) => void
}) {
  const [name, setName] = useState('')
  return (
    <div className="flex items-end gap-2 pt-2 border-t border-[var(--zf-hairline)]">
      <input
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder={placeholder}
        className="w-48 px-2.5 py-1.5 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-sm"
      />
      <button
        type="button"
        disabled={disabled || !name.trim()}
        onClick={() => {
          onCreate(name.trim())
          setName('')
        }}
        className="zf-btn zf-btn-primary zf-btn-sm"
      >
        <Plus className="w-4 h-4" />
        Create
      </button>
    </div>
  )
}

const ATLAS_HEALTH_STYLES: Record<string, string> = {
  ok: 'text-[var(--zf-success)] bg-[var(--zf-success)]/10 border-[var(--zf-success)]/25',
  warn: 'text-[var(--zf-warning)] bg-[var(--zf-warning)]/10 border-[var(--zf-warning)]/25',
  critical: 'text-[var(--zf-danger)] bg-[var(--zf-danger)]/10 border-[var(--zf-danger)]/25',
  unknown: 'text-[var(--zf-muted)] bg-[var(--zf-canvas)] border-[var(--zf-hairline)]',
}

function AtlasSection() {
  const toast = useToastContext()
  const { confirmState, confirm, cancel } = useConfirm()
  const [status, setStatus] = useState<AtlasStatus | null>(null)
  const [backends, setBackends] = useState<AtlasBackend[]>([])
  const [clusters, setClusters] = useState<AtlasCluster[]>([])
  const [pools, setPools] = useState<AtlasPool[]>([])
  const [volumes, setVolumes] = useState<AtlasVolume[]>([])
  const [jobs, setJobs] = useState<AtlasJob[]>([])
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState<string | null>(null)

  const load = async () => {
    try {
      const s = await getAtlasStatus()
      setStatus(s)
      if (!s.enabled || !s.connected) {
        setBackends([])
        setClusters([])
        setPools([])
        setVolumes([])
        setJobs([])
        return
      }
      const [b, c, p, v, j] = await Promise.all([
        listAtlasBackends().catch(() => []),
        listAtlasClusters().catch(() => []),
        listAtlasPools().catch(() => []),
        listAtlasVolumes().catch(() => []),
        listAtlasJobs().catch(() => []),
      ])
      setBackends(b)
      setClusters(c)
      setPools(p)
      setVolumes(v)
      setJobs(j)
    } catch {
      // Status fetch itself failing (network/auth) leaves status null; the
      // section below just shows "Not configured" rather than an alarming
      // banner — Atlas is an optional secondary integration on this page.
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
    const interval = setInterval(() => void load(), 15000)
    return () => clearInterval(interval)
  }, [])

  /** Every Atlas write only ever returns "requested" synchronously -- this
      polls the real job to a terminal state in the background and toasts
      the actual outcome, instead of leaving the user to guess from the next
      15s refresh whether it actually succeeded. */
  const runAtlasWrite = async (busyKey: string, actionLabel: string, volumeName: string, op: () => Promise<{ job_id: string }>) => {
    setBusy(busyKey)
    try {
      const envelope = await op()
      toast.success(`${actionLabel} requested for '${volumeName}'`)
      await load()
      const job = await pollAtlasJob(envelope.job_id)
      if (isAtlasJobTerminal(job)) {
        if (job.state === 'succeeded') {
          toast.success(`${actionLabel} succeeded for '${volumeName}'`)
        } else {
          toast.error(`${actionLabel} failed for '${volumeName}': ${job.error ?? 'unknown error'}`)
        }
      }
      await load()
    } catch (e) {
      toastFailure(toast, `Failed to ${actionLabel.toLowerCase()} volume`, e)
    } finally {
      setBusy(null)
    }
  }

  if (loading) return null

  const enabled = status?.enabled ?? false
  const connected = status?.connected ?? false

  return (
    <div className="bg-[var(--zf-surface)] rounded-lg border border-[var(--zf-hairline)] p-6 space-y-4">
      <div className="flex items-center justify-between flex-wrap gap-3">
        <h2 className="flex items-center gap-2 text-sm font-semibold text-[var(--zf-ink)]">
          <Layers className="w-4 h-4 text-[var(--zf-muted)]" />
          Atlas
        </h2>
        <span
          className={`px-3 py-1 rounded-full text-xs font-medium border ${
            !enabled
              ? ATLAS_HEALTH_STYLES.unknown
              : connected
                ? ATLAS_HEALTH_STYLES.ok
                : ATLAS_HEALTH_STYLES.critical
          }`}
        >
          {!enabled ? 'Not configured' : connected ? 'Connected' : 'Unreachable'}
        </span>
      </div>

      {!enabled && (
        <p className="text-sm text-[var(--zf-muted)]">
          Optional pluggable storage control plane for Ceph/NFS/ZFS with multi-backend governance
          and capacity forecasting. Set <code>ATLAS_URL</code> (and <code>ATLAS_TOKEN</code>) on
          the Zorvia server to enable — see{' '}
          <a href="/docs/ATLAS_INTEGRATION.md" className="underline">
            docs/ATLAS_INTEGRATION.md
          </a>
          .
        </p>
      )}

      {enabled && !connected && (
        <p className="text-sm text-[var(--zf-warning)] bg-[var(--zf-warning)]/10 border border-[var(--zf-warning)]/25 rounded px-3 py-2">
          {status?.error || 'Could not reach the configured Atlas control plane.'}
        </p>
      )}

      {enabled && connected && (
        <>
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
            <StatTile label="Backends" value={backends.length} />
            <StatTile label="Clusters" value={clusters.length} />
            <StatTile label="Pools" value={pools.length} />
            <StatTile label="Volumes" value={volumes.length} />
          </div>

          {backends.length > 0 && (
            <div className="divide-y divide-[var(--zf-hairline)]">
              {backends.map((b) => (
                <div key={b.id} className="flex items-center justify-between py-2 gap-3">
                  <div>
                    <div className="font-medium text-sm text-[var(--zf-ink)]">{b.name}</div>
                    <div className="text-xs text-[var(--zf-muted)]">
                      {b.backend_type} · {b.mode}
                      {b.cordoned ? ' · cordoned' : ''}
                    </div>
                  </div>
                  <span
                    className={`px-2 py-0.5 rounded-full text-xs font-medium border ${
                      b.cordoned ? ATLAS_HEALTH_STYLES.warn : ATLAS_HEALTH_STYLES.ok
                    }`}
                  >
                    {b.status}
                  </span>
                </div>
              ))}
            </div>
          )}

          {volumes.length > 0 && (
            <div>
              <h3 className="text-xs font-medium text-[var(--zf-muted)] mb-2">Volumes</h3>
              <div className="divide-y divide-[var(--zf-hairline)]">
                {volumes.map((v) => (
                  <AtlasVolumeRow
                    key={v.id}
                    volume={v}
                    busy={busy}
                    onExpand={(newSizeBytes) =>
                      void runAtlasWrite(`expand-${v.id}`, 'Expand', v.name, () => expandAtlasVolume(v.id, newSizeBytes))
                    }
                    onDelete={async () => {
                      if (
                        !(await confirm(`Delete volume '${v.name}'`, 'Delete this volume? Any data on it is destroyed.', {
                          variant: 'danger',
                          confirmLabel: 'Delete',
                        }))
                      ) {
                        return
                      }
                      // Always pass confirm=true -- Atlas itself decides (based on
                      // storage class / protection tier) whether that was required;
                      // Zorvia doesn't try to duplicate that judgment.
                      void runAtlasWrite(`delete-${v.id}`, 'Delete', v.name, () => deleteAtlasVolume(v.id, true))
                    }}
                  />
                ))}
              </div>
            </div>
          )}

          <CreateAtlasVolumeForm
            disabled={busy !== null}
            onCreate={(req) => void runAtlasWrite('create-volume', 'Create', req.name, () => createAtlasVolume(req))}
          />

          {jobs.length > 0 && (
            <div>
              <h3 className="text-xs font-medium text-[var(--zf-muted)] mb-2">Recent jobs</h3>
              <div className="divide-y divide-[var(--zf-hairline)]">
                {jobs.slice(0, 10).map((j) => (
                  <AtlasJobRow
                    key={j.id}
                    job={j}
                    busy={busy === `cancel-job-${j.id}`}
                    onCancel={() => {
                      setBusy(`cancel-job-${j.id}`)
                      cancelAtlasJob(j.id)
                        .then(() => {
                          toast.success(`Job '${j.id}' cancelled`)
                          return load()
                        })
                        .catch((e) => toastFailure(toast, 'Failed to cancel job', e))
                        .finally(() => setBusy(null))
                    }}
                  />
                ))}
              </div>
            </div>
          )}
        </>
      )}

      {confirmState && (
        <ConfirmDialog
          title={confirmState.title}
          message={confirmState.message}
          confirmLabel={confirmState.confirmLabel}
          variant={confirmState.variant}
          onConfirm={confirmState.onConfirm}
          onCancel={cancel}
        />
      )}
    </div>
  )
}

function AtlasVolumeRow({
  volume,
  busy,
  onExpand,
  onDelete,
}: {
  volume: AtlasVolume
  busy: string | null
  onExpand: (newSizeBytes: number) => void
  onDelete: () => void
}) {
  const currentGiB = Math.ceil(volume.size_bytes / (1024 * 1024 * 1024))
  const [newSizeGiB, setNewSizeGiB] = useState(currentGiB + 10)
  const rowBusy = busy === `expand-${volume.id}` || busy === `delete-${volume.id}`

  return (
    <div className="flex items-center justify-between py-2 gap-3">
      <div>
        <div className="font-medium text-sm text-[var(--zf-ink)]">{volume.name}</div>
        <div className="text-xs text-[var(--zf-muted)]">
          {volume.kind} · {formatBytes(volume.size_bytes)}
          {volume.storage_class_name ? ` · ${volume.storage_class_name}` : ''}
        </div>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <span
          className={`px-2 py-0.5 rounded-full text-xs font-medium border ${ATLAS_HEALTH_STYLES[volume.health] ?? ATLAS_HEALTH_STYLES.unknown}`}
        >
          {volume.state}
        </span>
        <input
          type="number"
          min={currentGiB + 1}
          value={newSizeGiB}
          onChange={(e) => setNewSizeGiB(parseInt(e.target.value) || currentGiB + 1)}
          className="w-20 px-2 py-1 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-xs"
          title="New size (GiB)"
        />
        <button
          type="button"
          disabled={busy !== null || newSizeGiB <= currentGiB}
          onClick={() => onExpand(newSizeGiB * 1024 * 1024 * 1024)}
          className="zf-btn zf-btn-ghost zf-btn-sm"
        >
          {rowBusy && busy === `expand-${volume.id}` ? 'Expanding…' : 'Expand'}
        </button>
        <button
          type="button"
          disabled={busy !== null}
          onClick={onDelete}
          className="zf-btn zf-btn-danger zf-btn-sm"
        >
          <Trash2 className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  )
}

const ATLAS_JOB_STATE_STYLES: Record<string, string> = {
  succeeded: ATLAS_HEALTH_STYLES.ok,
  failed: ATLAS_HEALTH_STYLES.critical,
  running: ATLAS_HEALTH_STYLES.warn,
  verifying: ATLAS_HEALTH_STYLES.warn,
  queued: ATLAS_HEALTH_STYLES.unknown,
  pending: ATLAS_HEALTH_STYLES.unknown,
}

function AtlasJobRow({ job, busy, onCancel }: { job: AtlasJob; busy: boolean; onCancel: () => void }) {
  return (
    <div className="flex items-center justify-between py-2 gap-3">
      <div>
        <div className="font-medium text-sm text-[var(--zf-ink)]">{job.job_type}</div>
        <div className="text-xs text-[var(--zf-muted)]">
          {job.id}
          {job.error ? ` · ${job.error}` : ''}
        </div>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        {!isAtlasJobTerminal(job) && (
          <span className="text-xs text-[var(--zf-muted)]">{job.progress_percent}%</span>
        )}
        <span
          className={`px-2 py-0.5 rounded-full text-xs font-medium border ${ATLAS_JOB_STATE_STYLES[job.state] ?? ATLAS_HEALTH_STYLES.unknown}`}
        >
          {job.state}
        </span>
        {!isAtlasJobTerminal(job) && (
          <button type="button" disabled={busy} onClick={onCancel} className="zf-btn zf-btn-ghost zf-btn-sm">
            {busy ? 'Cancelling…' : 'Cancel'}
          </button>
        )}
      </div>
    </div>
  )
}

function StatTile({ label, value }: { label: string; value: number }) {
  return (
    <div className="bg-[var(--zf-canvas)] rounded-md border border-[var(--zf-hairline)] px-3 py-2">
      <div className="text-lg font-semibold text-[var(--zf-ink)]">{value}</div>
      <div className="text-xs text-[var(--zf-muted)]">{label}</div>
    </div>
  )
}

function CreateAtlasVolumeForm({
  disabled,
  onCreate,
}: {
  disabled: boolean
  onCreate: (req: ReturnType<typeof buildCreateAtlasVolumeRequest>) => void
}) {
  const [name, setName] = useState('')
  const [sizeGiB, setSizeGiB] = useState(20)
  const [vmName, setVmName] = useState('')

  return (
    <div className="flex items-end gap-2 flex-wrap pt-2 border-t border-[var(--zf-hairline)]">
      <div>
        <label className="block text-xs text-[var(--zf-muted)] mb-1">Name</label>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="app-data"
          className="w-36 px-2.5 py-1.5 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-sm"
        />
      </div>
      <div>
        <label className="block text-xs text-[var(--zf-muted)] mb-1">Size (GiB)</label>
        <input
          type="number"
          min={1}
          value={sizeGiB}
          onChange={(e) => setSizeGiB(parseInt(e.target.value) || 1)}
          className="w-24 px-2.5 py-1.5 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-sm"
        />
      </div>
      <div>
        <label className="block text-xs text-[var(--zf-muted)] mb-1">Attribute to VM (optional)</label>
        <input
          value={vmName}
          onChange={(e) => setVmName(e.target.value)}
          placeholder="prod-db"
          className="w-40 px-2.5 py-1.5 bg-[var(--zf-surface)] border border-[var(--zf-hairline)] rounded-md text-sm"
        />
      </div>
      <button
        type="button"
        disabled={disabled || !name.trim() || sizeGiB < 1}
        onClick={() => onCreate(buildCreateAtlasVolumeRequest(name.trim(), sizeGiB, vmName.trim()))}
        className="zf-btn zf-btn-primary zf-btn-sm"
      >
        <Plus className="w-4 h-4" />
        Create volume
      </button>
    </div>
  )
}

function buildCreateAtlasVolumeRequest(name: string, sizeGiB: number, vmName: string) {
  return {
    name,
    size_bytes: sizeGiB * 1024 * 1024 * 1024,
    ...(vmName ? { owner: atlasVolumeOwnerForVm(vmName) } : {}),
  }
}
