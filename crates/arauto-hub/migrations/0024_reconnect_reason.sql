-- Arauto hub: why an invite was issued, and which program enrolled a device (decision 0053,
-- proposal, 19/11/2026). The reason is derived by the hub; the versions are numbers only.
ALTER TABLE invite ADD COLUMN reason TEXT CHECK (reason IS NULL OR reason IN ('quarantined', 'restored', 'request_approved', 'request_pending', 'approved_timeout', 'previous_request_expired', 'previous_request_denied', 'expired_inactive', 'revoked', 'left', 'invite_unused', 'other'));
ALTER TABLE invite ADD COLUMN version_clause TEXT CHECK (version_clause IS NULL OR version_clause IN ('numbers', 'older', 'other_build'));
ALTER TABLE invite ADD COLUMN version_from TEXT CHECK (version_from IS NULL OR version_from ~ '^[0-9]{1,9}\.[0-9]{1,9}\.[0-9]{1,9}$');
ALTER TABLE invite ADD COLUMN version_to TEXT CHECK (version_to IS NULL OR version_to ~ '^[0-9]{1,9}\.[0-9]{1,9}\.[0-9]{1,9}$');
ALTER TABLE invite ADD CONSTRAINT invite_versions_only_with_numbers
    CHECK ((version_from IS NULL) = (version_to IS NULL)
       AND (version_from IS NULL) = (version_clause IS DISTINCT FROM 'numbers'));
ALTER TABLE device ADD COLUMN enrolled_connector TEXT CHECK (enrolled_connector IS NULL OR char_length(enrolled_connector) <= 128);
ALTER TABLE device ADD COLUMN enrolled_proto INTEGER CHECK (enrolled_proto IS NULL OR enrolled_proto >= 1);
