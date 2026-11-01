package fixtures;
public final class Detached {
    static { System.setProperty("diana.detached.initialized", "yes"); }
    public static void touch() {}
}
