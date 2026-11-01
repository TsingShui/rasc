.class public Lfixtures/ShadowState;
.super Ljava/lang/Object;

.method public static loop(IZ)I
    .registers 4
    const/4 v0, 0x3
    :loop
    if-lez p0, :done
    add-int/lit8 v0, v0, 0x2
    :start
    invoke-static {p1}, Lfixtures/ThrowDep;->choose(Z)I
    move-result v1
    :end
    add-int/lit8 p0, p0, -0x1
    goto :loop
    :done
    return v0
    :handler
    move-exception v1
    return v0
    .catch Lfixtures/ThrowError; {:start .. :end} :handler
.end method

.method public static twoValues(Z)I
    .registers 4
    const/4 v0, 0x7
    const/16 v1, 0x9
    :start
    invoke-static {p0}, Lfixtures/ThrowDep;->choose(Z)I
    move-result v0
    :end
    add-int/2addr v0, v1
    return v0
    :handler
    move-exception v2
    add-int/2addr v0, v1
    return v0
    .catch Lfixtures/ThrowError; {:start .. :end} :handler
.end method
