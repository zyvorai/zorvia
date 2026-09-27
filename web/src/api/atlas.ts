// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { apiGet, apiPost } from './client'

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
export const createAtlasVolume = (body: CreateAtlasVolumeRequest) => apiPost<unknown>('/api/v1/atlas/volumes', body)
