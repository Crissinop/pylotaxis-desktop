/**
 * Simbolo del logo: griglia 3×3 che forma un Π, con la chiave di volta come unico accento.
 * Stessa geometria di branding/mark.svg. (v0.1.0)
 */

/** Da questa dimensione in giù la chiave di volta diventa un quadrato, come prevede il logo. */
const SQUARE_KEYSTONE_MAX_PX = 32;

const TILE_ORIGINS: readonly (readonly [number, number])[] = [
  [0, 0],
  [88, 0],
  [0, 44],
  [88, 44],
  [0, 88],
  [88, 88],
];

const KEYSTONE_PATH =
  'M45 0 L75 0 Q79 0 78.63 3.98 L76.37 28.02 Q76 32 72 32 L48 32 Q44 32 43.63 28.02 L41.37 3.98 Q41 0 45 0 Z';

interface MarkProps {
  size?: number;
}

export function Mark({ size = 32 }: MarkProps) {
  return (
    <svg
      className="mark"
      width={size}
      height={size}
      viewBox="0 0 120 120"
      aria-hidden="true"
      focusable="false"
    >
      {TILE_ORIGINS.map(([x, y]) => (
        <rect key={`${x}-${y}`} className="mark__tile" x={x} y={y} width={32} height={32} rx={4} />
      ))}
      {size <= SQUARE_KEYSTONE_MAX_PX ? (
        <rect className="mark__keystone" x={44} y={0} width={32} height={32} rx={4} />
      ) : (
        <path className="mark__keystone" d={KEYSTONE_PATH} />
      )}
    </svg>
  );
}
