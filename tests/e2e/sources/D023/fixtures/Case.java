package fixtures;

/** Target: interface dispatch keeps the implementation, and null fails at the call. */
public final class Case {
    public static int f(I x) {
        return x.f();
    }
}
