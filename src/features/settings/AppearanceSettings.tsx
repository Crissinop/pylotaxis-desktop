import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import {
  ACCENTS,
  THEMES,
  applyAppearance,
  loadAppearance,
  saveAppearance,
  type Appearance,
} from '../../lib/appearance';

/**
 * Tema e accento (v0.8.0). La scelta vale subito, in tutte e due le finestre, e resta tra un
 * avvio e l'altro. Il marchio resta verderame qualunque accento si scelga.
 */
export function AppearanceSettings() {
  const { t } = useTranslation();
  const ids = { title: useId(), theme: useId(), accent: useId(), hint: useId() };
  const [appearance, setAppearance] = useState<Appearance>(loadAppearance);

  const choose = (next: Appearance) => {
    setAppearance(next);
    applyAppearance(next);
    saveAppearance(next);
  };

  return (
    <section className="settings" aria-labelledby={ids.title}>
      <h2 id={ids.title} className="settings__title">
        {t('settings.appearanceTitle')}
      </h2>
      <div className="settings__group">
        <span id={ids.theme} className="settings__label">
          {t('settings.themeLabel')}
        </span>
        <div className="segmented" role="radiogroup" aria-labelledby={ids.theme}>
          {THEMES.map((theme) => (
            <label key={theme} className="segmented__option">
              <input
                type="radio"
                name={ids.theme}
                value={theme}
                checked={appearance.theme === theme}
                onChange={() => choose({ ...appearance, theme })}
              />
              <span>{t(`settings.theme.${theme}`)}</span>
            </label>
          ))}
        </div>
      </div>
      <div className="settings__group">
        <span id={ids.accent} className="settings__label">
          {t('settings.accentLabel')}
        </span>
        <div
          className="swatches"
          role="radiogroup"
          aria-labelledby={ids.accent}
          aria-describedby={ids.hint}
        >
          {ACCENTS.map((accent) => (
            <label key={accent} className="swatch" title={t(`settings.accent.${accent}`)}>
              <input
                type="radio"
                name={ids.accent}
                value={accent}
                checked={appearance.accent === accent}
                onChange={() => choose({ ...appearance, accent })}
              />
              <span className={`swatch__color swatch__color--${accent}`} aria-hidden="true" />
              <span className="visually-hidden">{t(`settings.accent.${accent}`)}</span>
            </label>
          ))}
        </div>
        <p id={ids.hint} className="field__hint">
          {t('settings.accentHint', { accent: t(`settings.accent.${appearance.accent}`) })}
        </p>
      </div>
    </section>
  );
}
