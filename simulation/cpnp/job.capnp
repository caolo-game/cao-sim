@0xc7d2eeb74935b4d2;

struct Uuid
{
    part0 @0: UInt64;
    part1 @1: UInt64;
}

struct ScriptBatchJob
{
    msgId @0: Uuid;
    fromIndex @1: UInt32;
    toIndex @2: UInt32;
}
