CREATE TABLE IF NOT EXISTS users
(
    id          INTEGER PRIMARY KEY NOT NULL,
    username    TEXT                NOT NULL
);

INSERT INTO users (id, username) VALUES (1, 'Bob');

