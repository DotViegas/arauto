-- Arauto hub: the instruction page of an invite (docs/decisoes/0042).
-- The handle in the address of the page is kept only as a digest, and only while the page serves.
ALTER TABLE invite ADD COLUMN join_digest BYTEA
    CHECK (join_digest IS NULL OR octet_length(join_digest) = 32);
ALTER TABLE invite ADD CONSTRAINT invite_join_page_only_while_open
    CHECK (join_digest IS NULL OR state IN ('issued', 'used'));
CREATE UNIQUE INDEX invite_join_digest ON invite (join_digest) WHERE join_digest IS NOT NULL;
