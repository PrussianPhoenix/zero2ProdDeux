-- migrations/20230318162402_create_users_table.sql
CREATE TABLE users(
    user_id uuid PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL
);
