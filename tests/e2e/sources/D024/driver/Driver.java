package fixtures;

import java.util.List;

/** Driver for D024: one-element array for the bounds path, null for the receiver path. */
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
        Dep.reset();
        List<String> events = Dep.events;
        int[] arg = "v1".equals(invocation) ? new int[] {7} : null;
        boolean completed = false;
        int value = 0;
        Throwable failure = null;
        try {
            value = Case.f(arg);
            completed = true;
        } catch (Throwable t) {
            failure = t;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D024\",\"invocation\":\"").append(invocation).append('"');
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
        out.append(",\"events\":[");
        for (int i = 0; i < events.size(); i++) {
            if (i > 0) {
                out.append(',');
            }
            out.append("{\"seq\":").append(i).append(",\"kind\":\"call\",\"label\":\"")
                    .append(events.get(i)).append("\"}");
        }
        out.append("]}");
        System.out.println(out);
    }
}
