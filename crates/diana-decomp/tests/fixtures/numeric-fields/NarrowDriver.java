public class NarrowDriver {
    static void observe(boolean nil, int index, boolean fail) {
        fixtures.Box b = nil ? null : new fixtures.Box();
        if (b != null) { b.b = -128; b.s = -32768; b.c = 65535; b.z = true; }
        fixtures.Box.sb = 127; fixtures.Box.ss = 32767; fixtures.Box.sc = 65535; fixtures.Box.sz = true;
        byte[] ab = nil ? null : new byte[]{127}; short[] as = new short[]{32767};
        char[] ac = new char[]{65535}; boolean[] az = new boolean[]{true};
        fixtures.Dep.fail = fail; fixtures.Dep.calls = 0;
        try {
            System.out.print("fields=" + fixtures.Case.exchangeByte(b, 255) + ":" + fixtures.Case.exchangeShort(b, 65535) + ":" + fixtures.Case.exchangeChar(b, -1) + ":" + fixtures.Case.exchangeBoolean(b, false) + ";");
        } catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try {
            System.out.print("static=" + fixtures.Case.staticByte(128) + ":" + fixtures.Case.staticShort(32768) + ":" + fixtures.Case.staticChar(65536) + ":" + fixtures.Case.staticBoolean(false) + ";");
        } catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try {
            System.out.print("arrays=" + fixtures.Case.arrayByte(ab, index, 256) + ":" + fixtures.Case.arrayShort(as, index, 65536) + ":" + fixtures.Case.arrayChar(ac, index, -32768) + ":" + fixtures.Case.arrayBoolean(az, index, false) + ";");
        } catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        System.out.println("calls=" + fixtures.Dep.calls + ";box=" + (nil ? "null" : b.b + ":" + b.s + ":" + (int)b.c + ":" + b.z) + ";static=" + fixtures.Box.sb + ":" + fixtures.Box.ss + ":" + (int)fixtures.Box.sc + ":" + fixtures.Box.sz + ";arrays=" + (ab == null ? "null" : ab[0]) + ":" + as[0] + ":" + (int)ac[0] + ":" + az[0]);
    }
    public static void main(String[] args) {
        observe(false, 0, false);
        observe(false, 0, true);
        observe(true, 0, false);
        observe(false, -1, false);
    }
}
