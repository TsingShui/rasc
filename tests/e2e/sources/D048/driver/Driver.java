package fixtures;

/** Driver for D048: invokes through a Base-typed reference to reach the bridge. */
public final class Driver {
    /** Every invocation id this Driver implements; any other id is rejected. */
    private static final java.util.List<String> DECLARED_INVOCATIONS =
            java.util.Arrays.asList(
                    "v1");

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
        Base receiver = new Case();
        boolean completed = false;
        Object value = null;
        Throwable failure = null;
        try {
            value = receiver.get();
            completed = true;
        } catch (Throwable t) {
            failure = t;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D048\",\"invocation\":\"").append(invocation).append('"');
        out.append(",\"completed\":").append(completed);
        out.append(",\"result\":");
        if (!completed) {
            out.append("null");
        } else if (value == null) {
            out.append("{\"kind\":\"null\"}");
        } else {
            out.append("{\"kind\":\"string\",\"units\":[").append(units((String) value)).append("]}");
        }
        out.append(",\"exception\":");
        if (failure == null) {
            out.append("null");
        } else {
            out.append('"').append(failure.getClass().getName()).append('"');
        }
        out.append('}');
        System.out.println(out);
    }

    private static String units(String text) {
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < text.length(); i++) {
            if (i > 0) {
                out.append(',');
            }
            out.append((int) text.charAt(i));
        }
        return out.toString();
    }
}
