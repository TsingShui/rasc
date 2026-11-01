public class TypeDriver {
    static void observe(Object value, int index, boolean fail) {
        fixtures.Dep.fail = fail; fixtures.Dep.calls = 0;
        try { System.out.println("cast=" + (fixtures.Case.asPeer(value) == value) + ":" + fixtures.Dep.calls); }
        catch (Throwable e) { System.out.println(e.getClass().getName() + ":" + fixtures.Dep.calls); }
        fixtures.Dep.calls = 0;
        try { System.out.println("array=" + Integer.toHexString(Float.floatToRawIntBits(fixtures.Case.asFloats(value, index))) + ":" + fixtures.Dep.calls); }
        catch (Throwable e) { System.out.println(e.getClass().getName() + ":" + fixtures.Dep.calls); }
        fixtures.Dep.calls = 0;
        try { fixtures.Case.unusedCast(value); System.out.println("unused:" + fixtures.Dep.calls); }
        catch (Throwable e) { System.out.println(e.getClass().getName() + ":" + fixtures.Dep.calls); }
        fixtures.Dep.calls = 0;
        try { System.out.println("peer=" + fixtures.Case.isPeer(value) + ":" + fixtures.Dep.calls); }
        catch (Throwable e) { System.out.println(e.getClass().getName() + ":" + fixtures.Dep.calls); }
        fixtures.Dep.calls = 0;
        try { System.out.println("floats=" + fixtures.Case.isFloats(value) + ":" + fixtures.Dep.calls); }
        catch (Throwable e) { System.out.println(e.getClass().getName() + ":" + fixtures.Dep.calls); }
    }
    public static void main(String[] args) {
        observe(new fixtures.Box(), 0, false);
        observe(new float[]{-0.0f}, 0, false);
        observe("text", 0, false);
        observe(null, 0, false);
        observe(new float[0], 0, false);
        observe("text", 0, true);
    }
}
