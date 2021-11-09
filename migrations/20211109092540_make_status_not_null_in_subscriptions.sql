-- Mark status as not NULL
-- Begin SQL transaction.
BEGIN;
  -- Change status from NULL to 'confirmed'
  UPDATE subscriptions SET status = 'confirmed' WHERE status IS NULL;
  -- Make status mandatory
  ALTER TABLE subscriptions ALTER COLUMN status SET NOT NULL;
COMMIT;