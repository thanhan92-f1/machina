// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

export function shellLabel(pathname: string): string | null {
  if (pathname.startsWith('/platform')) return null
  if (pathname.startsWith('/openstack')) return 'OpenStack'
  if (pathname.startsWith('/k8s')) return 'Kubernetes'
  if (pathname === '/login') return null
  return 'Classic Machina'
}
