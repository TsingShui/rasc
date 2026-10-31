.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.smali"

# D074 target role "too-short". f()[I allocates int[1] and then fills it with
# two I32 payload elements [7, 8]. The allocation is a FreshArray, but the
# proven length 1 is smaller than count 2, so the sufficient-length condition of
# the FillPlan fails and Java must stay Unsupported/ArrayFillShape.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f()[I
    .registers 1

    const/4 v0, 0x1

    new-array v0, v0, [I

    fill-array-data v0, :array_two

    return-object v0

    :array_two
    .array-data 4
        0x7
        0x8
    .end array-data
.end method
