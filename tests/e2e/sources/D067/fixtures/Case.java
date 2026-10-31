package fixtures;

/** Target: exactly two Dep.f() invoke sites in one method, at distinct PCs. */
public final class Case {
    public static void target() {
        Dep.f();
        Dep.f();
    }
}
