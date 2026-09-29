-- A single head binds catalog bytes and their keyless provenance bundle.
-- Only the protected, provenance-verifying promotion workflow writes this row.
CREATE TABLE marketplace_keyless_heads (
  catalog_id TEXT PRIMARY KEY NOT NULL,
  revision INTEGER NOT NULL CHECK (revision > 0),
  catalog_digest TEXT NOT NULL CHECK (length(catalog_digest) = 64),
  catalog_size INTEGER NOT NULL CHECK (catalog_size > 0 AND catalog_size <= 4194304),
  bundle_digest TEXT NOT NULL CHECK (length(bundle_digest) = 64),
  bundle_size INTEGER NOT NULL CHECK (bundle_size > 0 AND bundle_size <= 4194304),
  source_sha TEXT NOT NULL CHECK (length(source_sha) = 40)
);
