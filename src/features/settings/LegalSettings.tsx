import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { APP_NAME } from '../../constants/app';
import { LegalDialog } from '../legal/LegalDialog';
import type { LegalDocument } from '../legal/sections';

/**
 * Termini d'uso e privacy nelle impostazioni (v0.8.0): una scheda ciascuno, con una sintesi
 * e il testo intero in una finestra. Le schede restano della stessa altezza delle altre.
 */
function LegalCard({ document }: { document: LegalDocument }) {
  const { t } = useTranslation();
  const titleId = useId();
  const [open, setOpen] = useState(false);
  return (
    <section className="settings" aria-labelledby={titleId}>
      <h2 id={titleId} className="settings__title">
        {t(`legal.${document}Title`)}
      </h2>
      <p className="settings__text">{t(`legal.${document}Summary`, { name: APP_NAME })}</p>
      <div className="settings__actions">
        <button type="button" className="button button--secondary" onClick={() => setOpen(true)}>
          {t(`legal.${document}Open`)}
        </button>
      </div>
      {open && <LegalDialog document={document} onClose={() => setOpen(false)} />}
    </section>
  );
}

export function TermsSettings() {
  return <LegalCard document="terms" />;
}

export function PrivacySettings() {
  return <LegalCard document="privacy" />;
}
