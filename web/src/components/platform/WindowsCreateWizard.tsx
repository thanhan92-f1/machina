// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import SimpleCreateVmWizard, { sizeToSpec, type VmWizardPayload } from './SimpleCreateVmWizard'

interface WindowsCreateWizardProps {
  open: boolean
  onClose: () => void
  onCreate: (payload: VmWizardPayload) => Promise<void>
}

/** Opens the unified Create VM wizard with Windows defaults. */
export default function WindowsCreateWizard({ open, onClose, onCreate }: WindowsCreateWizardProps) {
  return (
    <SimpleCreateVmWizard
      open={open}
      onClose={onClose}
      onCreate={onCreate}
      initial={{ name: 'win11-01', os: 'win11', size: 'large', network: 'default' }}
    />
  )
}

export { sizeToSpec }
