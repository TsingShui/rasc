.class public Lfixtures/Case;
.super Lfixtures/Base;
.source "Case.smali"

# D084 original input. The class extends the complete self-made Base and holds
# three members:
#   <init>()V                     canonical Object super constructor
#   get()Ljava/lang/String;       ordinary override returning the child string
#   get()Ljava/lang/Object;       marked bridge|synthetic, but it is NOT the
#                                 canonical bridge: it first runs
#                                 invoke-static {v1}, Lfixtures/Dep;->log(Ljava/lang/String;)V
#                                 with the const-string "BRIDGE" and only then
#                                 invoke-virtual get()Ljava/lang/String; on the
#                                 same receiver and returns the result.
# A source override would generate the bridge without the log call, so the
# canonical-consumable pattern does not hold and this member must stay raw.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Lfixtures/Base;-><init>()V

    return-void
.end method

.method public get()Ljava/lang/String;
    .registers 2

    const-string v0, "child"

    return-object v0
.end method

.method public bridge synthetic get()Ljava/lang/Object;
    .registers 3

    const-string v1, "BRIDGE"

    invoke-static {v1}, Lfixtures/Dep;->log(Ljava/lang/String;)V

    invoke-virtual {p0}, Lfixtures/Case;->get()Ljava/lang/String;

    move-result-object v0

    return-object v0
.end method
