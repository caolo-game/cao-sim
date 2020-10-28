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
    
    # Timestamp of the World the worker is expected to use
    worldTime @3: UInt64;
}

struct ScriptBatchResult
{
    msgId @0: Uuid;
    intents @1: List(BotIntents);
}

struct BotIntents
{
    entityId @0: UInt32;
    # JSON data
    payload @1: Data;
}
