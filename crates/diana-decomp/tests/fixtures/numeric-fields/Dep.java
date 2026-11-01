package fixtures;
public final class Dep {
    public static boolean fail;
    public static int calls;
    public static void step() {
        calls++;
        if (fail) throw new IllegalStateException();
    }
}
