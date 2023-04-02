-- Add migration script here
INSERT INTO users(user_id, username, password_hash)
VALUES (
    'a8f72c49-2fb2-445a-a86a-9605a4b066d1',
    'admin',
    '$argon2id$v=19$m=15000,t=2,p=1$8EghJZ952zZ+TrvpN+NAGw$6Quktanx67cHJKysPQ6DAeGv3FGEj99NYyA8d5B27rw'
);

