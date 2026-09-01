// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useMemo } from 'react'
import { useLocation, useNavigate } from 'react-router'
import { usePlatformDesktopTier } from '../../../hooks/usePlatformDesktopTier'
import { usePlatformInfo } from '../../../contexts/PlatformInfoContext'
import { integrationNavItems } from '../../../utils/platformIntegrationsNav'
import {
  menubarProductGroupsForTier,
  type MenubarProductGroup,
} from '../../../utils/platformMacMenus'
import { navItemActive } from '../../../utils/routes'
import PlatformMacMenuDropdown, { PlatformMacMenuItem } from './PlatformMacMenuDropdown'

interface PlatformProductNavMenusProps {
  openMenu: string | null
  onToggleMenu: (id: string) => void
  onCloseMenu: () => void
}

function menuIdForGroup(group: MenubarProductGroup): string {
  return `nav-${group.id}`
}

export default function PlatformProductNavMenus({
  openMenu,
  onToggleMenu,
  onCloseMenu,
}: PlatformProductNavMenusProps) {
  const location = useLocation()
  const navigate = useNavigate()
  const [tier] = usePlatformDesktopTier()
  const { info } = usePlatformInfo()
  const groups = useMemo(
    () => menubarProductGroupsForTier(tier, integrationNavItems(info)),
    [tier, info],
  )

  return (
    <div className="hidden md:flex items-center gap-1 shrink-0 min-w-0">
      {groups.map((group) => {
        const menuId = menuIdForGroup(group)
        const itemCount = group.sections.reduce((sum, section) => sum + section.items.length, 0)
        const scrollable = itemCount > 12

        return (
          <PlatformMacMenuDropdown
            key={group.id}
            label={group.compact}
            open={openMenu === menuId}
            onToggle={() => onToggleMenu(menuId)}
            onClose={onCloseMenu}
          >
            <div className={scrollable ? 'max-h-[70vh] overflow-y-auto' : undefined}>
              {group.sections.map((section, sectionIndex) => (
                <div key={section.label || `${group.id}-${sectionIndex}`}>
                  {section.label ? (
                    <p className="px-3.5 py-1 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">
                      {section.label}
                    </p>
                  ) : null}
                  {section.items.map((item) => (
                    <PlatformMacMenuItem
                      key={item.to}
                      label={item.label}
                      checked={navItemActive(
                        { to: item.to, label: item.label, icon: null },
                        location.pathname,
                        location.search,
                      )}
                      onClick={() => {
                        navigate(item.to)
                        onCloseMenu()
                      }}
                    />
                  ))}
                  {sectionIndex < group.sections.length - 1 ? (
                    <div className="my-1 h-px bg-[var(--apple-hairline)]" />
                  ) : null}
                </div>
              ))}
            </div>
          </PlatformMacMenuDropdown>
        )
      })}
    </div>
  )
}
