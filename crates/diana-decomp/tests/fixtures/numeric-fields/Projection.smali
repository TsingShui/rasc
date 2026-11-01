.class public Lfixtures/Projection;
.super Ljava/lang/Object;

.method public static callRows(I)I
    .registers 2
    invoke-static {p0}, Lfixtures/Projection;->newDetached(I)[[[Lfixtures/Detached;
    move-result-object v0
    array-length v0, v0
    return v0
.end method

.method public static negativeZero()[F
    .registers 3
    const/4 v2, 0x1
    new-array v0, v2, [F
    const v1, -0x80000000
    const/4 v2, 0x0
    aput v1, v0, v2
    return-object v0
.end method

.method public static callFloat()F
    .registers 2
    invoke-static {}, Lfixtures/Projection;->negativeZero()[F
    move-result-object v0
    const/4 v1, 0x0
    aget v0, v0, v1
    return v0
.end method

.method public static castLeaf(Ljava/lang/Object;)Ljava/lang/Object;
    .registers 3
    check-cast p0, [[[Lfixtures/Detached;
    const/4 v1, 0x0
    aget-object v0, p0, v1
    aget-object v0, v0, v1
    aget-object v0, v0, v1
    return-object v0
.end method

.method public static unusedCast(Ljava/lang/Object;)V
    .registers 1
    check-cast p0, [[[Lfixtures/Detached;
    return-void
.end method

.method public static isDetached(Ljava/lang/Object;)Z
    .registers 2
    instance-of v0, p0, [[[Lfixtures/Detached;
    return v0
.end method

.method public static badWide([[I)J
    .registers 4
    const/4 v2, 0x0
    aget-wide v0, p0, v2
    return-wide v0
.end method

.method public static swap([[[Lfixtures/Detached;[[[Lfixtures/Detached;I)Ljava/lang/Object;
    .registers 8
    const/4 v4, 0x0
    aget-object v0, p0, v4
    aget-object v1, p1, v4
    :loop
    if-lez p2, :done
    move-object v2, v0
    move-object v0, v1
    move-object v1, v2
    add-int/lit8 p2, p2, -0x1
    goto :loop
    :done
    aget-object v0, v0, v4
    aget-object v0, v0, v4
    return-object v0
.end method

.method public static newDetached(I)[[[Lfixtures/Detached;
    .registers 2
    new-array v0, p0, [[[Lfixtures/Detached;
    return-object v0
.end method

.method public static nested([[III)I
    .registers 4
    aget-object v0, p0, p1
    aget v0, v0, p2
    return v0
.end method

.method public static matrixStore([[ILjava/lang/Object;)V
    .registers 3
    const/4 v0, 0x0
    aput-object p1, p0, v0
    return-void
.end method

.method public static stringsGet([Ljava/lang/String;I)Ljava/lang/String;
    .registers 3
    aget-object v0, p0, p1
    return-object v0
.end method

.method public static stringsStore([Ljava/lang/String;Ljava/lang/Object;)V
    .registers 3
    const/4 v0, 0x0
    aput-object p1, p0, v0
    return-void
.end method

.method public static filledTypedStrings(Ljava/lang/String;Ljava/lang/String;)[Ljava/lang/String;
    .registers 3
    filled-new-array {p0, p1}, [Ljava/lang/String;
    move-result-object v0
    return-object v0
.end method

.method public static filledStrings(Ljava/lang/Object;Ljava/lang/Object;)[Ljava/lang/String;
    .registers 3
    filled-new-array {p0, p1}, [Ljava/lang/String;
    move-result-object v0
    return-object v0
.end method
