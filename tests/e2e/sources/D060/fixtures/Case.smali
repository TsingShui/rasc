.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.smali"

# D060 original input. The class declares the canonical Object constructor and
# two ordinary static methods that differ only in their return descriptor:
#   f()I returns 1
#   f()J returns 2
# Neither method carries a bridge or synthetic flag. The two protos are distinct
# method_ids entries but have equal name and parameter lists, so Java source
# cannot declare both. The Mixed result comes from that representability
# conflict, not from a missing constructor or catalog entry.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f()I
    .registers 1

    const/4 v0, 0x1

    return v0
.end method

.method public static f()J
    .registers 2

    const-wide/16 v0, 0x2

    return-wide v0
.end method
