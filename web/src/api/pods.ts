// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { apiGet, getToken } from './client'

export interface PodContainer {
  name: string
  image: string
  init: boolean
  ready: boolean
  restarts: number
  state: string
}

export interface PodSummary {
  name: string
  namespace: string
  phase: string
  status: string
  ready: string
  restarts: number
  created: string | null
  age_seconds: number | null
  node: string | null
  pod_ip: string | null
  owner_kind: string | null
  containers: PodContainer[]
}

export async function listPods(namespace?: string): Promise<PodSummary[]> {
  const qs = namespace ? `?namespace=${encodeURIComponent(namespace)}` : ''
  const res = await apiGet<{ pods: PodSummary[] }>(`/api/v1/pods${qs}`)
  return res.pods ?? []
}

export async function listNamespaces(): Promise<string[]> {
  const res = await apiGet<{ namespaces: string[] }>('/api/v1/namespaces')
  return res.namespaces ?? []
}

/** Absolute ws(s):// URL for a `/ws/...` path, with the session token attached. */
export function wsUrl(path: string, params: Record<string, string | number | boolean | undefined> = {}): string {
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const qs = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== '') qs.set(k, String(v))
  }
  const token = getToken()
  if (token) qs.set('token', token)
  return `${proto}//${window.location.host}${path}?${qs.toString()}`
}

export function podLogsUrl(
  namespace: string,
  pod: string,
  opts: { container?: string; tail?: number; timestamps?: boolean; previous?: boolean },
): string {
  return wsUrl(`/ws/pods/${encodeURIComponent(namespace)}/${encodeURIComponent(pod)}/logs`, {
    container: opts.container,
    tail: opts.tail,
    timestamps: opts.timestamps ? 1 : 0,
    previous: opts.previous ? 1 : 0,
  })
}

export function podExecUrl(namespace: string, pod: string, opts: { container?: string; shell?: string }): string {
  return wsUrl(`/ws/pods/${encodeURIComponent(namespace)}/${encodeURIComponent(pod)}/exec`, {
    container: opts.container,
    shell: opts.shell,
  })
}
