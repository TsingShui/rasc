# Self-made 039 source for the invoke-polymorphic raw-profile regression.
# Assemble with smali 2.5.2 into poly.dex.
.class public final Lfixtures/Poly;
.super Ljava/lang/Object;

.method public constructor <init>()V
    .registers 1
    invoke-direct {p0}, Ljava/lang/Object;-><init>()V
    return-void
.end method

# Two argument words, then the call-site prototype (I)I describes them.
.method public static call(Ljava/lang/invoke/MethodHandle;I)I
    .registers 3
    invoke-polymorphic {p0, p1}, Ljava/lang/invoke/MethodHandle;->invokeExact([Ljava/lang/Object;)Ljava/lang/Object;, (I)I
    move-result p1
    return p1
.end method

# A prototype word count that disagrees with the register list must be rejected.
.method public static mismatch(Ljava/lang/invoke/MethodHandle;I)I
    .registers 3
    invoke-polymorphic {p0}, Ljava/lang/invoke/MethodHandle;->invokeExact([Ljava/lang/Object;)Ljava/lang/Object;, ()I
    move-result p1
    return p1
.end method
