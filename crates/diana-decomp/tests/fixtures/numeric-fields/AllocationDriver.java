public class AllocationDriver {
    static void observe(int count, boolean fail) {
        Object value = new Object();
        fixtures.Dep.fail = fail; fixtures.Dep.calls = 0;
        try {
            Object[] a = fixtures.Case.objects(count, value);
            System.out.println("objects=" + a.length + ":" + (a[0] == value) + ":" + (a.length < 2 || a[1] == null) + ":" + a.getClass().getName() + ":" + fixtures.Dep.calls);
        } catch (Throwable e) { System.out.println(e.getClass().getName() + ":" + fixtures.Dep.calls); }
        fixtures.Dep.calls = 0;
        try {
            fixtures.Case.unusedObjects(count);
            System.out.println("unused:" + fixtures.Dep.calls);
        } catch (Throwable e) { System.out.println(e.getClass().getName() + ":" + fixtures.Dep.calls); }
        fixtures.Dep.calls = 0;
        try {
            Object[] a = fixtures.Case.zeroObjects(count);
            System.out.println("zeros=" + a.length + ":" + (a.length == 0 || a[0] == null) + ":" + a.getClass().getName() + ":" + fixtures.Dep.calls);
        } catch (Throwable e) { System.out.println(e.getClass().getName() + ":" + fixtures.Dep.calls); }
    }
    public static void main(String[] args) {
        observe(2, false);
        observe(0, false);
        observe(-1, false);
        observe(1, true);
        observe(-1, true);
    }
}
