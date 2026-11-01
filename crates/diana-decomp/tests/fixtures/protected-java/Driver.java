public class Driver {
    static void run(String method, boolean fail, boolean cleanup) {
        fixtures.Dep.trace = "";
        String result;
        try {
            int value;
            if (method.equals("rethrow")) value = fixtures.Protected.rethrow(fail, cleanup);
            else if (method.equals("catchAll")) value = fixtures.Protected.catchAll(fail);
            else if (method.equals("typed")) value = fixtures.Protected.typed(fail);
            else value = fixtures.Protected.outside(cleanup);
            result = Integer.toString(value);
        } catch (Throwable error) { result = error.getClass().getName(); }
        System.out.println(method + ":" + fail + ":" + cleanup + ":" + result + ":" + fixtures.Dep.trace);
    }
    public static void main(String[] args) {
        run("rethrow", false, false);
        run("rethrow", false, true);
        run("rethrow", true, false);
        run("rethrow", true, true);
        run("catchAll", false, false);
        run("catchAll", true, false);
        run("typed", false, false);
        run("typed", true, false);
        run("outside", false, false);
        run("outside", false, true);
    }
}
