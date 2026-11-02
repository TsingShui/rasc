.class public Lfixtures/StringMapper;
.super Ljava/lang/Object;
.implements Lfixtures/Mapper;
.annotation system Ldalvik/annotation/Signature;
    value = {"Ljava/lang/Object;", "Lfixtures/Mapper<Ljava/lang/String;>;"}
.end annotation
.method public constructor <init>()V
    .registers 1
    invoke-direct {p0}, Ljava/lang/Object;-><init>()V
    return-void
.end method
.method public map(Ljava/lang/String;)Ljava/lang/String;
    .registers 2
    return-object p1
.end method
.method public bridge synthetic map(Ljava/lang/Object;)Ljava/lang/Object;
    .registers 3
    check-cast p1, Ljava/lang/String;
    invoke-virtual {p0, p1}, Lfixtures/StringMapper;->map(Ljava/lang/String;)Ljava/lang/String;
    move-result-object v0
    return-object v0
.end method
