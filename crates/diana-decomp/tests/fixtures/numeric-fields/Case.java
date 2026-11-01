package fixtures;
public final class Case {
    public static Object[] objects(int count, Object value) {
        Dep.step(); Object[] a = new Object[count]; Dep.step(); a[0] = value; return a;
    }
    public static Object[] zeroObjects(int count) {
        return new Object[count];
    }
    public static void unusedObjects(int count) {
        Object[] ignored = new Object[count]; Dep.step();
    }
    public static Box asPeer(Object value) {
        Dep.step(); Box peer = (Box)value; Dep.step(); return peer;
    }
    public static float asFloats(Object value, int index) {
        Dep.step(); float[] a = (float[])value; Dep.step(); return a[index];
    }
    public static void unusedCast(Object value) {
        Box ignored = (Box)value; Dep.step();
    }
    public static boolean isPeer(Object value) {
        Dep.step(); boolean result = value instanceof Box; Dep.step(); return result;
    }
    public static boolean isFloats(Object value) {
        Dep.step(); boolean result = value instanceof float[]; Dep.step(); return result;
    }
    public static Object exchangeObject(Box b, Object value) {
        Object old = b.object; Dep.step(); b.object = value; return old;
    }
    public static String exchangeString(Box b, String value) {
        String old = b.text; Dep.step(); b.text = value; return old;
    }
    public static Box exchangePeer(Box b, Box value) {
        Box old = b.peer; Dep.step(); b.peer = value; return old;
    }
    public static float[] exchangeValues(Box b, float[] value) {
        float[] old = b.values; Dep.step(); b.values = value; return old;
    }
    public static float fieldArray(Box b, int index, float value) {
        float[] a = b.values; float old = a[index]; Dep.step(); a[index] = value; return old;
    }
    public static Object staticReference(Object value) {
        Object old = Box.staticObject; Dep.step(); Box.staticObject = value; return old;
    }
    public static Object widenPeer(Box b, Box value) {
        b.object = value; return b.object;
    }
    public static Box nullReferences(Box b) {
        b.peer = null; b.text = null; b.values = null; b.object = null; Box.staticPeer = null; return b.peer;
    }
    public static void unusedReferences(Box b) {
        Object object = b.object; String text = b.text; Box peer = b.peer;
        float[] values = b.values; Object last = b.finalObject; Dep.step();
    }
    public static int exchangeByte(Box b, int value) {
        int old = b.b; Dep.step(); b.b = (byte)value; return old;
    }
    public static int exchangeShort(Box b, int value) {
        int old = b.s; Dep.step(); b.s = (short)value; return old;
    }
    public static int exchangeChar(Box b, int value) {
        int old = b.c; Dep.step(); b.c = (char)value; return old;
    }
    public static boolean exchangeBoolean(Box b, boolean value) {
        boolean old = b.z; Dep.step(); b.z = value; return old;
    }
    public static int staticByte(int value) {
        int old = Box.sb; Dep.step(); Box.sb = (byte)value; return old;
    }
    public static int staticShort(int value) {
        int old = Box.ss; Dep.step(); Box.ss = (short)value; return old;
    }
    public static int staticChar(int value) {
        int old = Box.sc; Dep.step(); Box.sc = (char)value; return old;
    }
    public static boolean staticBoolean(boolean value) {
        boolean old = Box.sz; Dep.step(); Box.sz = value; return old;
    }
    public static int arrayByte(byte[] a, int index, int value) {
        int old = a[index]; Dep.step(); a[index] = (byte)value; return old;
    }
    public static int arrayShort(short[] a, int index, int value) {
        int old = a[index]; Dep.step(); a[index] = (short)value; return old;
    }
    public static int arrayChar(char[] a, int index, int value) {
        int old = a[index]; Dep.step(); a[index] = (char)value; return old;
    }
    public static boolean arrayBoolean(boolean[] a, int index, boolean value) {
        boolean old = a[index]; Dep.step(); a[index] = value; return old;
    }
    public static long exchangeLong(Box b, long value) {
        long old = b.l;
        Dep.step();
        b.l = value;
        return old;
    }
    public static double exchangeDouble(Box b, double value) {
        double old = b.d;
        Dep.step();
        b.d = value;
        return old;
    }
    public static float exchangeFloat(Box b, float value) {
        float old = b.f;
        Dep.step();
        b.f = value;
        return old;
    }
    public static long staticLong(long value) {
        long old = Box.sl;
        Dep.step();
        Box.sl = value;
        return old;
    }
    public static double staticDouble(double value) {
        double old = Box.sd;
        Dep.step();
        Box.sd = value;
        return old;
    }
    public static float staticFloat(float value) {
        float old = Box.sf;
        Dep.step();
        Box.sf = value;
        return old;
    }
    public static long arrayLong(long[] a, int index, long value) {
        long old = a[index];
        Dep.step();
        a[index] = value;
        return old;
    }
    public static double arrayDouble(double[] a, int index, double value) {
        double old = a[index];
        Dep.step();
        a[index] = value;
        return old;
    }
    public static float arrayFloat(float[] a, int index, float value) {
        float old = a[index];
        Dep.step();
        a[index] = value;
        return old;
    }
    public static float chooseFloat(float[] left, float[] right, boolean pick, int index) {
        float[] selected = pick ? left : right;
        return selected[index];
    }
    public static void arrayLiterals(float[] f, double[] d, long[] l) {
        f[0] = -0.0f;
        d[0] = Double.POSITIVE_INFINITY;
        l[0] = Long.MIN_VALUE;
    }
    public static void arrayUnused(float[] f, double[] d, long[] l) {
        float fv = f[0];
        double dv = d[0];
        long lv = l[0];
        Dep.step();
    }
    public static void unused(Box b) {
        long l = b.l;
        double d = b.d;
        float f = b.f;
        Dep.step();
    }
    public static void literals(Box b) {
        b.l = Long.MIN_VALUE;
        b.d = -0.0d;
        b.f = Float.POSITIVE_INFINITY;
        Box.sd = Double.NEGATIVE_INFINITY;
        Box.sf = -0.0f;
    }
}
