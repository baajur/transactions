CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
ALTER TABLE transactions ADD COLUMN gid UUID NOT NULL DEFAULT uuid_generate_v4();
