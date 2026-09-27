import { useTranslation } from 'react-i18next';

import { Dialog } from '../../components/Dialog';
import { APP_AUTHOR, APP_NAME } from '../../constants/app';
import { LEGAL_UPDATED, SECTIONS, type LegalDocument } from './sections';

interface LegalDialogProps {
  document: LegalDocument;
  onClose: () => void;
}

/** Termini d'uso o informativa sulla privacy, per sezioni, in una finestra da leggere. */
export function LegalDialog({ document, onClose }: LegalDialogProps) {
  const { t, i18n } = useTranslation();
  const values = { name: APP_NAME, author: APP_AUTHOR };
  const updated = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'long' }).format(
    LEGAL_UPDATED,
  );
  return (
    <Dialog title={t(`legal.${document}Title`)} onClose={onClose} wide>
      <div className="legal">
        {SECTIONS[document].map((id) => (
          <section key={id} className="legal__section">
            <h3 className="legal__heading">{t(`legal.${document}.${id}.title`, values)}</h3>
            <p className="legal__text">{t(`legal.${document}.${id}.body`, values)}</p>
          </section>
        ))}
        <p className="legal__updated">{t('legal.updated', { date: updated })}</p>
      </div>
      <div className="dialog__actions">
        <button type="button" className="button button--primary" onClick={onClose}>
          {t('actions.close')}
        </button>
      </div>
    </Dialog>
  );
}
