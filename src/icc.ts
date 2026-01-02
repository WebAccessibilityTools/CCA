// =============================================================================
// icc.ts - ICC Profile Management (Frontend)
// =============================================================================

import { invoke } from "@tauri-apps/api/core";

export interface ICCProfile {
  name: string;
  description: string;
  is_current: boolean;
}

/**
 * Liste tous les profils ICC disponibles
 * Lists all available ICC profiles
 */
export async function listICCProfiles(): Promise<ICCProfile[]> {
  return await invoke<ICCProfile[]>("list_icc_profiles");
}

/**
 * Sélectionne un profil ICC
 * Selects an ICC profile
 */
export async function selectICCProfile(profileName: string): Promise<void> {
  await invoke("select_icc_profile", { profileName });
}

/**
 * Récupère le profil ICC actuellement sélectionné
 * Gets the currently selected ICC profile
 */
export async function getSelectedICCProfile(): Promise<string | null> {
  return await invoke<string | null>("get_selected_icc_profile");
}
