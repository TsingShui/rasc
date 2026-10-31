.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.java"

# D073 original input form. f([I)V holds exactly one fill-array-data of the
# parameter array with payload count 0 and element_width 4, then return-void.
# There is no handler and no other effect, so the only observable behavior is
# the length read: a non-null array completes normally without writes and a null
# array throws NPE. The payload must really declare count 0; a smali assembler
# that drops the empty payload changes the input.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f([I)V
    .registers 1

    fill-array-data p0, :array_zero

    return-void

    :array_zero
    .array-data 4
    .end array-data
.end method
