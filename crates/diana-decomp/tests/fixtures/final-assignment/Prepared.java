package fixtures;

public class Prepared {
    public static final int N;
    public static final String S;
    public static final boolean Z;
    public static boolean b;
    public static char c;
    public static double d;
    public static float f;
    public static int i;
    public static long j;
    public static String s;
    static {
        N = 7;
        S = "constant";
        Z = false;
        b = true;
        c = '\uffff';
        d = 1.5;
        f = -0.0f;
        i = -7;
        j = Long.MIN_VALUE;
        s = "x\n\"";
    }
}
