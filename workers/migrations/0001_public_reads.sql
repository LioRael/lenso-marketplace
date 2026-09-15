-- Public catalog storage. Apply explicitly before deployment, never at App prepare.
CREATE TABLE marketplace_publications (
  catalog_id TEXT PRIMARY KEY NOT NULL,
  revision INTEGER NOT NULL CHECK (revision > 0),
  object_key TEXT NOT NULL,
  digest TEXT NOT NULL
);
CREATE TABLE marketplace_accepted (
  catalog_id TEXT PRIMARY KEY NOT NULL,
  token TEXT NOT NULL,
  object_key TEXT NOT NULL
);
