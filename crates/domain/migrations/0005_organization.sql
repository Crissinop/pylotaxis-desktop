-- Organizzazione (v0.6.0): ambienti, stato delle app web, gruppi di avvio.

-- Ambiente di un'app. Le app esistenti restano senza: nessun comportamento cambia (A.7.4).
ALTER TABLE apps ADD COLUMN environment TEXT
    CHECK (environment IN ('development', 'test', 'production'));

-- Controllo dello stato: spento per default, lo si accende app per app (solo web).
ALTER TABLE apps ADD COLUMN health_check INTEGER NOT NULL DEFAULT 0
    CHECK (health_check IN (0, 1));

-- Lo stesso nome una volta per ambiente: "Portale" in sviluppo e in produzione convivono.
-- coalesce perché in un indice UNIQUE due NULL non sono uguali: senza, due app senza
-- ambiente potrebbero chiamarsi allo stesso modo.
DROP INDEX apps_name_unique;
CREATE UNIQUE INDEX apps_name_environment_unique
    ON apps (name COLLATE NOCASE, coalesce(environment, ''));

-- Gruppi di avvio. "groups" è una parola chiave di SQLite dalla 3.28: da qui il prefisso.
CREATE TABLE launch_groups (
    id   TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX launch_groups_name_unique ON launch_groups (name COLLATE NOCASE);

-- App di un gruppo, nell'ordine di avvio. Eliminare un'app la toglie dai gruppi; eliminare un
-- gruppo non tocca le app.
CREATE TABLE launch_group_apps (
    group_id TEXT NOT NULL REFERENCES launch_groups (id) ON DELETE CASCADE,
    app_id   TEXT NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY (group_id, app_id)
) STRICT;

CREATE INDEX launch_group_apps_app ON launch_group_apps (app_id);
