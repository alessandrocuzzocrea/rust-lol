CREATE TABLE IF NOT EXISTS counters (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    value INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO counters (id, name, value) VALUES (1, 'visits', 0);
