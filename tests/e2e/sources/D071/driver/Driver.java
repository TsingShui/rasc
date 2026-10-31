package driver;

import fixtures.Case;

public final class Driver {

    private static final String CASE = "D071";

    public static void main(String[] args) {
        if (args.length != 1) {
            System.err.println("usage: Driver <invocation-id>");
            System.exit(2);
            return;
        }
        String invocation = args[0];
        String result = null;
        boolean completed = false;
        if ("byte-low".equals(invocation)) {
            result = byteResult(Case.b(true, (byte) -128, (byte) 127));
            completed = true;
        } else if ("byte-high".equals(invocation)) {
            result = byteResult(Case.b(false, (byte) -128, (byte) 127));
            completed = true;
        } else if ("short-low".equals(invocation)) {
            result = shortResult(Case.s(true, (short) -32768, (short) 32767));
            completed = true;
        } else if ("short-high".equals(invocation)) {
            result = shortResult(Case.s(false, (short) -32768, (short) 32767));
            completed = true;
        } else if ("char-low".equals(invocation)) {
            result = charResult(Case.c(true, (char) 0, (char) 65535));
            completed = true;
        } else if ("char-high".equals(invocation)) {
            result = charResult(Case.c(false, (char) 0, (char) 65535));
            completed = true;
        } else if ("call-byte".equals(invocation)) {
            result = intResult(Case.call(true, (byte) -128, (byte) 127));
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

    private static String byteResult(byte value) {
        return "{\"kind\":\"byte\",\"value\":" + value + "}";
    }

    private static String shortResult(short value) {
        return "{\"kind\":\"short\",\"value\":" + value + "}";
    }

    private static String charResult(char value) {
        return "{\"kind\":\"char\",\"value\":" + (int) value + "}";
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
