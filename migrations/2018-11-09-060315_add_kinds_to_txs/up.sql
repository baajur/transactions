ALTER TABLE transactions
  ADD COLUMN kind VARCHAR NOT NULL,
  ADD COLUMN group_kind VARCHAR NOT NULL,
  ADD COLUMN related_tx UUID;