package fixtures;

/** Driver for D050: names, ordinals, a mutated values result and valueOf edges. */
public final class Driver {
    /** Every invocation id this Driver implements; any other id is rejected. */
    private static final java.util.List<String> DECLARED_INVOCATIONS =
            java.util.Arrays.asList(
                    "enum-contract");

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
        // The first values() result is read, then mutated; the next call must be fresh.
        Case[] first = Case.values();
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D050\",\"invocation\":\"").append(invocation).append('"');
        out.append(",\"completed\":true");
        out.append(",\"enumNames\":[").append(quoted(first)).append(']');
        out.append(",\"enumOrdinals\":[");
        for (int i = 0; i < first.length; i++) {
            if (i > 0) {
                out.append(',');
            }
            out.append(first[i].ordinal());
        }
        out.append(']');
        first[0] = first[1];
        Case[] second = Case.values();
        boolean valuesFresh = second.length == 2 && second[0] == Case.Z && second[1] == Case.A;
        out.append(",\"valuesFresh\":").append(valuesFresh);
        StringBuilder valueOfNames = new StringBuilder();
        String unknownException = "none";
        String nullException = "none";
        for (int i = 0; i < 2; i++) {
            String name = i == 0 ? "Z" : "A";
            try {
                Case found = Case.valueOf(name);
                if (i > 0) {
                    valueOfNames.append(',');
                }
                valueOfNames.append('"').append(found.name()).append('"');
            } catch (Throwable t) {
                if (i > 0) {
                    valueOfNames.append(',');
                }
                valueOfNames.append("\"exception:").append(t.getClass().getName()).append('"');
            }
        }
        try {
            Case.valueOf("Q");
        } catch (Throwable t) {
            unknownException = t.getClass().getName();
        }
        try {
            Case.valueOf(null);
        } catch (Throwable t) {
            nullException = t.getClass().getName();
        }
        out.append(",\"valueOfNames\":[").append(valueOfNames).append(']');
        out.append(",\"valueOfUnknownException\":\"").append(unknownException).append('"');
        out.append(",\"valueOfNullException\":\"").append(nullException).append('"');
        out.append('}');
        System.out.println(out);
    }

    private static String quoted(Case[] constants) {
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < constants.length; i++) {
            if (i > 0) {
                out.append(',');
            }
            out.append('"').append(constants[i].name()).append('"');
        }
        return out.toString();
    }
}
