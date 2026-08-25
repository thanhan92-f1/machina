// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { BrowserRouter, Routes, Route, useNavigate, useLocation, Navigate, useSearchParams } from 'react-router'
import { MotionConfig } from 'framer-motion'
import { ZyvorFooter } from './components/ZyvorBrand';
import { Suspense, useState, useCallback, useMemo, useEffect } from 'react'
import { lazyWithRetry } from './utils/lazyWithRetry'
import AppErrorBoundary from './components/AppErrorBoundary'
import { ToastProvider, ToastRenderer } from './contexts/ToastContext'
import { WebSocketProvider } from './contexts/WebSocketContext'
import { AuthProvider, useAuth } from './contexts/AuthContext'
import { ThemeProvider, useTheme } from './contexts/ThemeContext'
import { PlatformInfoProvider } from './contexts/PlatformInfoContext'
import Navbar from './components/Navbar'
import ShellBridgeBar from './components/ShellBridgeBar'
import NotFound from './pages/NotFound'
import LoginPage from './pages/Login'
import CommandPalette from './components/CommandPalette'
import ZyraSpotlight from './components/ai/ZyraSpotlight'
import ZyraAssistant from './components/ai/ZyraAssistant'
import ZyraAmbientBar from './components/ai/ZyraAmbientBar'
import Breadcrumb from './components/Breadcrumb'
import { BreadcrumbNameProvider } from './contexts/BreadcrumbNameContext'
import HelpDialog, { type HelpTab } from './components/HelpDialog'
import { OPEN_HELP_EVENT } from './utils/openHelp'
import PageSkeleton from './components/PageSkeleton'
import { AiProvider } from './contexts/AiContext'
import { useSequenceShortcuts } from './hooks/useSequenceShortcut'
import { safePostLoginPath } from './api/authRedirect'
import { useKeyboardShortcut, isInputFocused } from './hooks/useKeyboardShortcut'
import { useRecordRecentPage } from './hooks/useRecordRecentPage'
import { usePlatformInfo } from './contexts/PlatformInfoContext'
import { routeLabels } from './utils/routes'

const Dashboard = lazyWithRetry(() => import('./pages/Dashboard'))
const VMList = lazyWithRetry(() => import('./pages/VMList'))
const VMDetails = lazyWithRetry(() => import('./pages/VMDetails'))
const CreateVM = lazyWithRetry(() => import('./pages/CreateVM'))
const Sprites = lazyWithRetry(() => import('./pages/Sprites'))
const Networks = lazyWithRetry(() => import('./pages/Networks'))
const Storage = lazyWithRetry(() => import('./pages/Storage'))
const Snapshots = lazyWithRetry(() => import('./pages/Snapshots'))
const NodeInfo = lazyWithRetry(() => import('./pages/NodeInfo'))
const Events = lazyWithRetry(() => import('./pages/Events'))
const ClassicConsoleHub = lazyWithRetry(() => import('./pages/ClassicConsoleHub'))
const ClassicConsoleRedirect = lazyWithRetry(() => import('./pages/ClassicConsoleRedirect'))
const Capabilities = lazyWithRetry(() => import('./pages/Capabilities'))
const Devices = lazyWithRetry(() => import('./pages/Devices'))
const NWFilters = lazyWithRetry(() => import('./pages/NWFilters'))
const Secrets = lazyWithRetry(() => import('./pages/Secrets'))
const Backups = lazyWithRetry(() => import('./pages/Backups'))
const HostNetworking = lazyWithRetry(() => import('./pages/HostNetworking'))
const AuditLog = lazyWithRetry(() => import('./pages/AuditLog'))
const SettingsPage = lazyWithRetry(() => import('./pages/Settings'))
const ImportVM = lazyWithRetry(() => import('./pages/ImportVM'))
const SSHPage = lazyWithRetry(() => import('./pages/SSHPage'))
const HostSSHPage = lazyWithRetry(() => import('./pages/HostSSHPage'))
const ApiDocs = lazyWithRetry(() => import('./pages/ApiDocs'))
const Services = lazyWithRetry(() => import('./pages/Services'))
const Logs = lazyWithRetry(() => import('./pages/Logs'))
const StoragePoolDetail = lazyWithRetry(() => import('./pages/StoragePoolDetail'))
const AdminSessions = lazyWithRetry(() => import('./pages/AdminSessions'))
const DiskImages = lazyWithRetry(() => import('./pages/DiskImages'))
const Jobs = lazyWithRetry(() => import('./pages/Jobs'))
const K8sOverview = lazyWithRetry(() => import('./pages/K8sOverview'))
const K8sWorkloads = lazyWithRetry(() => import('./pages/K8sWorkloads'))
const KataContainers = lazyWithRetry(() => import('./pages/KataContainers'))
const OpenStackOverview = lazyWithRetry(() => import('./pages/OpenStackOverview'))
const OpenStackInstances = lazyWithRetry(() => import('./pages/OpenStackInstances'))
const OpenStackInstanceDetail = lazyWithRetry(() => import('./pages/OpenStackInstanceDetail'))
const OpenStackCreateInstance = lazyWithRetry(() => import('./pages/OpenStackCreateInstance'))
const OpenStackImages = lazyWithRetry(() => import('./pages/OpenStackImages'))
const OpenStackMigrations = lazyWithRetry(() => import('./pages/OpenStackMigrations'))
const OpenStackSecurityGroups = lazyWithRetry(() => import('./pages/OpenStackSecurityGroups'))
const OpenStackConsole = lazyWithRetry(() => import('./pages/OpenStackConsole'))
const OpenStackVolumes = lazyWithRetry(() => import('./pages/OpenStackVolumes'))
const OpenStackNetworking = lazyWithRetry(() => import('./pages/OpenStackNetworking'))
const OpenStackKeypairs = lazyWithRetry(() => import('./pages/OpenStackKeypairs'))
const OpenStackFlavors = lazyWithRetry(() => import('./pages/OpenStackFlavors'))
const OpenStackServerGroups = lazyWithRetry(() => import('./pages/OpenStackServerGroups'))
const OpenStackImageDetail = lazyWithRetry(() => import('./pages/OpenStackImageDetail'))
const OpenStackVolumeDetail = lazyWithRetry(() => import('./pages/OpenStackVolumeDetail'))
const OpenStackFloatingIps = lazyWithRetry(() => import('./pages/OpenStackFloatingIps'))
const OpenStackVolumeSnapshots = lazyWithRetry(() => import('./pages/OpenStackVolumeSnapshots'))
const OpenStackNetworkDetail = lazyWithRetry(() => import('./pages/OpenStackNetworkDetail'))
const OpenStackSubnetDetail = lazyWithRetry(() => import('./pages/OpenStackSubnetDetail'))
const OpenStackRouterDetail = lazyWithRetry(() => import('./pages/OpenStackRouterDetail'))
const OpenStackPortDetail = lazyWithRetry(() => import('./pages/OpenStackPortDetail'))
const OpenStackFlavorDetail = lazyWithRetry(() => import('./pages/OpenStackFlavorDetail'))
const OpenStackHypervisorDetail = lazyWithRetry(() => import('./pages/OpenStackHypervisorDetail'))
const OpenStackServerGroupDetail = lazyWithRetry(() => import('./pages/OpenStackServerGroupDetail'))
const OpenStackVolumeTransferDetail = lazyWithRetry(() => import('./pages/OpenStackVolumeTransferDetail'))
const OpenStackInstanceInterfaces = lazyWithRetry(() => import('./pages/OpenStackInstanceInterfaces'))
const OpenStackSecurityGroupDetail = lazyWithRetry(() => import('./pages/OpenStackSecurityGroupDetail'))
const OpenStackFloatingIpDetail = lazyWithRetry(() => import('./pages/OpenStackFloatingIpDetail'))
const OpenStackVolumeSnapshotDetail = lazyWithRetry(() => import('./pages/OpenStackVolumeSnapshotDetail'))
const OpenStackHeat = lazyWithRetry(() => import('./pages/OpenStackHeat'))
const OpenStackHeatDetail = lazyWithRetry(() => import('./pages/OpenStackHeatDetail'))
const OpenStackLoadBalancers = lazyWithRetry(() => import('./pages/OpenStackLoadBalancers'))
const OpenStackLoadBalancerDetail = lazyWithRetry(() => import('./pages/OpenStackLoadBalancerDetail'))
const OpenStackIdentity = lazyWithRetry(() => import('./pages/OpenStackIdentity'))
const OpenStackIdentityProjectDetail = lazyWithRetry(() => import('./pages/OpenStackIdentityProjectDetail'))
const OpenStackIdentityUserDetail = lazyWithRetry(() => import('./pages/OpenStackIdentityUserDetail'))
const OpenStackTopology = lazyWithRetry(() => import('./pages/OpenStackTopology'))
const Fleet = lazyWithRetry(() => import('./pages/Fleet'))
const PlatformLayout = lazyWithRetry(() => import('./layouts/PlatformLayout'))
const PlatformDashboard = lazyWithRetry(() => import('./pages/platform/PlatformDashboard'))
const PlatformHosts = lazyWithRetry(() => import('./pages/platform/PlatformHosts'))
const PlatformMachineFinder = lazyWithRetry(() => import('./pages/platform/PlatformMachineFinder'))
const PlatformGpuCommandCenter = lazyWithRetry(() => import('./pages/platform/PlatformGpuCommandCenter'))
const PlatformVms = lazyWithRetry(() => import('./pages/platform/PlatformVms'))
const PlatformVmDetail = lazyWithRetry(() => import('./pages/platform/PlatformVmDetail'))
const PlatformConsoleHub = lazyWithRetry(() => import('./pages/platform/PlatformConsoleHub'))
const MissionControlLiveWall = lazyWithRetry(() => import('./pages/platform/MissionControlLiveWall'))
const PlatformConsoleRedirect = lazyWithRetry(() => import('./pages/platform/PlatformConsoleRedirect'))
const PlatformContent = lazyWithRetry(() => import('./pages/platform/PlatformContent'))
const PlatformIsoCreate = lazyWithRetry(() => import('./pages/platform/PlatformIsoCreate'))
const PlatformVirtInstallCreate = lazyWithRetry(() => import('./pages/platform/PlatformVirtInstallCreate'))
const PlatformVmBuilder = lazyWithRetry(() => import('./pages/platform/PlatformVmBuilder'))
const PlatformTemplates = lazyWithRetry(() => import('./pages/platform/PlatformTemplates'))
const CloudInitStudio = lazyWithRetry(() => import('./pages/platform/CloudInitStudio'))
const PlatformNetworkCanvas = lazyWithRetry(() => import('./pages/platform/PlatformNetworkCanvas'))
const PlatformDatacenter = lazyWithRetry(() => import('./pages/platform/PlatformDatacenter'))
const PlatformFleetSnapshots = lazyWithRetry(() => import('./pages/platform/PlatformFleetSnapshots'))
const PlatformAlertRules = lazyWithRetry(() => import('./pages/platform/PlatformAlertRules'))
const PlatformScheduledJobs = lazyWithRetry(() => import('./pages/platform/PlatformScheduledJobs'))
const PlatformEnroll = lazyWithRetry(() => import('./pages/platform/PlatformEnroll'))
const PlatformPlacement = lazyWithRetry(() => import('./pages/platform/PlatformPlacement'))
const PlatformTasks = lazyWithRetry(() => import('./pages/platform/PlatformTasks'))
const PlatformEvents = lazyWithRetry(() => import('./pages/platform/PlatformEvents'))
const PlatformStorage = lazyWithRetry(() => import('./pages/platform/PlatformStorage'))
const PlatformAtlasStorage = lazyWithRetry(() => import('./pages/platform/PlatformAtlasStorage'))
const PlatformNetworks = lazyWithRetry(() => import('./pages/platform/PlatformNetworks'))
const PlatformHostDetail = lazyWithRetry(() => import('./pages/platform/PlatformHostDetail'))
const PlatformUsers = lazyWithRetry(() => import('./pages/platform/PlatformUsers'))
const PlatformWebhooks = lazyWithRetry(() => import('./pages/platform/PlatformWebhooks'))
const PlatformReports = lazyWithRetry(() => import('./pages/platform/PlatformReports'))
const PlatformApiKeys = lazyWithRetry(() => import('./pages/platform/PlatformApiKeys'))
const PlatformMaintenance = lazyWithRetry(() => import('./pages/platform/PlatformMaintenance'))
const PlatformProjects = lazyWithRetry(() => import('./pages/platform/PlatformProjects'))
const PlatformNotifications = lazyWithRetry(() => import('./pages/platform/PlatformNotifications'))
const PlatformResourcesHub = lazyWithRetry(() => import('./pages/platform/PlatformResourcesHub'))
const PlatformInfrastructureHub = lazyWithRetry(() => import('./pages/platform/PlatformInfrastructureHub'))
const PlatformWorkloadsHub = lazyWithRetry(() => import('./pages/platform/PlatformWorkloadsHub'))
const PlatformAdministrationHub = lazyWithRetry(() => import('./pages/platform/PlatformAdministrationHub'))
const PlatformOperationsHub = lazyWithRetry(() => import('./pages/platform/PlatformOperationsHub'))
const PlatformSettingsHub = lazyWithRetry(() => import('./pages/platform/PlatformSettingsHub'))
const PlatformNotFound = lazyWithRetry(() => import('./pages/platform/PlatformNotFound'))
const PlatformMigration = lazyWithRetry(() => import('./pages/platform/PlatformMigration'))
const PlatformActivityMonitor = lazyWithRetry(() => import('./pages/platform/PlatformActivityMonitor'))
const PlatformRecommendations = lazyWithRetry(() => import('./pages/platform/PlatformRecommendations'))
const PlatformApplications = lazyWithRetry(() => import('./pages/platform/PlatformApplications'))
const PlatformLaunchpad = lazyWithRetry(() => import('./pages/platform/PlatformLaunchpad'))
const PlatformLaunchpadAppDetail = lazyWithRetry(() => import('./pages/platform/PlatformLaunchpadAppDetail'))
const PlatformLaunchpadSpace = lazyWithRetry(() => import('./pages/platform/PlatformLaunchpadSpace'))
const PlatformBackups = lazyWithRetry(() => import('./pages/platform/PlatformBackups'))
const PlatformTopology = lazyWithRetry(() => import('./pages/platform/PlatformTopology'))
const PlatformZyraOs = lazyWithRetry(() => import('./pages/platform/PlatformZyraOs'))
const PlatformZyraSettings = lazyWithRetry(() => import('./pages/platform/PlatformZyraSettings'))
const PlatformAiProviders = lazyWithRetry(() => import('./pages/platform/PlatformAiProviders'))
const PlatformHa = lazyWithRetry(() => import('./pages/platform/PlatformHa'))
const PlatformBareMetal = lazyWithRetry(() => import('./pages/platform/PlatformBareMetal'))
const PlatformStorageTiers = lazyWithRetry(() => import('./pages/platform/PlatformStorageTiers'))
const PlatformMarketplace = lazyWithRetry(() => import('./pages/platform/PlatformMarketplace'))
const PlatformUpgrade = lazyWithRetry(() => import('./pages/platform/PlatformUpgrade'))
const PlatformRightsizing = lazyWithRetry(() => import('./pages/platform/PlatformRightsizing'))
const PlatformIncidentCommander = lazyWithRetry(() => import('./pages/platform/PlatformIncidentCommander'))
const PlatformZyraApprovals = lazyWithRetry(() => import('./pages/platform/PlatformZyraApprovals'))
const PlatformSecurityCenter = lazyWithRetry(() => import('./pages/platform/PlatformSecurityCenter'))
const PlatformSoc = lazyWithRetry(() => import('./pages/platform/PlatformSoc'))
const PlatformMachineSecurity = lazyWithRetry(() => import('./pages/platform/PlatformMachineSecurity'))
const PlatformThreatHunting = lazyWithRetry(() => import('./pages/platform/PlatformThreatHunting'))
const PlatformRuntimeEnforcement = lazyWithRetry(() => import('./pages/platform/PlatformRuntimeEnforcement'))
const PlatformFirewallOverview = lazyWithRetry(() => import('./pages/platform/security/PlatformFirewallOverview'))
const PlatformFirewallTargetDetail = lazyWithRetry(() => import('./pages/platform/security/PlatformFirewallTargetDetail'))
const PlatformFirewallPorts = lazyWithRetry(() => import('./pages/platform/security/PlatformFirewallPorts'))
const PlatformFirewallServices = lazyWithRetry(() => import('./pages/platform/security/PlatformFirewallServices'))
const PlatformFirewallActivity = lazyWithRetry(() => import('./pages/platform/security/PlatformFirewallActivity'))
const PlatformFirewallCompliance = lazyWithRetry(() => import('./pages/platform/security/PlatformFirewallCompliance'))
const PlatformFirewallK8s = lazyWithRetry(() => import('./pages/platform/security/PlatformFirewallK8s'))
const PlatformFirewallCloud = lazyWithRetry(() => import('./pages/platform/security/PlatformFirewallCloud'))
const PlatformFirewallConnectivity = lazyWithRetry(() => import('./pages/platform/security/PlatformFirewallConnectivity'))
const PlatformFirewallPolicies = lazyWithRetry(() => import('./pages/platform/security/PlatformFirewallPolicies'))
const PlatformPolicy = lazyWithRetry(() => import('./pages/platform/PlatformPolicy'))
const PlatformIntegrations = lazyWithRetry(() => import('./pages/platform/PlatformIntegrations'))
const PlatformBlueprints = lazyWithRetry(() => import('./pages/platform/PlatformBlueprints'))
const PlatformSupport = lazyWithRetry(() => import('./pages/platform/PlatformSupport'))
const PlatformDeveloper = lazyWithRetry(() => import('./pages/platform/PlatformDeveloper'))
const PlatformObservability = lazyWithRetry(() => import('./pages/platform/PlatformObservability'))
const PlatformEnterprise = lazyWithRetry(() => import('./pages/platform/PlatformEnterprise'))
const RdpConsole = lazyWithRetry(() => import('./pages/RdpConsole'))
const MissionControl = lazyWithRetry(() => import('./pages/MissionControl'))
const SystemCheck = lazyWithRetry(() => import('./pages/SystemCheck'))

function AppZyvorFooter() {
  const { info } = usePlatformInfo()
  const hostOs = info?.host?.os_pretty_name?.trim()
  return <ZyvorFooter hostOs={hostOs || undefined} />
}

function GlobalShortcuts({
  helpOpen,
  helpTab,
  onOpenHelp,
  onCloseHelp,
  onHelpTabChange,
}: {
  helpOpen: boolean
  helpTab: HelpTab
  onOpenHelp: (tab?: HelpTab) => void
  onCloseHelp: () => void
  onHelpTabChange: (tab: HelpTab) => void
}) {
  const navigate = useNavigate()
  const location = useLocation()
  const shortcuts = useMemo(() => {
    const base: { sequence: [string, string]; handler: () => void }[] = [
      { sequence: ['g', 'd'], handler: () => navigate('/') },
      { sequence: ['g', 'v'], handler: () => navigate('/vms') },
      { sequence: ['g', 'n'], handler: () => navigate('/networks') },
      { sequence: ['g', 's'], handler: () => navigate('/storage') },
      { sequence: ['g', 'c'], handler: () => navigate('/create') },
      { sequence: ['g', 'e'], handler: () => navigate('/events') },
      { sequence: ['g', 'j'], handler: () => navigate('/jobs') },
      { sequence: ['g', 'b'], handler: () => navigate('/backups') },
      { sequence: ['g', 'i'], handler: () => navigate('/disk-images') },
      { sequence: ['g', 'k'], handler: () => navigate('/k8s/workloads') },
      { sequence: ['g', 'o'], handler: () => navigate('/openstack') },
    ]
    return base
  }, [navigate])

  useSequenceShortcuts(shortcuts)

  const toggleHelp = useCallback(
    (e: KeyboardEvent) => {
      if (isInputFocused()) return
      e.preventDefault()
      if (helpOpen) onCloseHelp()
      else onOpenHelp('shortcuts')
    },
    [helpOpen, onCloseHelp, onOpenHelp],
  )

  useKeyboardShortcut({ key: '?', handler: toggleHelp })
  useEffect(() => {
    const onOpen = (e: Event) => {
      const tab = (e as CustomEvent<{ tab?: HelpTab }>).detail?.tab ?? 'shortcuts'
      onOpenHelp(tab)
    }
    window.addEventListener(OPEN_HELP_EVENT, onOpen)
    return () => window.removeEventListener(OPEN_HELP_EVENT, onOpen)
  }, [onOpenHelp])
  useKeyboardShortcut({
    key: 'F3',
    handler: (e) => {
      if (isInputFocused()) return
      e.preventDefault()
      if (location.pathname.startsWith('/platform')) {
        window.dispatchEvent(new CustomEvent('machina-open-mission-control'))
      } else {
        navigate('/platform?mission=1')
      }
    },
  })

  return (
    <HelpDialog open={helpOpen} tab={helpTab} onClose={onCloseHelp} onTabChange={onHelpTabChange} />
  )
}

function AuthenticatedApp() {
  const { isAuthenticated, loading } = useAuth()

  if (loading) {
    return (
      <div className="min-h-screen dashboard-liquid-glass liquid-glass-app flex items-center justify-center">
        <div className="glass glass-elevated p-8 flex items-center justify-center">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500" />
        </div>
      </div>
    )
  }

  if (!isAuthenticated) {
    return <LoginPage />
  }

  return <AuthenticatedShell />
}

function RouteRecorder() {
  useRecordRecentPage()
  return null
}

/** When the platform control plane is active, land on the macOS desktop (dock + menubar). */
function HomeRoute() {
  const { info, loading } = usePlatformInfo()
  if (!loading && info?.control_plane?.proxy_url) {
    return <Navigate to="/platform" replace />
  }
  return <Dashboard />
}

/** Authenticated visit to `/login?next=…` must honor `next` (not drop to `/` → Mission Control). */
function PostLoginRedirect() {
  const [params] = useSearchParams()
  const dest = safePostLoginPath(params.get('next'), '/')
  return <Navigate to={dest} replace />
}

function AuthenticatedShell() {
  return (
    <WebSocketProvider>
      <PlatformInfoProvider>
        <BrowserRouter>
          <BreadcrumbNameProvider>
            <AiProvider>
              <AuthenticatedShellRoutes />
            </AiProvider>
          </BreadcrumbNameProvider>
        </BrowserRouter>
      </PlatformInfoProvider>
    </WebSocketProvider>
  )
}

function AuthenticatedShellRoutes() {
  const { theme } = useTheme()
  const location = useLocation()
  const isPlatformRoute = location.pathname.startsWith('/platform')
  const [helpOpen, setHelpOpen] = useState(false)
  const [helpTab, setHelpTab] = useState<HelpTab>('shortcuts')

  useEffect(() => {
    const label = routeLabels[location.pathname] ?? routeLabels[location.pathname.replace(/\/[^/]+$/, '/:id')]
    document.title = label ? `${label} — Machina` : 'Machina'
  }, [location.pathname])

  const openHelp = useCallback((tab: HelpTab = 'shortcuts') => {
    setHelpTab(tab)
    setHelpOpen(true)
  }, [])

  const closeHelp = useCallback(() => setHelpOpen(false), [])

  useEffect(() => {
    setHelpOpen(false)
  }, [location.pathname, location.search])

  // Reset scroll to the top on navigation between pages (but honor in-page #hash
  // anchors). Without this, opening a detail page lands mid-page at the previous
  // list's scroll offset.
  useEffect(() => {
    if (location.hash) return
    window.scrollTo(0, 0)
    document.getElementById('main-content')?.scrollTo?.(0, 0)
  }, [location.pathname])

  const shellClass =
    theme === 'steel'
      ? 'dashboard-steel min-h-screen flex flex-col text-[#d7dde5]'
      : theme === 'aurora'
        ? 'dashboard-aurora min-h-screen flex flex-col text-[#e8e4f8]'
        : 'dashboard-liquid-glass liquid-glass-app min-h-screen flex flex-col text-[var(--text-primary)]'

  return (
    <>
          <RouteRecorder />
          <ToastRenderer />
          <div className={`${shellClass} flex flex-col min-h-dvh`}>
            <a
              href="#main-content"
              className="sr-only focus:not-sr-only focus:absolute focus:top-4 focus:left-4 focus:z-[9999] focus:px-4 focus:py-2 focus:bg-slate-900 focus:text-white focus:rounded-lg focus:shadow-lg focus:outline-none"
            >
              Skip to main content
            </a>
            {!isPlatformRoute && <Navbar onOpenHelp={openHelp} />}
            {!isPlatformRoute && <ShellBridgeBar />}
            <ZyraSpotlight onOpenHelp={openHelp} />
            <ZyraAssistant />
            <ZyraAmbientBar />
            <GlobalShortcuts
              helpOpen={helpOpen}
              helpTab={helpTab}
              onOpenHelp={openHelp}
              onCloseHelp={closeHelp}
              onHelpTabChange={setHelpTab}
            />
            <main
              id="main-content"
              className={
                isPlatformRoute
                  ? 'platform-route-main flex-1 min-w-0 flex flex-col w-full'
                  : `app-shell tahoe-page-root platform-readable flex-1 min-w-0 py-6 lg:py-8${theme === 'steel' ? ' steel-content' : ''}${theme === 'aurora' ? ' aurora-content' : ''}`
              }
            >
              {!isPlatformRoute && <Breadcrumb />}
              <AppErrorBoundary resetKey={location.pathname}>
              <Suspense fallback={<PageSkeleton />}>
                <Routes>
                <Route path="/login" element={<PostLoginRedirect />} />
                <Route path="/" element={<HomeRoute />} />
                <Route path="/vms" element={<VMList />} />
                <Route path="/vms/:name" element={<VMDetails />} />
                <Route path="/vms/:name/console" element={<ClassicConsoleRedirect />} />
                <Route path="/vms/:name/consolehub" element={<ClassicConsoleHub />} />
                <Route path="/vms/:name/rdp" element={<RdpConsole />} />
                <Route path="/fleet" element={<Fleet />} />
                <Route path="/platform" element={<PlatformLayout />}>
                  <Route index element={<PlatformDashboard />} />
                  <Route path="hosts" element={<PlatformHosts />} />
                  <Route path="hosts/finder" element={<PlatformMachineFinder />} />
                  <Route path="hosts/:id" element={<PlatformHostDetail />} />
                  <Route path="vms" element={<PlatformVms />} />
                  <Route path="vms/:id" element={<PlatformVmDetail />} />
                  <Route path="vms/:id/consolehub" element={<PlatformConsoleHub />} />
                  <Route path="mission-control/live" element={<MissionControlLiveWall />} />
                  <Route path="vms/:id/console" element={<PlatformConsoleRedirect />} />
                  <Route path="applications" element={<PlatformApplications />} />
                  <Route path="launchpad/apps/:id" element={<PlatformLaunchpadAppDetail />} />
                  <Route path="launchpad/spaces/:spaceId" element={<PlatformLaunchpadSpace />} />
                  <Route path="launchpad" element={<PlatformLaunchpad />} />
                  <Route path="content" element={<PlatformContent />} />
                  <Route path="create-iso" element={<PlatformIsoCreate />} />
                  <Route path="create-advanced" element={<PlatformVirtInstallCreate />} />
                  <Route path="vm-builder" element={<PlatformVmBuilder />} />
                  <Route path="templates" element={<PlatformTemplates />} />
                  <Route path="cloud-init" element={<CloudInitStudio />} />
                  <Route path="network-canvas" element={<PlatformNetworkCanvas />} />
                  <Route path="datacenter" element={<PlatformDatacenter />} />
                  <Route path="migration" element={<PlatformMigration />} />
                  <Route path="backups" element={<PlatformBackups />} />
                  <Route path="fleet-snapshots" element={<PlatformFleetSnapshots />} />
                  <Route path="alert-rules" element={<PlatformAlertRules />} />
                  <Route path="scheduled-jobs" element={<PlatformScheduledJobs />} />
                  <Route path="enroll" element={<PlatformEnroll />} />
                  <Route path="placement" element={<PlatformPlacement />} />
                  <Route path="tasks" element={<PlatformTasks />} />
                  <Route path="events" element={<PlatformEvents />} />
                  <Route path="activity" element={<PlatformActivityMonitor />} />
                  <Route path="recommendations" element={<PlatformRecommendations />} />
                  <Route path="topology" element={<PlatformTopology />} />
                  <Route path="developer" element={<PlatformDeveloper />} />
                  <Route path="observability" element={<PlatformObservability />} />
                  <Route path="enterprise" element={<PlatformEnterprise />} />
                  <Route path="zyra" element={<PlatformZyraOs />} />
                  <Route path="zyra/configure" element={<PlatformZyraSettings />} />
                  <Route path="ai-providers" element={<PlatformAiProviders />} />
                  <Route path="ha" element={<PlatformHa />} />
                  <Route path="baremetal" element={<PlatformBareMetal />} />
                  <Route path="storage-tiers" element={<PlatformStorageTiers />} />
                  <Route path="marketplace" element={<PlatformMarketplace />} />
                  <Route path="upgrade" element={<PlatformUpgrade />} />
                  <Route path="zyra/rightsizing" element={<PlatformRightsizing />} />
                  <Route path="zyra/incidents" element={<PlatformIncidentCommander />} />
                  <Route path="zyra/approvals" element={<PlatformZyraApprovals />} />
                  <Route path="zyra/security/hunt" element={<PlatformThreatHunting />} />
                  <Route path="zyra/security/enforcement" element={<PlatformRuntimeEnforcement />} />
                  <Route path="soc" element={<PlatformSoc />} />
                  <Route path="zyra/security" element={<PlatformSecurityCenter />} />
                  <Route path="zyra/machines/:hostId" element={<PlatformMachineSecurity />} />
                  <Route path="zeus/security/firewall" element={<PlatformFirewallOverview />} />
                  <Route path="zeus/security/firewall/:id" element={<PlatformFirewallTargetDetail />} />
                  <Route path="zeus/security/ports" element={<PlatformFirewallPorts />} />
                  <Route path="zeus/security/services" element={<PlatformFirewallServices />} />
                  <Route path="zeus/security/activity" element={<PlatformFirewallActivity />} />
                  <Route path="zeus/security/compliance" element={<PlatformFirewallCompliance />} />
                  <Route path="zeus/security/k8s" element={<PlatformFirewallK8s />} />
                  <Route path="zeus/security/cloud" element={<PlatformFirewallCloud />} />
                  <Route path="zeus/security/connectivity" element={<PlatformFirewallConnectivity />} />
                  <Route path="zeus/security/policies" element={<PlatformFirewallPolicies />} />
                  <Route path="policy" element={<PlatformPolicy />} />
                  <Route path="integrations" element={<PlatformIntegrations />} />
                  <Route path="infrastructure" element={<PlatformInfrastructureHub />} />
                  <Route path="workloads" element={<PlatformWorkloadsHub />} />
                  <Route path="administration" element={<PlatformAdministrationHub />} />
                  <Route path="resources" element={<PlatformResourcesHub />} />
                  <Route path="operations" element={<PlatformOperationsHub />} />
                  <Route path="blueprints" element={<PlatformBlueprints />} />
                  <Route path="support" element={<PlatformSupport />} />
                  <Route path="storage" element={<PlatformStorage />} />
                  <Route path="storage-atlas" element={<PlatformAtlasStorage />} />
                  <Route path="gpu" element={<PlatformGpuCommandCenter />} />
                  <Route path="networks" element={<PlatformNetworks />} />
                  <Route path="users" element={<PlatformUsers />} />
                  <Route path="webhooks" element={<PlatformWebhooks />} />
                  <Route path="reports" element={<PlatformReports />} />
                  <Route path="api-keys" element={<PlatformApiKeys />} />
                  <Route path="maintenance" element={<PlatformMaintenance />} />
                  <Route path="projects" element={<PlatformProjects />} />
                  <Route path="notifications" element={<PlatformNotifications />} />
                  <Route path="settings" element={<PlatformSettingsHub />} />
                  <Route path="*" element={<PlatformNotFound />} />
                </Route>
                <Route path="/create" element={<CreateVM />} />
                <Route path="/jobs/:jobId" element={<Jobs />} />
                <Route path="/jobs" element={<Jobs />} />
                <Route path="/k8s" element={<K8sOverview />} />
                <Route path="/k8s/workloads" element={<K8sWorkloads />} />
                <Route path="/k8s/kata" element={<KataContainers />} />
                <Route path="/openstack" element={<OpenStackOverview />} />
                <Route path="/openstack/instances" element={<OpenStackInstances />} />
                <Route path="/openstack/instances/:id" element={<OpenStackInstanceDetail />} />
                <Route path="/openstack/instances/:id/interfaces" element={<OpenStackInstanceInterfaces />} />
                <Route path="/openstack/create" element={<OpenStackCreateInstance />} />
                <Route path="/openstack/images" element={<OpenStackImages />} />
                <Route path="/openstack/images/:id" element={<OpenStackImageDetail />} />
                <Route path="/openstack/migrations" element={<OpenStackMigrations />} />
                <Route path="/openstack/security-groups" element={<OpenStackSecurityGroups />} />
                <Route path="/openstack/security-groups/:id" element={<OpenStackSecurityGroupDetail />} />
                <Route path="/openstack/volumes" element={<OpenStackVolumes />} />
                <Route path="/openstack/volumes/:id" element={<OpenStackVolumeDetail />} />
                <Route path="/openstack/floating-ips" element={<OpenStackFloatingIps />} />
                <Route path="/openstack/floating-ips/:id" element={<OpenStackFloatingIpDetail />} />
                <Route path="/openstack/networking" element={<OpenStackNetworking />} />
                <Route path="/openstack/keypairs" element={<OpenStackKeypairs />} />
                <Route path="/openstack/flavors" element={<OpenStackFlavors />} />
                <Route path="/openstack/flavors/:id" element={<OpenStackFlavorDetail />} />
                <Route path="/openstack/hypervisors/:id" element={<OpenStackHypervisorDetail />} />
                <Route path="/openstack/server-groups" element={<OpenStackServerGroups />} />
                <Route path="/openstack/server-groups/:id" element={<OpenStackServerGroupDetail />} />
                <Route path="/openstack/networks/:id" element={<OpenStackNetworkDetail />} />
                <Route path="/openstack/subnets/:id" element={<OpenStackSubnetDetail />} />
                <Route path="/openstack/routers/:id" element={<OpenStackRouterDetail />} />
                <Route path="/openstack/ports/:id" element={<OpenStackPortDetail />} />
                <Route path="/openstack/volume-transfers/:id" element={<OpenStackVolumeTransferDetail />} />
                <Route path="/openstack/volume-snapshots" element={<OpenStackVolumeSnapshots />} />
                <Route path="/openstack/volume-snapshots/:id" element={<OpenStackVolumeSnapshotDetail />} />
                <Route path="/openstack/heat" element={<OpenStackHeat />} />
                <Route path="/openstack/heat/:name/:id" element={<OpenStackHeatDetail />} />
                <Route path="/openstack/load-balancers" element={<OpenStackLoadBalancers />} />
                <Route path="/openstack/load-balancers/:id" element={<OpenStackLoadBalancerDetail />} />
                <Route path="/openstack/identity" element={<OpenStackIdentity />} />
                <Route path="/openstack/identity/projects/:id" element={<OpenStackIdentityProjectDetail />} />
                <Route path="/openstack/identity/users/:id" element={<OpenStackIdentityUserDetail />} />
                <Route path="/openstack/topology" element={<OpenStackTopology />} />
                <Route path="/openstack/instances/:id/console" element={<OpenStackConsole />} />
                <Route path="/networks" element={<Networks />} />
                <Route path="/storage" element={<Storage />} />
                <Route path="/storage/:pool" element={<StoragePoolDetail />} />
                <Route path="/disk-images" element={<DiskImages />} />
                <Route path="/snapshots" element={<Snapshots />} />
                <Route path="/node" element={<NodeInfo />} />
                <Route path="/events" element={<Events />} />
                <Route path="/capabilities" element={<Capabilities />} />
                <Route path="/devices" element={<Devices />} />
                <Route path="/nwfilters" element={<NWFilters />} />
                <Route path="/secrets" element={<Secrets />} />
                <Route path="/backups" element={<Backups />} />
                <Route path="/host-networking" element={<HostNetworking />} />
                <Route path="/host-ssh" element={<HostSSHPage />} />
                <Route path="/audit" element={<AuditLog />} />
                <Route path="/import" element={<ImportVM />} />
                <Route path="/sprites" element={<Sprites />} />
                <Route path="/ssh/:host" element={<SSHPage />} />
                <Route path="/ssh" element={<SSHPage />} />
                <Route path="/api-docs" element={<ApiDocs />} />
                <Route path="/services" element={<Services />} />
                <Route path="/mission-control" element={<MissionControl />} />
                <Route path="/system-check" element={<SystemCheck />} />
                <Route path="/logs" element={<Logs />} />
                <Route path="/settings" element={<SettingsPage />} />
                <Route path="/admin/sessions" element={<AdminSessions />} />
                <Route path="*" element={<NotFound />} />
              </Routes>
            </Suspense>
            </AppErrorBoundary>
          </main>
          {!isPlatformRoute && <AppZyvorFooter />}
        </div>
    </>
  )
}

function App() {
  return (
    <MotionConfig reducedMotion="user">
      <ThemeProvider>
        <ToastProvider>
          <AuthProvider>
            <AuthenticatedApp />
          </AuthProvider>
        </ToastProvider>
      </ThemeProvider>
    </MotionConfig>
  )
}

export default App
