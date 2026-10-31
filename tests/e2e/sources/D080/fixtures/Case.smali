.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.smali"

# D080 original input. f()I runs exactly one read of the external field
# Lfixtures/External;->C:I (sget v0) and returns v0. The field is declared
# public static final in both catalog variants; only JavaConstantKind differs
# (Constant vs Unknown). The DEX ref and metadata are complete and valid, and
# External has no class_def: the field id is a reference only.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f()I
    .registers 1

    sget v0, Lfixtures/External;->C:I

    return v0
.end method
