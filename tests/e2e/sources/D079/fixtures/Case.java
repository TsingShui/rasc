package fixtures;

/**
 * Target owner: both fields are blank finals assigned by the static block, so
 * neither carries a ConstantValue and Reader must really issue getstatic.
 */
public final class Case {
    public static final int N;
    public static final String S;

    static {
        N = 7;
        S = "x";
        Dep.log("INIT");
    }
}
