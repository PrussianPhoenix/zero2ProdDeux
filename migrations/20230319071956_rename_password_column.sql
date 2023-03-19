-- migrations/20230319071956_rename_password_column.sql
ALTER TABLE users RENAME password TO password_hash;