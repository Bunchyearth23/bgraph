import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// `base: './'` so a built page works from a subdirectory or straight off the
// filesystem index — the artefact is a folder somebody unzips, not a deploy.
export default defineConfig({
  base: './',
  plugins: [svelte()],
  build: { target: 'es2022', chunkSizeWarningLimit: 1500 },
})
