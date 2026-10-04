// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { describe, expect, it } from 'vitest'
import { summariseDoctorOutput } from './guestRepair'

describe('summariseDoctorOutput', () => {
  it('reads a boot score and finding messages from GuestKit JSON', () => {
    const out = JSON.stringify({ boot_score: 42, blockers: [{ title: 'fstab references a missing disk' }], warnings: ['GRUB timeout is 0'] })
    const s = summariseDoctorOutput(out)
    expect(s.score).toBe(42)
    expect(s.findings).toEqual(['fstab references a missing disk', 'GRUB timeout is 0'])
  })
  it('finds JSON surrounded by log lines and clamps the score', () => {
    const s = summariseDoctorOutput('inspecting...\n{"score": 140, "issues": ["x"]}\ndone')
    expect(s.score).toBe(100)
    expect(s.findings).toEqual(['x'])
  })
  it('falls back to raw text when there is no JSON', () => {
    const s = summariseDoctorOutput('plain text report')
    expect(s.score).toBeNull()
    expect(s.findings).toEqual([])
    expect(s.raw).toBe('plain text report')
  })
  it('de-duplicates findings', () => {
    expect(summariseDoctorOutput(JSON.stringify({ blockers: ['a', 'a'], warnings: ['a'] })).findings).toEqual(['a'])
  })
})
