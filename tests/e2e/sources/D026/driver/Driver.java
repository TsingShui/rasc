package fixtures;

/** Driver for D026: passes a real String[] so the component check is observable. */
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
        String[] strings = new String[] {"old"};
        Object[] array = invocation.equals("v3") ? null : (Object[]) strings;
        Object value = "v2".equals(invocation) ? (Object) "ok" : new Object();
        boolean completed = false;
        Throwable failure = null;
        try {
            Case.f(array, value);
            completed = true;
        } catch (Throwable t) {
            failure = t;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D026\",\"invocation\":\"").append(invocation).append('"');
        out.append(",\"completed\":").append(completed);
        out.append(",\"returnVoid\":").append(completed);
        out.append(",\"exception\":");
        if (failure == null) {
            out.append("null");
        } else {
            out.append('"').append(failure.getClass().getName()).append('"');
        }
        if ("v2".equals(invocation)) {
            out.append(",\"heap\":{\"firstString\":\"").append(strings[0]).append("\"}}");
        } else {
            out.append('}');
        }
        System.out.println(out);
    }
}
