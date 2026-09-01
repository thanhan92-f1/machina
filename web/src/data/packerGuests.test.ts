// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { describe, expect, it } from 'vitest'

import { MACHINA_PACKER_SCRIPT_GUESTS } from './packerGuests'

describe('MACHINA_PACKER_SCRIPT_GUESTS', () => {
  it('includes Windows dockur profiles win10 and win11', () => {
    const ids = MACHINA_PACKER_SCRIPT_GUESTS.map((g) => g.id)
    expect(ids).toContain('win10')
    expect(ids).toContain('win11')
  })

  it('marks Windows guests with windows family and os hints', () => {
    const win11 = MACHINA_PACKER_SCRIPT_GUESTS.find((g) => g.id === 'win11')
    const win10 = MACHINA_PACKER_SCRIPT_GUESTS.find((g) => g.id === 'win10')
    expect(win11?.family).toBe('windows')
    expect(win10?.family).toBe('windows')
    expect(win11?.osVariantHint).toBe('win11')
    expect(win10?.osVariantHint).toBe('win10')
  })
})
