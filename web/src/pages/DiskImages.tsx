// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { useState, useEffect, useMemo } from 'react'
import { Search, HardDrive } from 'lucide-react'
import { apiFetch } from '../api/client'
import ErrorBanner from '../components/ErrorBanner'
import { PageHeader, Card, CardBody, EmptyState, DataTable, type DataTableColumn } from '../components/ui'
import { SkeletonCard, SkeletonTable } from '../components/Skeleton'
import { formatUserError } from '../utils/apiError'
import { toastFailure } from '../utils/toastError'
import { hintsForError } from '../utils/daemonHints'
import { useToastContext } from '../contexts/ToastContext'

interface DiskImage { name: string; path: string; format: string; size_bytes: number; mod_time?: string }

function formatBytes(bytes: number): string {
  if (!bytes) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

const formatBadge = 'text-[var(--zf-muted)] bg-[var(--zf-canvas)] border-[var(--zf-hairline)]'

export default function DiskImages() {
  const toast = useToastContext()
  const [images, setImages] = useState<DiskImage[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [search, setSearch] = useState('')
  const [selected, setSelected] = useState<Set<string>>(new Set())

  const fetchImages = async () => {
    setLoading(true); setError(null)
    try {
      const res = await apiFetch('/api/images')
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()
      setImages(Array.isArray(data) ? data : data.images || data.disk_images || [])
    } catch (err: unknown) {
      const msg = formatUserError(err)
      setError(msg)
      toastFailure(toast, 'Failed to load disk images', err)
    } finally { setLoading(false) }
  }

  useEffect(() => { fetchImages() }, [])

  const filtered = useMemo(() => {
    if (!search) return images
    const q = search.toLowerCase()
    return images.filter(img => img.name.toLowerCase().includes(q) || img.format.toLowerCase().includes(q) || img.path.toLowerCase().includes(q))
  }, [images, search])

  const totalSize = images.reduce((sum, img) => sum + (img.size_bytes || 0), 0)
  const toggleSelect = (path: string) => setSelected(prev => { const next = new Set(prev); if (next.has(path)) next.delete(path); else next.add(path); return next })

  const imageColumns: DataTableColumn<DiskImage>[] = [
    { key: 'name', header: 'Name', render: (img) => <span className="text-[var(--zf-ink)] font-medium">{img.name}</span> },
    {
      key: 'format',
      header: 'Format',
      render: (img) => <span className={`px-2 py-0.5 rounded-full text-xs font-medium border ${formatBadge}`}>{img.format}</span>,
    },
    { key: 'size', header: 'Size', render: (img) => <span className="text-[var(--zf-ink)] font-mono text-xs">{formatBytes(img.size_bytes)}</span> },
    {
      key: 'path',
      header: 'Path',
      render: (img) => (
        <span className="text-[var(--zf-muted)] text-xs font-mono truncate block max-w-[250px]" title={img.path}>
          {img.path}
        </span>
      ),
    },
  ]

  if (loading) {
    return (
      <div className="space-y-6">
        <PageHeader title="Disk Images" description="Browse and manage VM disk images" />
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          {Array.from({ length: 4 }).map((_, i) => <SkeletonCard key={i} />)}
        </div>
        <SkeletonTable rows={5} cols={5} />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Disk Images"
        description="Browse and manage VM disk images"
        onRefresh={fetchImages}
        refreshing={loading}
      />

      {error && (
        <ErrorBanner
          title="Could not load disk images"
          headline={error}
          hints={hintsForError(error, 'storage')}
          onRetry={fetchImages}
        />
      )}

      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
        <Card><CardBody className="px-4 py-3"><div className="text-2xl font-bold text-[var(--zf-ink)]">{images.length}</div><div className="text-xs text-[var(--zf-muted)] mt-1">Total Images</div></CardBody></Card>
        <Card><CardBody className="px-4 py-3"><div className="text-2xl font-bold text-[var(--zf-ink)]">{formatBytes(totalSize)}</div><div className="text-xs text-[var(--zf-muted)] mt-1">Total Size</div></CardBody></Card>
        <Card><CardBody className="px-4 py-3"><div className="text-2xl font-bold text-[var(--zf-ink)]">{new Set(images.map(i => i.format)).size}</div><div className="text-xs text-[var(--zf-muted)] mt-1">Formats</div></CardBody></Card>
        <Card><CardBody className="px-4 py-3"><div className="text-2xl font-bold text-[var(--zf-ink)]">{selected.size}</div><div className="text-xs text-[var(--zf-muted)] mt-1">Selected</div></CardBody></Card>
      </div>

      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--zf-muted)]" />
        <input type="text" value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Search by name, format, or path..." aria-label="Search disk images"
          className="input-field pl-10" />
      </div>

      <Card className="overflow-hidden">
        <DataTable
          columns={imageColumns}
          rows={filtered}
          getRowKey={(img) => img.path}
          bordered={false}
          selectedKeys={selected}
          onToggleRow={toggleSelect}
          onToggleAll={() => {
            const allSelected = filtered.length > 0 && filtered.every((img) => selected.has(img.path))
            setSelected((prev) => {
              const next = new Set(prev)
              filtered.forEach((img) => (allSelected ? next.delete(img.path) : next.add(img.path)))
              return next
            })
          }}
          onRowClick={(img) => toggleSelect(img.path)}
          emptyState={
            <EmptyState
              icon={<HardDrive className="w-10 h-10" />}
              title={images.length === 0 ? 'No disk images found' : 'No images match your search'}
            />
          }
        />
      </Card>
    </div>
  )
}
