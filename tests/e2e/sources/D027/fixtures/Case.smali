.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.java"

# D027 original input form. Both methods fix the DEX shape the case requires:
# new-array immediately followed by one fill-array-data payload, no call, no
# handler and no other entry point. a() uses width 4 with the int payload
# [-1,0,2147483647]; b() uses width 4 with the float bit payload
# [0x80000000,0x3f800000] (-0.0f and 1.0f).

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static a()[I
    .registers 1

    const/4 v0, 0x3

    new-array v0, v0, [I

    fill-array-data v0, :array_a

    return-object v0

    :array_a
    .array-data 4
        -0x1
        0x0
        0x7fffffff
    .end array-data
.end method

.method public static b()[F
    .registers 1

    const/4 v0, 0x2

    new-array v0, v0, [F

    fill-array-data v0, :array_b

    return-object v0

    :array_b
    .array-data 4
        0x80000000
        0x3f800000
    .end array-data
.end method
