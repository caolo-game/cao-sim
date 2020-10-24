@0xc7d2eeb74935b4d2;

struct Uuid
{
    data @0: Data;
}

struct Timestamp
{
    valueMs @0: Int64;
}

struct ScriptBatchJob
{
    msgId @0: Uuid;
    fromIndex @1: UInt32;
    toIndex @2: UInt32;
}

struct ScriptBatchResult
{
    msgId @0: Uuid;
    payload: union {
        startTime @1: Timestamp;
        intents @2: List(BotIntents);
    }
}

struct BotIntents
{
    entityId @0: UInt32;
    # JSON data
    payload @1: Data;
}
