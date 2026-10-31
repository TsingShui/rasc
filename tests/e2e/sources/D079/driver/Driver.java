package fixtures;

import java.util.List;

/**
 * Driver for D079: calls Reader twice in one JVM and never names Case, so the first
 * Reader.readInt/readString is what triggers the Case static initialization. The
 * evidence is the executed event order READ,INIT,READ; the only Reader references in
 * this class are the two intended calls.
 */
public final class Driver {
    /** Every invocation id this Driver implements; any other id is rejected. */
    private static final java.util.List<String> DECLARED_INVOCATIONS =
            java.util.Arrays.asList(
                    "int-read",
                    "string-read");

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
        boolean completed = false;
        Throwable failure = null;
        int[] ints = new int[2];
        String[] strings = new String[2];
        try {
            if ("int-read".equals(invocation)) {
                ints[0] = Reader.readInt();
                ints[1] = Reader.readInt();
            } else {
                strings[0] = Reader.readString();
                strings[1] = Reader.readString();
            }
            completed = true;
        } catch (Throwable t) {
            failure = t;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D079\",\"invocation\":\"").append(invocation).append('"');
        out.append(",\"completed\":").append(completed);
        if ("int-read".equals(invocation)) {
            out.append(",\"returns\":");
            if (completed) {
                out.append('[').append(ints[0]).append(',').append(ints[1]).append(']');
            } else {
                out.append("null");
            }
        } else {
            out.append(",\"returnsStringUnits\":");
            if (completed) {
                out.append("[[").append(units(strings[0])).append("],[")
                        .append(units(strings[1])).append("]]");
            } else {
                out.append("null");
            }
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
