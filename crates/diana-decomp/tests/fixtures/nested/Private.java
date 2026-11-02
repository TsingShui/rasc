package fixtures;

public class Private {
    private static int n = 7;
    public Private() {}
    public static class Inner {
        public Inner() {}
        public int f() { return n; }
    }
}
