.class public Lfixtures/Own;
.super Ljava/lang/Object;
.source "Own.smali"

# D066 original input. The class defines exactly two members: the canonical
# Object constructor and the static f()V. The constructor's invoke-direct
# references Ljava/lang/Object;-><init>()V and f's invoke-static references
# Lfixtures/External;->g()V; neither referenced owner has a class_def, so both
# entries exist only as method_ids references. The defined-symbol query must list
# the two class_data definitions and neither of the two references.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f()V
    .registers 0

    invoke-static {}, Lfixtures/External;->g()V

    return-void
.end method
