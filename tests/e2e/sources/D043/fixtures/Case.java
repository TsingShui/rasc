package fixtures;

/** Target: the argument effect must happen once, before the constructor call. */
public final class Case {
    public static Box f() {
        int n = Dep.arg();
        return new Box(n);
    }
}
