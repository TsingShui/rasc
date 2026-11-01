package fixtures;
public final class Invalid {
    public static Object[] filledObjects(Object first, Object second) { return new Object[]{first, second}; }
    public static int[] filledInts(int first, int second) { return new int[]{first, second}; }
    public static int[] filledRange(int a, int b, int c, int d, int e, int f) { return new int[]{a, b, c, d, e, f}; }
    public static void filledUnused(int first, int second) { int[] ignored = new int[]{first, second}; }
    public static int[] filledReuse(int first, int second) { return new int[]{second, first}; }
    public static Object[] filledZero() { return new Object[0]; }
    public static Box castUnrelated(String value) {
        return (Box)(Object)value;
    }
    public static boolean testUnrelated(String value) {
        return ((Object)value) instanceof Box;
    }
    public static int storeByte(byte[] a, int value) {
        a[0] = (byte)value;
        return a[0];
    }
    public static boolean booleanProof(boolean[] a, int value) {
        a[0] = (value & 1) != 0;
        return a[0];
    }
}
