package fixtures;

/** Driver for D082: zero-trip null lock, two-trip lock, one-trip null lock. */
public final class Driver {
    /** Every invocation id this Driver implements; any other id is rejected. */
    private static final java.util.List<String> DECLARED_INVOCATIONS =
            java.util.Arrays.asList(
                    "zero-null",
                    "twice",
                    "null-enter");

    public static void main(String[] args) {
        if (args.length != 1) {
            // The invocation contract is exactly one declared id: a missing or
            // extra argument must fail instead of producing a plausible run.
            System.err.println("usage: Driver <invocation-id>; declared ids: "
                    + String.join(", ", DECLARED_INVOCATIONS));
            System.exit(2);
        }
        String invocation = args[0];
        if (!DECLARED_INVOCATIONS.contains(invocation)) {
            System.err.println("unknown invocation id: " + invocation
                    + "; declared ids: " + String.join(", ", DECLARED_INVOCATIONS));
            System.exit(2);
        }
        Object lock;
        int n;
        if ("twice".equals(invocation)) {
            lock = new Object();
            n = 2;
        } else if ("null-enter".equals(invocation)) {
            lock = null;
            n = 1;
        } else {
            lock = null;
            n = 0;
        }
        boolean completed = false;
        int value = 0;
        Throwable failure = null;
        try {
            value = Case.f(lock, n);
            completed = true;
        } catch (Throwable t) {
            failure = t;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D082\",\"invocation\":\"").append(invocation).append('"');
        out.append(",\"completed\":").append(completed);
        out.append(",\"result\":");
        if (completed) {
            out.append("{\"kind\":\"int\",\"value\":").append(value).append('}');
        } else {
            out.append("null");
        }
        out.append(",\"exception\":");
        if (failure == null) {
            out.append("null");
        } else {
            out.append('"').append(failure.getClass().getName()).append('"');
        }
        if (lock != null) {
            out.append(",\"monitorAfter\":").append(Thread.holdsLock(lock));
        }
        out.append('}');
        System.out.println(out);
    }
}
