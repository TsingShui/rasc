public class Driver {
    static void observe(boolean nil, boolean fail) {
        fixtures.Box b = nil ? null : new fixtures.Box();
        if (b != null) { b.l = 0x1122334455667788L; b.d = -0.0d; b.f = Float.intBitsToFloat(0x7fc12345); }
        fixtures.Box.sl = Long.MAX_VALUE;
        fixtures.Box.sd = Double.longBitsToDouble(0x7ff8123412341234L);
        fixtures.Box.sf = Float.MIN_VALUE;
        fixtures.Dep.calls = 0; fixtures.Dep.fail = fail;
        try { System.out.print("L=" + fixtures.Case.exchangeLong(b, Long.MIN_VALUE) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("D=" + Long.toHexString(Double.doubleToRawLongBits(fixtures.Case.exchangeDouble(b, Double.POSITIVE_INFINITY))) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("F=" + Integer.toHexString(Float.floatToRawIntBits(fixtures.Case.exchangeFloat(b, -0.0f))) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("SL=" + fixtures.Case.staticLong(3L) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("SD=" + Long.toHexString(Double.doubleToRawLongBits(fixtures.Case.staticDouble(-0.0d))) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("SF=" + Integer.toHexString(Float.floatToRawIntBits(fixtures.Case.staticFloat(Float.NEGATIVE_INFINITY))) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { fixtures.Case.unused(b); System.out.print("unused;"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        System.out.println("calls=" + fixtures.Dep.calls + ";box=" + (b == null ? "null" : b.l + ":" + Long.toHexString(Double.doubleToRawLongBits(b.d)) + ":" + Integer.toHexString(Float.floatToRawIntBits(b.f))) + ";static=" + fixtures.Box.sl + ":" + Long.toHexString(Double.doubleToRawLongBits(fixtures.Box.sd)) + ":" + Integer.toHexString(Float.floatToRawIntBits(fixtures.Box.sf)));
    }
    public static void main(String[] args) {
        observe(false, false);
        observe(false, true);
        observe(true, false);
        observe(true, true);
        fixtures.Box b = new fixtures.Box();
        fixtures.Case.literals(b);
        System.out.println(b.l + ":" + Long.toHexString(Double.doubleToRawLongBits(b.d)) + ":" + Integer.toHexString(Float.floatToRawIntBits(b.f)) + ":" + Long.toHexString(Double.doubleToRawLongBits(fixtures.Box.sd)) + ":" + Integer.toHexString(Float.floatToRawIntBits(fixtures.Box.sf)));
    }
}
