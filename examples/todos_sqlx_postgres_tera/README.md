# Before running this example

Before running this example you should run this SQL script:

```sql
CREATE TABLE IF NOT EXISTS todos (
id SERIAL PRIMARY KEY,
login VARCHAR(255) NOT NULL,
title VARCHAR(255) NOT NULL,
description VARCHAR(255) NOT NULL,
checked BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS accounts (
login VARCHAR(255) NOT NULL,
password VARCHAR(255) NOT NULL
);
```

# Features

1) **No** JS
2) Login system based on [Cookies](https://github.com/imbolc/tower-cookies) (don't use in production)
3) Using [Tera](https://github.com/Keats/tera) Template Engine
4) This example doesn't use ORM instead of it uses [SQLX](https://github.com/launchbadge/sqlx) based on SQL queries
