// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { apiGet, apiPost } from './client'

export interface PassthroughDevice {
  name: string
  device_name: string
  kind?: 'gpu' | 'host-device'
}

export interface DeviceNode {
  node: string
  /** Passthrough candidates: extended resources the device plugin advertises, with free counts. */
  devices: Record<string, number>
  /** KubeVirt's own pseudo-devices (kvm, tun, ...). */
  builtin: Record<string, number>
}

export interface PermittedDevice {
  resource_name: string
  kind: 'pci' | 'mediated' | 'usb'
  selector: string
  external: boolean
}

export interface DeviceInventory {
  nodes: DeviceNode[]
  /** Null when the KubeVirt CR could not be read. */
  kubevirt: { feature_gates: string[]; permitted: PermittedDevice[] } | null
  multus_installed: boolean
}

export interface DeviceIssue {
  severity: 'error' | 'warning'
  subject: string
  message: string
}

export interface DevicePreflight {
  ok: boolean
  issues: DeviceIssue[]
}

/** Cluster-admin only. */
export async function getDeviceInventory(): Promise<DeviceInventory> {
  return apiGet<DeviceInventory>('/api/v1/devices')
}

/** Would these devices / SR-IOV networks start? Read-only; VM creation runs the same check. */
export async function preflightDevices(
  devices: PassthroughDevice[],
  sriovNetworks: string[] = [],
  namespace?: string,
): Promise<DevicePreflight> {
  return apiPost<DevicePreflight>('/api/v1/devices/preflight', {
    devices,
    sriov_networks: sriovNetworks,
    ...(namespace ? { namespace } : {}),
  })
}

/** Resource names worth suggesting: advertised on a node and with units free. */
export function suggestedResources(inv: DeviceInventory): string[] {
  const names = new Set<string>()
  for (const n of inv.nodes) for (const [k, v] of Object.entries(n.devices)) if (v > 0) names.add(k)
  return [...names].sort()
}
