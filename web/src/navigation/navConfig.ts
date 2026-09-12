// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import type { LucideIcon } from 'lucide-react'
import { Home, Server, Star, Camera, Plus, MonitorCog, ArrowRightLeft, Database, ShieldCheck, Gauge, GitCompare, UploadCloud, ListChecks, Radio, HeartPulse, Network, Save } from 'lucide-react'

export interface NavItem {
  label: string
  path: string
  icon: LucideIcon
  /** Hidden from the sidebar unless usePermissions().canAdmin is true --
   * the backend independently enforces this too, this is just UX so a
   * non-admin isn't shown a link that always 403s. */
  adminOnly?: boolean
}

export interface NavSection {
  label: string
  items: NavItem[]
}

export interface NavGroup {
  name: string
  compact: string
  barIcon: LucideIcon
  items?: NavItem[]
  sections?: NavSection[]
}

export function flattenNavGroup(g: NavGroup): NavItem[] {
  if (g.items?.length) return g.items
  if (g.sections?.length) return g.sections.flatMap((s) => s.items)
  return []
}

export function navDropdownSections(group: NavGroup): NavSection[] {
  if (group.items?.length) return [{ label: '', items: group.items }]
  return group.sections ?? []
}

export const TOP_BAR_QUICK_LINKS: NavItem[] = []

function dedupeNavPaths(items: NavItem[]): NavItem[] {
  const seen = new Set<string>()
  return items.filter((item) => {
    if (seen.has(item.path)) return false
    seen.add(item.path)
    return true
  })
}

export const NAV_GROUPS: NavGroup[] = [
  {
    name: 'Core',
    compact: 'Core',
    barIcon: Home,
    items: [
      { label: 'Dashboard', path: '/app', icon: Home },
      { label: 'Virtual Machines', path: '/app/vms', icon: Server },
      { label: 'Create VM', path: '/app/create', icon: Plus },
      { label: 'Favorites', path: '/app/favorites', icon: Star },
      { label: 'Snapshots', path: '/app/snapshots', icon: Camera },
      { label: 'Migrations', path: '/app/migrations', icon: ArrowRightLeft },
      { label: 'Migration Readiness', path: '/app/migrations/readiness', icon: ListChecks },
      { label: 'Event Stream', path: '/app/events', icon: Radio },
      { label: 'Health Check', path: '/app/health-check', icon: HeartPulse },
      { label: 'Service Map', path: '/app/service-map', icon: Network },
      { label: 'Backups', path: '/app/backups', icon: Save },
      { label: 'Storage', path: '/app/storage', icon: Database },
      { label: 'Quotas', path: '/app/quotas', icon: Gauge },
      { label: 'Compare', path: '/app/compare', icon: GitCompare },
      { label: 'Batch Import', path: '/app/batch-import', icon: UploadCloud },
      { label: 'Windows', path: '/app/windows', icon: MonitorCog },
      { label: 'Access Control', path: '/app/access-control', icon: ShieldCheck, adminOnly: true },
    ],
  },
]

export const ALL_NAV_ITEMS: NavItem[] = dedupeNavPaths([
  ...NAV_GROUPS.flatMap(flattenNavGroup),
  ...TOP_BAR_QUICK_LINKS,
])

export const PAGE_TITLE_BY_PATH: Record<string, string> = Object.fromEntries(
  ALL_NAV_ITEMS.map((item) => [item.path, item.label]),
)

export const routeLabels: Record<string, string> = {
  ...PAGE_TITLE_BY_PATH,
  '/app/create': 'Create VM',
}
