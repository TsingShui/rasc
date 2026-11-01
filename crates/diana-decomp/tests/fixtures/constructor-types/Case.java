package fixtures;

public final class Case {
    public static Box make(String text, long a, float b, double c, boolean d, byte e) { return new Box(text, a, b, c, d, e); }
    public static Box simple(String text) { return new Box(text); }
}
