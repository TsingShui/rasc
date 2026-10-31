.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.smali"

# D074 target role "parameter". f([I)V fills the argument array with the two
# I32 payload elements [7, 8] and returns. The array is not created in this
# method, so there is no FreshArray proof: Java must stay Unsupported/ArrayFillShape
# and the complete raw method must survive.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f([I)V
    .registers 1

    fill-array-data p0, :array_two

    return-void

    :array_two
    .array-data 4
        0x7
        0x8
    .end array-data
.end method
