CREATE extension IF NOT EXISTS "uuid-ossp";


CREATE TABLE world(field VARCHAR PRIMARY KEY,
                   queen_tag UUID NOT NULL,
                   world_timestamp BIGINT NOT NULL,
                   created TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                   updated TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                   value_message_packed BYTEA NOT NULL);


CREATE UNIQUE INDEX world_field_queen_unique ON world (field, queen_tag);
