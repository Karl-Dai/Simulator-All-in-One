import { createRequire } from 'node:module'
import { fileURLToPath, URL } from 'node:url'

// Resolve bare imports made from ui/ against the host frontend's node_modules,
// following the shared-frontend alias pattern used by iec104/opcua. The host
// passes its own import.meta.url so resolution matches a normal src import.
export function buildSimlabAliases(hostImportMetaUrl: string, options: { tauri?: boolean } = {}) {
  const require = createRequire(hostImportMetaUrl)
  const aliases: Record<string, string> = {
    'simlab-ui': fileURLToPath(new URL('../ui', import.meta.url)),
    vue: require.resolve('vue'),
  }
  if (options.tauri) {
    aliases['@tauri-apps/api/core'] = require.resolve('@tauri-apps/api/core')
  }
  return aliases
}