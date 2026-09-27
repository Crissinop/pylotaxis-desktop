import { useEffect, useRef, useState } from 'react';

import { nextIndex } from './menu';

export interface TileMenuItem {
  key: string;
  label: string;
  onSelect: () => void;
  danger?: boolean;
}

interface TileMenuProps {
  id: string;
  /** Id del pulsante che apre il menu: gli dà il nome. */
  labelledBy: string;
  items: TileMenuItem[];
  /** `restoreFocus`: Esc riporta il fuoco al pulsante che ha aperto il menu. */
  onClose: (restoreFocus: boolean) => void;
}

/**
 * Menu delle azioni secondarie di una tessera (v0.7.0). Tastiera prima di tutto: frecce, Home
 * e Fine spostano, Invio sceglie, Esc chiude e restituisce il fuoco, Tab esce (A.7.8). Va reso
 * come figlio diretto della tessera: un clic fuori da lei lo chiude.
 */
export function TileMenu({ id, labelledBy, items, onClose }: TileMenuProps) {
  const list = useRef<HTMLUListElement>(null);
  const [active, setActive] = useState(0);

  useEffect(() => {
    list.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')[active]?.focus();
  }, [active]);

  useEffect(() => {
    const onPointer = (event: PointerEvent) => {
      const tile = list.current?.parentElement;
      if (tile && event.target instanceof Node && !tile.contains(event.target)) onClose(false);
    };
    document.addEventListener('pointerdown', onPointer);
    return () => document.removeEventListener('pointerdown', onPointer);
  }, [onClose]);

  return (
    <ul
      ref={list}
      id={id}
      className="tile-menu"
      role="menu"
      aria-labelledby={labelledBy}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          onClose(true);
          return;
        }
        if (event.key === 'Tab') {
          onClose(false);
          return;
        }
        const next = nextIndex(active, event.key, items.length);
        if (next === null) return;
        event.preventDefault();
        setActive(next);
      }}
    >
      {items.map((item, index) => (
        <li key={item.key} role="none">
          <button
            type="button"
            role="menuitem"
            tabIndex={index === active ? 0 : -1}
            className={item.danger ? 'tile-menu__item tile-menu__item--danger' : 'tile-menu__item'}
            onClick={() => {
              onClose(false);
              item.onSelect();
            }}
          >
            {item.label}
          </button>
        </li>
      ))}
    </ul>
  );
}
