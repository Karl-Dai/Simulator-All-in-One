import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import { buildSharedAliases } from '../shared-frontend/vite/aliases'

export default defineConfig({
  plugins: [vue()],
  test: { environment: 'jsdom', setupFiles: ['./tests/setup.ts'] },
  resolve: { alias: { '@': '/src', ...buildSharedAliases(import.meta.url) } },
})
