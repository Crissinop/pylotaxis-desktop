import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import { FALLBACK_LANGUAGE, pickLanguage } from './language';
import en from './locales/en.json';
import it from './locales/it.json';

// L'attributo lang segue la lingua attiva: lettori di schermo e sillabazione ne dipendono.
i18n.on('languageChanged', (language) => {
  document.documentElement.lang = language;
});

void i18n.use(initReactI18next).init({
  resources: { it: { translation: it }, en: { translation: en } },
  lng: pickLanguage(navigator.languages),
  fallbackLng: FALLBACK_LANGUAGE,
  // React esegue già l'escape: un secondo escape mostrerebbe entità come &amp;.
  interpolation: { escapeValue: false },
});

export default i18n;
