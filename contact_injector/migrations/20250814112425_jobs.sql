-- Add migration script here

CREATE TABLE IF NOT EXISTS jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    source INTEGER NOT NULL,
    destination INTEGER NOT NULL,
    delay INTEGER NOT NULL,
    FOREIGN KEY (source) REFERENCES sources(id),
    FOREIGN KEY (destination) REFERENCES destinations(id)
);
