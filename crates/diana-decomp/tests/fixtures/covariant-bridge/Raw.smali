.class public abstract Lfixtures/Raw;
.super Ljava/lang/Object;

.method public constructor <init>()V
    .registers 1
    invoke-direct {p0}, Ljava/lang/Object;-><init>()V
    return-void
.end method

.method public abstract identity(Ljava/lang/Object;)Ljava/lang/Object;
    .annotation system Ldalvik/annotation/Signature;
        value = {"<T:Ljava/lang/Object;>", "(TT;)TT;"}
    .end annotation
.end method

.method public abstract values()Ljava/util/List;
    .annotation system Ldalvik/annotation/Signature;
        value = {"()Ljava/util/List<", "Ljava/lang/String;", ">;"}
    .end annotation
.end method
