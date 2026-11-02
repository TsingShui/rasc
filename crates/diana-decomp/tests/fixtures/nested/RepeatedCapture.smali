.class public Lfixtures/Outer;
.super Ljava/lang/Object;
.annotation system Ldalvik/annotation/MemberClasses;
    value = {Lfixtures/Outer$Inner;}
.end annotation

.class public Lfixtures/Outer$Inner;
.super Ljava/lang/Object;
.annotation system Ldalvik/annotation/EnclosingClass;
    value = Lfixtures/Outer;
.end annotation
.annotation system Ldalvik/annotation/InnerClass;
    accessFlags = 0x1
    name = "Inner"
.end annotation
.field final synthetic this$0:Lfixtures/Outer;
.method public constructor <init>(Lfixtures/Outer;)V
    .registers 2
    iput-object p1, p0, Lfixtures/Outer$Inner;->this$0:Lfixtures/Outer;
    invoke-direct {p0}, Ljava/lang/Object;-><init>()V
    iput-object p1, p0, Lfixtures/Outer$Inner;->this$0:Lfixtures/Outer;
    return-void
.end method
