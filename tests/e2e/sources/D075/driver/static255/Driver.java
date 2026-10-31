package driver;

import fixtures.Case;

/**
 * D075 behavior Driver for the static255 role. Each role owns its own
 * `fixtures/Case` class, so this Driver is compiled against that role's single
 * input asset in an isolated classpath; it never links two roles at once. It
 * runs exactly one invocation id and prints the observations of that real
 * execution. Every argument after the first is 0 and the first is 7, matching
 * the specification's argsPattern; the body returns p0, so a correct input and
 * a correct recovery both observe 7. The Driver never reads an engine state
 * getter and never copies an expected value.
 */
public final class Driver {

    private static final String CASE = "D075";

    public static void main(String[] args) {
        if (args.length != 1) {
            System.err.println("usage: Driver <invocation-id>");
            System.exit(2);
            return;
        }
        String invocation = args[0];
        int value;
        if ("static-at".equals(invocation)) {
            value = Case.f(7,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0);
        } else {
            System.err.println("unknown invocation: " + invocation);
            System.exit(2);
            return;
        }
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"").append(CASE).append("\",\"invocation\":").append(quote(invocation));
        out.append(",\"result\":{\"kind\":\"int\",\"value\":").append(value).append('}');
        out.append(",\"exception\":null,\"completed\":true");
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
