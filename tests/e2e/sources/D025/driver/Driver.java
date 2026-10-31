package fixtures;

/** Driver for D025: String passes, a plain Object fails the check, null passes. */
public final class Driver {
    /** Every invocation id this Driver implements; any other id is rejected. */
    private static final java.util.List<String> DECLARED_INVOCATIONS =
            java.util.Arrays.asList(
                    "v1",
                    "v2",
                    "v3");

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
        Object arg;
        if ("v1".equals(invocation)) {
            arg = "x";
        } else if ("v2".equals(invocation)) {
            arg = new Object();
        } else {
            arg = null;
        }
        boolean completed = false;
        Throwable failure = null;
        try {
            Case.f(arg);
            completed = true;
        } catch (Throwable t) {
            failure = t;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D025\",\"invocation\":\"").append(invocation).append('"');
        out.append(",\"completed\":").append(completed);
        out.append(",\"returnVoid\":").append(completed);
        out.append(",\"exception\":");
        if (failure == null) {
            out.append("null");
        } else {
            out.append('"').append(failure.getClass().getName()).append('"');
        }
        out.append('}');
        System.out.println(out);
    }
}
