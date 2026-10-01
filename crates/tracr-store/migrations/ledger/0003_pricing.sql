-- tracr ledger schema v3 (architecture §8.3): pricing catalogs by content.
-- Additive. A catalog is copied here on first use, keyed by its content hash,
-- so every estimate's pricing revision stays resolvable after the app's
-- bundled catalog changes. Insert-only.

CREATE TABLE pricing_catalog (
  revision TEXT PRIMARY KEY,                 -- "<catalog_version>+<blake3 prefix>"
  catalog_version TEXT NOT NULL,
  content_hash TEXT NOT NULL UNIQUE,         -- BLAKE3 of the catalog text
  content TEXT NOT NULL,                     -- the catalog text as bundled
  first_used_at INTEGER NOT NULL
);

CREATE TRIGGER pricing_catalog_insert_only_u BEFORE UPDATE ON pricing_catalog
  BEGIN SELECT RAISE(ABORT, 'pricing catalogs are insert-only'); END;
CREATE TRIGGER pricing_catalog_insert_only_d BEFORE DELETE ON pricing_catalog
  BEGIN SELECT RAISE(ABORT, 'pricing catalogs are insert-only'); END;
