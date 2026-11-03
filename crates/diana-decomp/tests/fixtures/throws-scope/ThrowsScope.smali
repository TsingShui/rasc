.class public Lfixtures/ThrowsScope;
.super Ljava/lang/Object;

.method public static plain()I
    .registers 1
    const/4 v0, 0x7
    return v0
.end method

.method public static signature()I
.annotation system Ldalvik/annotation/Signature;
value = {"()I"}
.end annotation
    .registers 1
    const/4 v0, 0x7
    return v0
.end method

.method public static custom()I
.annotation runtime Lfixtures/Marker;
value = "kept"
.end annotation
    .registers 1
    const/4 v0, 0x7
    return v0
.end method
