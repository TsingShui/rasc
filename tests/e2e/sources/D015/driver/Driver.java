package driver;

import fixtures.Case;

public final class Driver {

    private static final String CASE = "D015";

    public static void main(String[] args) {
        if (args.length != 1) {
            System.err.println("usage: Driver <invocation-id>");
            System.exit(2);
            return;
        }
        String invocation = args[0];
        if (!"v1".equals(invocation) && !"v2".equals(invocation)) {
            System.err.println("unknown invocation: " + invocation);
            System.exit(2);
            return;
        }
        float a;
        float b;
        if ("v1".equals(invocation)) {
            a = Float.intBitsToFloat(0x7fc00000);
            b = Float.intBitsToFloat(0x3f800000);
        } else {
            a = Float.intBitsToFloat(0x3f800000);
            b = Float.intBitsToFloat(0x7fc00000);
        }
        StringBuilder observations = new StringBuilder("[");
        observations.append(observation("lt(FF)Z", Case.lt(a, b)));
        observations.append(',').append(observation("le(FF)Z", Case.le(a, b)));
        observations.append(',').append(observation("gt(FF)Z", Case.gt(a, b)));
        observations.append(',').append(observation("ge(FF)Z", Case.ge(a, b)));
        observations.append(']');
        System.out.println(record(invocation, observations.toString()));
    }

    private static String observation(String method, boolean value) {
        return "{\"method\":\"" + method + "\",\"result\":{\"kind\":\"boolean\",\"value\":" + value + "}}";
    }

    private static String record(String invocation, String observations) {
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"").append(CASE).append("\",\"invocation\":").append(quote(invocation));
        out.append(",\"observations\":").append(observations);
        out.append(",\"events\":[]");
        out.append(",\"exception\":null");
        out.append(",\"heap\":{}");
        out.append(",\"completed\":true");
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
