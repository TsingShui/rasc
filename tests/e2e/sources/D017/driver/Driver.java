package driver;

import fixtures.Case;

public final class Driver {

    private static final String CASE = "D017";

    public static void main(String[] args) {
        if (args.length != 1) {
            System.err.println("usage: Driver <invocation-id>");
            System.exit(2);
            return;
        }
        String invocation = args[0];
        int bits;
        if ("v1".equals(invocation)) {
            bits = 0x7fc00000;
        } else if ("v2".equals(invocation)) {
            bits = 0x7f800000;
        } else if ("v3".equals(invocation)) {
            bits = 0xff800000;
        } else if ("v4".equals(invocation)) {
            bits = 0x4079999a;
        } else if ("v5".equals(invocation)) {
            bits = 0xc079999a;
        } else {
            System.err.println("unknown invocation: " + invocation);
            System.exit(2);
            return;
        }
        String result = intResult(Case.f(Float.intBitsToFloat(bits)));
        System.out.println(record(invocation, result, "[]", null, "{}", true));
    }

    private static String intResult(int value) {
        return "{\"kind\":\"int\",\"value\":" + value + "}";
    }

    private static String record(String invocation, String result, String events, String exception,
            String heap, boolean completed) {
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"").append(CASE).append("\",\"invocation\":").append(quote(invocation));
        out.append(",\"result\":").append(result == null ? "null" : result);
        out.append(",\"events\":").append(events);
        out.append(",\"exception\":").append(exception == null ? "null" : quote(exception));
        out.append(",\"heap\":").append(heap);
        out.append(",\"completed\":").append(completed);
        return out.append('}').toString();
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
