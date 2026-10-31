package fixtures;

/** Driver for D049: one-outer and two-outer capture scenarios. */
public final class Driver {
    /** Every invocation id this Driver implements; any other id is rejected. */
    private static final java.util.List<String> DECLARED_INVOCATIONS =
            java.util.Arrays.asList(
                    "v1",
                    "two-outers");

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
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D049\",\"invocation\":\"").append(invocation).append('"');
        boolean completed = false;
        Throwable failure = null;
        if (!"two-outers".equals(invocation)) {
            int value = 0;
            try {
                Outer outer = new Outer();
                Outer.Inner inner = outer.new Inner();
                value = inner.f();
                completed = true;
            } catch (Throwable t) {
                failure = t;
            }
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
        } else {
            int[] values = new int[2];
            try {
                Outer first = new Outer();
                Outer second = new Outer();
                Outer.Inner firstInner = first.new Inner();
                Outer.Inner secondInner = second.new Inner();
                second.n = 9;
                first.n = 11;
                values[0] = firstInner.f();
                values[1] = secondInner.f();
                completed = true;
            } catch (Throwable t) {
                failure = t;
            }
            out.append(",\"completed\":").append(completed);
            out.append(",\"returns\":");
            if (completed) {
                out.append('[').append(values[0]).append(',').append(values[1]).append(']');
            } else {
                out.append("null");
            }
            out.append(",\"exception\":");
            if (failure == null) {
                out.append("null");
            } else {
                out.append('"').append(failure.getClass().getName()).append('"');
            }
        }
        out.append('}');
        System.out.println(out);
    }
}
