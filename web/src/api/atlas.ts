// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { apiFetch, apiGet, apiPost } from './client'
import { formatHttpErrorBody } from '../utils/apiError'
import { parseJsonResponse } from '../utils/parseJsonResponse'

export interface AtlasStatus {
  enabled: boolean
  connected: boolean
  tenant_id?: string | null
  health?: unknown
  error?: string
}

export interface AtlasCapabilities {
  block: boolean
  file: boolean
  object: boolean
  snapshots: boolean
  clone: boolean
  expansion: boolean
  replication: boolean
}

export interface AtlasBackend {
  id: string
  name: string
  backend_type: 'ceph' | 'nfs' | 'zfs' | 'san' | 'cloud_block' | 'kubernetes'
  mode: 'managed_rook' | 'external' | 'read_only'
  status: string
  capabilities: AtlasCapabilities
  connection_ref?: string | null
  cordoned: boolean
}

export type AtlasHealth = 'ok' | 'warn' | 'critical' | 'unknown'

export interface AtlasCluster {
  id: string
  backend_id: string
  name: string
  native_fsid?: string | null
  health: AtlasHealth
  raw_capacity_bytes?: number | null
  used_capacity_bytes?: number | null
  available_capacity_bytes?: number | null
}

export interface AtlasPool {
  id: string
  cluster_id: string
  name: string
  kind: string
  device_class?: string | null
  replica_size?: number | null
  used_bytes?: number | null
  max_bytes?: number | null
  health: AtlasHealth
}

export interface AtlasVolume {
  id: string
  cluster_id?: string | null
  pool_id?: string | null
  name: string
  kind: 'block' | 'filesystem' | 'object'
  backend_native_id?: string | null
  size_bytes: number
  used_bytes?: number | null
  state: string
  health: AtlasHealth
  kubernetes_namespace?: string | null
  pvc_name?: string | null
  storage_class_name?: string | null
}

export type AtlasJobState = 'pending' | 'queued' | 'running' | 'verifying' | 'succeeded' | 'failed'

/** Every Atlas write (volume create/expand/delete) returns one of these ids --
    this is how to find out what actually happened, instead of only ever seeing "queued". */
export interface AtlasJob {
  id: string
  tenant_id: string
  job_type: string
  state: AtlasJobState
  requested_by: string
  progress_percent: number
  error?: string | null
  result?: unknown
  created_at?: string | null
  updated_at?: string | null
}

export const isAtlasJobTerminal = (job: AtlasJob): boolean => job.state === 'succeeded' || job.state === 'failed'

/** The envelope every Atlas write (volume create/expand/delete) returns --
    a 202 + job id, never a finished result. Poll `getAtlasJob(job_id)`
    with it to find out what actually happened. */
export interface AtlasJobEnvelope {
  job_id: string
  state: string
  resource?: unknown
  links?: { job?: string }
}

export interface AtlasStorageClass {
  name: string
  provisioner: string
  reclaim_policy?: string | null
  volume_binding_mode?: string | null
  allow_volume_expansion?: boolean | null
  is_ceph: boolean
  labels: Record<string, string>
}

export interface AtlasVolumeOwner {
  product: string
  resource_type: string
  resource_id: string
  role?: string
}

export interface AtlasK8sVolumeOpts {
  namespace?: string
  create_pvc?: boolean
  access_modes?: string[]
  volume_mode?: string
  storage_class?: string
}

export interface CreateAtlasVolumeRequest {
  tenant_id?: string
  name: string
  size_bytes: number
  kind?: 'block' | 'filesystem' | 'object'
  policy?: string
  pool?: string
  owner?: AtlasVolumeOwner
  kubernetes?: AtlasK8sVolumeOpts
}

/** Tags a volume as owned by one of Zorvia's own VMs, so it's traceable back
    to Zorvia in Atlas's own inventory. */
export const atlasVolumeOwnerForVm = (vmName: string): AtlasVolumeOwner => ({
  product: 'zorvia',
  resource_type: 'vm',
  resource_id: vmName,
  role: 'data_disk',
})

export const getAtlasStatus = () => apiGet<AtlasStatus>('/api/v1/atlas/status')
export const listAtlasBackends = () => apiGet<AtlasBackend[]>('/api/v1/atlas/backends')
export const getAtlasBackendsSummary = () => apiGet<unknown>('/api/v1/atlas/backends/summary')
export const listAtlasClusters = () => apiGet<AtlasCluster[]>('/api/v1/atlas/clusters')
export const getAtlasClusterHealth = (id: string) => apiGet<unknown>(`/api/v1/atlas/clusters/${encodeURIComponent(id)}/health`)
export const listAtlasPools = () => apiGet<AtlasPool[]>('/api/v1/atlas/pools')
export const getAtlasCephStatus = () => apiGet<unknown>('/api/v1/atlas/ceph/status')
export const getAtlasCephDf = () => apiGet<unknown>('/api/v1/atlas/ceph/df')
export const listAtlasStorageClasses = () => apiGet<AtlasStorageClass[]>('/api/v1/atlas/storage-classes')
export const listAtlasVolumes = () => apiGet<AtlasVolume[]>('/api/v1/atlas/volumes')
export const createAtlasVolume = (body: CreateAtlasVolumeRequest) =>
  apiPost<AtlasJobEnvelope>('/api/v1/atlas/volumes', body)
export const expandAtlasVolume = (id: string, newSizeBytes: number) =>
  apiPost<AtlasJobEnvelope>(`/api/v1/atlas/volumes/${encodeURIComponent(id)}/expand`, { new_size_bytes: newSizeBytes })
/** `confirm` must be true for production/protected-class volumes -- Atlas rejects the delete
    with a 400 naming that requirement if it's needed and omitted. Unlike the other writes this
    uses apiFetch directly (not the shared apiDelete, which discards the response body) because
    delete also returns a real job envelope that's worth polling like create/expand. */
export const deleteAtlasVolume = async (id: string, confirm = false): Promise<AtlasJobEnvelope> => {
  const url = `/api/v1/atlas/volumes/${encodeURIComponent(id)}${confirm ? '?confirm=true' : ''}`
  const res = await apiFetch(url, { method: 'DELETE' })
  if (!res.ok) {
    const body = await res.text().catch(() => '')
    throw new Error(formatHttpErrorBody(res.status, res.statusText, body))
  }
  return parseJsonResponse<AtlasJobEnvelope>(res)
}

export const listAtlasJobs = () => apiGet<AtlasJob[]>('/api/v1/atlas/jobs')
export const getAtlasJob = (id: string) => apiGet<AtlasJob>(`/api/v1/atlas/jobs/${encodeURIComponent(id)}`)
export const cancelAtlasJob = (id: string) => apiPost<unknown>(`/api/v1/atlas/jobs/${encodeURIComponent(id)}/cancel`)

/** Polls a job to a terminal state (or gives up after `timeoutMs`, returning
    the last-seen state) -- the UI can then show the real outcome of a
    create/expand/delete instead of just "requested". */
export async function pollAtlasJob(
  jobId: string,
  opts?: { intervalMs?: number; timeoutMs?: number },
): Promise<AtlasJob> {
  const intervalMs = opts?.intervalMs ?? 1500
  const deadline = Date.now() + (opts?.timeoutMs ?? 60_000)
  for (;;) {
    const job = await getAtlasJob(jobId)
    if (isAtlasJobTerminal(job) || Date.now() >= deadline) return job
    await new Promise((r) => setTimeout(r, intervalMs))
  }
}

// ── Backend lifecycle ──

export interface CreateAtlasBackendRequest {
  name: string
  backend_type?: 'ceph' | 'nfs' | 'zfs' | 'san' | 'cloud_block' | 'kubernetes'
  mode?: 'managed_rook' | 'external' | 'read_only'
  server?: string
  targets?: string[]
}

export const createAtlasBackend = (body: CreateAtlasBackendRequest) =>
  apiPost<AtlasBackend>('/api/v1/atlas/backends', body)
export const deleteAtlasBackend = (id: string, purge = false) =>
  apiFetchDelete(`/api/v1/atlas/backends/${encodeURIComponent(id)}${purge ? '?purge=true' : ''}`)
export const discoverAtlasBackend = (id: string) =>
  apiPost<unknown>(`/api/v1/atlas/backends/${encodeURIComponent(id)}/discover`)
export const cordonAtlasBackend = (id: string) =>
  apiPost<unknown>(`/api/v1/atlas/backends/${encodeURIComponent(id)}/cordon`)
export const uncordonAtlasBackend = (id: string) =>
  apiPost<unknown>(`/api/v1/atlas/backends/${encodeURIComponent(id)}/uncordon`)

/** Shared with deleteAtlasVolume -- the DELETE routes here return a real
    body worth reading (not just success/fail), so they use apiFetch
    directly instead of the shared void-returning apiDelete. */
async function apiFetchDelete<T>(url: string): Promise<T> {
  const res = await apiFetch(url, { method: 'DELETE' })
  if (!res.ok) {
    const body = await res.text().catch(() => '')
    throw new Error(formatHttpErrorBody(res.status, res.statusText, body))
  }
  return parseJsonResponse<T>(res)
}

// ── RBD images -- a separate identity space (rbd:<pool>/<image>) from the
// AtlasVolume list above; Atlas returns just names for the list route, not
// full objects. ──

export interface CreateAtlasRbdImageRequest {
  name: string
  size_bytes: number
  pool?: string
  tenant_id?: string
}

export const listAtlasRbdImages = (pool?: string) =>
  apiGet<{ pool: string; images: string[] }>(`/api/v1/atlas/rbd-images${pool ? `?pool=${encodeURIComponent(pool)}` : ''}`)
export const createAtlasRbdImage = (body: CreateAtlasRbdImageRequest) =>
  apiPost<AtlasJobEnvelope>('/api/v1/atlas/rbd-images', body)
export const deleteAtlasRbdImage = (pool: string, image: string) =>
  apiFetchDelete<AtlasJobEnvelope>(`/api/v1/atlas/rbd-images/${encodeURIComponent(pool)}/${encodeURIComponent(image)}`)
export const resizeAtlasRbdImage = (pool: string, image: string, sizeBytes: number, allowShrink = false) =>
  apiPost<AtlasJobEnvelope>(
    `/api/v1/atlas/rbd-images/${encodeURIComponent(pool)}/${encodeURIComponent(image)}/resize`,
    { size_bytes: sizeBytes, allow_shrink: allowShrink },
  )

// ── Object-store buckets + backups ──

export interface AtlasBucket {
  id: string
  tenant_id: string
  name: string
  bucket_name?: string | null
  endpoint?: string | null
  region?: string | null
  namespace?: string | null
  state: string
  created_at?: string | null
}

export interface AtlasBackup {
  id: string
  tenant_id: string
  volume_id: string
  snapshot_id?: string | null
  bucket_id: string
  object_key: string
  format: string
  state: string
  created_at?: string | null
}

export interface CreateAtlasBucketRequest {
  name: string
  namespace?: string
  storage_class?: string
  max_objects?: number
  max_size?: string
}

export const listAtlasBuckets = () => apiGet<AtlasBucket[]>('/api/v1/atlas/buckets')
export const getAtlasBucketStats = (id: string) => apiGet<unknown>(`/api/v1/atlas/buckets/${encodeURIComponent(id)}/stats`)
export const createAtlasBucket = (body: CreateAtlasBucketRequest) =>
  apiPost<AtlasJobEnvelope>('/api/v1/atlas/buckets', body)
export const deleteAtlasBucket = (id: string, force = false) =>
  apiFetchDelete<AtlasJobEnvelope>(`/api/v1/atlas/buckets/${encodeURIComponent(id)}${force ? '?force=true' : ''}`)

export const listAtlasBackups = (volumeId?: string) =>
  apiGet<AtlasBackup[]>(`/api/v1/atlas/backups${volumeId ? `?volume_id=${encodeURIComponent(volumeId)}` : ''}`)
export const deleteAtlasBackup = (id: string) =>
  apiFetchDelete<AtlasJobEnvelope>(`/api/v1/atlas/backups/${encodeURIComponent(id)}`)
