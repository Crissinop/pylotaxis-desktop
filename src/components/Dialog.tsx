import { useEffect, useId, useLayoutEffect, useRef, type ReactNode } from 'react';

import { GHOST_MIN_AGE_MS, leaveGhost } from '../lib/motion';

interface DialogProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
}

/**
 * Finestra modale sull'elemento nativo `<dialog>`: gestisce da sé il fuoco, il tasto Esc e
 * l'inerzia del resto della pagina (A.7.8). Si mostra montandola e si chiude smontandola,
 * nello stesso ciclo di render (A.7.7). (v0.2.0)
 */
export function Dialog({ title, onClose, children }: DialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();

  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) dialog.showModal();
    return () => dialog?.close();
  }, []);

  // Uscita animata (v0.7.0): nel cleanup di un layout effect la finestra è ancora nel DOM e
  // aperta, quindi se ne può lasciare una copia che svanisce. L'entrata la fa il CSS.
  useLayoutEffect(() => {
    const dialog = ref.current;
    const openedAt = performance.now();
    return () => {
      if (dialog && performance.now() - openedAt >= GHOST_MIN_AGE_MS) leaveGhost(dialog);
    };
  }, []);

  return (
    <dialog
      ref={ref}
      className="dialog"
      aria-labelledby={titleId}
      onCancel={(event) => {
        // Esc chiude passando dallo stato di React, non dal solo DOM.
        event.preventDefault();
        onClose();
      }}
    >
      <h2 id={titleId} className="dialog__title">
        {title}
      </h2>
      {children}
    </dialog>
  );
}
