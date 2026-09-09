// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

import { BrowserRouter, Routes, Route, Navigate, useLocation } from 'react-router'
import { Suspense, lazy, ReactNode } from 'react'
import { ThemeProvider } from './contexts/ThemeContext'
import { ToastProvider } from './contexts/ToastContext'
import { WebSocketProvider } from './contexts/WebSocketContext'
import { AuthProvider, useAuth } from './contexts/AuthContext'
import { PlatformInfoProvider } from './contexts/PlatformInfoContext'
import PageSkeleton from './components/PageSkeleton'
import { PageErrorBoundary } from './components/ErrorBoundary'
import ConsoleLayout from './components/ConsoleLayout'
import SignIn from './pages/SignIn'
import NotFound from './pages/NotFound'
import Home from './pages/marketing/Home'
import Product from './pages/marketing/Product'
import Platform from './pages/marketing/Platform'
import SecurityPage from './pages/marketing/Security'

const Dashboard = lazy(() => import('./pages/Dashboard'))
const VMList = lazy(() => import('./pages/VMList'))
const VMDetails = lazy(() => import('./pages/VMDetails'))
const CreateVM = lazy(() => import('./pages/CreateVM'))
const Console = lazy(() => import('./pages/Console'))
const Snapshots = lazy(() => import('./pages/Snapshots'))
const FavoriteVMs = lazy(() => import('./pages/FavoriteVMs'))
const KrytonWindows = lazy(() => import('./pages/KrytonWindows'))

function ProtectedRoute({ children }: { children: ReactNode }) {
  const { isAuthenticated, loading } = useAuth()

  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[var(--zf-canvas)]">
        <div className="flex flex-col items-center gap-3">
          <div className="w-7 h-7 border-2 border-[var(--zf-ink)] border-t-transparent rounded-full animate-spin" />
          <span className="text-sm font-medium text-[var(--zf-muted)] tracking-wide">Loading…</span>
        </div>
      </div>
    )
  }

  if (!isAuthenticated) {
    return <Navigate to="/sign-in" replace />
  }

  return <>{children}</>
}

function ConsoleRoutes() {
  return (
    <ConsoleLayout>
      <PageErrorBoundary>
        <Suspense fallback={<PageSkeleton />}>
          <Routes>
            <Route index element={<Dashboard />} />
            <Route path="vms" element={<VMList />} />
            <Route path="vms/:name" element={<VMDetails />} />
            <Route path="vms/:name/console" element={<Console />} />
            <Route path="create" element={<CreateVM />} />
            <Route path="favorites" element={<FavoriteVMs />} />
            <Route path="snapshots" element={<Snapshots />} />
            <Route path="windows" element={<KrytonWindows />} />
            <Route path="*" element={<Navigate to="/app" replace />} />
          </Routes>
        </Suspense>
      </PageErrorBoundary>
    </ConsoleLayout>
  )
}

function LegacyRedirect() {
  const { pathname } = useLocation()
  if (pathname === '/' || pathname === '') return <Navigate to="/app" replace />
  if (pathname.startsWith('/app')) return <Navigate to="/app" replace />
  return <Navigate to={`/app${pathname}`} replace />
}

function AppRoutes() {
  return (
    <Routes>
      <Route path="/" element={<Home />} />
      <Route path="/product" element={<Product />} />
      <Route path="/platform" element={<Platform />} />
      <Route path="/security" element={<SecurityPage />} />
      <Route path="/sign-in" element={<SignIn />} />
      <Route path="/login" element={<Navigate to="/sign-in" replace />} />
      <Route
        path="/app/*"
        element={
          <ProtectedRoute>
            <ConsoleRoutes />
          </ProtectedRoute>
        }
      />
      <Route path="/vms/*" element={<LegacyRedirect />} />
      <Route path="/vms" element={<Navigate to="/app/vms" replace />} />
      <Route path="/create" element={<Navigate to="/app/create" replace />} />
      <Route path="/favorites" element={<Navigate to="/app/favorites" replace />} />
      <Route path="/snapshots" element={<Navigate to="/app/snapshots" replace />} />
      <Route path="*" element={<NotFound />} />
    </Routes>
  )
}

function App() {
  return (
    <ThemeProvider>
      <AuthProvider>
        <ToastProvider>
          <WebSocketProvider>
            <PlatformInfoProvider>
              <BrowserRouter>
                <AppRoutes />
              </BrowserRouter>
            </PlatformInfoProvider>
          </WebSocketProvider>
        </ToastProvider>
      </AuthProvider>
    </ThemeProvider>
  )
}

export default App
