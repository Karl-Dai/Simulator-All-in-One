export type Theme = 'system' | 'light' | 'dark'
export type Language = 'auto' | 'zh-CN' | 'en'
export type ResolvedLocale = Exclude<Language, 'auto'>

export interface AppearanceV1 {
  schemaVersion: 1
  theme: Theme
  language: Language
}

export interface AppearanceReader {
  readAppearance(): Promise<AppearanceV1>
}

export const DEFAULT_APPEARANCE: Readonly<AppearanceV1> = Object.freeze({
  schemaVersion: 1,
  theme: 'system',
  language: 'en',
})

export class AppearanceContractError extends Error {
  readonly code: 'invalid_response' | 'unsupported_version'

  constructor(code: 'invalid_response' | 'unsupported_version') {
    super(code === 'unsupported_version'
      ? 'Unsupported appearance interface version; repair the complete suite.'
      : 'The appearance adapter returned an invalid response.')
    this.name = 'AppearanceContractError'
    this.code = code
  }
}

/** Validate the native response without returning unrelated settings. */
export function parseAppearance(value: unknown): AppearanceV1 {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new AppearanceContractError('invalid_response')
  }
  const input = value as Record<string, unknown>
  if (input.schemaVersion !== 1) throw new AppearanceContractError('unsupported_version')
  return {
    schemaVersion: 1,
    theme: input.theme === 'light' || input.theme === 'dark' ? input.theme : 'system',
    language: input.language === 'auto' || input.language === 'zh-CN' ? input.language : 'en',
  }
}

export function resolveLanguage(language: Language, systemLanguage: string): ResolvedLocale {
  if (language !== 'auto') return language
  return systemLanguage.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en'
}
