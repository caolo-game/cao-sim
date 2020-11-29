@0xc7d2eeb74935b4d2;

struct Timestamp
{
    valueMs @0: Int64;
}

struct ScriptBatchJob
{
    fromIndex @0: UInt32;
    toIndex @1: UInt32;

    # Timestamp of the World the worker is expected to use
    worldTime @2: UInt64;
}

struct ScriptBatchResult
{
    intents @0: List(BotIntents);
    # Timestamp of the World the worker used
    worldTime @1: UInt64;
}

struct BotIntents
{
    entityId @0: UInt32;
    # JSON data
    payload @1: Data;
}
