-- 0004 (v0.4.0): segreti delle app, senza il loro valore.
-- Il valore sta nel Credential Manager di Windows, sotto l'id del segreto: qui restano solo
-- i dati che l'interfaccia può mostrare. Eliminare un'app ne elimina le righe (CASCADE); le
-- credenziali le elimina il guscio, e quelle rimaste orfane per un errore a metà le ritrova
-- la pulizia all'avvio.

CREATE TABLE secrets (
    id         TEXT PRIMARY KEY NOT NULL,
    app_id     TEXT NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
    label      TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 60),
    username   TEXT CHECK (username IS NULL OR length(username) BETWEEN 1 AND 200),
    created_ms INTEGER NOT NULL,
    -- Nessun vincolo updated_ms >= created_ms: un orologio corretto all'indietro non deve
    -- impedire di sostituire un valore.
    updated_ms INTEGER NOT NULL
) STRICT;

-- Due "Password" nella stessa app sarebbero indistinguibili a schermo.
CREATE UNIQUE INDEX secrets_label_unique ON secrets (app_id, label COLLATE NOCASE);
