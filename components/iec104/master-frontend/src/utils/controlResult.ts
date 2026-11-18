import type { ControlResult } from '../types'

// A successful IPC send is not necessarily a positive protocol confirmation.
export function controlResultStatus(result: ControlResult) {
  const action = result.steps.at(-1)?.action
  const confirmedLabel = {
    select_confirmed: 'control.selectConfirmed',
    execute_confirmed: 'control.executeConfirmed',
    deactivate_confirmed: 'control.deactivateConfirmed',
  } as const
  if (action && action in confirmedLabel) {
    return {
      confirmed: true,
      label: confirmedLabel[action as keyof typeof confirmedLabel],
      duration: 'control.confirmationDuration' as const,
    }
  }
  const sent = action === 'select_sent' || action === 'execute_sent' || action === 'deactivate_sent'
  return {
    confirmed: false,
    label: sent ? 'control.sentUnconfirmed' as const : 'control.resultUnconfirmed' as const,
    duration: sent ? 'control.sendDuration' as const : 'control.elapsedDuration' as const,
  }
}
