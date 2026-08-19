export type AppEnv = 'local' | 'test' | 'prod'

export interface FirebasePublicConfig {
  apiKey: string
  authDomain: string
  projectId: string
  appId: string
}

interface AppConfig {
  appEnv?: AppEnv
  firebase?: FirebasePublicConfig
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

const firebase = window.APP_CONFIG?.firebase
export const firebaseConfig = firebase?.apiKey && firebase.authDomain && firebase.projectId && firebase.appId
  && ![firebase.apiKey, firebase.appId].some((value) => value.startsWith('configure-in-'))
  ? firebase
  : null
