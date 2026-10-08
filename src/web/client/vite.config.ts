import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'
import { DEFAULT_WEB_PORT } from '../../lib/config'
import { version } from '../../../menubar-tauri/src-tauri/tauri.conf.json'

const backendTarget = `http://localhost:${DEFAULT_WEB_PORT}`
const backendWsTarget = `ws://localhost:${DEFAULT_WEB_PORT}`

export default defineConfig({
  define: { __KEEPLINE_VERSION__: JSON.stringify(version) },
  plugins: [react()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  build: {
    outDir: '../../../public/dist',
    emptyOutDir: true,
    sourcemap: true,
  },
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: backendTarget,
        changeOrigin: true,
      },
      '/ws': {
        target: backendWsTarget,
        ws: true,
      },
    },
  },
})
