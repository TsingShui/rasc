package fixtures;

public final class Box extends Parent {
    public Box(String text) { this(text, 7L, -0.0f, 0.5d, true, (byte)-2); }
    public Box(String text, long a, float b, double c, boolean d, byte e) {
        super(text, a);
        Dep.body(b, c, d, e);
    }
}
