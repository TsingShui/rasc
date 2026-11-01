package fixtures;

public final class Dep {
    public static String trace = "";
    public static void parent(String text, long value) { trace += "P:" + text + ":" + value + ";"; if (text == null) throw new IllegalStateException(); }
    public static void body(float a, double b, boolean c, byte d) { trace += "B:" + Float.floatToRawIntBits(a) + ":" + Double.doubleToRawLongBits(b) + ":" + c + ":" + d + ";"; }
}
