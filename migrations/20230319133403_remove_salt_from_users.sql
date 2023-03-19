-- migrations/20230319133403_remove_salt_from_users.sql

ALTER TABLE users DROP COLUMN salt;