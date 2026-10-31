.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.smali"

# D074 target role "escaped". f()[I allocates int[2], publishes the array with
# Lfixtures/Dep;->publish([I)V and then fills it with the two I32 payload elements
# [7, 8]. The allocation and length are sufficient, but the array has already
# escaped before the fill, so the no-escape condition of the FreshArray proof
# fails and Java must stay Unsupported/ArrayFillShape. The catalog holds the
# exact external declaration of publish; Dep has no class_def.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f()[I
    .registers 1

    const/4 v0, 0x2

    new-array v0, v0, [I

    invoke-static {v0}, Lfixtures/Dep;->publish([I)V

    fill-array-data v0, :array_two

    return-object v0

    :array_two
    .array-data 4
        0x7
        0x8
    .end array-data
.end method
