.class public Lfixtures/ThrowState;
.super Ljava/lang/Object;

.method public static zzHandlers(Z)I
    .registers 2
    const/4 v0, 0x7
    :multi_start
    invoke-static {p0}, Lfixtures/ThrowDep;->choose(Z)I
    move-result v0
    :multi_end
    return v0
    :multi_first
    move-exception v1
    return v0
    :multi_second
    move-exception v1
    return v0
    :multi_all
    move-exception v1
    return v0
    .catch Ljava/lang/IllegalArgumentException; {:multi_start .. :multi_end} :multi_first
    .catch Lfixtures/ThrowError; {:multi_start .. :multi_end} :multi_second
    .catchall {:multi_start .. :multi_end} :multi_all
.end method

.method public static zzShared(Z)I
    .registers 2
    const/4 v0, 0x7
    :shared_start
    invoke-static {p0}, Lfixtures/ThrowDep;->choose(Z)I
    move-result v0
    :shared_end
    return v0
    :shared_handler
    move-exception v1
    return v0
    .catch Lfixtures/ThrowError; {:shared_start .. :shared_end} :shared_handler
    .catch Ljava/lang/IllegalArgumentException; {:shared_start .. :shared_end} :shared_handler
.end method

.method public static zzThrow(Z)I
    .registers 3
    const/4 v0, 0x7
    :inner_start
    invoke-static {p0}, Lfixtures/ThrowDep;->choose(Z)I
    move-result v0
    :inner_end
    return v0
    :inner_handler
    move-exception v1
    :outer_start
    throw v1
    :outer_end
    :outer_handler
    move-exception v1
    return v0
    .catch Lfixtures/ThrowError; {:inner_start .. :inner_end} :inner_handler
    .catchall {:outer_start .. :outer_end} :outer_handler
.end method

.method public static zzWide(Z)J
    .registers 4
    const-wide/16 v0, 0x7
    :wide_start
    invoke-static {p0}, Lfixtures/ThrowDep;->choose(Z)I
    move-result v0
    :wide_end
    const-wide/16 v0, 0xb
    return-wide v0
    :wide_handler
    move-exception v2
    return-wide v0
    .catch Lfixtures/ThrowError; {:wide_start .. :wide_end} :wide_handler
.end method

.method public static conservative(Z)I
    .registers 3
    :start_conservative
    div-int/lit8 v1, p0, 0x1
    const/4 v0, 0x7
    invoke-static {p0}, Lfixtures/ThrowDep;->choose(Z)I
    move-result v0
    :end_conservative
    return v0
    :handler_conservative
    move-exception v1
    return v0
    .catch Lfixtures/ThrowError; {:start_conservative .. :end_conservative} :handler_conservative
.end method

.method public static zUndefined(Z)I
    .registers 3
    :start_undefined
    invoke-static {p0}, Lfixtures/ThrowDep;->choose(Z)I
    move-result v1
    :end_undefined
    return v1
    :handler_undefined
    move-exception v1
    return v0
    .catch Lfixtures/ThrowError; {:start_undefined .. :end_undefined} :handler_undefined
.end method

.method public static overwrite(Z)I
    .registers 2
    const/4 v0, 0x7
    :start
    invoke-static {p0}, Lfixtures/ThrowDep;->choose(Z)I
    move-result v0
    :end
    return v0
    :handler
    move-exception v1
    return v0
    .catch Lfixtures/ThrowError; {:start .. :end} :handler
.end method

.method public static callInside(Z)I
    .registers 2
    const/4 v0, 0x7
    :start
    invoke-static {p0}, Lfixtures/ThrowDep;->choose(Z)I
    :end
    move-result v0
    return v0
    :handler
    move-exception v1
    return v0
    .catch Lfixtures/ThrowError; {:start .. :end} :handler
.end method

.method public static callOutside(Z)I
    .registers 2
    const/4 v0, 0x7
    invoke-static {p0}, Lfixtures/ThrowDep;->choose(Z)I
    :start
    move-result v0
    :end
    return v0
    :handler
    move-exception v1
    return v0
    .catch Lfixtures/ThrowError; {:start .. :end} :handler
.end method
