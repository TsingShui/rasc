package fixtures;

public final class ThrowCase {
    public static void nil() { throw null; }
    public static void unchecked(RuntimeException error) { throw error; }
    public static int branch(boolean fail, RuntimeException error) {
        if (fail) throw error;
        return 7;
    }
    public static void checked(Exception error) throws Exception { throw error; }
}
