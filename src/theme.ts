// =============================================================================
// theme.ts - Gestion du thème light/dark/auto
// theme.ts - Light/dark/auto theme management
// =============================================================================

export type ThemePreference = 'auto' | 'light' | 'dark';

const STORAGE_KEY = 'cca-theme';

const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

/** Retourne la préférence sauvegardée / Returns saved preference */
export function getThemePreference(): ThemePreference {
  return (localStorage.getItem(STORAGE_KEY) as ThemePreference) || 'auto';
}

/** Sauvegarde la préférence / Save preference */
export function setThemePreference(pref: ThemePreference): void {
  localStorage.setItem(STORAGE_KEY, pref);
  applyTheme(pref);
}

/** Résout le thème effectif / Resolves effective theme */
function resolveTheme(pref: ThemePreference): 'light' | 'dark' {
  if (pref === 'auto') {
    return mediaQuery.matches ? 'dark' : 'light';
  }
  return pref;
}

/** Applique le thème sur le document / Apply theme to document */
export function applyTheme(pref?: ThemePreference): void {
  const resolved = resolveTheme(pref ?? getThemePreference());
  document.documentElement.setAttribute('data-theme', resolved);
}

/** Initialise le thème et écoute les changements système / Init theme and listen for system changes */
export function initTheme(): void {
  applyTheme();
  mediaQuery.addEventListener('change', () => {
    if (getThemePreference() === 'auto') {
      applyTheme('auto');
    }
  });
}
