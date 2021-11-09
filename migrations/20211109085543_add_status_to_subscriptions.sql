-- Create an optional status column in the subscriptions table.
ALTER TABLE subscriptions ADD COLUMN status TEXT NULL;