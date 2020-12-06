DELETE FROM scripting_schema;
ALTER TABLE scripting_schema ADD COLUMN payload JSONB NOT NULL;
ALTER TABLE scripting_schema ADD COLUMN created TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now();
ALTER TABLE scripting_schema DROP COLUMN schema_message_packed;
