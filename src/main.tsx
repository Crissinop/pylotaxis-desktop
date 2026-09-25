import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import App from './App';
import { APP_NAME } from './constants/app';
import './i18n';
import './styles/fonts.css';
import './styles/tokens.css';
import './styles/app.css';

document.title = APP_NAME;

const container = document.getElementById('root');
if (!container) {
  throw new Error('Elemento #root mancante in index.html');
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
