type Translate = (key: string, params?: Record<string, unknown>) => string

interface CommandError {
  code?: string
  message?: string
  addr?: string
  os_error?: number
}

function commandError(error: unknown): CommandError | null {
  if (error && typeof error === 'object') return error as CommandError
  if (typeof error !== 'string') return null
  try {
    const parsed = JSON.parse(error)
    return parsed && typeof parsed === 'object' ? parsed as CommandError : null
  } catch {
    return null
  }
}

export function formatStartServerError(error: unknown, t: Translate): string {
  const parsed = commandError(error)
  if (!parsed?.code) return String(error)
  const params = {
    addr: parsed.addr ?? '-',
    osError: parsed.os_error ?? 0,
    message: parsed.message ?? String(error),
  }
  switch (parsed.code) {
    case 'addr_in_use': return t('errors.startBindInUse', params)
    case 'access_denied': return t('errors.startBindDenied', params)
    case 'addr_not_available': return t('errors.startBindUnavailable', params)
    default: return t('errors.startFailed', params)
  }
}
