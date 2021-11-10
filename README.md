# Supporting Course Code

https://github.com/LukeMathWalker/zero-to-production

# Requirements

- docker
- [just](https://github.com/casey/just) (optional)
- [psql](https://blog.timescale.com/blog/how-to-install-psql-on-mac-ubuntu-debian-windows/)
- sqlx (see instructions below)

# sqlx-cli

Install the CLI:

```bash
cargo install sqlx-cli --version=0.5.7 --no-default-features --features postgres
```

Creating a migration:

```bash
# DATABASE_URL needs to be exported only if direnv hasn't been set up yet.
export DATABASE_URL=postgres://postgres:password@localhost:5432/newsletter
sqlx migrate add create_subscriptions_table
```

Run migration:

```bash
sqlx migrate run
```
