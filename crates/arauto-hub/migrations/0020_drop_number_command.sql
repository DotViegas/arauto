-- Arauto hub: the "command" mark of a number goes (design of 17/11/2026, D-C).
-- The mark said nothing about whether the number gives orders or takes them, and it only raised a
-- warning. There is no carry-over: what is recorded about orders is recorded per Hermes, later.
-- Past "number.added" audit events keep the flag they were written with.

ALTER TABLE number DROP COLUMN command;
