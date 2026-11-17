-- Arauto hub: a WhatsApp message wakes a Hermes by the verb "be woken" alone
-- (docs/decisoes/0049-acordar-pelo-whatsapp-so-pelo-verbo.md). The switch of the Hermes goes away.
-- Until now a message woke a Hermes only when the switch AND the verb held. So that no Hermes starts
-- being woken by WhatsApp because of the upgrade, the verb is turned off, on the accesses and on the
-- lines of the chats, of every Hermes whose switch was off (removed ones too). Turning "be woken" off
-- never breaks CHECK (NOT can_wake OR can_read). This migration writes no audit event: the list given
-- to the admin before the upgrade and the backup taken by `migrate` are the record (decision 0049).

UPDATE access_chat SET can_wake = FALSE
 WHERE can_wake
   AND access_id IN (SELECT a.id FROM access a
                     JOIN hermes h ON h.organization_id = a.organization_id AND h.id = a.hermes_id
                     WHERE NOT h.can_be_woken);

UPDATE access SET can_wake = FALSE
 WHERE can_wake
   AND (organization_id, hermes_id) IN (SELECT organization_id, id FROM hermes WHERE NOT can_be_woken);

ALTER TABLE hermes DROP COLUMN can_be_woken;
ALTER TABLE hermes DROP COLUMN wake_reason;
