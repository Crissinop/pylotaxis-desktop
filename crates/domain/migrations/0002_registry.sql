-- 0002 (v0.2.0): registro delle app e categorie.
-- I vincoli qui sono l'ultima difesa: la validazione vera sta nel dominio, ma un dato
-- incoerente non deve poter entrare nemmeno per errore.

CREATE TABLE categories (
    id       TEXT PRIMARY KEY NOT NULL,
    name     TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE UNIQUE INDEX categories_name_unique ON categories (name COLLATE NOCASE);

CREATE TABLE apps (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('executable', 'web', 'protocol')),
    target      TEXT NOT NULL,
    -- Impronta SHA-256 in esadecimale: presente se e solo se l'app è un eseguibile.
    sha256      TEXT CHECK ((kind = 'executable') = (sha256 IS NOT NULL)),
    category_id TEXT REFERENCES categories (id) ON DELETE SET NULL
) STRICT;

CREATE UNIQUE INDEX apps_name_unique ON apps (name COLLATE NOCASE);

CREATE TABLE app_tags (
    app_id TEXT NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
    tag    TEXT NOT NULL,
    PRIMARY KEY (app_id, tag)
) STRICT;
