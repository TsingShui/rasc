package fixtures;

/** Driver for D040: one invocation for the instance lock, one for the class lock. */
public final class Driver {
    /** Every invocation id this Driver implements; any other id is rejected. */
    private static final java.util.List<String> DECLARED_INVOCATIONS =
            java.util.Arrays.asList(
                    "v1",
                    "v2");

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
        Case receiver = new Case();
        boolean completed = false;
        boolean value = false;
        Throwable failure = null;
        Object observed;
        try {
            if ("v1".equals(invocation)) {
                observed = receiver;
                value = receiver.inst();
            } else {
                observed = Case.class;
                value = Case.stat();
            }
            completed = true;
        } catch (Throwable t) {
            observed = receiver;
            failure = t;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D040\",\"invocation\":\"").append(invocation).append('"');
        out.append(",\"completed\":").append(completed);
        out.append(",\"result\":");
        if (completed) {
            out.append("{\"kind\":\"boolean\",\"value\":").append(value).append('}');
        } else {
            out.append("null");
        }
        out.append(",\"exception\":");
        if (failure == null) {
            out.append("null");
        } else {
            out.append('"').append(failure.getClass().getName()).append('"');
        }
        out.append(",\"monitorAfter\":").append(Thread.holdsLock(observed)).append('}');
        System.out.println(out);
    }
}
