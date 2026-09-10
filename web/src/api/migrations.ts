// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { apiGet, apiPost, apiPostVoid } from './client'

const API_BASE = '/api'

/** KubeVirt VirtualMachineInstanceMigration phases. */
export type MigrationPhase =
  | 'Pending'
  | 'Scheduling'
  | 'Scheduled'
  | 'PreparingTarget'
  | 'TargetReady'
  | 'Running'
  | 'Succeeded'
  | 'Failed'
  | 'Unknown'

export interface MigrationStatus {
  id: string
  vm_name: string
  phase: MigrationPhase
  source_node?: string
  target_node?: string
  start_timestamp?: string
  end_timestamp?: string
  completed: boolean
  failed: boolean
  created?: string
}

/** Start a live migration for a VM. KubeVirt's scheduler selects the target
 * node automatically — there is no user-supplied host or URI. */
export async function startMigration(vmName: string): Promise<MigrationStatus> {
  return apiPost<MigrationStatus>(`${API_BASE}/vms/${encodeURIComponent(vmName)}/migrate`)
}

export async function listVmMigrations(vmName: string): Promise<MigrationStatus[]> {
  return apiGet<MigrationStatus[]>(`${API_BASE}/vms/${encodeURIComponent(vmName)}/migrations`)
}

export async function getMigration(id: string): Promise<MigrationStatus> {
  return apiGet<MigrationStatus>(`${API_BASE}/migrations/${encodeURIComponent(id)}`)
}

export async function cancelMigration(id: string): Promise<void> {
  await apiPostVoid(`${API_BASE}/migrations/${encodeURIComponent(id)}/cancel`)
}
