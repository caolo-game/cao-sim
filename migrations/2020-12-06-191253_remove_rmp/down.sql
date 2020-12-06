DELETE FROM scripting_schema;
ALTER TABLE scripting_schema DROP COLUMN payload;
ALTER TABLE scripting_schema DROP COLUMN created;
ALTER TABLE scripting_schema ADD COLUMN schema_message_packed BYTEA NOT NULL;
