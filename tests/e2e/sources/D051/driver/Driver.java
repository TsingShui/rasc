package fixtures;

/** Driver for D051: reflects the runtime annotation and reports its values. */
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
        boolean completed = false;
        String valueUnits = "null";
        String nums = "null";
        Throwable failure = null;
        try {
            Mark mark = Case.class.getAnnotation(Mark.class);
            if (mark != null) {
                valueUnits = "[" + units(mark.value()) + "]";
                StringBuilder builder = new StringBuilder("[");
                for (int i = 0; i < mark.nums().length; i++) {
                    if (i > 0) {
                        builder.append(',');
                    }
                    builder.append(mark.nums()[i]);
                }
                nums = builder.append(']').toString();
            }
            completed = true;
        } catch (Throwable t) {
            failure = t;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D051\",\"invocation\":\"").append(invocation).append('"');
        out.append(",\"completed\":").append(completed);
        out.append(",\"exception\":");
        if (failure == null) {
            out.append("null");
        } else {
            out.append('"').append(failure.getClass().getName()).append('"');
        }
        out.append(",\"annotation\":{\"valueUnits\":").append(valueUnits);
        out.append(",\"nums\":").append(nums).append("}}");
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
