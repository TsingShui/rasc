.class public Lfixtures/Bootstrap;
.super Ljava/lang/Object;
.source "Bootstrap.smali"

# D085 dependency shell. The declared bootstrap target of the invoke-custom
# call site. Only the declaration is part of the input facts: the case never
# executes this method and no CallSite is constructed. It exists so the
# method_handle reference and the call-site metadata are complete and legal.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static link(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;)Ljava/lang/invoke/CallSite;
    .registers 4

    const/4 v0, 0x0

    return-object v0
.end method
