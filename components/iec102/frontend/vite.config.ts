import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { buildSimlabAliases } from '../../../scripts/simlab-vite'

export default defineConfig({
  resolve: { alias: buildSimlabAliases(import.meta.url, { tauri: true }) },
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 5184,
  },
  build: {
    target: 'safari13',
  },
})
