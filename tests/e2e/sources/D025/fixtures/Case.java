package fixtures;

/** Target: the cast result is unused, so only the runtime check is observable. */
public final class Case {
    public static void f(Object a) {
        String s = (String) a;
    }
}
