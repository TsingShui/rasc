.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.smali"

# D057 original input. f()I has registers_size=1 with ins/outs/tries all zero and
# its single instruction is return v0 (code unit 0x000f). Nothing defines v0: no
# parameter word and no earlier instruction writes it, so the return reads an
# undefined register. Every other structure of the owner (constructor, proto,
# string/type/proto/method references, code range) is independently valid. The
# intended rejection is this reachable undefined use, not a missing catalog.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f()I
    .registers 1

    return v0
.end method
