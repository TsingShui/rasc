package fixtures;

/** Driver for D028: a Box with n=4, or null for the field-access exception. */
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
        Box box = "v1".equals(invocation) ? new Box() : null;
        if (box != null) {
            box.n = 4;
        }
        boolean completed = false;
        int value = 0;
        Throwable failure = null;
        try {
            value = Case.f(box);
            completed = true;
        } catch (Throwable t) {
            failure = t;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D028\",\"invocation\":\"").append(invocation).append('"');
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
        if (box != null) {
            out.append(",\"heap\":{\"n\":").append(box.n).append('}');
        }
        out.append('}');
        System.out.println(out);
    }
}
