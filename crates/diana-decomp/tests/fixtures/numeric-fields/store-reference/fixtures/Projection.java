package fixtures;
public class Projection {
    public static int callRows(int count) { return newDetached(count).length; }
    public static float[] negativeZero() { return new float[]{-0.0f}; }
    public static float callFloat() { return negativeZero()[0]; }
    public static Object castLeaf(Object value) { return ((Detached[][][])value)[0][0][0]; }
    public static void unusedCast(Object value) { Detached[][][] ignored = (Detached[][][])value; }
    public static boolean isDetached(Object value) { return value instanceof Detached[][][]; }
    public static Object swap(Detached[][][] a, Detached[][][] b, int count) {
        Detached[][] x = a[0]; Detached[][] y = b[0];
        while (count > 0) { Detached[][] t = x; x = y; y = t; count--; }
        return x[0][0];
    }
    public static Detached[][][] newDetached(int count) { return new Detached[count][][]; }
    public static int nested(int[][] a, int row, int column) { return a[row][column]; }
    public static void matrixStore(int[][] a, Object value) { ((Object[])a)[0] = value; }
    public static String stringsGet(String[] a, int index) { return a[index]; }
    public static void stringsStore(String[] a, Object value) { ((Object[])a)[0] = value; }
    public static String[] filledTypedStrings(String first, String second) { return new String[]{first, second}; }
}
