-- Independent pointer to the immutable signed release-content v2 snapshot.
CREATE TABLE marketplace_release_content (
  catalog_id TEXT PRIMARY KEY NOT NULL,
  revision INTEGER NOT NULL CHECK (revision > 0),
  object_key TEXT NOT NULL,
  digest TEXT NOT NULL
);
