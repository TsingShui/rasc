.class public Lfixtures/SharedTail;
.super Ljava/lang/Object;
.method public static f(Z)I
    .registers 2
    :start
    invoke-static {p0}, Lfixtures/Dep;->choose(Z)I
    move-result v0
    :end
    return v0
    :first
    move-exception v0
    goto :tail
    :second
    move-exception v0
    :tail
    const/4 v0, 0x0
    invoke-static {v0}, Lfixtures/Dep;->cleanup(Z)V
    const/4 v0, 0x5
    return v0
    .catch Lfixtures/Checked; {:start .. :end} :first
    .catchall {:start .. :end} :second
.end method
