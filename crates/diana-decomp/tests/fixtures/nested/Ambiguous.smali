.class public Lfixtures/First;
.super Ljava/lang/Object;
.annotation system Ldalvik/annotation/MemberClasses;
    value = {Lfixtures/First$Child;}
.end annotation

.class public Lfixtures/First$Child;
.super Ljava/lang/Object;
.annotation system Ldalvik/annotation/EnclosingClass;
    value = Lfixtures/First;
.end annotation
.annotation system Ldalvik/annotation/InnerClass;
    accessFlags = 0x1
    name = "Child"
.end annotation

.class public Lfixtures/Second;
.super Ljava/lang/Object;
.annotation system Ldalvik/annotation/MemberClasses;
    value = {Lfixtures/First$Child;}
.end annotation
