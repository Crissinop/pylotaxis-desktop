import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Mark } from '../../components/Mark';
import { APP_AUTHOR, APP_NAME } from '../../constants/app';
import { LegalDialog } from '../legal/LegalDialog';
import type { LegalDocument } from '../legal/sections';

/** Righe della scheda, nell'ordine in cui si leggono. */
const FACTS = ['developer', 'stack', 'platform', 'data', 'fonts'] as const;

/**
 * Informazioni (v0.8.0): che cos'è l'app, da dove viene il nome, chi la sviluppa e con che
 * cosa. Breve per scelta; i testi legali si aprono da qui come dalle impostazioni.
 */
export function AboutView({ version }: { version: string | null }) {
  const { t } = useTranslation();
  const [legal, setLegal] = useState<LegalDocument | null>(null);
  const values = { name: APP_NAME, author: APP_AUTHOR };
  return (
    <article className="about" aria-labelledby="about-title">
      <header className="about__header">
        <Mark size={64} />
        <h1 id="about-title" className="about__title">
          {APP_NAME}
        </h1>
        <p className="about__tagline">{t('about.tagline')}</p>
      </header>
      <p className="about__etymology">{t('about.etymology')}</p>
      <dl className="about__facts">
        <div className="about__fact">
          <dt>{t('about.version')}</dt>
          <dd>{version ?? t('footer.versionUnavailable')}</dd>
        </div>
        {FACTS.map((fact) => (
          <div key={fact} className="about__fact">
            <dt>{t(`about.${fact}`)}</dt>
            <dd>{t(`about.${fact}Value`, values)}</dd>
          </div>
        ))}
      </dl>
      <div className="about__links">
        <button type="button" className="button button--ghost" onClick={() => setLegal('terms')}>
          {t('legal.termsTitle')}
        </button>
        <button type="button" className="button button--ghost" onClick={() => setLegal('privacy')}>
          {t('legal.privacyTitle')}
        </button>
      </div>
      {legal && <LegalDialog document={legal} onClose={() => setLegal(null)} />}
    </article>
  );
}
