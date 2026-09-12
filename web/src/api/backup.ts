// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { apiGet, apiPost, apiPostVoid, apiDelete } from './client'

export interface Backup {
  id: string
  vm_name: string
  backup_type: 'full' | 'incremental'
  size_bytes: number
  compressed: boolean
  created: string
  status: 'completed' | 'in_progress' | 'failed'
  storage_location: string
  retention_days: number
  expires_at?: string
  metadata?: Record<string, any>
}

export interface CreateBackupRequest {
  vm_name: string
  backup_type: 'full' | 'incremental'
  compress?: boolean
  retention_days?: number
  description?: string
}

export interface RestoreOptions {
  backup_id: string
  target_vm_name?: string
  restore_config?: boolean
  restore_disks?: boolean
  restore_state?: boolean
}

export interface BackupJob {
  id: string
  backup_id?: string
  vm_name: string
  operation: 'backup' | 'restore'
  status: 'queued' | 'running' | 'completed' | 'failed'
  progress: number
  started_at?: string
  completed_at?: string
  error?: string
}

/** Mirrors the backend's real, disk-persisted `crate::backup::schedule::BackupSchedule`
 * (src/api/http_server/web/backup_scheduler_handlers.rs) -- `vm_names` lists
 * exact VM names (empty means "every VM in the namespace"), not tags. */
export interface BackupPolicy {
  id: string
  name: string
  vm_names: string[]
  schedule_type: 'hourly' | 'daily' | 'weekly' | 'monthly'
  backup_type: 'full' | 'incremental'
  retention_days: number | null
  enabled: boolean
  last_run?: string | null
  next_run?: string | null
}

export interface CreateBackupPolicyRequest {
  name: string
  schedule_type: 'hourly' | 'daily' | 'weekly' | 'monthly'
  /** 0-23, defaults to 2 (2am). Ignored for "hourly". */
  hour?: number
  /** 0-59, defaults to 0. */
  minute?: number
  /** Required for "weekly", e.g. "sunday". */
  weekday?: string
  /** Required for "monthly", 1-31. */
  day_of_month?: number
  /** Exact VM names; omit/empty for every VM in the namespace. */
  vm_names?: string[]
  backup_type?: 'full' | 'incremental'
  retention_days?: number
  enabled?: boolean
}

const API_BASE = '/api'

export async function listBackups(vmName?: string): Promise<Backup[]> {
  const url = vmName ? `${API_BASE}/backups?vm=${vmName}` : `${API_BASE}/backups`
  return apiGet<Backup[]>(url)
}

export async function getBackup(id: string): Promise<Backup> {
  return apiGet<Backup>(`${API_BASE}/backups/${id}`)
}

export async function createBackup(req: CreateBackupRequest): Promise<BackupJob> {
  return apiPost<BackupJob>(`${API_BASE}/backups`, req)
}

export async function deleteBackup(id: string): Promise<void> {
  return apiDelete(`${API_BASE}/backups/${id}`)
}

export async function restoreBackup(options: RestoreOptions): Promise<BackupJob> {
  return apiPost<BackupJob>(`${API_BASE}/backups/restore`, options)
}

export async function getBackupJobs(): Promise<BackupJob[]> {
  return apiGet<BackupJob[]>(`${API_BASE}/backups/jobs`)
}

export async function getBackupJob(id: string): Promise<BackupJob> {
  return apiGet<BackupJob>(`${API_BASE}/backups/jobs/${id}`)
}

export async function listBackupPolicies(): Promise<BackupPolicy[]> {
  return apiGet<BackupPolicy[]>(`${API_BASE}/backups/policies`)
}

export async function createBackupPolicy(policy: CreateBackupPolicyRequest): Promise<BackupPolicy> {
  return apiPost<BackupPolicy>(`${API_BASE}/backups/policies`, policy)
}

export async function deleteBackupPolicy(id: string): Promise<void> {
  return apiDelete(`${API_BASE}/backups/policies/${id}`)
}

export async function enableBackupPolicy(id: string): Promise<void> {
  return apiPostVoid(`${API_BASE}/backups/policies/${id}/enable`)
}

export async function disableBackupPolicy(id: string): Promise<void> {
  return apiPostVoid(`${API_BASE}/backups/policies/${id}/disable`)
}

export interface BackupStats {
  total_backups: number
  total_size_bytes: number
  by_type: Record<string, number>
  by_vm: Record<string, number>
  oldest_backup: string
  newest_backup: string
}

export async function getBackupStats(): Promise<BackupStats> {
  return apiGet<BackupStats>(`${API_BASE}/backups/stats`)
}
