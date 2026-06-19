export type AppEnv = 'local' | 'test' | 'prod'

interface AppConfig {
  appEnv?: AppEnv
}

declare global {
  interface Window {
    APP_CONFIG?: AppConfig
  }
}

function normalizeAppEnv(value: unknown): AppEnv {
  if (value === 'local' || value === 'test' || value === 'prod') {
    return value
  }

  return 'prod'
}

export const appEnv = normalizeAppEnv(window.APP_CONFIG?.appEnv)
export const showEnvBanner = appEnv !== 'prod'
export const envBannerLabel = appEnv === 'test'
  ? 'TEST SERVER'
  : appEnv === 'local'
    ? 'LOCAL DEV'
    : ''
