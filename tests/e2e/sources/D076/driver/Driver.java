package driver;

import fixtures.Case;

/**
 * D076 behavior Driver for the two accepted NUL roles. Runs exactly one
 * invocation id and prints one JSON object built from that real execution: it
 * calls the actual fixtures.Case.f() in a new JVM and reports the returned
 * String's complete u16 vector, never a copied expected value and never a
 * source-length inference.
 *
 * The same Driver source runs both legal roles by being compiled against each
 * reference classpath in turn, because each role owns its own fixtures.Case.
 * The over-limit role has no Driver: a 65536-byte CONSTANT_Utf8 has no legal
 * Java form, so there is no executable baseline for it.
 *
 * Output size is bounded by the input, not by a truncation rule: the vector is
 * 32767 or 32768 units, so the run-length pattern plus the full hex vector
 * always fits and nothing is silently dropped.
 */
public final class Driver {

    private static final String CASE = "D076";

    public static void main(String[] args) {
        if (args.length != 2) {
            System.err.println("usage: Driver <role> <invocation-id>");
            System.exit(2);
            return;
        }
        String role = args[0];
        String invocation = args[1];
        if (!"nul32767".equals(role) && !"nul32767-A".equals(role)) {
            System.err.println("unknown role: " + role);
            System.exit(2);
            return;
        }
        String returned;
        if ("below".equals(invocation) && "nul32767".equals(role)) {
            returned = Case.f();
        } else if ("at".equals(invocation) && "nul32767-A".equals(role)) {
            returned = Case.f();
        } else {
            System.err.println("unknown invocation for role " + role + ": " + invocation);
            System.exit(2);
            return;
        }

        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"").append(CASE).append("\",\"role\":").append(quote(role));
        out.append(",\"invocation\":").append(quote(invocation));
        out.append(",\"returned\":").append(returned != null);
        out.append(",\"exception\":null");
        if (returned == null) {
            out.append(",\"completed\":false}");
            System.out.println(out.toString());
            return;
        }
        out.append(",\"utf16Units\":").append(returned.length());
        out.append(",\"codeUnits\":[");
        for (int i = 0; i < returned.length(); i++) {
            if (i > 0) {
                out.append(',');
            }
            out.append((int) returned.charAt(i));
        }
        out.append(']');
        out.append(",\"codeUnitsHex\":\"").append(hex(returned)).append('"');
        out.append(",\"completed\":true}");
        System.out.println(out.toString());
    }

    private static String hex(String value) {
        StringBuilder out = new StringBuilder(value.length() * 4);
        for (int i = 0; i < value.length(); i++) {
            out.append(String.format("%04x", (int) value.charAt(i)));
        }
        return out.toString();
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
