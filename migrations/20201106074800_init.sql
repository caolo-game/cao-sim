CREATE extension IF NOT EXISTS "uuid-ossp";

CREATE TABLE world(field VARCHAR NOT NULL,
                   queen_tag UUID NOT NULL,
                   world_timestamp BIGINT NOT NULL,
                   created TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                   updated TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                   value_message_packed BYTEA NOT NULL,
                   PRIMARY KEY (field, queen_tag)
            );


CREATE TABLE scripting_schema (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    queen_tag UUID NOT NULL UNIQUE,
    schema_message_packed BYTEA NOT NULL
);

CREATE TABLE world_output (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    queen_tag UUID NOT NULL,
    world_time BIGINT NOT NULL,

    created TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    payload JSONB NOT NULL,

    UNIQUE (queen_tag, world_time)
);


CREATE OR REPLACE FUNCTION on_world_ouput_insert () RETURNS TRIGGER
AS $$
BEGIN
    DELETE FROM world_output
    WHERE
        id NOT IN (
            SELECT foo.id
            FROM (
                SELECT id
                FROM world_output
                ORDER BY created DESC
                -- TODO this should consider the queen_tag as well...
                LIMIT 200
            ) foo
        );

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;


CREATE TRIGGER world_cleanup AFTER INSERT
    ON world_output
    EXECUTE PROCEDURE on_world_ouput_insert();

