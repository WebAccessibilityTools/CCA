import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';

@customElement('progress-bar')
export class ProgressBar extends LitElement {
  @property({ type: Number }) position = 30;
  @property({ type: Number }) split1 = 10;
  // We accepte Number or undefined
  @property({ type: Number }) split2?: number;
  @property({ type: String }) labels: 'levels' | 'ratios' = 'levels';

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

      background-color: var(--background-color-inverted); /* indicator color */
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
      color: var(--text-color);
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .label-ratio {
      position: absolute;
      left: 0;
      transform: translateX(-50%);
      font-size: 13px;
      font-weight: bold;
      color: var(--text-color);
      letter-spacing: 0.5px;
      white-space: nowrap;
    }

    .label-ratio.above {
      bottom: 100%;
      margin-bottom: 4px;
    }

    .label-ratio.below {
      top: 100%;
      margin-top: 4px;
    }
  `;

  // Convertit une valeur sur l'échelle 0-21 en pourcentage
  private toPercent(value: number): number {
    return (value / 21) * 100;
  }

  render() {
    const posPercent = this.toPercent(this.position);
    const s1Percent = this.toPercent(this.split1);
    const s2Percent = this.split2 != null ? this.toPercent(this.split2) : 100;

    // Dégradé dynamique : Si pas de split2, on ne passe que du rouge au jaune/vert
    const gradient = this.split2 != null
      ? `linear-gradient(to right,
          ${ProgressBar.failColor} 0%, ${ProgressBar.failColor} ${s1Percent}%,
          ${ProgressBar.aaColor} ${s1Percent}%, ${ProgressBar.aaColor} ${s2Percent}%,
          ${ProgressBar.aaaColor} ${s2Percent}%, ${ProgressBar.aaaColor} 100%)`
      : `linear-gradient(to right,
          ${ProgressBar.failColor} 0%, ${ProgressBar.failColor} ${s1Percent}%,
          ${ProgressBar.aaaColor} ${s1Percent}%, ${ProgressBar.aaaColor} 100%)`;

    return html`
      <div class="progress-bar" style="background: ${gradient}" aria-hidden="true">
        <div class="divider" style="left: ${s1Percent}%"></div>

        ${this.split2 != null && s2Percent < 100
          ? html`<div class="divider" style="left: ${s2Percent}%"></div>`
          : ''}

        <div class="progress-indicator" style="left: ${posPercent}%"></div>

        ${this.labels === 'ratios'
          ? html`
            <span class="label-ratio above" style="left: ${s1Percent}%">${this.split1}</span>
            ${this.split2 != null
              ? html`<span class="label-ratio below" style="left: ${s2Percent}%">${this.split2}</span>`
              : ''}
          `
          : ''}
      </div>

      ${this.labels === 'levels'
        ? html`
          <div class="labels-container">
            <div class="label-item" style="width: ${s1Percent}%"></div>
            ${this.split2 != null
              ? html`
                  <div class="label-item" style="width: ${s2Percent - s1Percent}%">AA</div>
                  <div class="label-item" style="width: ${100 - s2Percent}%">AAA</div>
                `
              : html`
                  <div class="label-item" style="width: ${100 - s1Percent}%">AA</div>
                `
            }
          </div>`
        : ''
      }
    `;
  }
}

// Optionnel : déclaration du type pour l'IntelliSense dans JSX/TSX
declare global {
  interface HTMLElementTagNameMap {
    'progress-bar': ProgressBar;
  }
}