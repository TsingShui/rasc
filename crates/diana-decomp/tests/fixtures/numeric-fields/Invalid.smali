.class public Lfixtures/Invalid;
.super Ljava/lang/Object;

.field public volatile l:J
.field public volatile f:F

.method public static overwrittenLoad(Lfixtures/Invalid;)J
    .registers 3
    iget-wide v0, p0, Lfixtures/Invalid;->l:J
    const/4 v1, 0x0
    return-wide v0
.end method

.method public static partialStore(Lfixtures/Invalid;J)V
    .registers 3
    const/4 p2, 0x0
    iput-wide p1, p0, Lfixtures/Invalid;->l:J
    return-void
.end method

.method public static arrayOverwrittenLoad([J)J
    .registers 4
    const/4 v2, 0x0
    aget-wide v0, p0, v2
    const/4 v1, 0x0
    return-wide v0
.end method

.method public static arrayPartialStore([JJ)V
    .registers 4
    const/4 v0, 0x0
    const/4 p2, 0x0
    aput-wide p1, p0, v0
    return-void
.end method

.method public static wrongArray([B)I
    .registers 3
    const/4 v1, 0x0
    aget v0, p0, v1
    return v0
.end method

.method public static filledObjects(Ljava/lang/Object;Ljava/lang/Object;)[Ljava/lang/Object;
    .registers 3
    filled-new-array {p0, p1}, [Ljava/lang/Object;
    move-result-object v0
    return-object v0
.end method

.method public static filledInts(II)[I
    .registers 3
    filled-new-array {p0, p1}, [I
    move-result-object v0
    return-object v0
.end method

.method public static filledFloats(FF)[F
    .registers 3
    filled-new-array {p0, p1}, [F
    move-result-object v0
    return-object v0
.end method

.method public static filledRange(IIIIII)[I
    .registers 7
    filled-new-array/range {p0 .. p5}, [I
    move-result-object v0
    return-object v0
.end method

.method public static filledUnused(II)V
    .registers 2
    filled-new-array {p0, p1}, [I
    return-void
.end method

.method public static filledReuse(II)[I
    .registers 2
    filled-new-array {p1, p0}, [I
    move-result-object p0
    return-object p0
.end method

.method public static filledZero()[Ljava/lang/Object;
    .registers 1
    filled-new-array {}, [Ljava/lang/Object;
    move-result-object v0
    return-object v0
.end method

.method public static filledWrongResult()V
    .registers 1
    filled-new-array {}, [I
    move-result v0
    return-void
.end method

.method public static objectPayload()V
    .registers 2
    const/4 v1, 0x1
    new-array v0, v1, [Ljava/lang/Object;
    fill-array-data v0, :data
    return-void
    :data
    .array-data 4
        0x1
    .end array-data
.end method

.method public static castUnrelated(Ljava/lang/String;)Lfixtures/Box;
    .registers 1
    check-cast p0, Lfixtures/Box;
    return-object p0
.end method

.method public static testUnrelated(Ljava/lang/String;)Z
    .registers 2
    instance-of v0, p0, Lfixtures/Box;
    return v0
.end method

.method public static primitiveCast(I)V
    .registers 1
    check-cast p0, Lfixtures/Box;
    return-void
.end method

.method public static badTarget(Ljava/lang/Object;)V
    .registers 1
    check-cast p0, I
    return-void
.end method

.method public static badInstanceTarget(Ljava/lang/Object;)Z
    .registers 2
    instance-of v0, p0, I
    return v0
.end method

.method public static peerExact(Lfixtures/Box;Lfixtures/Box;)V
    .registers 2
    iput-object p1, p0, Lfixtures/Box;->peer:Lfixtures/Box;
    return-void
.end method

.method public static peerMissingProof(Lfixtures/Box;Ljava/lang/Object;)V
    .registers 2
    iput-object p1, p0, Lfixtures/Box;->peer:Lfixtures/Box;
    return-void
.end method

.method public static storeByte([BI)I
    .registers 4
    const/4 v0, 0x0
    aput-byte p1, p0, v0
    aget-byte v1, p0, v0
    return v1
.end method

.method public static booleanProof([ZI)Z
    .registers 4
    const/4 v0, 0x0
    and-int/lit8 v1, p1, 0x1
    aput-boolean v1, p0, v0
    aget-boolean v1, p0, v0
    return v1
.end method

.method public static booleanUnknown([ZI)V
    .registers 3
    const/4 v0, 0x0
    aput-boolean p1, p0, v0
    return-void
.end method

.method public static wrongKind(Lfixtures/Invalid;)J
    .registers 3
    iget-wide v0, p0, Lfixtures/Invalid;->f:F
    return-wide v0
.end method
