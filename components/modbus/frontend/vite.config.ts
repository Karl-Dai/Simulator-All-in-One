import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { buildSimlabAliases } from '../../../scripts/simlab-vite'

// https://vite.dev/config/
export default defineConfig({
  resolve: { alias: buildSimlabAliases(import.meta.url) },
  plugins: [vue()],
  build: { outDir: '../go/cmd/slavegui/assets/dist', emptyOutDir: true },
  server: { strictPort: true },
})
