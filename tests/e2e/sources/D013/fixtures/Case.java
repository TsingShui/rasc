package fixtures;

public final class Case {

    public static boolean f(boolean a, boolean b) {
        return (a ^ b) & !a;
    }
}
