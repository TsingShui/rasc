.class public Lfixtures/Dep;
.super Ljava/lang/Object;
.source "Dep.smali"

# D084 dependency shell. The visible-effect target used by the non-canonical
# bridge. It only records the argument so the call has a real declaration; the
# case does not execute it.

.field public static last:Ljava/lang/String;


.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static log(Ljava/lang/String;)V
    .registers 1

    sput-object p0, Lfixtures/Dep;->last:Ljava/lang/String;

    return-void
.end method
