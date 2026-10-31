package driver;

import fixtures.Case;

/**
 * D086 behavior Driver. Runs exactly one invocation id and prints the
 * observations of that real execution. The Driver calls f()V once and records
 * that it returned normally; it does not read the configured Limits values,
 * which are request inputs rather than observations, and it does not infer
 * success from the source length.
 *
 * The same Driver runs the reference Java and, after recovery, the recovered
 * source for the unlimited-default invocation. The explicit-code-limit
 * invocation expects a normal budget failure with a complete raw round trip,
 * so it has no Java body to execute and no Driver entry.
 */
public final class Driver {

    private static final String CASE = "D086";

    public static void main(String[] args) {
        if (args.length != 1) {
            System.err.println("usage: Driver <invocation-id>");
            System.exit(2);
            return;
        }
        String invocation = args[0];
        boolean returnVoid = false;
        boolean completed = false;
        if ("unlimited-default".equals(invocation)) {
            Case.f();
            returnVoid = true;
            completed = true;
        } else {
            System.err.println("unknown invocation: " + invocation);
            System.exit(2);
            return;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"").append(CASE).append("\",\"invocation\":").append(quote(invocation));
        out.append(",\"returnVoid\":").append(returnVoid);
        out.append(",\"exception\":null");
        out.append(",\"completed\":").append(completed);
        out.append('}');
        System.out.println(out.toString());
    }

    private static String quote(String value) {
        StringBuilder out = new StringBuilder();
        out.append('"');
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            if (c == '"' || c == '\\') {
                out.append('\\').append(c);
            } else if (c < 0x20) {
                out.append(String.format("\\u%04x", (int) c));
            } else {
                out.append(c);
            }
        }
        return out.append('"').toString();
    }
}
