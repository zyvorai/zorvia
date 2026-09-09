// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createKrytonMachine, listKrytonMachines } from '../kryton'

const storage = new Map<string, string>()
const localStorageMock = {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
}
Object.defineProperty(globalThis, 'localStorage', { value: localStorageMock, writable: true, configurable: true })

const fetchMock = vi.fn()
globalThis.fetch = fetchMock

function okJson(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

beforeEach(() => {
  storage.clear()
  fetchMock.mockReset()
  storage.set('zorvia_token', 'zorvia-jwt')
})

describe('Kryton API adapter', () => {
  it('scopes machine listing through the Zorvia adapter', async () => {
    fetchMock.mockResolvedValue(okJson({ items: [], nextCursor: 'next' }))

    const result = await listKrytonMachines('finance')

    expect(result.nextCursor).toBe('next')
    expect(fetchMock).toHaveBeenCalledTimes(1)
    const [url, init] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/kryton/machines?project=finance')
    expect((init.headers as Headers).get('Authorization')).toBe('Bearer zorvia-jwt')
  })

  it('sends the Kryton machine contract without any upstream token', async () => {
    fetchMock.mockResolvedValue(okJson({
      id: 'vm-uuid', project: 'finance', provider: 'kubevirt', state: 'provisioning',
      spec: { name: 'win11-01', image: 'windows-11-enterprise', compute: { cpu: 4, memoryMiB: 8192 }, disk: { sizeGiB: 80 } },
      providerRef: { provider: 'kubevirt', name: 'win11-01' }, createdAt: '2026-09-09T00:00:00Z', updatedAt: '2026-09-09T00:00:00Z',
    }, 201))

    await createKrytonMachine({
      project: 'finance', name: 'win11-01', image: 'windows-11-enterprise',
      compute: { cpu: 4, memoryMiB: 8192 }, disk: { sizeGiB: 80 }, ttlMinutes: 60,
    })

    const [url, init] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/kryton/machines')
    expect(init.method).toBe('POST')
    expect(JSON.parse(init.body as string)).toMatchObject({
      project: 'finance', compute: { memoryMiB: 8192 }, disk: { sizeGiB: 80 }, ttlMinutes: 60,
    })
    expect((init.headers as Headers).get('Authorization')).toBe('Bearer zorvia-jwt')
    expect(init.body).not.toContain('KRYTON_TOKEN')
  })
})
