CREATE TABLE scripting_schema (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    queen_tag UUID NOT NULL UNIQUE,
    schema_message_packed BYTEA NOT NULL
);
