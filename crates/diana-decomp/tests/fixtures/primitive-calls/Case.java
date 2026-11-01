package fixtures;

public final class Case {
    public static long longCall(long x) { return Dep.longId(x); }
    public static float floatCall(float x) { return Dep.floatId(x); }
    public static double doubleCall(double x) { return Dep.doubleId(x); }
    public static float literalFloat() { return Dep.floatId(-0.0f); }
    public static double literalDouble() { return Dep.doubleId(-0.0d); }
    public static double mixed(Dep receiver, long a, float b, double c, byte d) { return receiver.mix(a, b, c, d); }
    public static String text(Dep receiver, String text) { return receiver.text(text); }
    public static void unused(float x) { Dep.floatId(x); }
    public static void receiverVoid(Dep receiver, double x) { receiver.touch(x); }
}
