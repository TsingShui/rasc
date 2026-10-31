package fixtures;

/** Target: a failing clinit surfaces as ExceptionInInitializerError, not the cause. */
public final class Case {
    public static int n = Dep.badInit();
}
