CREATE extension IF NOT EXISTS "uuid-ossp";


CREATE TABLE queen_mutex (onerow_id bool PRIMARY KEY DEFAULT TRUE,
                                                             worker_id UUID NOT NULL,
                                                                            aquired TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                                                                                                                              valid_until TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                                                                                                                                                                                    CONSTRAINT singlerow CHECK (onerow_id));


CREATE TABLE world(field VARCHAR PRIMARY KEY,
                                         world_timestamp BIGINT NOT NULL,
                                                                created TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                                                                                                                  updated TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                                                                                                                                                                    value_message_packed BYTEA NOT NULL);


CREATE TYPE queen_mutex_result AS (f1 UUID,
                                      f2 TIMESTAMP WITH TIME ZONE);

-- Try aquiring the mutex. Returns the current queen id and the contention time of the mutex.
--

CREATE OR REPLACE FUNCTION caolo_sim_try_aquire_queen_mutex(worker_id_in UUID, new_until TIMESTAMP WITH TIME ZONE) RETURNS queen_mutex_result AS $$
DECLARE
    output queen_mutex_result;
BEGIN
    INSERT INTO queen_mutex
    (valid_until, worker_id) VALUES (new_until, worker_id_in)
    ON CONFLICT (onerow_id)
    DO UPDATE
        SET valid_until = new_until, worker_id = worker_id_in, aquired = now()
        WHERE EXCLUDED.valid_until < now() AND now() < new_until
    RETURNING worker_id, valid_until
    INTO output;

    RETURN output;
END;
$$ LANGUAGE PLPGSQL;

--  SELECT caolo_sim_try_aquire_queen_mutex(uuid_generate_v4(), now() + interval '1 second'); -- smoke test
