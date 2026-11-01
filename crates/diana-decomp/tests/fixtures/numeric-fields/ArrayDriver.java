public class ArrayDriver {
    static void observe(boolean nil, int index, boolean fail) {
        long[] l = nil ? null : new long[]{Long.MAX_VALUE};
        double[] d = nil ? null : new double[]{Double.longBitsToDouble(0x7ff8123412341234L)};
        float[] f = nil ? null : new float[]{-0.0f};
        fixtures.Dep.calls = 0;
        fixtures.Dep.fail = fail;
        try { System.out.print("L=" + fixtures.Case.arrayLong(l, index, Long.MIN_VALUE) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("D=" + Long.toHexString(Double.doubleToRawLongBits(fixtures.Case.arrayDouble(d, index, -0.0d))) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("F=" + Integer.toHexString(Float.floatToRawIntBits(fixtures.Case.arrayFloat(f, index, Float.POSITIVE_INFINITY))) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { fixtures.Case.arrayUnused(f, d, l); System.out.print("unused;"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        System.out.println("calls=" + fixtures.Dep.calls + ";arrays=" + (nil ? "null" : l[0] + ":" + Long.toHexString(Double.doubleToRawLongBits(d[0])) + ":" + Integer.toHexString(Float.floatToRawIntBits(f[0]))));
    }
    public static void main(String[] args) {
        observe(false, 0, false);
        observe(false, 0, true);
        observe(true, 0, false);
        observe(false, -1, false);
        long[] l = new long[1]; double[] d = new double[1]; float[] f = new float[1];
        fixtures.Case.arrayLiterals(f, d, l);
        System.out.println(l[0] + ":" + Long.toHexString(Double.doubleToRawLongBits(d[0])) + ":" + Integer.toHexString(Float.floatToRawIntBits(f[0])));
        System.out.println(Integer.toHexString(Float.floatToRawIntBits(fixtures.Case.chooseFloat(f, new float[]{Float.MIN_VALUE}, true, 0))));
        System.out.println(Integer.toHexString(Float.floatToRawIntBits(fixtures.Case.chooseFloat(f, new float[]{Float.MIN_VALUE}, false, 0))));
    }
}
