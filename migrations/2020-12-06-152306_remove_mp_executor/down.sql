CREATE TABLE world(field VARCHAR NOT NULL,
                   queen_tag UUID NOT NULL,
                   world_timestamp BIGINT NOT NULL,
                   created TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                   updated TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                   value_message_packed BYTEA NOT NULL,
                   PRIMARY KEY (field, queen_tag)
            );

