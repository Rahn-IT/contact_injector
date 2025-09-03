-- Add migration script here

CREATE TABLE IF NOT EXISTS destinations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    destination_type TEXT NOT NULL,
    access_data TEXT NOT NULL
);
