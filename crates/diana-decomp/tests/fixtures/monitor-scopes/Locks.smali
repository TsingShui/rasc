.class public Lfixtures/Locks;
.super Ljava/lang/Object;

.method public constructor <init>()V
    .registers 1
    invoke-direct {p0}, Ljava/lang/Object;-><init>()V
    return-void
.end method

.method public static mismatch(Ljava/lang/Object;Ljava/lang/Object;)I
    .registers 3
    monitor-enter p0
    const/4 v0, 1
    monitor-exit p1
    return v0
.end method

.method public static missing(Ljava/lang/Object;)I
    .registers 2
    monitor-enter p0
    const/4 v0, 1
    return v0
.end method

.method public static extra(Ljava/lang/Object;)I
    .registers 2
    monitor-exit p0
    const/4 v0, 1
    return v0
.end method

.method public static multipleEntry(Ljava/lang/Object;I)I
    .registers 3
    if-eqz p1, :body
    monitor-enter p0
    :body
    const/4 v0, 1
    monitor-exit p0
    return v0
.end method

.method public static ambiguous(Ljava/lang/Object;Ljava/lang/Object;I)I
    .registers 4
    if-eqz p2, :other
    move-object v0, p0
    goto :join
    :other
    move-object v0, p1
    :join
    monitor-enter v0
    monitor-exit v0
    const/4 v0, 1
    return v0
.end method

.method public synchronized implicitInstance()Z
    .registers 2
    invoke-static {p0}, Ljava/lang/Thread;->holdsLock(Ljava/lang/Object;)Z
    move-result v0
    return v0
.end method

.method public static synchronized implicitStatic()Z
    .registers 1
    const-class v0, Lfixtures/Locks;
    invoke-static {v0}, Ljava/lang/Thread;->holdsLock(Ljava/lang/Object;)Z
    move-result v0
    return v0
.end method

.method public synchronized implicitThrow()V
    .registers 2
    const/4 v0, 0
    throw v0
.end method

.method public static declared-synchronized declaredOnly(Ljava/lang/Object;)Z
    .registers 2
    invoke-static {p0}, Ljava/lang/Thread;->holdsLock(Ljava/lang/Object;)Z
    move-result v0
    return v0
.end method
