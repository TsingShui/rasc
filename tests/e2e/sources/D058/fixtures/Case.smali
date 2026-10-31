.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.smali"

# D058 original input. f()J has registers_size=2, ins/outs/tries all zero and
# code units [0x0016, 0x0000, 0x1112, 0x0010]:
#   const-wide/16 v0, 0   (0x0016 0x0000)
#   const/4 v1, 1         (0x1112)
#   return-wide v0        (0x0010)
# Both register words are in range, but the const/4 writes the high half of the
# wide pair (v0,v1) and invalidates the original wide definition. The owner
# constructor, proto, refs and byte ranges are otherwise valid.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f()J
    .registers 2

    const-wide/16 v0, 0x0

    const/4 v1, 0x1

    return-wide v0
.end method
