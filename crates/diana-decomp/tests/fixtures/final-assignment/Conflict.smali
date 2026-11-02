.class public Lfixtures/Prepared;
.super Ljava/lang/Object;
.field public static final N:I = 7

.method public constructor <init>()V
    .registers 1
    invoke-direct {p0}, Ljava/lang/Object;-><init>()V
    return-void
.end method

.method static constructor <clinit>()V
    .registers 1
    const/4 v0, 1
    sput v0, Lfixtures/Prepared;->N:I
    return-void
.end method
