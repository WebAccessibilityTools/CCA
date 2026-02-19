import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';

@customElement('progress-bar')
export class ProgressBar extends LitElement {
  @property({ type: Number }) position = 30;
  @property({ type: Number }) split1 = 10;
  // We accepte Number or undefined
  @property({ type: Number }) split2?: number;

  static aaColor: string = 'oklch(0.55 0.01 286)';
  static aaaColor: string = 'oklch(0.37 0.01 286)';
  static failColor: string = 'oklch(0.51 0.19 28)';

  static styles = css`
    :host {
      display: block;
      width: 100%;
      margin: 20px 0;
      font-family: system-ui, -apple-system, sans-serif;
    }

    .progress-bar {
      position: relative;
      width: 100%;
      height: 0.5rem;
      border-radius: 0.3rem;
      overflow: visible;
    }

    /* Les traits de séparation blancs */
    .divider {
      position: absolute;
      top: 0;
      width: 2px;
      height: 100%;
      background-color: white;
      z-index: 1;
    }

    .progress-indicator {
      position: absolute;
      top: 50%;
      transform: translate(-50%, -50%);

      /* On définit une largeur fine et une hauteur qui dépasse un peu */
      width: 3px;
      height: 30px; 

      background-color: var(--color-600); /* indicator color */
      border-radius: 2px;
      box-shadow: 0 0 4px rgba(0,0,0,0.2);

      z-index: 3; /* Above the bar */
      transition: left 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    }

    .labels-container {
      display: flex;
      margin-top: 5px;
      width: 100%;
    }

    .label-item {
      text-align: center;
      font-size: 13px;
      font-weight: bold;
      color: var(--color-600);
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }
  `;

  render() {
    // Si split2 n'est pas défini, on considère qu'il est à 100% (fin de la barre)
    const s2 = this.split2 ?? 100;

    // Dégradé dynamique : Si pas de split2, on ne passe que du rouge au jaune/vert
    const gradient = this.split2 
      ? `linear-gradient(to right, 
          ${ProgressBar.failColor} 0%, ${ProgressBar.failColor} ${this.split1}%, 
          ${ProgressBar.aaColor} ${this.split1}%, ${ProgressBar.aaColor} ${s2}%, 
          ${ProgressBar.aaaColor} ${s2}%, ${ProgressBar.aaaColor} 100%)`
      : `linear-gradient(to right, 
          ${ProgressBar.failColor} 0%, ${ProgressBar.failColor} ${this.split1}%, 
          ${ProgressBar.aaaColor} ${this.split1}%, ${ProgressBar.aaaColor} 100%)`;

    return html`
      <div class="progress-bar" style="background: ${gradient}">
        <div class="divider" style="left: ${this.split1}%"></div>

        ${this.split2 && this.split2 < 100 
          ? html`<div class="divider" style="left: ${this.split2}%"></div>` 
          : ''}

        <div class="progress-indicator" style="left: ${this.position}%"></div>
      </div>

      <div class="labels-container">
        <div class="label-item" style="width: ${this.split1}%"></div>

        ${this.split2 
          ? html`
              <div class="label-item" style="width: ${s2 - this.split1}%">AA</div>
              <div class="label-item" style="width: ${100 - s2}%">AAA</div>
            `
          : html`
              <div class="label-item" style="width: ${100 - this.split1}%">AA</div>
            `
        }
      </div>
    `;
  }
}

// Optionnel : déclaration du type pour l'IntelliSense dans JSX/TSX
declare global {
  interface HTMLElementTagNameMap {
    'progress-bar': ProgressBar;
  }
}