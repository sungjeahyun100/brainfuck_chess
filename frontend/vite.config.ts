import { defineConfig } from 'vite'
import type { Plugin } from 'vite'
import vue from '@vitejs/plugin-vue'

type AppEnv = 'local' | 'test' | 'prod'

function resolveAppEnv(defaultEnv: AppEnv): AppEnv {
  const value = process.env.APP_ENV
  if (value === 'local' || value === 'test' || value === 'prod') {
    return value
  }

  return defaultEnv
}

function appConfigPlugin(): Plugin {
  const runtimeConfig = (defaultEnv: AppEnv) => JSON.stringify({
    appEnv: resolveAppEnv(defaultEnv),
    firebase: {
      apiKey: process.env.FIREBASE_API_KEY ?? '',
      authDomain: process.env.FIREBASE_AUTH_DOMAIN ?? '',
      projectId: process.env.IDENTITY_PLATFORM_PROJECT_ID ?? '',
      appId: process.env.FIREBASE_APP_ID ?? '',
    },
  }).replaceAll('<', '\\u003c')
  return {
    name: 'brainfuck-chess-app-config',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (req.url?.split('?')[0] !== '/config.js') {
          next()
          return
        }

        res.setHeader('Content-Type', 'application/javascript; charset=utf-8')
        res.end(`window.APP_CONFIG = Object.freeze(${runtimeConfig('local')});\n`)
      })
    },
    configurePreviewServer(server) {
      server.middlewares.use((req, res, next) => {
        if (req.url?.split('?')[0] !== '/config.js') {
          next()
          return
        }

        res.setHeader('Content-Type', 'application/javascript; charset=utf-8')
        res.end(`window.APP_CONFIG = Object.freeze(${runtimeConfig('local')});\n`)
      })
    },
  }
}

export default defineConfig({
  plugins: [appConfigPlugin(), vue()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
    },
  },
})
