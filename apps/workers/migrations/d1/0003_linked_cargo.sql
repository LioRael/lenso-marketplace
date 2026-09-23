-- Independent signed source-only linked Cargo publication.
CREATE TABLE marketplace_linked_cargo (
  catalog_id TEXT PRIMARY KEY NOT NULL,
  revision INTEGER NOT NULL CHECK (revision > 0),
  object_key TEXT NOT NULL,
  digest TEXT NOT NULL
);
