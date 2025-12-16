import { invoke } from "@tauri-apps/api/core";

interface ColorResult {
  foreground: [number, number, number];
}

const pickBtn = document.getElementById('pickBtn') as HTMLButtonElement;
const result = document.getElementById('result') as HTMLDivElement;
const colorPreview = document.getElementById('colorPreview') as HTMLDivElement;
const hexValue = document.getElementById('hexValue') as HTMLParagraphElement;
const rgbValue = document.getElementById('rgbValue') as HTMLSpanElement;
const copied = document.getElementById('copied') as HTMLParagraphElement;

pickBtn.addEventListener('click', async () => {
  pickBtn.disabled = true;
  pickBtn.textContent = 'Picking...';

  try {
    // Appelle pick_color avec fg=true (foreground mode)
    // Call pick_color with fg=true (foreground mode)
    const color = await invoke<ColorResult>('pick_color', { fg: true });

    if (color && color.foreground) {
      // foreground est un tuple [r, g, b]
      // foreground is a tuple [r, g, b]
      const [r, g, b] = color.foreground;
      const hex = `#${r.toString(16).padStart(2, '0').toUpperCase()}${g.toString(16).padStart(2, '0').toUpperCase()}${b.toString(16).padStart(2, '0').toUpperCase()}`;

      colorPreview.style.backgroundColor = hex;
      hexValue.textContent = hex;
      rgbValue.textContent = `${r}, ${g}, ${b}`;
      result.classList.add('visible');
    }
  } catch (error) {
    console.error('Error:', error);
  } finally {
    pickBtn.disabled = false;
    pickBtn.textContent = 'Pick a Color';
  }
});

// Copy hex on click
hexValue.addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(hexValue.textContent || '');
    copied.classList.add('show');
    setTimeout(() => copied.classList.remove('show'), 1500);
  } catch (err) {
    console.error('Failed to copy:', err);
  }
});
