package fixtures;

/** Target: a Base-typed receiver still dispatches to the Child override. */
public final class Case {
    public static int target() {
        Base b = new Child();
        return b.f();
    }
}
