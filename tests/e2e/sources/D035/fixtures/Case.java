package fixtures;

/** Target: a throw from the inner handler must still be protected by the outer try. */
public final class Case {
    public static int f() {
        try {
            try {
                Dep.failA();
            } catch (ErrorA e) {
                Dep.failB();
            }
        } catch (ErrorB e) {
            Dep.log("OUTER");
            return 4;
        }
        return 0;
    }
}
