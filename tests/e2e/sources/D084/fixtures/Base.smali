.class public Lfixtures/Base;
.super Ljava/lang/Object;
.source "Base.smali"

# D084 dependency shell. Complete self-made parent: canonical constructor and an
# ordinary instance method get()Ljava/lang/String; so the child override has
# complete parent/overrides/access/throws facts.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public get()Ljava/lang/String;
    .registers 2

    const-string v0, "base"

    return-object v0
.end method
