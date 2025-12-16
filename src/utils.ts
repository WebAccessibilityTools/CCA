// =============================================================================
// utils.ts - Fonctions utilitaires
// utils.ts - Utility functions
// =============================================================================

// Fonction pour convertir une couleur HEX en RGB
// Function to convert HEX color to RGB
export function hexToRgb(hex: string): string {
  // Extrait et convertit les composantes rouge (bytes 1-2)
  // Extract and convert red component (bytes 1-2)
  const r = parseInt(hex.slice(1, 3), 16);

  // Extrait et convertit les composantes verte (bytes 3-4)
  // Extract and convert green component (bytes 3-4)
  const g = parseInt(hex.slice(3, 5), 16);

  // Extrait et convertit les composantes bleue (bytes 5-6)
  // Extract and convert blue component (bytes 5-6)
  const b = parseInt(hex.slice(5, 7), 16);

  // Retourne la chaîne au format "r, g, b"
  // Return string in "r, g, b" format
  return `${r}, ${g}, ${b}`;
}

// Fonction pour convertir RGB en HEX
// Function to convert RGB to HEX
export function rgbToHex(r: number, g: number, b: number): string {
  // Convertit chaque composante en hexadécimal et assure 2 chiffres avec padStart
  // Convert each component to hexadecimal and ensure 2 digits with padStart
  return `#${r.toString(16).padStart(2, '0').toUpperCase()}${g.toString(16).padStart(2, '0').toUpperCase()}${b.toString(16).padStart(2, '0').toUpperCase()}`;
}
