CREATE TABLE IF NOT EXISTS subscription_tokens (
  subscription_token TEXT NOT NULL,
  subscriber_id uuid NOT NULL,
  PRIMARY KEY (subscription_token),
  FOREIGN KEY (subscriber_id) REFERENCES subscriptions (id)
);
