.class public Lfixtures/Conditions;
.super Ljava/lang/Object;

.method public static classNull()I
    .registers 2
    const-class v0, Lfixtures/Conditions;
    if-eqz v0, :zero
    const/4 v1, 1
    return v1
    :zero
    const/4 v1, 0
    return v1
.end method

.method public static classNonNull()I
    .registers 2
    const-class v0, Lfixtures/Conditions;
    if-nez v0, :one
    const/4 v1, 0
    return v1
    :one
    const/4 v1, 1
    return v1
.end method

.method public static sameClass()I
    .registers 2
    const-class v0, Lfixtures/Conditions;
    const-class v1, Lfixtures/Conditions;
    if-eq v0, v1, :one
    const/4 v0, 0
    return v0
    :one
    const/4 v0, 1
    return v0
.end method

.method public static classLess()I
    .registers 2
    const-class v0, Lfixtures/Conditions;
    if-ltz v0, :one
    const/4 v1, 0
    return v1
    :one
    const/4 v1, 1
    return v1
.end method
