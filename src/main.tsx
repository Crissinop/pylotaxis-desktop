import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { getCurrentWindow } from '@tauri-apps/api/window';

import App from './App';
import { APP_NAME } from './constants/app';
import { Palette } from './features/palette/Palette';
import { APPEARANCE_KEY, applyAppearance, loadAppearance } from './lib/appearance';
import { disableDefaultContextMenu } from './lib/contextMenu';
import { PALETTE_WINDOW } from './lib/ipc';
import './i18n';
import './styles/fonts.css';
import './styles/tokens.css';
import './styles/app.css';

document.title = APP_NAME;
// Vale per le due finestre: stessa pagina (v0.7.0).
disableDefaultContextMenu();
// Aspetto prima del primo disegno, in entrambe le finestre; quando l'altra finestra lo
// cambia, l'evento storage lo porta anche qui (v0.8.0).
applyAppearance(loadAppearance());
window.addEventListener('storage', (event) => {
  if (event.key === APPEARANCE_KEY) applyAppearance(loadAppearance());
});

const container = document.getElementById('root');
if (!container) {
  throw new Error('Elemento #root mancante in index.html');
}

createRoot(container).render(
  <StrictMode>
    {/* Stessa pagina per le due finestre: l'etichetta decide che cosa mostrare (v0.5.0). */}
    {getCurrentWindow().label === PALETTE_WINDOW ? <Palette /> : <App />}
  </StrictMode>,
);
