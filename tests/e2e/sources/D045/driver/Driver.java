package fixtures;

/** Driver for D045: constructs with the selected branch and reads the final field. */
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
        boolean flag = "v1".equals(invocation);
        boolean completed = false;
        Case created = null;
        Throwable failure = null;
        try {
            created = new Case(flag);
            completed = true;
        } catch (Throwable t) {
            failure = t;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D045\",\"invocation\":\"").append(invocation).append('"');
        out.append(",\"completed\":").append(completed);
        out.append(",\"exception\":");
        if (failure == null) {
            out.append("null");
        } else {
            out.append('"').append(failure.getClass().getName()).append('"');
        }
        out.append(",\"heap\":");
        if (created == null) {
            out.append("null");
        } else {
            out.append("{\"x\":").append(created.x).append('}');
        }
        out.append('}');
        System.out.println(out);
    }
}
