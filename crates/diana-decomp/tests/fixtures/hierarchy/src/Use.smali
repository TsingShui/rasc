.class public final Lfixtures/Use;
.super Ljava/lang/Object;

.method public static sites(Ljava/lang/Object;)V
    .registers 3
    instance-of v0, p0, Lfixtures/Repo;
    check-cast p0, Lfixtures/Repo;
    move-object v1, p0
    invoke-interface {v1}, Lfixtures/Repo;->save()V
    const-class v1, Lfixtures/Repo;
    return-void
.end method
