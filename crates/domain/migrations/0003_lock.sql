-- 0003 (v0.3.0): configurazione del blocco, in una sola riga.
-- Una tabella e non la coppia chiave/valore di `settings`: colonne tipizzate, vincoli che
-- legano i campi tra loro, e contatori aggiornati con una sola istruzione.

CREATE TABLE lock_config (
    id              INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    -- Impronta Argon2id del PIN in formato PHC; NULL = blocco non configurato.
    -- Il PIN in chiaro non entra mai nel database.
    pin_hash        TEXT CHECK (pin_hash IS NULL OR pin_hash LIKE '$argon2id$%'),
    hello_enabled   INTEGER NOT NULL DEFAULT 0 CHECK (hello_enabled IN (0, 1)),
    -- Minuti di inattività di sistema prima del blocco; NULL = mai.
    idle_minutes    INTEGER CHECK (idle_minutes IS NULL OR idle_minutes BETWEEN 1 AND 240),
    -- Tentativi di PIN falliti dall'ultimo riuscito, e quando è fallito l'ultimo (ms Unix).
    -- Stanno qui, non in memoria: riavviare l'app non azzera l'attesa.
    failed_attempts INTEGER NOT NULL DEFAULT 0 CHECK (failed_attempts >= 0),
    last_failure_ms INTEGER,
    -- Windows Hello è un'alternativa al PIN, mai l'unico modo per entrare: se Hello smette
    -- di funzionare, il PIN c'è sempre.
    CHECK (hello_enabled = 0 OR pin_hash IS NOT NULL)
) STRICT;

-- Il blocco nasce spento, con l'inattività al valore predefinito: i dati esistenti non
-- cambiano comportamento finché l'utente non imposta un PIN.
INSERT INTO lock_config (id, idle_minutes) VALUES (1, 15);
