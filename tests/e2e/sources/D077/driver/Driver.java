package driver;

import fixtures.Case;
import fixtures.Dep;

/**
 * D077 behavior Driver. Runs exactly one invocation id and prints one JSON
 * object built from that real execution. The Driver calls f()V once in a new
 * JVM, then reads the actual Dep.ticks value; it never infers success from
 * source length, from a constant, or from an engine state getter.
 *
 * The same Driver runs the original self-made source and, after recovery, the
 * recovered source, so ticks=10000 is an observation of the real call effect
 * rather than a copied expected value.
 */
public final class Driver {

    private static final String CASE = "D077";

    public static void main(String[] args) {
        if (args.length != 1) {
            System.err.println("usage: Driver <invocation-id>");
            System.exit(2);
            return;
        }
        String invocation = args[0];
        boolean completed = false;
        boolean returnVoid = false;
        int ticks;
        if ("code-upper-bound".equals(invocation)) {
            Case.f();
            returnVoid = true;
            ticks = Dep.ticks;
            completed = true;
        } else {
            System.err.println("unknown invocation: " + invocation);
            System.exit(2);
            return;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"").append(CASE).append("\",\"invocation\":").append(quote(invocation));
        out.append(",\"returnVoid\":").append(returnVoid);
        out.append(",\"heap\":{\"ticks\":").append(ticks).append('}');
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
