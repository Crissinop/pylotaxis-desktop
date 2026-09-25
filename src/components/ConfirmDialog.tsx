import { useTranslation } from 'react-i18next';

import { Dialog } from './Dialog';

interface ConfirmDialogProps {
  title: string;
  body: string;
  confirmLabel: string;
  /** Azione distruttiva: il pulsante di conferma lo dichiara con il suo stile (A.8). */
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  title,
  body,
  confirmLabel,
  destructive = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog title={title} onClose={onCancel}>
      <p className="dialog__body">{body}</p>
      <div className="dialog__actions">
        <button type="button" className="button button--secondary" onClick={onCancel} autoFocus>
          {t('actions.cancel')}
        </button>
        <button
          type="button"
          className={destructive ? 'button button--danger' : 'button button--primary'}
          onClick={onConfirm}
        >
          {confirmLabel}
        </button>
      </div>
    </Dialog>
  );
}
