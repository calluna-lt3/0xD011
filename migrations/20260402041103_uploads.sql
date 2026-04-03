CREATE TABLE IF NOT EXISTS uploads
(
    hash            CHAR(64)            PRIMARY KEY NOT NULL,
    owner           VARCHAR(15)         NOT NULL,
    extension       VARCHAR(255)        NOT NULL,
    time_uploaded   DATETIME            NOT NULL
);
