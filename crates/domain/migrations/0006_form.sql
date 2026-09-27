-- Forma (v0.7.0): icone delle app e lunghezza del PIN per lo sblocco automatico.

-- Icona dell'app in PNG. `executable`: estratta dall'eseguibile, si rinnova quando cambia
-- l'impronta; `custom`: scelta dall'utente, ha la precedenza. Nessuna icona = iniziali.
ALTER TABLE apps ADD COLUMN icon BLOB;
ALTER TABLE apps ADD COLUMN icon_kind TEXT CHECK (icon_kind IN ('executable', 'custom'));
-- Revisione dell'icona (primi 8 byte dello SHA-256): il frontend rilegge le immagini solo
-- quando cambia, e il registro non trasporta i PNG.
ALTER TABLE apps ADD COLUMN icon_rev TEXT;

-- Lunghezza del PIN, mai il PIN: la schermata di blocco tenta una sola volta, quando le cifre
-- digitate la raggiungono. NULL = non ancora nota (PIN impostati prima della v0.7.0): si
-- impara al primo sblocco riuscito.
ALTER TABLE lock_config ADD COLUMN pin_length INTEGER CHECK (pin_length BETWEEN 6 AND 16);
