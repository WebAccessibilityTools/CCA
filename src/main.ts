import { invoke } from "@tauri-apps/api/core";
import Alpine from 'alpinejs';

interface ColorResult {
  foreground: [number, number, number];
}

interface ColorPickerStore {
  isPicking: boolean;
  resultVisible: boolean;
  hex: string;
  rgb: string;
  backgroundColor: string;
  copiedVisible: boolean;
  pickColor(): Promise<void>;
  copyHex(): Promise<void>;
}

// Fonction pour convertir RGB en HEX
function rgbToHex(r: number, g: number, b: number): string {
  return `#${r.toString(16).padStart(2, '0').toUpperCase()}${g.toString(16).padStart(2, '0').toUpperCase()}${b.toString(16).padStart(2, '0').toUpperCase()}`;
}

// Alpine.js store pour le color picker
Alpine.store('colorPicker', {
  isPicking: false,
  resultVisible: false,
  hex: '',
  rgb: '',
  backgroundColor: '',
  copiedVisible: false,

  async pickColor(this: ColorPickerStore) {
    this.isPicking = true;

    try {
      const color = await invoke<ColorResult>('pick_color', { fg: true });

      if (color && color.foreground) {
        const [r, g, b] = color.foreground;
        const hex = rgbToHex(r, g, b);

        this.hex = hex;
        this.rgb = `${r}, ${g}, ${b}`;
        this.backgroundColor = hex;
        this.resultVisible = true;
      }
    } catch (error) {
      console.error('Error:', error);
    } finally {
      this.isPicking = false;
    }
  },

  async copyHex(this: ColorPickerStore) {
    try {
      await navigator.clipboard.writeText(this.hex);
      this.copiedVisible = true;
      setTimeout(() => {
        this.copiedVisible = false;
      }, 1500);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  }
});

// Démarrer Alpine.js
Alpine.start();
