-- Additive signed release distribution and documentation metadata.
-- Kept separate from the base catalog pointer so either stream can advance
-- independently while consumers join only exact verified release identities.
CREATE TABLE marketplace_release_details (
  catalog_id TEXT PRIMARY KEY NOT NULL,
  revision INTEGER NOT NULL CHECK (revision > 0),
  object_key TEXT NOT NULL,
  digest TEXT NOT NULL
);
