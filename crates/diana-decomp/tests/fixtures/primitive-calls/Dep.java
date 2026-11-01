package fixtures;

public class Dep {
    public static int trace;
    public static long longId(long x) { trace = trace * 10 + 1; return x; }
    public static float floatId(float x) { trace = trace * 10 + 2; return x; }
    public static double doubleId(double x) { trace = trace * 10 + 3; return x; }
    public double mix(long a, float b, double c, byte d) { trace = trace * 10 + 4; return a + b + c + d; }
    public String text(String x) { trace = trace * 10 + 5; return x; }
    public void touch(double x) { trace = trace * 10 + 6; }
}
