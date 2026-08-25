import { authApi, type AuthUser, type ProfileVisibility } from './api/authApi.ts'

export interface AccountSettingsDraft {
  displayName: string
  profileVisibility: ProfileVisibility
}

export function accountSettingsDraft(user: AuthUser): AccountSettingsDraft {
  return {
    displayName: user.displayName ?? '덱 체스 사용자',
    profileVisibility: user.profileVisibility,
  }
}

export async function persistAccountSettings(
  draft: AccountSettingsDraft,
  updateProfile = authApi.updateProfile,
): Promise<AuthUser> {
  const result = await updateProfile({
    displayName: draft.displayName.trim(),
    profileVisibility: draft.profileVisibility,
  })
  return result.user
}
