-- 0001 (v0.1.0): impostazioni chiave/valore del portale.
-- Serve anche alla prova tecnica di SQLCipher; le tabelle del registro arrivano con la v0.2.0.
-- Regola: una migrazione rilasciata non si modifica mai, se ne aggiunge una nuova (A.7.4).
CREATE TABLE settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;
