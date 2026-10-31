.class public Lfixtures/Case;
.super Ljava/lang/Object;
.source "Case.smali"

# D061 original input. f(I)I is a valid composite raw method that must survive an
# independent reassembly:
#   - a packed-switch payload with keys 0 and 1 and a distinct default target;
#   - a const-string whose value is the two UTF-16 units [U+D800, U+0041], an
#     isolated high surrogate followed by a normal unit, so a lossy UTF-8
#     round trip changes the u16 vector;
#   - one try region with two typed handlers and a catchall; the declared handler
#     order fixes the handler priority. The two typed catch types are self-made
#     and unrelated to each other, so no handler is shadowed by an earlier
#     supertype match and every declared priority is reachable;
#   - one reachable external invoke, Lfixtures/External;->g()V, whose owner has
#     no class_def, so the binding catalog is exactly missing that declaration and
#     the method outcome is Unsupported -> complete raw.
# The class shell declares only the canonical Object constructor. No source
# engine and no execution oracle is claimed for this hand-written graph.

.method public constructor <init>()V
    .registers 1

    invoke-direct {p0}, Ljava/lang/Object;-><init>()V

    return-void
.end method

.method public static f(I)I
    .registers 3

    const-string v0, "\ud800\u0041"

    packed-switch p0, :packed_switch

    :default_case
    const/4 v1, 0x3

    goto :merge

    :branch_zero
    const/4 v1, 0x1

    goto :merge

    :branch_one
    :try_start
    invoke-static {}, Lfixtures/External;->g()V
    :try_end
    const/4 v1, 0x2

    :merge
    return v1

    :handler_err_a
    move-exception v1

    const/4 v1, 0x4

    goto :merge

    :handler_err_b
    move-exception v1

    const/4 v1, 0x5

    goto :merge

    :handler_catchall
    move-exception v1

    const/4 v1, 0x6

    goto :merge

    :packed_switch
    .packed-switch 0x0
        :branch_zero
        :branch_one
    .end packed-switch

    .catch Lfixtures/ErrA; {:try_start .. :try_end} :handler_err_a
    .catch Lfixtures/ErrB; {:try_start .. :try_end} :handler_err_b
    .catchall {:try_start .. :try_end} :handler_catchall
.end method
