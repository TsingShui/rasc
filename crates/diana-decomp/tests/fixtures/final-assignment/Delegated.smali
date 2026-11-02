.class public Lfixtures/Case;
.super Ljava/lang/Object;
.field public final x:I

.method public constructor <init>()V
    .registers 2
    const/4 v0, 7
    invoke-direct {p0, v0}, Lfixtures/Case;-><init>(I)V
    return-void
.end method

.method public constructor <init>(I)V
    .registers 2
    invoke-direct {p0}, Ljava/lang/Object;-><init>()V
    iput p1, p0, Lfixtures/Case;->x:I
    return-void
.end method
