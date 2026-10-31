.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.smali"

# D081 original input. f()V uses one local int register as the loop flag:
#   const/4 v1, 0x1
#   :again
#   new-instance v0, Ljava/lang/Object;   (adjacent invoke-direct <init>)
#   if-eqz v1, :exit
#   monitor-enter v0
#   const/4 v1, 0x0
#   goto :again
#   :exit
#   monitor-exit v0
#   return-void
# The backedge re-executes the same static new-instance/init site, so the object
# released at exit is a *different dynamic object* from the one locked on the
# earlier iteration. There is no encoded handler, the Object constructor
# declaration is complete, and every bytecode format/ref/branch is legal.
# The flag is a local register only: the case declares no field and no static
# state, so no heap write or class-initialization effect participates in the
# lock mismatch.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f()V
    .registers 2

    const/4 v1, 0x1

    :again
    new-instance v0, Ljava/lang/Object;

    invoke-direct {v0}, Ljava/lang/Object;-><init>()V

    if-eqz v1, :exit

    monitor-enter v0

    const/4 v1, 0x0

    goto :again

    :exit
    monitor-exit v0

    return-void
.end method
