package driver;

import fixtures.Case;

public final class Driver {

    private static final String CASE = "D072";

    public static void main(String[] args) {
        if (args.length != 1) {
            System.err.println("usage: Driver <invocation-id>");
            System.exit(2);
            return;
        }
        String invocation = args[0];
        String result = null;
        boolean completed = false;
        if ("sum-positive".equals(invocation)) {
            result = intResult(Case.sum((byte) 127, (byte) 127));
            completed = true;
        } else if ("sum-negative".equals(invocation)) {
            result = intResult(Case.sum((byte) -128, (byte) -128));
            completed = true;
        } else if ("mixed-byte".equals(invocation)) {
            result = intResult(Case.mix(true, (byte) -128, (char) 65535));
            completed = true;
        } else if ("mixed-char".equals(invocation)) {
            result = intResult(Case.mix(false, (byte) -128, (char) 65535));
            completed = true;
        } else {
            System.err.println("unknown invocation: " + invocation);
            System.exit(2);
            return;
        }
        System.out.println(record(invocation, result, "[]", null, "{}", completed));
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
