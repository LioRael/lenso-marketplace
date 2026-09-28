-- Independent signed npm-only package publication.
CREATE TABLE marketplace_packages (
  catalog_id TEXT PRIMARY KEY NOT NULL,
  revision INTEGER NOT NULL CHECK (revision > 0),
  object_key TEXT NOT NULL,
  digest TEXT NOT NULL
);
